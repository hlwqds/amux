use super::*;

impl App {
    pub(crate) fn request_delete(&mut self) {
        let node = self.selected_node().cloned();
        match &node {
            Some(TreeNode::Workspace(wi)) => {
                let name = self.sessions.workspaces[*wi].name.clone();
                let session_count = self
                    .sessions
                    .ws_session_map
                    .get(*wi)
                    .map(|v| v.len())
                    .unwrap_or(0);
                self.view.status =
                    format!("Delete workspace \"{name}\" ({session_count} sessions)? y/n");
                self.pending_delete = node;
                self.view.input_mode = InputMode::ConfirmDelete;
            }
            Some(TreeNode::Session(_wi, _si)) => {
                if self.view.selected_set.is_empty() {
                    if let Some(TreeNode::Session(_, si)) = node.as_ref() {
                        if *si >= self.sessions.sessions.len() {
                            return;
                        }
                        let title = self.sessions.sessions[*si].title.clone();
                        self.view.status = format!("Delete session \"{title}\"? y/n");
                        self.pending_delete = node;
                        self.view.input_mode = InputMode::ConfirmDelete;
                    }
                } else {
                    // Batch delete all marked sessions
                    let count = self.view.selected_set.len();
                    self.view.status = format!("Delete {count} marked session(s)? y/n");
                    self.pending_batch_delete = true;
                    self.view.input_mode = InputMode::ConfirmDelete;
                }
            }
            Some(TreeNode::ActiveTab(pi)) => {
                // Closing a tab doesn't destroy data, no confirmation needed
                let title = self
                    .ptys
                    .ptys
                    .get(*pi)
                    .map(|s| s.info.title.clone())
                    .unwrap_or_default();
                if let Some(slot) = self.ptys.ptys.get(*pi) {
                    self.unregister_pty(&slot.id);
                }
                self.ptys.ptys.remove(*pi);
                if let Some(cur) = self.ptys.active_pty
                    && cur >= self.ptys.ptys.len()
                {
                    self.ptys.active_pty = if self.ptys.ptys.is_empty() {
                        None
                    } else {
                        Some(self.ptys.ptys.len() - 1)
                    };
                }
                if self.ptys.ptys.is_empty() {
                    self.view.focus = Focus::Sidebar;
                }
                self.rebuild_tree();
                self.view.status = format!("Closed tab: {title}");
            }
            _ => {}
        }
    }

    pub(crate) fn confirm_delete(&mut self) {
        self.view.input_mode = InputMode::None;

        if self.pending_batch_delete {
            // Batch delete all marked sessions
            let count = self.view.selected_set.len();
            // Sort descending so indices stay valid as we remove
            let mut to_delete: Vec<usize> = self.view.selected_set.iter().copied().collect();
            to_delete.sort_by(|a, b| b.cmp(a));
            for si in to_delete {
                if si >= self.sessions.sessions.len() {
                    continue;
                }
                let session = self.sessions.sessions[si].clone();
                if let Some(pi) = self.pty_index_for_session(&session.id) {
                    if let Some(slot) = self.ptys.ptys.get(pi) {
                        self.unregister_pty(&slot.id);
                    }
                    self.ptys.ptys.remove(pi);
                }
                let title_path = title_override_path(&session.id);
                let _ = fs::remove_file(&title_path);
                if let Some(jsonl) = find_session_jsonl(&session) {
                    let _ = fs::remove_file(&jsonl);
                }
                self.sessions.sessions.remove(si);
            }
            if let Some(cur) = self.ptys.active_pty
                && cur >= self.ptys.ptys.len()
            {
                self.ptys.active_pty = if self.ptys.ptys.is_empty() {
                    None
                } else {
                    Some(self.ptys.ptys.len() - 1)
                };
            }
            if self.ptys.ptys.is_empty() {
                self.view.focus = Focus::Sidebar;
            }
            self.view.selected_set.clear();
            self.pending_batch_delete = false;
            self.rebuild_tree();
            self.view.status = format!("Deleted {count} session(s)");
            return;
        }

        let node = self.pending_delete.take();
        match node {
            Some(TreeNode::Workspace(wi)) => {
                let name = self.sessions.workspaces[wi].name.clone();
                self.sessions.workspaces.remove(wi);
                self.save_config();
                self.refresh_sessions();
                self.view.status = format!("Deleted workspace: {name}");
            }
            Some(TreeNode::Session(_wi, si)) => {
                if si >= self.sessions.sessions.len() {
                    return;
                }
                let session = self.sessions.sessions[si].clone();
                if let Some(pi) = self.pty_index_for_session(&session.id) {
                    if let Some(slot) = self.ptys.ptys.get(pi) {
                        self.unregister_pty(&slot.id);
                    }
                    self.ptys.ptys.remove(pi);
                    if let Some(cur) = self.ptys.active_pty
                        && cur >= self.ptys.ptys.len()
                    {
                        self.ptys.active_pty = if self.ptys.ptys.is_empty() {
                            None
                        } else {
                            Some(self.ptys.ptys.len() - 1)
                        };
                    }
                }
                let title_path = title_override_path(&session.id);
                let _ = fs::remove_file(&title_path);
                if let Some(jsonl) = find_session_jsonl(&session) {
                    let _ = fs::remove_file(&jsonl);
                }
                let title = session.title;
                self.sessions.sessions.remove(si);
                self.rebuild_tree();
                self.view.status = format!("Deleted session: {title}");
            }
            _ => {}
        }
    }

    pub(crate) fn cancel_delete(&mut self) {
        self.pending_delete = None;
        self.pending_batch_delete = false;
        self.view.input_mode = InputMode::None;
        self.view.status.clear();
    }
}
