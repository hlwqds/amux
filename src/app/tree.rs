use super::*;

impl App {
    pub(crate) fn rebuild_tree(&mut self) {
        let mut tree = Vec::new();
        let mut ws_map = Vec::new();
        let parsed = self
            .view
            .search_query
            .as_deref()
            .map(crate::util::parse_search_query);
        let query = parsed.as_ref().and_then(|p| p.text.as_deref());
        let date_cutoff = parsed.as_ref().and_then(|p| p.min_last_active);
        // Collect pinned session indices first — they go into a virtual "Pinned" workspace
        let pinned_idxs: Vec<usize> = self
            .sessions
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| s.pinned)
            .map(|(i, _)| i)
            .collect();
        // Always show Pinned workspace
        tree.push(TreeNode::PinnedWorkspace);
        if self.sessions.pinned_expanded {
            let mut sorted_pins = pinned_idxs;
            sorted_pins.sort_by(|&a, &b| {
                self.sessions.sessions[b]
                    .last_active
                    .cmp(&self.sessions.sessions[a].last_active)
            });
            for &si in &sorted_pins {
                let ws_path = &self.sessions.sessions[si].workspace_path;
                let wi = self
                    .sessions
                    .workspaces
                    .iter()
                    .position(|w| {
                        w.path.as_deref() == Some(ws_path)
                            || w.path.as_ref().is_some_and(|p| ws_path.starts_with(p))
                    })
                    .unwrap_or(0);
                tree.push(TreeNode::Session(wi, si));
            }
        }
        // Collect recent sessions: non-pinned, non-active, sorted by last_active desc, top 10
        let active_session_ids: Vec<String> = self
            .ptys
            .ptys
            .iter()
            .filter_map(|slot| slot.info.session_id.clone())
            .collect();
        let mut recent_idxs: Vec<usize> = self
            .sessions
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                !s.pinned
                    && !active_session_ids.iter().any(|sid| sid == &s.id)
                    && s.last_active > 0
                    && {
                        // Only sessions active within the last 7 days
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        now.saturating_sub(s.last_active) < 7 * 24 * 3600
                    }
                    && self.view.agent_filter.is_none_or(|agent| s.agent == agent)
                    && self
                        .view
                        .tag_filter
                        .as_ref()
                        .is_none_or(|tag| s.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
                    && date_cutoff.is_none_or(|cutoff| s.last_active >= cutoff)
            })
            .map(|(i, _)| i)
            .collect();
        recent_idxs.sort_by(|&a, &b| {
            self.sessions.sessions[b]
                .last_active
                .cmp(&self.sessions.sessions[a].last_active)
        });
        recent_idxs.truncate(10);
        self.sessions.recent_count = recent_idxs.len();
        // Always show Recent workspace
        tree.push(TreeNode::RecentWorkspace);
        if self.sessions.recent_expanded {
            for &si in &recent_idxs {
                let ws_path = &self.sessions.sessions[si].workspace_path;
                let wi = self
                    .sessions
                    .workspaces
                    .iter()
                    .position(|w| {
                        w.path.as_deref() == Some(ws_path)
                            || w.path.as_ref().is_some_and(|p| ws_path.starts_with(p))
                    })
                    .unwrap_or(usize::MAX);
                tree.push(TreeNode::Session(wi, si));
            }
        }
        for (wi, _ws) in self.sessions.workspaces.iter().enumerate() {
            let sess_idxs: Vec<usize> = self
                .sessions
                .sessions
                .iter()
                .enumerate()
                .filter(|(_i, s)| {
                    self.ws_matches_path(wi, &s.workspace_path)
                            && !s.pinned  // pinned sessions shown in virtual workspace above
                            // recent sessions are ALSO shown in their normal workspaces (per spec)
                            && self.view.agent_filter.is_none_or(|agent| s.agent == agent)
                            && self.view.tag_filter.as_ref().is_none_or(|tag| {
                                s.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
                            })
                            && date_cutoff.is_none_or(|cutoff| s.last_active >= cutoff)
                })
                .map(|(i, _)| i)
                .collect();
            if let Some(q) = query {
                // Fuzzy-filter sessions for this workspace
                let mut matching_sessions: Vec<usize> = sess_idxs
                    .into_iter()
                    .filter(|&si| {
                        let session = &self.sessions.sessions[si];
                        let short_id = &session.id[..session.id.len().min(8)];
                        session_fuzzy_score(session.title.as_str(), short_id, q)
                            || session_fuzzy_score(&self.sessions.workspaces[wi].name, short_id, q)
                    })
                    .collect();
                self.sort_session_indices(&mut matching_sessions);
                // Fuzzy-filter active PTYs for this workspace
                let matching_ptys: Vec<usize> = self
                    .ptys
                    .ptys
                    .iter()
                    .enumerate()
                    .filter(|(_pi, slot)| {
                        self.ws_matches_path(wi, &slot.info.workspace_path)
                            && slot.info.session_id.is_none()
                            && self.view.agent_filter.is_none_or(|a| slot.info.agent == a)
                            && session_fuzzy_score(&slot.info.title, &slot.info.title, q)
                    })
                    .map(|(pi, _)| pi)
                    .collect();
                // Include workspace only if it matches itself or has matching children
                let ws_matches = session_fuzzy_score(&self.sessions.workspaces[wi].name, "", q);
                if ws_matches || !matching_sessions.is_empty() || !matching_ptys.is_empty() {
                    tree.push(TreeNode::Workspace(wi));
                    if let Some(ref ws_path) = self.sessions.workspaces[wi].path
                        && !ws_path.exists()
                    {
                        tree.push(TreeNode::WorkspaceWarning(
                            wi,
                            format!(
                                "Path not found: {}. Update config.json or create the directory.",
                                ws_path.display()
                            ),
                        ));
                    }
                    for &pi in &matching_ptys {
                        tree.push(TreeNode::ActiveTab(pi));
                    }
                    if self.view.sort_mode == SortMode::AgentGroup {
                        Self::append_agent_grouped(
                            &self.sessions.sessions,
                            &matching_sessions,
                            wi,
                            &mut tree,
                        );
                    } else {
                        for &si in &matching_sessions {
                            tree.push(TreeNode::Session(wi, si));
                        }
                    }
                }
                ws_map.push(matching_sessions);
            } else {
                let mut sorted_idxs = sess_idxs.clone();
                self.sort_session_indices(&mut sorted_idxs);
                tree.push(TreeNode::Workspace(wi));
                if let Some(ref ws_path) = self.sessions.workspaces[wi].path
                    && !ws_path.exists()
                {
                    tree.push(TreeNode::WorkspaceWarning(
                        wi,
                        format!(
                            "Path not found: {}. Update config.json or create the directory.",
                            ws_path.display()
                        ),
                    ));
                }
                if self.sessions.workspaces[wi].expanded {
                    for (pi, slot) in self.ptys.ptys.iter().enumerate() {
                        if self.ws_matches_path(wi, &slot.info.workspace_path)
                            && slot.info.session_id.is_none()
                            && self.view.agent_filter.is_none_or(|a| slot.info.agent == a)
                        {
                            tree.push(TreeNode::ActiveTab(pi));
                        }
                    }
                    if self.view.sort_mode == SortMode::AgentGroup {
                        Self::append_agent_grouped(
                            &self.sessions.sessions,
                            &sorted_idxs,
                            wi,
                            &mut tree,
                        );
                    } else {
                        for &si in &sorted_idxs {
                            tree.push(TreeNode::Session(wi, si));
                        }
                    }
                }
                ws_map.push(sess_idxs);
            }
        }

        // Append archived section when toggled visible
        if self.sessions.show_archived && !self.sessions.archived_sessions.is_empty() {
            tree.push(TreeNode::ArchivedHeader);
            let mut archived_idxs: Vec<usize> =
                (0..self.sessions.archived_sessions.len()).collect();
            archived_idxs.sort_by(|&a, &b| {
                self.sessions.archived_sessions[b]
                    .last_active
                    .cmp(&self.sessions.archived_sessions[a].last_active)
            });
            for ai in archived_idxs {
                // Find workspace index for the archived session
                let ws_path = &self.sessions.archived_sessions[ai].workspace_path;
                let wi = self
                    .sessions
                    .workspaces
                    .iter()
                    .position(|w| {
                        w.path.as_deref() == Some(ws_path)
                            || w.path.as_ref().is_some_and(|p| ws_path.starts_with(p))
                    })
                    .unwrap_or(0);
                tree.push(TreeNode::ArchivedSession(wi, ai));
            }
        }

        self.sessions.tree = tree;
        self.sessions.ws_session_map = ws_map;

        // Clamp selection to valid range
        if !self.sessions.tree.is_empty() {
            self.move_sel(0);
        }
    }

    pub(crate) fn move_sel(&mut self, delta: isize) {
        let len = self.sessions.tree.len();
        if len == 0 {
            return;
        }
        let cur = self
            .sessions
            .tree_state
            .selected()
            .unwrap_or(0)
            .min(len - 1) as isize;
        self.sessions
            .tree_state
            .select(Some(((cur + delta).rem_euclid(len as isize)) as usize));
    }

    /// Navigate the sidebar tree to select a specific session by ID.
    pub(crate) fn navigate_to_session(&mut self, session_id: &str) {
        // Find the tree index for the session.
        let tree_idx = self.sessions.tree.iter().position(|node| {
            if let TreeNode::Session(_wi, si) = node {
                self.sessions
                    .sessions
                    .get(*si)
                    .is_some_and(|s| s.id == session_id)
            } else {
                false
            }
        });
        if let Some(idx) = tree_idx {
            self.sessions.tree_state.select(Some(idx));
            self.view.status = format!(
                "Selected session {}.",
                &session_id[..8.min(session_id.len())]
            );
        } else {
            self.view.status = format!(
                "Session {} not found in sidebar.",
                &session_id[..8.min(session_id.len())]
            );
        }
    }

    pub(crate) fn toggle_expand(&mut self) {
        match self.selected_node() {
            Some(TreeNode::Workspace(wi)) => {
                let wi = *wi;
                self.sessions.workspaces[wi].expanded = !self.sessions.workspaces[wi].expanded;
                self.rebuild_tree();
                self.save_config();
            }
            Some(TreeNode::PinnedWorkspace) => {
                self.sessions.pinned_expanded = !self.sessions.pinned_expanded;
                self.rebuild_tree();
                self.save_config();
            }
            Some(TreeNode::RecentWorkspace) => {
                self.sessions.recent_expanded = !self.sessions.recent_expanded;
                self.rebuild_tree();
                self.save_config();
            }
            _ => {}
        }
    }

    pub(crate) fn toggle_selection(&mut self) {
        if let Some(TreeNode::Session(_wi, si)) = self.selected_node().cloned() {
            if self.view.selected_set.contains(&si) {
                self.view.selected_set.remove(&si);
                self.view.status = format!("Unmarked session ({})", self.view.selected_set.len());
            } else {
                self.view.selected_set.insert(si);
                self.view.status = format!("Marked session ({})", self.view.selected_set.len());
            }
        }
    }
}
