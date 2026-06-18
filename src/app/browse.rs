use std::env;
use std::fs;
use std::path::PathBuf;

use crate::config::generate_id;
use crate::types::*;
use crate::util::*;

impl super::App {
    pub(super) fn start_browse_dir(&mut self) {
        let home = PathBuf::from(env::var("HOME").unwrap_or_else(|_| "/".into()));
        self.browse.dir = home;
        self.load_browse_entries();
        self.view.input_mode = InputMode::BrowseDir;
        self.view.status = "Select directory \u{00b7} Enter: open/select \u{00b7} Backspace: up \u{00b7} Esc: cancel".into();
    }

    pub(super) fn load_browse_entries(&mut self) {
        let mut entries = Vec::new();

        entries.push(DirEntry {
            name: SELECT_CURRENT.into(),
            path: self.browse.dir.clone(),
            is_dir: true,
        });
        entries.push(DirEntry {
            name: SELECT_VIRTUAL.into(),
            path: PathBuf::new(),
            is_dir: false,
        });

        if self.browse.dir.parent().is_some() {
            entries.push(DirEntry {
                name: PARENT_DIR.into(),
                path: self
                    .browse
                    .dir
                    .parent()
                    .unwrap_or(&self.browse.dir)
                    .to_path_buf(),
                is_dir: true,
            });
        }

        if let Ok(rd) = fs::read_dir(&self.browse.dir) {
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

        self.browse.entries = entries;
        self.browse.state.select(Some(0));
    }

    pub(super) fn browse_move(&mut self, delta: isize) {
        let len = self.browse.entries.len();
        if len == 0 {
            return;
        }
        let cur = self.browse.state.selected().unwrap_or(0).min(len - 1) as isize;
        self.browse
            .state
            .select(Some(((cur + delta).rem_euclid(len as isize)) as usize));
    }

    pub(super) fn browse_select(&mut self) {
        let idx = match self.browse.state.selected() {
            Some(i) => i,
            None => return,
        };
        let entry = match self.browse.entries.get(idx) {
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
                    session_ids: Vec::new(),
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
                    session_ids: Vec::new(),
                    expanded: true,
                };
                self.view.status = format!("Created virtual workspace: {}", ws.name);
                self.sessions.workspaces.push(ws);
                self.save_config();
                self.rebuild_tree();
                self.view.input_mode = InputMode::None;
            }
            _ => {
                self.browse.dir = entry.path;
                self.load_browse_entries();
            }
        }
    }

    pub(super) fn browse_up(&mut self) {
        if let Some(parent) = self.browse.dir.parent() {
            self.browse.dir = parent.to_path_buf();
            self.load_browse_entries();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::test_app;

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
        app.browse.entries = vec![
            make_entry("a", "/a", true),
            make_entry("b", "/b", true),
            make_entry("c", "/c", true),
        ];
        app.browse.state.select(Some(0));

        // Move forward wraps
        app.browse_move(1);
        assert_eq!(app.browse.state.selected(), Some(1));

        // Wrap past end back to 0
        app.browse_move(2);
        assert_eq!(app.browse.state.selected(), Some(0));

        // Wrap backwards from 0 to last
        app.browse_move(-1);
        assert_eq!(app.browse.state.selected(), Some(2));
    }

    /// browse_move with empty entries is a no-op (no panic).
    #[test]
    fn browse_move_empty_is_noop() {
        let mut app = test_app(vec![], vec![]);
        app.browse.entries = vec![];
        app.browse.state.select(Some(0));
        app.browse_move(1);
        // selection unchanged, no panic
        assert_eq!(app.browse.state.selected(), Some(0));
    }

    /// browse_select with "Select this directory" creates a workspace and exits browse mode.
    #[test]
    fn browse_select_current_creates_workspace() {
        let mut app = test_app(vec![], vec![]);
        app.browse.dir = PathBuf::from("/tmp");
        app.browse.entries = vec![
            make_entry(SELECT_CURRENT, "/tmp", true),
            make_entry(SELECT_VIRTUAL, "", false),
        ];
        app.browse.state.select(Some(0));
        app.new_workspace_name = Some("myws".into());
        app.view.input_mode = InputMode::BrowseDir;

        app.browse_select();

        assert_eq!(app.sessions.workspaces.len(), 1);
        assert_eq!(app.sessions.workspaces[0].name, "myws");
        assert_eq!(
            app.sessions.workspaces[0]
                .path
                .as_ref()
                .map(|p| p.as_os_str()),
            Some(std::ffi::OsStr::new("/tmp"))
        );
        assert_eq!(app.view.input_mode, InputMode::None);
        assert!(app.view.status.contains("myws"));
    }

    /// browse_select with "Virtual" creates a workspace with no path.
    #[test]
    fn browse_select_virtual_creates_virtual_workspace() {
        let mut app = test_app(vec![], vec![]);
        app.browse.dir = PathBuf::from("/tmp");
        app.browse.entries = vec![
            make_entry(SELECT_CURRENT, "/tmp", true),
            make_entry(SELECT_VIRTUAL, "", false),
        ];
        app.browse.state.select(Some(1));
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
        app.browse.dir = PathBuf::from("/tmp/sub");

        app.browse_up();

        assert_eq!(app.browse.dir, PathBuf::from("/tmp"));
    }
}
