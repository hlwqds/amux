use std::env;
use std::fs;
use std::path::PathBuf;

use crate::config::generate_id;
use crate::types::*;
use crate::util::*;

impl super::App {
    pub(super) fn start_browse_dir(&mut self) {
        let home = PathBuf::from(env::var("HOME").unwrap_or_else(|_| "/".into()));
        self.browse_dir = home;
        self.load_browse_entries();
        self.view.input_mode = InputMode::BrowseDir;
        self.view.status = "Select directory \u{00b7} Enter: open/select \u{00b7} Backspace: up \u{00b7} Esc: cancel".into();
    }

    pub(super) fn load_browse_entries(&mut self) {
        let mut entries = Vec::new();

        entries.push(DirEntry {
            name: SELECT_CURRENT.into(),
            path: self.browse_dir.clone(),
            is_dir: true,
        });
        entries.push(DirEntry {
            name: SELECT_VIRTUAL.into(),
            path: PathBuf::new(),
            is_dir: false,
        });

        if self.browse_dir.parent().is_some() {
            entries.push(DirEntry {
                name: PARENT_DIR.into(),
                path: self
                    .browse_dir
                    .parent()
                    .unwrap_or(&self.browse_dir)
                    .to_path_buf(),
                is_dir: true,
            });
        }

        if let Ok(rd) = fs::read_dir(&self.browse_dir) {
            let mut subdirs: Vec<DirEntry> = rd
                .flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') {
                        return None;
                    }
                    Some(DirEntry {
                        name,
                        path: e.path(),
                        is_dir: true,
                    })
                })
                .collect();
            subdirs.sort_by_key(|a| a.name.to_lowercase());
            entries.extend(subdirs);
        }

        self.browse_entries = entries;
        self.browse_state.select(Some(0));
    }

    pub(super) fn browse_move(&mut self, delta: isize) {
        let len = self.browse_entries.len();
        if len == 0 {
            return;
        }
        let cur = self.browse_state.selected().unwrap_or(0).min(len - 1) as isize;
        self.browse_state
            .select(Some(((cur + delta).rem_euclid(len as isize)) as usize));
    }

    pub(super) fn browse_select(&mut self) {
        let idx = match self.browse_state.selected() {
            Some(i) => i,
            None => return,
        };
        let entry = match self.browse_entries.get(idx) {
            Some(e) => e.clone(),
            None => return,
        };

        match entry.name.as_str() {
            SELECT_CURRENT => {
                let name = self.new_workspace_name.take().unwrap_or_default();
                let ws = Workspace {
                    id: generate_id(),
                    name,
                    path: Some(entry.path.clone()),
                    created_at: now_secs(),
                    expanded: true,
                };
                self.view.status = match ws.path.as_ref() {
                    Some(p) => format!("Created workspace: {} \u{2192} {}", ws.name, p.display()),
                    None => format!("Created workspace: {}", ws.name),
                };
                self.sessions.workspaces.push(ws);
                self.save_config();
                self.rebuild_tree();
                self.view.input_mode = InputMode::None;
            }
            SELECT_VIRTUAL => {
                let name = self.new_workspace_name.take().unwrap_or_default();
                let ws = Workspace {
                    id: generate_id(),
                    name,
                    path: None,
                    created_at: now_secs(),
                    expanded: true,
                };
                self.view.status = format!("Created virtual workspace: {}", ws.name);
                self.sessions.workspaces.push(ws);
                self.save_config();
                self.rebuild_tree();
                self.view.input_mode = InputMode::None;
            }
            PARENT_DIR => {
                self.browse_dir = entry.path;
                self.load_browse_entries();
            }
            _ => {
                self.browse_dir = entry.path;
                self.load_browse_entries();
            }
        }
    }

    pub(super) fn browse_up(&mut self) {
        if let Some(parent) = self.browse_dir.parent() {
            self.browse_dir = parent.to_path_buf();
            self.load_browse_entries();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::test_app;
    use crate::types::*;
    use ratatui::widgets::ListState;

    fn make_entry(name: &str, path: &str, is_dir: bool) -> DirEntry {
        DirEntry {
            name: name.into(),
            path: PathBuf::from(path),
            is_dir,
        }
    }

    /// browse_move wraps around and clamps correctly.
    #[test]
    fn browse_move_wraps_and_clamps() {
        let mut app = test_app(vec![], vec![]);
        app.browse_entries = vec![
            make_entry("a", "/a", true),
            make_entry("b", "/b", true),
            make_entry("c", "/c", true),
        ];
        app.browse_state.select(Some(0));

        // Move forward wraps
        app.browse_move(1);
        assert_eq!(app.browse_state.selected(), Some(1));

        // Wrap past end back to 0
        app.browse_move(2);
        assert_eq!(app.browse_state.selected(), Some(0));

        // Wrap backwards from 0 to last
        app.browse_move(-1);
        assert_eq!(app.browse_state.selected(), Some(2));
    }

    /// browse_move with empty entries is a no-op (no panic).
    #[test]
    fn browse_move_empty_is_noop() {
        let mut app = test_app(vec![], vec![]);
        app.browse_entries = vec![];
        app.browse_state.select(Some(0));
        app.browse_move(1);
        // selection unchanged, no panic
        assert_eq!(app.browse_state.selected(), Some(0));
    }

    /// browse_select with "Select this directory" creates a workspace and exits browse mode.
    #[test]
    fn browse_select_current_creates_workspace() {
        let mut app = test_app(vec![], vec![]);
        app.browse_dir = PathBuf::from("/tmp");
        app.browse_entries = vec![
            make_entry(SELECT_CURRENT, "/tmp", true),
            make_entry(SELECT_VIRTUAL, "", false),
        ];
        app.browse_state.select(Some(0));
        app.new_workspace_name = Some("myws".into());
        app.view.input_mode = InputMode::BrowseDir;

        app.browse_select();

        assert_eq!(app.sessions.workspaces.len(), 1);
        assert_eq!(app.sessions.workspaces[0].name, "myws");
        assert_eq!(
            app.sessions.workspaces[0].path.as_ref().map(|p| p.as_os_str()),
            Some(std::ffi::OsStr::new("/tmp"))
        );
        assert_eq!(app.view.input_mode, InputMode::None);
        assert!(app.view.status.contains("myws"));
    }

    /// browse_select with "Virtual" creates a workspace with no path.
    #[test]
    fn browse_select_virtual_creates_virtual_workspace() {
        let mut app = test_app(vec![], vec![]);
        app.browse_dir = PathBuf::from("/tmp");
        app.browse_entries = vec![
            make_entry(SELECT_CURRENT, "/tmp", true),
            make_entry(SELECT_VIRTUAL, "", false),
        ];
        app.browse_state.select(Some(1));
        app.new_workspace_name = Some("virt".into());
        app.view.input_mode = InputMode::BrowseDir;

        app.browse_select();

        assert_eq!(app.sessions.workspaces.len(), 1);
        assert!(app.sessions.workspaces[0].path.is_none());
        assert!(app.view.status.contains("virtual"));
        assert!(app.view.status.contains("virt"));
    }

    /// browse_up navigates to parent directory.
    #[test]
    fn browse_up_goes_to_parent() {
        let mut app = test_app(vec![], vec![]);
        app.browse_dir = PathBuf::from("/tmp/sub");

        app.browse_up();

        assert_eq!(app.browse_dir, PathBuf::from("/tmp"));
    }
}
