use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::types::*;
use crate::util::key_to_bytes;

impl super::App {
    /// Handle key events specific to Amux mode (single-letter command keys,
    /// Page Up/Down, Home/End, and fallback forwarding).
    ///
    /// Called from `handle_key` when the focus is Chat, a PTY is active,
    /// shared keys (F12, Tab, Alt+key, Ctrl+Q/J/K/Y) have been handled,
    /// and we are NOT in Passthrough mode.
    pub(super) fn handle_amux_key(&mut self, idx: usize, key: KeyEvent) -> Result<Action> {
        // Plain letters are commands (like vim normal mode).
        // Modified keys (Ctrl+X, Alt+X, etc.) still forward to PTY.
        if key.modifiers.is_empty()
            && let Some(slot) = self.ptys.ptys.get(idx)
        {
            match key.code {
                // b: scrollback page up
                KeyCode::Char('b') => {
                    if slot.handle.is_alternate_screen() {
                        let _ = slot.handle.write_input(&[27, 91, 53, 126]);
                    } else {
                        slot.handle.scroll_page_up(
                            self.view.last_chat_area.height.saturating_sub(2) as usize,
                        );
                    }
                    return Ok(Action::Continue);
                }
                // f: scrollback search
                KeyCode::Char('f') => {
                    if slot.handle.is_alternate_screen() {
                        let _ = slot.handle.write_input(&[27, 91, 54, 126]);
                    } else {
                        self.view.input_mode = InputMode::ScrollbackSearch;
                        self.view.scrollback_query.clear();
                        self.view.scrollback_matches.clear();
                        self.view.scrollback_match_idx = 0;
                    }
                    return Ok(Action::Continue);
                }
                // t: token usage
                KeyCode::Char('t') => {
                    self.view.input_mode = InputMode::TokenStats;
                    self.view.status = "Token Usage (any key to close)".into();
                    return Ok(Action::Continue);
                }
                // s: stats
                KeyCode::Char('s') => {
                    self.view.input_mode = InputMode::Stats;
                    self.view.status = "Activity Statistics (any key to close)".into();
                    return Ok(Action::Continue);
                }
                // e: chain select
                KeyCode::Char('e') => {
                    if self.chains.chains.is_empty() {
                        self.view.status = "No chains configured. Add chains to config.json.".into();
                    } else {
                        self.view.input_mode = InputMode::ChainSelect;
                        self.chains.chain_state.select(Some(0));
                        self.view.status = "Select chain (Enter=start, Esc=cancel)".into();
                    }
                    return Ok(Action::Continue);
                }
                // g: timeline
                KeyCode::Char('g') => {
                    use crate::discovery::extract_timeline;
                    let timeline = extract_timeline(&self.sessions.sessions);
                    if timeline.is_empty() {
                        self.view.status = "No timeline events found.".into();
                    } else {
                        self.timeline_events = timeline;
                        self.view.input_mode = InputMode::Timeline;
                        self.view.status = format!(
                            "Timeline ({} events, any key to close)",
                            self.timeline_events.len()
                        );
                    }
                    return Ok(Action::Continue);
                }
                // w: agent recommendations
                KeyCode::Char('w') => {
                    use crate::discovery::compute_agent_recommendations;
                    let recs = compute_agent_recommendations(&self.sessions.sessions);
                    if recs.is_empty() {
                        self.view.status = "No session history for recommendations.".into();
                    } else {
                        self.agent_recommendations = recs;
                        self.view.input_mode = InputMode::AgentRecommend;
                        self.view.status = "Agent Recommendations (any key to close)".into();
                    }
                    return Ok(Action::Continue);
                }
                // r: remote view
                KeyCode::Char('r') => {
                    if self.remote_hosts.is_empty() {
                        self.view.status =
                            "No remote hosts configured. Add to config.json remote_hosts.".into();
                    } else {
                        self.remote_sessions.clear();
                        for host in &self.remote_hosts.clone() {
                            let sessions = crate::discovery::discover_remote_sessions(host);
                            self.remote_sessions.extend(sessions);
                        }
                        if self.remote_sessions.is_empty() {
                            self.view.status = "No remote sessions found.".into();
                        } else {
                            self.view.input_mode = InputMode::RemoteView;
                            self.view.status = format!(
                                "Remote Sessions ({} found, any key to close)",
                                self.remote_sessions.len()
                            );
                        }
                    }
                    return Ok(Action::Continue);
                }
                // x: diff
                KeyCode::Char('x') => {
                    let _ = slot;
                    self.start_diff()?;
                    return Ok(Action::Continue);
                }
                // c: toggle bottom terminal split
                KeyCode::Char('c') => {
                    if self.terminal.is_some() {
                        self.terminal = None;
                        self.view.status = "Terminal closed".into();
                    } else {
                        let cwd = slot.info.workspace_path.clone();
                        let size = self.chat_size();
                        // Terminal takes bottom third
                        let term_size = (size.0, (size.1 / 3).max(5));
                        match crate::pty::PtyHandle::spawn_shell(&cwd, term_size) {
                            Ok(handle) => {
                                let id = self.next_pty_id();
                                self.terminal = Some(PtySlot {
                                    id,
                                    handle,
                                    info: RunningInfo {
                                        workspace_path: cwd,
                                        title: "Shell".into(),
                                        session_id: None,
                                        started_at: crate::util::now_secs(),
                                        completed: false,
                                        agent: Agent::Omp, // placeholder
                                        git_info: GitInfo::default(),
                                        check_status: CheckStatus::Pending,
                                        diff_summary: DiffSummary::default(),
                                        project_type: crate::discovery::ProjectType::Unknown,
                                        worktree_branch: None,
                                        snapshot_commit: None,
                                    },
                                    last_screen_hash: 0,
                                    last_recording_at: std::time::Instant::now(),
                                    process_stats: None,
                                });
                                self.view.status = "Terminal opened (c to close)".into();
                            }
                            Err(e) => {
                                self.view.status = format!("Failed to open terminal: {}", e);
                            }
                        }
                    }
                    return Ok(Action::Continue);
                }
                // y: copy visible screen when scrolled up
                KeyCode::Char('y') => {
                    let offset = slot.handle.scrollback_offset();
                    if offset > 0 {
                        let text = slot.handle.screen_contents();
                        match crate::util::clipboard_copy(&text) {
                            Ok(()) => self.view.status = "Screen copied to clipboard".into(),
                            Err(e) => self.view.status = format!("Clipboard error: {e}"),
                        }
                        return Ok(Action::Continue);
                    }
                }
                _ => return Ok(Action::Continue), // swallow all plain letters
            }
        }

        // Page Up/Down (Amux mode only)
        if key.code == KeyCode::PageUp || key.code == KeyCode::PageDown {
            if let Some(slot) = self.ptys.ptys.get(idx) {
                if slot.handle.is_alternate_screen() {
                    let bytes = crate::util::key_to_bytes(&key);
                    if let Err(e) = slot.handle.write_input(&bytes) { self.view.status = format!("Write error: {e}"); }
                } else if key.code == KeyCode::PageUp {
                    slot.handle.scroll_page_up(
                        self.view.last_chat_area.height.saturating_sub(2) as usize,
                    );
                } else {
                    slot.handle.scroll_page_down(
                        self.view.last_chat_area.height.saturating_sub(2) as usize,
                    );
                }
            }
            return Ok(Action::Continue);
        }
        // Home (Amux mode)
        if key.code == KeyCode::Home {
            if let Some(slot) = self.ptys.ptys.get(idx) {
                if slot.handle.is_alternate_screen() {
                    let bytes = crate::util::key_to_bytes(&key);
                    if let Err(e) = slot.handle.write_input(&bytes) { self.view.status = format!("Write error: {e}"); }
                } else {
                    slot.handle.scroll_page_up(99999);
                }
            }
            return Ok(Action::Continue);
        }
        // End (Amux mode)
        if key.code == KeyCode::End {
            if let Some(slot) = self.ptys.ptys.get(idx) {
                if slot.handle.is_alternate_screen() {
                    let bytes = crate::util::key_to_bytes(&key);
                    if let Err(e) = slot.handle.write_input(&bytes) { self.view.status = format!("Write error: {e}"); }
                } else {
                    slot.handle.reset_scroll();
                }
            }
            return Ok(Action::Continue);
        }

        // Amux mode fallback: forward to PTY (non-letter keys, modified keys)
        let bytes = key_to_bytes(&key);
        if !bytes.is_empty() {
            // When terminal split is open, forward to shell instead of main PTY
            if let Some(term) = &self.terminal {
                if let Err(e) = term.handle.write_input(&bytes) { self.view.status = format!("Write error: {e}"); }
            } else if let Some(slot) = self.ptys.ptys.get(idx) {
                slot.handle.reset_scroll();
                if let Err(e) = slot.handle.write_input(&bytes) { self.view.status = format!("Write error: {e}"); }
            }
        }
        Ok(Action::Continue)
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::app::tests::{test_app, ws, sess};
    use crate::pty::PtyHandle;
    use crate::types::*;

    /// Build a minimal PtySlot with a real shell in /tmp.
    fn make_slot(id: &str) -> PtySlot {
        let handle = PtyHandle::spawn_shell(std::path::Path::new("/tmp"), (80, 24))
            .expect("spawn_shell failed");
        PtySlot {
            id: id.into(),
            handle,
            info: RunningInfo {
                workspace_path: std::path::PathBuf::from("/tmp"),
                title: "test".into(),
                session_id: None,
                started_at: crate::util::now_secs(),
                completed: false,
                agent: Agent::Omp,
                git_info: GitInfo::default(),
                check_status: CheckStatus::Pending,
                diff_summary: DiffSummary::default(),
                project_type: crate::discovery::ProjectType::Unknown,
                worktree_branch: None,
                snapshot_commit: None,
            },
            last_screen_hash: 0,
            last_recording_at: std::time::Instant::now(),
            process_stats: None,
        }
    }

    fn plain_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    // 1. Pressing 'c' opens terminal split mode
    #[test]
    fn c_opens_terminal_split() {
        let mut app = test_app(vec![], vec![]);
        app.view.last_chat_area = ratatui::layout::Rect::new(0, 0, 80, 24);
        app.ptys.ptys.push(make_slot("pty-1"));
        assert!(app.terminal.is_none());

        let result = app.handle_amux_key(0, plain_key(KeyCode::Char('c')));
        assert!(result.is_ok());
        assert!(app.terminal.is_some(), "terminal split should be opened");
        assert!(
            app.view.status.contains("Terminal opened"),
            "status should confirm terminal opened, got: {:?}",
            app.view.status,
        );

        // Pressing 'c' again closes it
        let result = app.handle_amux_key(0, plain_key(KeyCode::Char('c')));
        assert!(result.is_ok());
        assert!(app.terminal.is_none(), "terminal split should be closed");
        assert!(app.view.status.contains("Terminal closed"));
    }

    // 2. Pressing 'x' triggers diff (first press selects left session)
    #[test]
    fn x_triggers_diff() {
        let workspaces = vec![ws("w1", "Project", "/tmp")];
        let sessions = vec![sess("s1", "fix bug", "/tmp")];
        let mut app = test_app(workspaces, sessions);
        app.ptys.ptys.push(make_slot("pty-1"));

        // Select the session node in the tree.
        // test_app selects index 0; with 1 ws + 1 session, tree is [Ws, Session].
        // Select index 1 (the session).
        if app.sessions.tree.len() > 1 {
            app.sessions.tree_state.select(Some(1));
        }

        let result = app.handle_amux_key(0, plain_key(KeyCode::Char('x')));
        assert!(result.is_ok());
        // First press selects left session for diff
        assert!(
            app.popup.diff_left_session.is_some(),
            "first 'x' should select left session for diff, got status: {:?}",
            app.view.status,
        );
        assert!(
            app.view.status.contains("Diff:"),
            "status should mention diff, got: {:?}",
            app.view.status,
        );
    }

    // 3. Pressing '?' is swallowed by the catch-all arm
    #[test]
    fn question_mark_is_swallowed() {
        let mut app = test_app(vec![], vec![]);
        app.ptys.ptys.push(make_slot("pty-1"));

        let result = app.handle_amux_key(0, plain_key(KeyCode::Char('?')));
        assert!(result.is_ok());
        assert!(
            matches!(result, Ok(Action::Continue)),
            "'?' should return Ok(Continue), got {:?}",
            result.as_ref().map(|_| "Continue").err()
        );
    }

    // 4. Pressing 'o' is swallowed by the catch-all arm
    #[test]
    fn o_is_swallowed() {
        let mut app = test_app(vec![], vec![]);
        app.ptys.ptys.push(make_slot("pty-1"));

        let result = app.handle_amux_key(0, plain_key(KeyCode::Char('o')));
        assert!(result.is_ok());
        assert!(
            matches!(result, Ok(Action::Continue)),
            "'o' should return Ok(Continue)"
        );
    }

    // 5. Pressing 'p' is swallowed by the catch-all arm
    #[test]
    fn p_is_swallowed() {
        let mut app = test_app(vec![], vec![]);
        app.ptys.ptys.push(make_slot("pty-1"));

        let result = app.handle_amux_key(0, plain_key(KeyCode::Char('p')));
        assert!(result.is_ok());
        assert!(
            matches!(result, Ok(Action::Continue)),
            "'p' should return Ok(Continue)"
        );
    }
}
