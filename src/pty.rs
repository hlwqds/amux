use std::{
    io::Write,
    sync::Arc,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    sync::mpsc::SyncSender,
    thread,
};
use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};
use anyhow::{Context, Result};
use bytes::Bytes;
use parking_lot::RwLock;
use portable_pty::{NativePtySystem, PtySize, PtySystem};
use tokio::sync::Notify;
use tracing::{error, info, warn};

use crate::types::Agent;
use crate::util::now_secs;

pub const IDLE_THRESHOLD_SECS: u64 = 3;
pub const DEFAULT_SCROLLBACK_LINES: usize = 10000;

// ---------------------------------------------------------------------------
// EventListener — forwards DA1/DSR/OSC replies back to the PTY
// ---------------------------------------------------------------------------

/// Listener for terminal query events (DA1, DSR, etc.) that alacritty's
/// parser generates. These must be written back to the PTY so interactive
/// TUIs (Codex/Ink, helix, fzf, atuin) don't hang waiting for replies.
#[derive(Clone, Default)]
pub(crate) struct PtyEventListener {
    response_tx: Option<std::sync::mpsc::Sender<String>>,
    size: Option<std::sync::Arc<std::sync::Mutex<(u16, u16)>>>,
}


impl EventListener for PtyEventListener {
    fn send_event(&self, event: Event) {
        let Some(tx) = self.response_tx.as_ref() else {
            return;
        };
        match event {
            Event::PtyWrite(text) => {
                let _ = tx.send(text);
            }
            Event::TextAreaSizeRequest(cb) => {
                let Some(size) = self.size.as_ref() else {
                    return;
                };
                let (cols, rows) = *size.lock().unwrap_or_else(|e| e.into_inner());
                let ws = alacritty_terminal::event::WindowSize {
                    num_lines: rows,
                    num_cols: cols,
                    cell_width: 1,
                    cell_height: 1,
                };
                let _ = tx.send(cb(ws));
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// PtyState / PtyHandle
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PtyState {
    Running,
    Completed,
}

#[derive(Clone)]
pub struct PtyHandle {
    term: Arc<FairMutex<Term<PtyEventListener>>>,
    writer_tx: SyncSender<Bytes>,
    alive: Arc<AtomicBool>,
    last_output_at: Arc<AtomicU64>,
    last_raw_output: Arc<RwLock<Vec<u8>>>,
    output_notify: Arc<Notify>,
    /// PID of the child process (captured before child is moved to wait thread).
    child_pid: Option<u32>,
    /// Screen snapshots for scrollback in alternate screen mode.
    snapshots: Arc<RwLock<std::collections::VecDeque<Vec<String>>>>,
    /// Current scrollback position: 0 = live, N = N snapshots back.
    snap_scroll: Arc<AtomicUsize>,
    // Asciinema recording — the Arc is cloned into the reader thread.
}

/// Create an asciinema v2 recording file at the given path.
/// Returns the file with the header already written.
fn create_recording(path: &std::path::Path, cols: u16, rows: u16) -> Result<std::fs::File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(path).context("failed to create recording file")?;
    let header = serde_json::json!({
        "version": 2,
        "width": cols,
        "height": rows,
        "timestamp": now_secs(),
        "env": {
            "SHELL": std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()),
            "TERM": std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".into()),
        }
    });
    use std::io::Write;
    writeln!(file, "{}", header)?;
    Ok(file)
}

/// Write an asciinema v2 output event to the recording file.
fn write_recording_event(file: &mut std::fs::File, start: std::time::Instant, data: &[u8]) {
    let elapsed = start.elapsed().as_secs_f64();
    // Serialize the raw bytes as a JSON string.
    let s = String::from_utf8_lossy(data);
    let json = serde_json::json!(["o", format!("{elapsed:.6}"), s.as_ref()]);
    let _ = writeln!(file, "{json}");
}

/// Simple timestamp without chrono dependency.
fn chrono_free_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
        unset_env: &[String],
    ) -> Result<Self> {
        info!("spawning PTY: {} in {}", agent.label(), workspace_path.display());
        let pty_system = NativePtySystem::default();
        let pty_size = PtySize {
            rows: size.1,
            cols: size.0,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = match pty_system.openpty(pty_size) {
            Ok(p) => p,
            Err(e) => {
                error!("failed to open PTY: {e:?}");
                anyhow::bail!("failed to open PTY: {e:#}");
            }
        };
        let mut cmd = if let Some(id) = session_id {
            agent.build_resume_cmd(workspace_path, id, unset_env)
        } else {
            agent.build_new_cmd(workspace_path, session_name, unset_env)
        };
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .context(format!("failed to spawn {}", agent.label()))?;

        let child_pid = child.process_id();

        let master = pair.master;
        let mut reader = master
            .try_clone_reader()
            .context("failed to clone PTY reader")?;

        let last_raw_output: Arc<RwLock<Vec<u8>>> =
            Arc::new(RwLock::new(Vec::with_capacity(8192)));
        let alive = Arc::new(AtomicBool::new(true));
        let last_output_at = Arc::new(AtomicU64::new(now_secs()));
        let output_notify = Arc::new(Notify::new());
        let snapshots: Arc<RwLock<std::collections::VecDeque<Vec<String>>>> =
            Arc::new(RwLock::new(std::collections::VecDeque::with_capacity(500)));
        let snap_scroll = Arc::new(AtomicUsize::new(0));

        // Create asciinema recording file.
        let recordings_dir = crate::config::data_dir().join("recordings");
        let recording_path = recordings_dir.join(format!(
            "{}-{}.cast",
            session_id.unwrap_or("unknown"),
            chrono_free_timestamp()
        ));
        let recording_file = create_recording(&recording_path, size.0, size.1)
            .map_err(|e| {
                warn!("failed to create recording: {e}");
                e
            })
            .ok();
        let recording: Arc<RwLock<Option<std::fs::File>>> =
            Arc::new(RwLock::new(recording_file));
        info!("recording to {}", recording_path.display());

        // Create writer channel before reader thread so the reader can
        // forward terminal query responses (DA1, DSR, etc.) back to the PTY.
        let (writer_tx, writer_rx) = std::sync::mpsc::sync_channel::<Bytes>(1024);

        // Set up alacritty_terminal Term with event listener for query responses.
        let term_size = TermSize::new(size.0 as usize, size.1 as usize);
        let term_config = Config {
            scrolling_history: DEFAULT_SCROLLBACK_LINES,
            ..Config::default()
        };
        let size_shared = Arc::new(std::sync::Mutex::new((size.0, size.1)));
        let (response_tx, response_rx) = std::sync::mpsc::channel::<String>();
        let listener = PtyEventListener {
            response_tx: Some(response_tx),
            size: Some(size_shared),
        };
        let term = Term::new(term_config, &term_size, listener);
        let term = Arc::new(FairMutex::new(term));

        // Background thread: forward alacritty's reply bytes (DA1, DSR,
        // TextAreaSizeRequest) back to the PTY master.
        {
            let writer_tx_resp = writer_tx.clone();
            thread::spawn(move || {
                while let Ok(text) = response_rx.recv() {
                    if writer_tx_resp
                        .try_send(Bytes::from(text.into_bytes()))
                        .is_err()
                    {
                        break;
                    }
                }
            });
        }

        // Reader thread: read PTY output → feed alacritty parser → update state.
        {
            let term = term.clone();
            let last_output_at = last_output_at.clone();
            let last_raw_output = last_raw_output.clone();
            let output_notify = output_notify.clone();
            let snapshots = snapshots.clone();
            let snap_scroll = snap_scroll.clone();
            let recording = recording.clone();
            thread::spawn(move || {
                let mut processor = Processor::<StdSyncHandler>::new();
                let mut buf = [0u8; 8192];
                let mut snap_counter: u32 = 0;
                let recording_start = std::time::Instant::now();
                loop {
                    match std::io::Read::read(&mut reader, &mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let data = &buf[..n];
                            {
                                let mut t = term.lock();
                                processor.advance(&mut *t, data);

                                // Snapshot screen periodically for alternate-screen scrollback.
                                snap_counter += 1;
                                if snap_counter >= 4 {
                                    snap_counter = 0;
                                    let grid = t.grid();
                                    let cols = grid.columns();
                                    let rows = grid.screen_lines();
                                    let row_strings: Vec<String> = (0..rows)
                                        .map(|r| {
                                            let mut line = String::with_capacity(cols);
                                            for c in 0..cols {
                                                let p = Point::new(Line(r as i32), Column(c));
                                                let cell = &grid[p];
                                                if cell.flags.contains(
                                                    alacritty_terminal::term::cell::Flags::WIDE_CHAR_SPACER,
                                                ) {
                                                    continue;
                                                }
                                                if cell.c == '\0' {
                                                    line.push(' ');
                                                } else {
                                                    let mut tmp = [0u8; 4];
                                                    line.push_str(cell.c.encode_utf8(&mut tmp));
                                                }
                                            }
                                            line
                                        })
                                        .collect();
                                    let mut snaps = snapshots.write();
                                    if snaps.back() != Some(&row_strings) {
                                        if snaps.len() == snaps.capacity() {
                                            snaps.pop_front();
                                        }
                                        snaps.push_back(row_strings);
                                        snap_scroll.store(0, Ordering::Relaxed);
                                    }
                                }
                            }
                            let mut raw = last_raw_output.write();
                            raw.extend_from_slice(data);
                            const MAX_RAW: usize = 1024 * 1024;
                            if raw.len() > MAX_RAW {
                                let excess = raw.len() - MAX_RAW;
                                raw.drain(..excess);
                            }
                            last_output_at.store(now_secs(), Ordering::Relaxed);
                            output_notify.notify_waiters();
                            // Write to asciinema recording.
                            if let Some(ref mut rec) = *recording.write() {
                                write_recording_event(rec, recording_start, data);
                            }
                        }
                        Err(_) => {
                            warn!("PTY reader error");
                            break;
                        }
                    }
                }
            });
        }

        // Child wait thread.
        {
            let alive = alive.clone();
            thread::spawn(move || {
                let _ = child.wait();
                alive.store(false, Ordering::Relaxed);
            });
        }

        // Writer thread: serialise all writes to the PTY master.
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
            term,
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

    /// Spawn a shell PTY ($SHELL or /bin/sh) in the given directory.
    pub fn spawn_shell(
        workspace_path: &std::path::Path,
        size: (u16, u16),
    ) -> Result<Self> {
        info!("spawning shell in {}", workspace_path.display());
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let mut cmd = portable_pty::CommandBuilder::new(shell);
        cmd.cwd(workspace_path);
        crate::types::Agent::apply_term_env(&mut cmd);

        let pty_system = NativePtySystem::default();
        let pty_size = PtySize {
            rows: size.1,
            cols: size.0,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = pty_system.openpty(pty_size).context("failed to open PTY")?;

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .context("failed to spawn shell")?;
        let child_pid = child.process_id();

        let master = pair.master;
        let mut reader = master
            .try_clone_reader()
            .context("failed to clone PTY reader")?;

        let last_raw_output: Arc<RwLock<Vec<u8>>> =
            Arc::new(RwLock::new(Vec::with_capacity(8192)));
        let alive = Arc::new(AtomicBool::new(true));
        let last_output_at = Arc::new(AtomicU64::new(now_secs()));
        let output_notify = Arc::new(Notify::new());
        let snapshots: Arc<RwLock<std::collections::VecDeque<Vec<String>>>> =
            Arc::new(RwLock::new(std::collections::VecDeque::with_capacity(500)));
        let snap_scroll = Arc::new(AtomicUsize::new(0));

        let (writer_tx, writer_rx) = std::sync::mpsc::sync_channel::<Bytes>(1024);

        let term_size = TermSize::new(size.0 as usize, size.1 as usize);
        let term_config = Config {
            scrolling_history: DEFAULT_SCROLLBACK_LINES,
            ..Config::default()
        };
        let size_shared = Arc::new(std::sync::Mutex::new((size.0, size.1)));
        let (response_tx, response_rx) = std::sync::mpsc::channel::<String>();
        let listener = PtyEventListener {
            response_tx: Some(response_tx),
            size: Some(size_shared),
        };
        let term = Term::new(term_config, &term_size, listener);
        let term = Arc::new(FairMutex::new(term));

        {
            let writer_tx_resp = writer_tx.clone();
            thread::spawn(move || {
                while let Ok(text) = response_rx.recv() {
                    if writer_tx_resp
                        .try_send(Bytes::from(text.into_bytes()))
                        .is_err()
                    {
                        break;
                    }
                }
            });
        }

        {
            let term = term.clone();
            let last_output_at = last_output_at.clone();
            let last_raw_output = last_raw_output.clone();
            let output_notify = output_notify.clone();
            let snapshots = snapshots.clone();
            let snap_scroll = snap_scroll.clone();
            thread::spawn(move || {
                let mut processor = Processor::<StdSyncHandler>::new();
                let mut buf = [0u8; 8192];
                let mut snap_counter: u32 = 0;
                loop {
                    match std::io::Read::read(&mut reader, &mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let data = &buf[..n];
                            {
                                let mut t = term.lock();
                                processor.advance(&mut *t, data);
                                snap_counter += 1;
                                if snap_counter >= 4 {
                                    snap_counter = 0;
                                    let grid = t.grid();
                                    let cols = grid.columns();
                                    let rows = grid.screen_lines();
                                    let row_strings: Vec<String> = (0..rows)
                                        .map(|r| {
                                            let mut line = String::with_capacity(cols);
                                            for c in 0..cols {
                                                let p = Point::new(Line(r as i32), Column(c));
                                                let cell = &grid[p];
                                                if cell.flags.contains(
                                                    alacritty_terminal::term::cell::Flags::WIDE_CHAR_SPACER,
                                                ) {
                                                    continue;
                                                }
                                                if cell.c == '\0' {
                                                    line.push(' ');
                                                } else {
                                                    let mut tmp = [0u8; 4];
                                                    line.push_str(cell.c.encode_utf8(&mut tmp));
                                                }
                                            }
                                            line
                                        })
                                        .collect();
                                    let mut snaps = snapshots.write();
                                    if snaps.back() != Some(&row_strings) {
                                        if snaps.len() == snaps.capacity() {
                                            snaps.pop_front();
                                        }
                                        snaps.push_back(row_strings);
                                        snap_scroll.store(0, Ordering::Relaxed);
                                    }
                                }
                            }
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
            term,
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
        // Chunk large writes to avoid overflowing the bounded channel.
        // Each chunk that fails to send is silently dropped (backpressure).
        const CHUNK: usize = 4096;
        for chunk in data.chunks(CHUNK) {
            let _ = self.writer_tx.try_send(Bytes::from(chunk.to_vec()));
        }
        Ok(())
    }

    pub fn resize(&self, size: (u16, u16)) {
        let mut t = self.term.lock();
        let term_size = TermSize::new(size.0 as usize, size.1 as usize);
        t.resize(term_size);
    }

    /// Returns a cloned Arc to the term mutex for rendering.
    pub(crate) fn term(&self) -> Arc<FairMutex<Term<PtyEventListener>>> {
        self.term.clone()
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Returns the PID of the child process, if available.
    pub fn child_pid(&self) -> Option<u32> {
        self.child_pid
    }

    pub fn scrollback_offset(&self) -> usize {
        self.term.lock().grid().display_offset()
    }

    /// Scroll up: in normal mode uses alacritty scrollback, in alternate screen
    /// mode uses the snapshot buffer.
    pub fn scroll_page_up(&self, page_size: usize) {
        if self.is_alternate_screen() {
            let snaps = self.snapshots.read();
            let max_scroll = snaps.len().saturating_sub(1);
            let current = self.snap_scroll.load(Ordering::Relaxed);
            let new_scroll = (current + 1).min(max_scroll);
            self.snap_scroll.store(new_scroll, Ordering::Relaxed);
        } else {
            let mut t = self.term.lock();
            t.scroll_display(Scroll::Delta(page_size as i32));
        }
    }

    pub fn scroll_page_down(&self, page_size: usize) {
        if self.is_alternate_screen() {
            let current = self.snap_scroll.load(Ordering::Relaxed);
            self.snap_scroll.store(current.saturating_sub(1), Ordering::Relaxed);
        } else {
            let mut t = self.term.lock();
            t.scroll_display(Scroll::Delta(-(page_size as i32)));
        }
    }

    pub fn reset_scroll(&self) {
        self.snap_scroll.store(0, Ordering::Relaxed);
        self.term.lock().scroll_display(Scroll::Bottom);
    }

    pub fn set_scrollback(&self, offset: usize) {
        let mut t = self.term.lock();
        // Alacritty uses display_offset = number of rows scrolled up from bottom.
        t.scroll_display(Scroll::Top);
        // Then scroll back down by (total_history - offset).
        // Actually, let's use a direct approach.
        let total = t.grid().display_offset();
        if offset < total {
            t.scroll_display(Scroll::Delta((total - offset) as i32));
        }
    }

    /// Returns the snapshot at the current scroll position, if any.
    pub fn scrolled_snapshot(&self) -> Option<Vec<String>> {
        let pos = self.snap_scroll.load(Ordering::Relaxed);
        if pos == 0 {
            return None;
        }
        let snaps = self.snapshots.read();
        let idx = snaps.len().saturating_sub(pos);
        snaps.get(idx).cloned()
    }

    pub fn snap_count(&self) -> usize {
        self.snapshots.read().len()
    }

    pub fn snap_scroll_pos(&self) -> usize {
        self.snap_scroll.load(Ordering::Relaxed)
    }

    pub fn is_alternate_screen(&self) -> bool {
        self.term.lock().mode().contains(TermMode::ALT_SCREEN)
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
    pub fn output_notify(&self) -> Arc<Notify> {
        self.output_notify.clone()
    }

    /// Returns the terminal cell at the given viewport-relative position.
    /// Used for search match text extraction.
    pub fn cell_contents(&self, row: usize, col: usize) -> Option<String> {
        let t = self.term.lock();
        let display_offset = t.grid().display_offset();
        let line_idx = row as i32 - display_offset as i32;
        let cols = t.columns();
        if col >= cols {
            return None;
        }
        let p = Point::new(Line(line_idx), Column(col));
        let cell = &t.grid()[p];
        let c = if cell.c == '\0' { ' ' } else { cell.c };
        let mut tmp = [0u8; 4];
        Some(c.encode_utf8(&mut tmp).to_string())
    }

    /// Extract the full visible screen text, one line per row, trailing
    /// whitespace trimmed. Used for clipboard copy and search.
    pub fn screen_contents(&self) -> String {
        let t = self.term.lock();
        let display_offset = t.grid().display_offset();
        let rows = t.screen_lines();
        let cols = t.columns();
        let mut out = String::with_capacity(rows * cols);
        for r in 0..rows {
            let line_idx = r as i32 - display_offset as i32;
            let mut line_buf = String::with_capacity(cols);
            for c in 0..cols {
                let p = Point::new(Line(line_idx), Column(c));
                let cell = &t.grid()[p];
                if cell.flags.contains(
                    alacritty_terminal::term::cell::Flags::WIDE_CHAR_SPACER,
                ) {
                    continue;
                }
                let ch = if cell.c == '\0' { ' ' } else { cell.c };
                let mut tmp = [0u8; 4];
                line_buf.push_str(ch.encode_utf8(&mut tmp));
            }
            let trimmed = line_buf.trim_end();
            out.push_str(trimmed);
            if r + 1 < rows {
                out.push('\n');
            }
        }
        out
    }

    /// Returns the terminal grid dimensions (rows, cols).
    pub fn grid_size(&self) -> (usize, usize) {
        let t = self.term.lock();
        (t.screen_lines(), t.columns())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::collections::VecDeque;

    /// Helper: build a PtyHandle from raw Arc state without spawning a real PTY.
    fn dead_pty_handle(last_output_epoch: u64, alive: bool) -> PtyHandle {
        let size = TermSize::new(80, 24);
        let term_config = Config::default();
        let listener = PtyEventListener::default();
        let term = Term::new(term_config, &size, listener);
        let (writer_tx, _) = std::sync::mpsc::sync_channel(1);
        PtyHandle {
            term: Arc::new(FairMutex::new(term)),
            writer_tx,
            alive: Arc::new(AtomicBool::new(alive)),
            last_output_at: Arc::new(AtomicU64::new(last_output_epoch)),
            last_raw_output: Arc::new(RwLock::new(Vec::new())),
            output_notify: Arc::new(Notify::new()),
            child_pid: None,
            snapshots: Arc::new(RwLock::new(VecDeque::new())),
            snap_scroll: Arc::new(AtomicUsize::new(0)),
        }
    }

    #[test]
    fn create_recording_writes_valid_asciinema_v2_header() {
        let dir = std::env::temp_dir().join("amux_test_recording");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("test.cast");
        let file = create_recording(&path, 120, 40).expect("create_recording");
        // File should be readable and contain valid JSON header.
        drop(file);
        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        let header: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(header["version"], 2);
        assert_eq!(header["width"], 120);
        assert_eq!(header["height"], 40);
        assert!(header["timestamp"].is_number());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_recording_event_formats_correctly() {
        let dir = std::env::temp_dir().join("amux_test_rec_event");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("ev.cast");
        let mut file = create_recording(&path, 80, 24).expect("create_recording");
        let start = std::time::Instant::now();
        write_recording_event(&mut file, start, b"hello world");
        drop(file);
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 2, "should have header + 1 event");
        let event: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(event[0], "o", "event type should be 'o' (output)");
        assert_eq!(event[2], "hello world");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn chrono_free_timestamp_is_reasonable() {
        let ts = chrono_free_timestamp();
        // After 2020-01-01 (1577836800) and before 2100-01-01 (4102444800).
        assert!(ts > 1_577_836_800, "timestamp should be after 2020");
        assert!(ts < 4_102_444_800, "timestamp should be before 2100");
    }

    #[test]
    fn event_listener_default_does_not_panic_on_send() {
        let listener = PtyEventListener::default();
        // Default has no response_tx, so all events should be silently dropped.
        listener.send_event(Event::PtyWrite("test".into()));
        // If we reach here, no panic occurred.
    }

    #[test]
    fn dead_pty_handle_reports_completed_state() {
        // last_output_at = 0 (epoch) → idle for decades, alive = false
        let handle = dead_pty_handle(0, false);
        assert_eq!(handle.state(), PtyState::Completed);
        assert!(!handle.is_alive());
        assert_eq!(handle.grid_size(), (24, 80));
        assert_eq!(handle.snap_count(), 0);
        assert_eq!(handle.snap_scroll_pos(), 0);
        assert_eq!(handle.scrolled_snapshot(), None);
        assert!(handle.take_raw_output().is_empty());
        assert_eq!(handle.child_pid(), None);
        // write_input should fail on dead PTY.
        assert!(handle.write_input(b"hello").is_err());
    }
}
