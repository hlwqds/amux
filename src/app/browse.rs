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
        self.input_mode = InputMode::BrowseDir;
        self.status = "Select directory \u{00b7} Enter: open/select \u{00b7} Backspace: up \u{00b7} Esc: cancel".into();
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
                path: self.browse_dir.parent().unwrap().to_path_buf(),
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
                self.status = format!(
                    "Created workspace: {} \u{2192} {}",
                    ws.name,
                    ws.path.as_ref().unwrap().display()
                );
                self.workspaces.push(ws);
                self.save_config();
                self.rebuild_tree();
                self.input_mode = InputMode::None;
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
                self.status = format!("Created virtual workspace: {}", ws.name);
                self.workspaces.push(ws);
                self.save_config();
                self.rebuild_tree();
                self.input_mode = InputMode::None;
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
