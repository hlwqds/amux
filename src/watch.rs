use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use tracing::warn;

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

/// Watches agent session directories for file system changes.
/// Sends a notification on the channel whenever a change is detected.
pub struct SessionWatcher {
    _watcher: Option<RecommendedWatcher>,
    rx: mpsc::Receiver<()>,
}

impl Default for SessionWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionWatcher {
    /// Create a new watcher that monitors all known agent session directories.
    /// Uses a debounce of 500ms to avoid rapid-fire events.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        let mut watcher = match RecommendedWatcher::new(
            move |_event: Result<notify::Event, notify::Error>| {
                // Ignore errors; just signal that something changed
                let _ = tx.send(());
            },
            Config::default().with_poll_interval(Duration::from_millis(500)),
        ) {
            Ok(w) => w,
            Err(e) => {
                warn!("watch error: failed to create file watcher: {e}");
                return SessionWatcher { _watcher: None, rx };
            }
        };

        // Watch all agent session directories that exist
        let home = std::env::var("HOME").unwrap_or_default();
        let dirs: Vec<PathBuf> = vec![
            PathBuf::from(format!("{home}/.claude/projects")),
            PathBuf::from(format!("{home}/.codex/sessions")),
            std::env::var("PI_CODING_AGENT_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(format!("{home}/.omp/agent")))
                .join("sessions"),
        ];

        for dir in &dirs {
            if dir.exists() {
                let _ = watcher.watch(dir, RecursiveMode::NonRecursive);
            }
        }

        SessionWatcher {
            _watcher: Some(watcher),
            rx,
        }
    }

    /// Check if any file system changes have been detected since last check.
    /// Returns true if sessions should be refreshed.
    pub fn poll(&self) -> bool {
        // Drain all pending notifications — we only need one refresh
        let mut changed = false;
        while self.rx.try_recv().is_ok() {
            changed = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn new_creates_valid_watcher() {
        let watcher = SessionWatcher::new();
        // A fresh watcher should have no pending changes
        assert!(!watcher.poll());
    }

    #[test]
    fn default_matches_new() {
        let watcher = SessionWatcher::default();
        assert!(!watcher.poll());
    }

    #[test]
    fn poll_returns_false_when_nothing_changed() {
        let watcher = SessionWatcher::new();
        assert!(!watcher.poll());
        assert!(!watcher.poll());
        assert!(!watcher.poll());
    }

    #[test]
    fn notify_detects_file_change() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let sessions_dir = temp.path().join("sessions");
        fs::create_dir_all(&sessions_dir).expect("create sessions dir");

        // Point PI_CODING_AGENT_DIR at temp so the watcher monitors our dir.
        // SAFETY: test-only; env var is restored before return.
        #[allow(clippy::env_set_var)]
        unsafe {
            std::env::set_var("PI_CODING_AGENT_DIR", temp.path());
        }

        let watcher = SessionWatcher::new();

        // Trigger a filesystem event inside the watched directory
        fs::write(sessions_dir.join("trigger.txt"), b"hello").expect("write file");

        // Give the OS watcher (inotify on Linux) a moment to deliver the event
        std::thread::sleep(Duration::from_millis(200));

        // Drain multiple times with a short back-off to handle timing jitter
        let mut detected = false;
        for _ in 0..5 {
            if watcher.poll() {
                detected = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        #[allow(clippy::env_set_var)]
        unsafe {
            std::env::remove_var("PI_CODING_AGENT_DIR");
        }

        assert!(detected, "watcher should detect file change in watched dir");
    }
}
