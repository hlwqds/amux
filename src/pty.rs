use std::{
    io,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::Sender,
    },
    thread,
};

use anyhow::{Context, Result};
use bytes::Bytes;
use portable_pty::{NativePtySystem, PtySize, PtySystem};

use crate::types::Agent;
use crate::util::now_secs;

pub const IDLE_THRESHOLD_SECS: u64 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PtyState {
    Running,
    Completed,
}

pub struct PtyHandle {
    parser: Arc<RwLock<vt100::Parser>>,
    writer_tx: Sender<Bytes>,
    alive: Arc<AtomicBool>,
    last_output_at: Arc<AtomicU64>,
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
    ) -> Result<Self> {
        let pty_system = NativePtySystem::default();

        let pty_size = PtySize {
            rows: size.1,
            cols: size.0,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system.openpty(pty_size).context("failed to open PTY")?;

        let cmd = if let Some(id) = session_id {
            agent.build_resume_cmd(workspace_path, id)
        } else {
            agent.build_new_cmd(workspace_path, session_name)
        };

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .context(format!("failed to spawn {}", agent.label()))?;

        let master = pair.master;
        let mut reader = master
            .try_clone_reader()
            .context("failed to clone PTY reader")?;

        let parser = Arc::new(RwLock::new(vt100::Parser::new(size.1, size.0, 10000)));
        let alive = Arc::new(AtomicBool::new(true));
        let last_output_at = Arc::new(AtomicU64::new(now_secs()));

        {
            let parser = parser.clone();
            let last_output_at = last_output_at.clone();
            thread::spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match io::Read::read(&mut reader, &mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let mut p = parser.write().unwrap();
                            p.process(&buf[..n]);
                            last_output_at.store(now_secs(), Ordering::Relaxed);
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

        let (writer_tx, writer_rx) = std::sync::mpsc::channel::<Bytes>();
        {
            thread::spawn(move || {
                let mut writer = master.take_writer().unwrap();
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
        })
    }

    pub fn write_input(&self, data: &[u8]) {
        let _ = self.writer_tx.send(Bytes::from(data.to_vec()));
    }

    pub fn resize(&self, size: (u16, u16)) {
        if let Ok(mut p) = self.parser.write() {
            p.screen_mut().set_size(size.1, size.0);
        }
    }

    pub fn screen(&self) -> Arc<RwLock<vt100::Parser>> {
        self.parser.clone()
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    pub fn scrollback_offset(&self) -> usize {
        self.parser.read().unwrap().screen().scrollback()
    }

    pub fn scroll_page_up(&self, page_size: usize) {
        let current = self.scrollback_offset();
        self.parser
            .write()
            .unwrap()
            .screen_mut()
            .set_scrollback(current + page_size);
    }

    pub fn scroll_page_down(&self, page_size: usize) {
        let current = self.scrollback_offset();
        self.parser
            .write()
            .unwrap()
            .screen_mut()
            .set_scrollback(current.saturating_sub(page_size));
    }

    pub fn reset_scroll(&self) {
        self.parser.write().unwrap().screen_mut().set_scrollback(0);
    }
}
