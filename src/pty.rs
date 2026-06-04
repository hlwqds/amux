use std::{
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::SyncSender,
    },
    thread,
};

use anyhow::{Context, Result};
use bytes::Bytes;
use parking_lot::RwLock;
use portable_pty::{NativePtySystem, PtySize, PtySystem};
use tokio::sync::Notify;

use crate::types::Agent;
use crate::util::now_secs;

pub const IDLE_THRESHOLD_SECS: u64 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PtyState {
    Running,
    Completed,
}

#[derive(Clone)]
pub struct PtyHandle {
    parser: Arc<RwLock<vt100::Parser>>,
    writer_tx: SyncSender<Bytes>,
    alive: Arc<AtomicBool>,
    last_output_at: Arc<AtomicU64>,
    last_raw_output: Arc<RwLock<Vec<u8>>>,
    output_notify: Arc<Notify>,
    /// PID of the child process (captured before child is moved to wait thread).
    child_pid: Option<u32>,
    /// Screen snapshots for scrollback in alternate screen mode.
    /// Each snapshot is the full screen rows at a point in time.
    snapshots: Arc<RwLock<std::collections::VecDeque<Vec<String>>>>,
    /// Current scrollback position: 0 = live, N = N snapshots back.
    snap_scroll: Arc<AtomicUsize>,
}

impl PtyHandle {
    pub fn state(&self) -> PtyState {
        let last = self.last_output_at.load(Ordering::Relaxed);
        let now = now_secs();
        if self.alive.load(Ordering::Relaxed) && now.saturating_sub(last) <= IDLE_THRESHOLD_SECS {
            PtyState::Running
        } else {
            PtyState::Completed
        }
    }

    pub fn spawn(
        agent: Agent,
        workspace_path: &std::path::Path,
        session_id: Option<&str>,
        session_name: Option<&str>,
        size: (u16, u16),
        env_vars: &[(String, String)],
    ) -> Result<Self> {
        let pty_system = NativePtySystem::default();
        let pty_size = PtySize {
            rows: size.1,
            cols: size.0,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = pty_system.openpty(pty_size).context("failed to open PTY")?;
        let mut cmd = if let Some(id) = session_id {
            agent.build_resume_cmd(workspace_path, id)
        } else {
            agent.build_new_cmd(workspace_path, session_name)
        };
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .context(format!("failed to spawn {}", agent.label()))?;

        // Capture PID before child is moved into the wait thread.
        let child_pid = child.process_id();

        let master = pair.master;
        let mut reader = master
            .try_clone_reader()
            .context("failed to clone PTY reader")?;
        let last_raw_output: Arc<RwLock<Vec<u8>>> = Arc::new(RwLock::new(Vec::with_capacity(8192)));
        let parser = Arc::new(RwLock::new(vt100::Parser::new(size.1, size.0, 10000)));
        let alive = Arc::new(AtomicBool::new(true));
        let last_output_at = Arc::new(AtomicU64::new(now_secs()));
        let output_notify = Arc::new(Notify::new());
        let snapshots: Arc<RwLock<std::collections::VecDeque<Vec<String>>>> =
            Arc::new(RwLock::new(std::collections::VecDeque::with_capacity(500)));
        let snap_scroll = Arc::new(AtomicUsize::new(0));
        {
            let parser = parser.clone();
            let last_output_at = last_output_at.clone();
            let last_raw_output = last_raw_output.clone();
            let output_notify = output_notify.clone();
            let snapshots = snapshots.clone();
            let snap_scroll = snap_scroll.clone();
            thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let mut snap_counter: u32 = 0;
                loop {
                    match std::io::Read::read(&mut reader, &mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let data = &buf[..n];
                            let mut p = parser.write();
                            p.process(data);
                            // Snapshot screen periodically (every ~8 reads) when in alternate screen.
                            // This gives scrollback for agents like Codex that use alternate screen.
                            snap_counter += 1;
                            if snap_counter >= 8 && p.screen().alternate_screen() {
                                snap_counter = 0;
                                let screen = p.screen();
                                let rows: Vec<String> = screen.rows(0, screen.size().1).collect();
                                let mut snaps = snapshots.write();
                                if snaps.back() != Some(&rows) {
                                    if snaps.len() == snaps.capacity() {
                                        snaps.pop_front();
                                    }
                                    snaps.push_back(rows);
                                    // New output resets scroll position
                                    snap_scroll.store(0, Ordering::Relaxed);
                                }
                            }
                            drop(p);
                            let mut raw = last_raw_output.write();
                            raw.extend_from_slice(data);
                            const MAX_RAW: usize = 1024 * 1024;
                            if raw.len() > MAX_RAW {
                                let excess = raw.len() - MAX_RAW;
                                raw.drain(..excess);
                            }
                            last_output_at.store(now_secs(), Ordering::Relaxed);
                            output_notify.notify_waiters();
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        {
            let alive = alive.clone();
            thread::spawn(move || {
                let _ = child.wait();
                alive.store(false, Ordering::Relaxed);
            });
        }
        let (writer_tx, writer_rx) = std::sync::mpsc::sync_channel::<Bytes>(1024);
        {
            thread::spawn(move || {
                let mut writer = match master.take_writer() {
                    Ok(w) => w,
                    Err(e) => {
                        eprintln!("warning: failed to take PTY writer: {e}");
                        return;
                    }
                };
                while let Ok(bytes) = writer_rx.recv() {
                    if writer.write_all(&bytes).is_err() {
                        break;
                    }
                }
            });
        }

        Ok(Self {
            parser,
            writer_tx,
            alive,
            last_output_at,
            last_raw_output,
            output_notify,
            child_pid,
            snapshots,
            snap_scroll,
        })
    }

    pub fn write_input(&self, data: &[u8]) -> Result<(), String> {
        if !self.alive.load(Ordering::Relaxed) {
            return Err("PTY closed".to_string());
        }
        self.snap_scroll.store(0, Ordering::Relaxed);
        self.writer_tx
            .try_send(Bytes::from(data.to_vec()))
            .map_err(|_| "PTY input buffer full".to_string())
    }

    pub fn resize(&self, size: (u16, u16)) {
        let mut p = self.parser.write();
        p.screen_mut().set_size(size.1, size.0);
    }

    pub fn screen(&self) -> Arc<RwLock<vt100::Parser>> {
        self.parser.clone()
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Returns the PID of the child process, if available.
    pub fn child_pid(&self) -> Option<u32> {
        self.child_pid
    }

    pub fn scrollback_offset(&self) -> usize {
        self.parser.read().screen().scrollback()
    }
    /// Scroll up: in normal mode uses vt100 scrollback, in alternate screen
    /// mode uses the snapshot buffer.
    pub fn scroll_page_up(&self, page_size: usize) {
        if self.is_alternate_screen() {
            let snaps = self.snapshots.read();
            let max_scroll = snaps.len().saturating_sub(1);
            let current = self.snap_scroll.load(Ordering::Relaxed);
            let new_scroll = (current + 1).min(max_scroll);
            self.snap_scroll.store(new_scroll, Ordering::Relaxed);
        } else {
            let current = self.scrollback_offset();
            self.parser
                .write()
                .screen_mut()
                .set_scrollback(current + page_size);
        }
    }
    pub fn scroll_page_down(&self, page_size: usize) {
        if self.is_alternate_screen() {
            let current = self.snap_scroll.load(Ordering::Relaxed);
            self.snap_scroll.store(current.saturating_sub(1), Ordering::Relaxed);
        } else {
            let current = self.scrollback_offset();
            self.parser
                .write()
                .screen_mut()
                .set_scrollback(current.saturating_sub(page_size));
        }
    }
    pub fn reset_scroll(&self) {
        self.snap_scroll.store(0, Ordering::Relaxed);
        self.parser.write().screen_mut().set_scrollback(0);
    }
    pub fn set_scrollback(&self, offset: usize) {
        self.parser.write().screen_mut().set_scrollback(offset);
    }
    /// Returns the snapshot at the current scroll position, if any.
    /// Returns None when scroll is at live position (snap_scroll == 0)
    /// or when there are no snapshots.
    pub fn scrolled_snapshot(&self) -> Option<Vec<String>> {
        let pos = self.snap_scroll.load(Ordering::Relaxed);
        if pos == 0 {
            return None;
        }
        let snaps = self.snapshots.read();
        let idx = snaps.len().saturating_sub(pos);
        snaps.get(idx).cloned()
    }

    pub fn is_alternate_screen(&self) -> bool {
        self.parser.read().screen().alternate_screen()
    }

    pub fn idle_secs(&self) -> u64 {
        let last = self.last_output_at.load(Ordering::Relaxed);
        now_secs().saturating_sub(last)
    }

    /// Drains and returns accumulated raw ANSI output bytes since last call.
    pub fn take_raw_output(&self) -> Vec<u8> {
        let mut raw = self.last_raw_output.write();
        std::mem::take(&mut *raw)
    }

    /// Returns a `Notify` that fires whenever new PTY output arrives.
    /// Use with `notify.notified()` for event-driven reads instead of polling.
    pub fn output_notify(&self) -> Arc<Notify> {
        self.output_notify.clone()
    }
}
