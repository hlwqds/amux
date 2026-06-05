use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::types::*;
use crate::util::{clipboard_copy, key_to_bytes};
impl super::App {
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Result<Action> {
        if self.view.input_mode != InputMode::None {
            return self.handle_input_key(key);
        }

        if self.view.focus == Focus::Chat {
            if let Some(idx) = self.ptys.active_pty {
                // ── Shared: always intercept these regardless of ChatMode ──

                // F12: toggle between Passthrough and Amux mode
                if key.code == KeyCode::F(12) {
                    self.view.chat_mode = match self.view.chat_mode {
                        ChatMode::Passthrough => {
                            self.view.status = "Amux".into();
                            ChatMode::Amux
                        }
                        ChatMode::Amux => {
                            self.view.status = "RAW".into();
                            ChatMode::Passthrough
                        }
                    };
                    return Ok(Action::Continue);
                }

                // Tab / Alt+h: go to sidebar (always)
                if (key.code == KeyCode::Tab && !key.modifiers.contains(KeyModifiers::SHIFT))
                    || (key.code == KeyCode::Char('h') && key.modifiers.contains(KeyModifiers::ALT))
                {
                    self.view.focus = Focus::Sidebar;
                    self.refresh_sessions();
                    self.view.status = "Sessions refreshed.".into();
                    return Ok(Action::Continue);
                }

                // Alt+key: amux operations (always intercepted, never forwarded)
                if key.modifiers.contains(KeyModifiers::ALT)
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    let kb = &self.view.keybinds;
                    if kb.quit.matches_event(&key) {
                        return Ok(Action::Quit);
                    }
                    if kb.refresh.matches_event(&key) {
                        self.refresh_sessions();
                        self.view.status = "Sessions refreshed.".into();
                        return Ok(Action::Continue);
                    }
                    if kb.new_session.matches_event(&key) {
                        self.view.focus = Focus::Sidebar;
                        self.refresh_sessions();
                        self.activate_selection()?;
                        return Ok(Action::Continue);
                    }
                    if kb.help.matches_event(&key) {
                        self.view.input_mode = InputMode::KeybindView;
                        return Ok(Action::Continue);
                    }
                    if kb.preview.matches_event(&key) {
                        self.start_session_preview();
                        return Ok(Action::Continue);
                    }
                    if kb.export.matches_event(&key) {
                        self.export_selected_session();
                        return Ok(Action::Continue);
                    }
                    if kb.copy.matches_event(&key) {
                        if let Some(slot) = self.ptys.ptys.get(idx) {
                            let text = format!("{} ({})", slot.info.title, slot.info.agent.label());
                            match clipboard_copy(&text) {
                                Ok(()) => self.view.status = format!("Copied: {}", text),
                                Err(e) => self.view.status = format!("Copy failed: {}", e),
                            }
                        }
                        return Ok(Action::Continue);
                    }
                    if kb.delete.matches_event(&key) {
                        self.view.focus = Focus::Sidebar;
                        self.request_delete();
                        return Ok(Action::Continue);
                    }
                    if kb.theme.matches_event(&key) {
                        self.open_theme_panel();
                        return Ok(Action::Continue);
                    }
                    if kb.settings.matches_event(&key) {
                        self.view.focus = Focus::Sidebar;
                        self.view.input_mode = InputMode::Settings;
                        self.view.status = "Settings: a=add ws  d=del ws  r=rename ws  k=keybinds  t=themes  Esc=close".into();
                        return Ok(Action::Continue);
                    }
                    if kb.tag_filter.matches_event(&key) {
                        if self.view.tag_filter.is_some() {
                            self.view.tag_filter = None;
                            self.rebuild_tree();
                            self.view.status = "Tag filter cleared.".into();
                        } else {
                            self.view.input_mode = InputMode::TagFilter;
                            self.input_buffer.clear();
                            self.view.status = "Filter by tag (Enter=apply, Esc=cancel):".into();
                        }
                        return Ok(Action::Continue);
                    }
                    // Alt+key with no match: fall through to PTY forward
                }


                // Ctrl+Q: terminate session (always)
                if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    if let Some(slot) = self.ptys.ptys.get(idx) {
                        self.unregister_pty(&slot.id);
                    }
                    self.ptys.ptys.remove(idx);
                    self.ptys.active_pty = None;
                    self.view.focus = Focus::Sidebar;
                    self.refresh_sessions();
                    self.view.status = "Session terminated. Sessions refreshed.".into();
                    return Ok(Action::Continue);
                }

                // Ctrl+J/K: tab switching (always)
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::SHIFT)
                    && (key.code == KeyCode::Char('j') || key.code == KeyCode::Char('k'))
                {
                    if self.ptys.ptys.len() > 1 {
                        let cur = self.ptys.active_pty.unwrap_or(0);
                        let delta = if key.code == KeyCode::Char('j') {
                            1isize
                        } else {
                            -1
                        };
                        let next = ((cur as isize + delta)
                            .rem_euclid(self.ptys.ptys.len() as isize))
                            as usize;
                        self.ptys.active_pty = Some(next);
                        if let Some(s) = self.ptys.ptys.get(next) {
                            s.handle.reset_scroll();
                        }
                        self.view.status = format!(
                            "Switched to: {} ({}/{})",
                            self.ptys.ptys[next].info.title,
                            next + 1,
                            self.ptys.ptys.len()
                        );
                    }
                    return Ok(Action::Continue);
                }

                // Ctrl+Shift+J/K: reorder tabs (always)
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.modifiers.contains(KeyModifiers::SHIFT)
                    && (key.code == KeyCode::Char('J') || key.code == KeyCode::Char('K'))
                {
                    if self.ptys.ptys.len() > 1 {
                        let cur = self.ptys.active_pty.unwrap_or(0);
                        let delta: isize = if key.code == KeyCode::Char('J') {
                            1
                        } else {
                            -1
                        };
                        let new_pos = (cur as isize + delta)
                            .rem_euclid(self.ptys.ptys.len() as isize)
                            as usize;
                        self.ptys.ptys.swap(cur, new_pos);
                        self.ptys.active_pty = Some(new_pos);
                        self.view.status = format!("Moved tab to position {}", new_pos + 1);
                    }
                    return Ok(Action::Continue);
                }

                // Ctrl+Y: copy session info (always)
                if key.code == KeyCode::Char('y') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    if let Some(slot) = self.ptys.ptys.get(idx) {
                        let text = format!("{} ({})", slot.info.title, slot.info.agent.label());
                        match clipboard_copy(&text) {
                            Ok(()) => self.view.status = format!("Copied: {}", text),
                            Err(e) => self.view.status = format!("Copy failed: {}", e),
                        }
                    }
                    return Ok(Action::Continue);
                }

                // ── Passthrough mode: forward everything else to PTY ──
                if self.view.chat_mode == ChatMode::Passthrough {
                    let bytes = key_to_bytes(&key);
                    if !bytes.is_empty()
                        && let Some(slot) = self.ptys.ptys.get(idx)
                    {
                        // Reset scrollback position so the view snaps back
                        // to the bottom (live content) when the user types —
                        // same behaviour as Kitty.
                        slot.handle.reset_scroll();
                        if let Err(e) = slot.handle.write_input(&bytes) { self.view.status = format!("Write error: {e}"); }
                        self.view.screen_changed = true;
                    }
                    return Ok(Action::Continue);
                }
                // ── Amux mode: delegated to handler_amux.rs ──
                return self.handle_amux_key(idx, key);
            }


            // Chat focus but no active PTY
            match key.code {
                KeyCode::Tab => {
                    self.view.focus = Focus::Sidebar;
                    self.refresh_sessions();
                }
                KeyCode::Char('q') | KeyCode::Esc => return Ok(Action::Quit),
                _ => {}
            }
            return Ok(Action::Continue);
        }

        // P1: Use keybinds lookup instead of hardcoded key matching.
        // Only keys NOT in Keybinds (digits, Space, Ctrl+key combos for
        // features without configurable bindings) remain hardcoded.
        let kb = &self.view.keybinds;
        if kb.quit.matches_event(&key) || key.code == KeyCode::Esc {
            return Ok(Action::Quit);
        }
        if kb.move_up.matches_event(&key) || key.code == KeyCode::Up {
            self.move_sel(-1);
            return Ok(Action::Continue);
        }
        if kb.move_down.matches_event(&key) || key.code == KeyCode::Down {
            self.move_sel(1);
            return Ok(Action::Continue);
        }
        if kb.expand.matches_event(&key) {
            self.toggle_expand();
            return Ok(Action::Continue);
        }
        if kb.refresh.matches_event(&key) {
            self.refresh_sessions();
            self.view.status = "Sessions refreshed.".into();
            return Ok(Action::Continue);
        }
        if kb.rename.matches_event(&key) {
            self.start_rename();
            return Ok(Action::Continue);
        }
        if kb.new_workspace.matches_event(&key) {
            self.start_new_workspace();
            return Ok(Action::Continue);
        }
        if kb.delete.matches_event(&key) {
            self.request_delete();
            return Ok(Action::Continue);
        }
        if key.code == KeyCode::Tab {
            if self.ptys.ptys.is_empty() {
                self.view.status = "No active session. Press Enter to start one.".into();
            } else {
                self.view.focus = Focus::Chat;
                if self.ptys.active_pty.is_none() {
                    self.ptys.active_pty = Some(0);
                }
            }
            return Ok(Action::Continue);
        }
        if kb.new_session.matches_event(&key) {
            // Digit keys for agent filters remain hardcoded (not configurable)
            if let KeyCode::Char(c) = key.code {
                match c {
                    '1' => {
                        self.toggle_agent_filter(Agent::Claude);
                        return Ok(Action::Continue);
                    }
                    '2' => {
                        self.toggle_agent_filter(Agent::Codex);
                        return Ok(Action::Continue);
                    }
                    '3' => {
                        self.toggle_agent_filter(Agent::Omp);
                        return Ok(Action::Continue);
                    }
                    _ => {}
                }
            }
            self.activate_selection()?;
            return Ok(Action::Continue);
        }
        // Space for batch toggle (not configurable)
        if key.code == KeyCode::Char(' ') {
            self.toggle_selection();
            return Ok(Action::Continue);
        }
        // Enter to activate
        if key.code == KeyCode::Enter {
            self.activate_selection()?;
            return Ok(Action::Continue);
        }
        if kb.search.matches_event(&key) {
            self.view.input_mode = InputMode::Search;
            self.view.status = "Search (prefix: >7d, >1h, >30m for date filter):".into();
            self.input_buffer.clear();
            return Ok(Action::Continue);
        }
        if kb.help.matches_event(&key) {
            self.view.input_mode = InputMode::KeybindView;
            return Ok(Action::Continue);
        }
        if kb.preview.matches_event(&key) {
            self.start_session_preview();
            return Ok(Action::Continue);
        }
        if kb.export.matches_event(&key) {
            self.export_selected_session();
            return Ok(Action::Continue);
        }
        if kb.copy.matches_event(&key) {
            self.copy_selected_info();
            return Ok(Action::Continue);
        }
        if kb.tag_filter.matches_event(&key) {
            if self.view.tag_filter.is_some() {
                self.view.tag_filter = None;
                self.rebuild_tree();
                self.view.status = "Tag filter cleared.".into();
            } else {
                self.view.input_mode = InputMode::TagFilter;
                self.input_buffer.clear();
                self.view.status = "Filter by tag (Enter=apply, Esc=cancel):".into();
            }
            return Ok(Action::Continue);
        }
        if kb.settings.matches_event(&key) {
            self.view.input_mode = InputMode::Settings;
            self.view.status =
                "Settings: a=add ws  d=del ws  r=rename ws  k=keybinds  t=themes  Esc=close".into();
            return Ok(Action::Continue);
        }
        if kb.theme.matches_event(&key) {
            self.open_theme_panel();
            return Ok(Action::Continue);
        }
        // '!': Toggle pin on selected session
        if key.code == KeyCode::Char('!') {
            self.toggle_pin();
            return Ok(Action::Continue);
        }
        // 'o': Open selected session's workspace directory in file manager
        if key.code == KeyCode::Char('o') && key.modifiers == KeyModifiers::NONE {
            self.open_workspace_dir();
            return Ok(Action::Continue);
        }
        // Sort mode cycle ('s' without modifiers — fallback when not matched by keybinds)
        if key.code == KeyCode::Char('s') && key.modifiers == KeyModifiers::NONE {
            self.cycle_sort_mode();
            return Ok(Action::Continue);
        }
        // Shift+S: Semantic search (BM25)
        if key.code == KeyCode::Char('S') && key.modifiers == KeyModifiers::NONE {
            self.search_results.clear();
            self.input_buffer.clear();
            self.search_result_state.select(None);
            self.view.input_mode = InputMode::SemanticSearch;
            self.view.status = "Semantic Search (type query, Enter=search, Esc=cancel):".into();
            return Ok(Action::Continue);
        }
        // Template select ('p' without modifiers)
        if key.code == KeyCode::Char('p') && key.modifiers == KeyModifiers::NONE {
            if self.templates.is_empty() {
                self.view.status = "No templates saved. Add templates to config.json.".into();
            } else {
                self.view.input_mode = InputMode::TemplateSelect;
                self.view.status = "Select template (Enter=launch, Esc=cancel)".into();
            }
            return Ok(Action::Continue);
        }
        // Alt+Shift+P: Plugin list
        if key.code == KeyCode::Char('P')
            && key.modifiers.contains(KeyModifiers::ALT)
            && key.modifiers.contains(KeyModifiers::SHIFT)
        {
            if self.plugins.is_empty() {
                self.view.status = "No plugins configured. Add plugins to config.json.".into();
            } else {
                self.view.input_mode = InputMode::PluginList;
                self.plugin_state.select(Some(0));
                self.view.status = "Select plugin (Enter=run, Esc=cancel)".into();
            }
            return Ok(Action::Continue);
        }
        // Alt+Shift+A: Automation select
        if key.code == KeyCode::Char('A')
            && key.modifiers.contains(KeyModifiers::ALT)
            && key.modifiers.contains(KeyModifiers::SHIFT)
        {
            if self.automations.is_empty() {
                self.view.status = "No automations saved. Add automations to config.json.".into();
            } else {
                self.view.input_mode = InputMode::AutomationSelect;
                self.automation_state.select(Some(0));
                self.view.status = "Select automation (Enter=run, Esc=cancel)".into();
            }
            return Ok(Action::Continue);
        }
        // Shift+B: Branch
        if key.code == KeyCode::Char('B') {
            self.start_branch()?;
            return Ok(Action::Continue);
        }
        // Alt+Shift+S: Stats
        if key.code == KeyCode::Char('S')
            && key.modifiers.contains(KeyModifiers::ALT)
            && key.modifiers.contains(KeyModifiers::SHIFT)
        {
            self.view.input_mode = InputMode::Stats;
            self.view.status = "Activity Statistics (any key to close)".into();
            return Ok(Action::Continue);
        }
        // Alt+Shift+T: Token stats
        if key.code == KeyCode::Char('T')
            && key.modifiers.contains(KeyModifiers::ALT)
            && key.modifiers.contains(KeyModifiers::SHIFT)
        {
            self.view.input_mode = InputMode::TokenStats;
            self.view.status = "Token Usage (any key to close)".into();
            return Ok(Action::Continue);
        }
        // Alt+Shift+X: Diff
        if key.code == KeyCode::Char('X')
            && key.modifiers.contains(KeyModifiers::ALT)
            && key.modifiers.contains(KeyModifiers::SHIFT)
        {
            self.start_diff()?;
            return Ok(Action::Continue);
        }
        // Alt+Shift+G: Toggle archived sessions visibility
        if key.code == KeyCode::Char('G')
            && key.modifiers.contains(KeyModifiers::ALT)
            && key.modifiers.contains(KeyModifiers::SHIFT)
        {
            self.sessions.show_archived = !self.sessions.show_archived;
            self.rebuild_tree();
            self.view.status = if self.sessions.show_archived {
                format!(
                    "Showing {} archived session(s)",
                    self.sessions.archived_sessions.len()
                )
            } else {
                "Archived sessions hidden".into()
            };
            return Ok(Action::Continue);
        }
        // Alt+Shift+R: Remote view
        if key.code == KeyCode::Char('R')
            && key.modifiers.contains(KeyModifiers::ALT)
            && key.modifiers.contains(KeyModifiers::SHIFT)
        {
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
        // Alt+Shift+G: Timeline
        if key.code == KeyCode::Char('G')
            && key.modifiers.contains(KeyModifiers::ALT)
            && key.modifiers.contains(KeyModifiers::SHIFT)
        {
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
        // Alt+Shift+W: Agent recommendations
        if key.code == KeyCode::Char('W')
            && key.modifiers.contains(KeyModifiers::ALT)
            && key.modifiers.contains(KeyModifiers::SHIFT)
        {
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
        // Alt+Shift+F: Cross-session search
        if key.code == KeyCode::Char('F')
            && key.modifiers.contains(KeyModifiers::ALT)
            && key.modifiers.contains(KeyModifiers::SHIFT)
        {
            if self.input_buffer.is_empty() {
                self.view.status =
                    "Type a search query first (/), then Alt+Shift+F to search all sessions.".into();
            } else {
                use crate::discovery::cross_session_search;
                let results = cross_session_search(&self.sessions.sessions, &self.input_buffer);
                if results.is_empty() {
                    self.view.status =
                        format!("No results for '{}' across sessions.", self.input_buffer);
                } else {
                    self.cross_search_results = results;
                    self.view.input_mode = InputMode::CrossSearch;
                    self.view.status = format!(
                        "Cross-session search ({} sessions, any key to close)",
                        self.cross_search_results.len()
                    );
                }
            }
            return Ok(Action::Continue);
        }
        // Alt+Shift+E: Chain select
        if key.code == KeyCode::Char('E')
            && key.modifiers.contains(KeyModifiers::ALT)
            && key.modifiers.contains(KeyModifiers::SHIFT)
        {
            if self.chains.chains.is_empty() {
                self.view.status = "No chains configured. Add chains to config.json.".into();
            } else {
                self.view.input_mode = InputMode::ChainSelect;
                self.chains.chain_state.select(Some(0));
                self.view.status = "Select chain (Enter=start, Esc=cancel)".into();
            }
            return Ok(Action::Continue);
        }
        Ok(Action::Continue)
    }

    fn handle_input_key(&mut self, key: KeyEvent) -> Result<Action> {
        if self.view.input_mode == InputMode::BrowseDir {
            return self.handle_browse_key(key);
        }
        if self.view.input_mode == InputMode::SelectAgent {
            return self.handle_agent_key(key);
        }
        if self.view.input_mode == InputMode::Search {
            return self.handle_search_key(key);
        }
        if self.view.input_mode == InputMode::ScrollbackSearch {
            return self.handle_scrollback_search_key(key);
        }
        if self.view.input_mode == InputMode::TagFilter {
            return self.handle_tag_filter_key(key);
        }
        if self.view.input_mode == InputMode::TemplateSelect {
            return self.handle_template_select_key(key);
        }
        if self.view.input_mode == InputMode::AutomationSelect {
            return self.handle_automation_select_key(key);
        }
        if self.view.input_mode == InputMode::SemanticSearch {
            return self.handle_semantic_search_key(key);
        }
        if self.view.input_mode == InputMode::BranchSelect {
            return self.handle_branch_select_key(key);
        }
        if self.view.input_mode == InputMode::ChainSelect {
            return self.handle_chain_select_key(key);
        }
        // KeybindView: scroll with ↑/↓ or j/k, Esc to close
        if self.view.input_mode == InputMode::KeybindView {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.popup.keybind_scroll = self.popup.keybind_scroll.saturating_sub(1);
                    return Ok(Action::Continue);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.popup.keybind_scroll += 1;
                    return Ok(Action::Continue);
                }
                KeyCode::Esc => {
                    self.popup.keybind_scroll = 0;
                    self.view.input_mode = InputMode::None;
                    return Ok(Action::Continue);
                }
                _ => {} // fall through to panel cycling
            }
        }
        // Panel cycling: Alt+h / Alt+l to switch between popup panels
        if key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::ALT)
            || key.code == KeyCode::Char('h') && key.modifiers.contains(KeyModifiers::ALT)
        {
            let panels: Vec<InputMode> = vec![
                InputMode::KeybindView,
                InputMode::Settings,
                InputMode::ThemeSelect,
                InputMode::Stats,
                InputMode::TokenStats,
            ];
            let current = panels
                .iter()
                .position(|m| *m == self.view.input_mode)
                .unwrap_or(0);
            let next = if key.code == KeyCode::Char('l') {
                (current + 1) % panels.len()
            } else {
                (current + panels.len() - 1) % panels.len()
            };
            let target = panels[next];
            self.view.input_mode = target;
            match target {
                InputMode::Settings => {
                    self.view.status =
                        "Settings: a=add ws  d=del ws  r=rename ws  k=keybinds  t=themes  Esc=close".into();
                }
                InputMode::ThemeSelect => {
                    // Re-sync list in case custom themes changed
                    self.open_theme_panel();
                    return Ok(Action::Continue);
                }
                InputMode::Stats => {
                    self.view.status = "Activity Statistics (any key to close)".into();
                }
                InputMode::TokenStats => {
                    self.view.status = "Token Usage (any key to close)".into();
                }
                _ => {}
            }
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::Help {
            // Any key closes help
            self.view.input_mode = InputMode::None;
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::Stats {
            self.view.input_mode = InputMode::None;
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::TokenStats {
            self.view.input_mode = InputMode::None;
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::CrossSearch {
            self.view.input_mode = InputMode::None;
            self.cross_search_results.clear();
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::DiffView {
            self.view.input_mode = InputMode::None;
            self.popup.diff_lines.clear();
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::AgentRecommend {
            self.view.input_mode = InputMode::None;
            self.agent_recommendations.clear();
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::PluginList {
            return self.handle_plugin_list_key(key);
        }
        if self.view.input_mode == InputMode::PluginOutput {
            return self.handle_plugin_output_key(key);
        }
        if self.view.input_mode == InputMode::Timeline {
            self.view.input_mode = InputMode::None;
            self.timeline_events.clear();
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::ConflictWarning {
            self.view.input_mode = InputMode::None;
            self.popup.conflict_warnings.clear();
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::BudgetWarning {
            self.view.input_mode = InputMode::None;
            // Don't clear budget_alert — it stays in status bar until budget is no longer exceeded
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::ConflictResolve {
            return self.handle_conflict_resolve_key(key);
        }
        if self.view.input_mode == InputMode::RemoteView {
            self.view.input_mode = InputMode::None;
            self.remote_sessions.clear();
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::Settings {
            return self.handle_settings_key(key);
        }
        if self.view.input_mode == InputMode::SessionPreview {
            match key.code {
                KeyCode::Char('s') => {
                    self.popup.preview_show_summary = !self.popup.preview_show_summary;
                    if self.popup.preview_show_summary {
                        self.load_preview_summary();
                    } else {
                        self.reload_preview_content();
                    }
                }
                KeyCode::Char('b') => {
                    // Start rollback: find the snapshot commit for the previewed session
                    if let Some(ref sid) = self.popup.preview_session_id {
                        let snapshot = crate::config::load_snapshot_meta(sid).or_else(|| {
                            // Also check running PTY for snapshot
                            self.ptys.ptys.iter().find_map(|s| {
                                if s.info.session_id.as_deref() == Some(sid) {
                                    s.info.snapshot_commit.clone()
                                } else {
                                    None
                                }
                            })
                        });
                        if let Some(ref commit) = snapshot {
                            // Find the workspace path
                            let ws_path = self
                                .sessions
                                .sessions
                                .iter()
                                .find(|s| s.id == *sid)
                                .map(|s| s.workspace_path.clone());
                            if let Some(ref wp) = ws_path {
                                // Get files that differ between HEAD and snapshot
                                let files = std::process::Command::new("git")
                                    .args(["diff", "--name-only", commit, "HEAD"])
                                    .current_dir(wp)
                                    .output()
                                    .ok()
                                    .filter(|o| o.status.success())
                                    .map(|o| {
                                        String::from_utf8_lossy(&o.stdout)
                                            .lines()
                                            .filter(|l| !l.is_empty())
                                            .map(|l| l.to_string())
                                            .collect::<Vec<_>>()
                                    })
                                    .unwrap_or_default();
                                self.popup.rollback_files = files;
                                self.popup.rollback_snapshot = Some(commit.clone());
                                self.popup.rollback_workspace = Some(wp.clone());
                                self.view.input_mode = InputMode::RollbackConfirm;
                                self.view.status = "Rollback: y=confirm, n=cancel".into();
                                return Ok(Action::Continue);
                            }
                        }
                        self.view.status =
                            "No snapshot found for this session (cannot rollback).".into();
                    }
                }
                KeyCode::Char('k') => {
                    self.popup.knowledge_view = !self.popup.knowledge_view;
                    if self.popup.knowledge_view {
                        self.load_knowledge_preview();
                    } else {
                        // Restore original preview
                        if self.popup.preview_show_summary {
                            self.load_preview_summary();
                        } else {
                            self.reload_preview_content();
                        }
                    }
                }
                KeyCode::Char('c') => {
                    if self.popup.knowledge_view {
                        self.clear_workspace_knowledge();
                    } else {
                        self.view.input_mode = InputMode::None;
                        self.popup.preview_lines.clear();
                        self.popup.preview_show_summary = false;
                    }
                }
                _ => {
                    self.view.input_mode = InputMode::None;
                    self.popup.preview_lines.clear();
                    self.popup.preview_show_summary = false;
                    self.popup.knowledge_view = false;
                }
            }
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::SummaryPreview {
            // Any key dismisses auto-popup
            self.view.input_mode = InputMode::None;
            self.popup.preview_lines.clear();
            self.popup.preview_show_summary = false;
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::ThemeSelect {
            return self.handle_theme_select_key(key);
        }
        if self.view.input_mode == InputMode::ConfirmDelete {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => self.confirm_delete(),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => self.cancel_delete(),
                _ => {}
            }
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::RollbackConfirm {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if let (Some(commit), Some(wp)) = (
                        &self.popup.rollback_snapshot,
                        &self.popup.rollback_workspace,
                    ) {
                        let result = std::process::Command::new("git")
                            .args(["reset", "--hard", commit])
                            .current_dir(wp)
                            .output();
                        match result {
                            Ok(o) if o.status.success() => {
                                self.view.status =
                                    format!("Rolled back to {}", &commit[..8.min(commit.len())]);
                            }
                            Ok(o) => {
                                let err = String::from_utf8_lossy(&o.stderr);
                                self.view.status = format!("Rollback failed: {}", err.trim());
                            }
                            Err(e) => {
                                self.view.status = format!("Rollback error: {}", e);
                            }
                        }
                    }
                    self.popup.rollback_files.clear();
                    self.popup.rollback_snapshot = None;
                    self.popup.rollback_workspace = None;
                    self.view.input_mode = InputMode::None;
                    self.refresh_sessions();
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.popup.rollback_files.clear();
                    self.popup.rollback_snapshot = None;
                    self.popup.rollback_workspace = None;
                    self.view.input_mode = InputMode::None;
                    self.view.status = "Rollback cancelled.".into();
                }
                _ => {}
            }
            return Ok(Action::Continue);
        }
        if self.view.input_mode == InputMode::PreflightConfirm {
            match key.code {
                KeyCode::Enter | KeyCode::Char('p') => {
                    // Proceed — spawn session, clearing preflight state so the
                    // check is not re-triggered.
                    let agent = self.popup.preflight_agent;
                    let name = self.popup.preflight_session_name.take();
                    self.popup.preflight_result = None;
                    self.popup.preflight_workspace = None;
                    self.popup.preflight_agent = None;
                    self.view.input_mode = InputMode::None;
                    if let Some(agent) = agent {
                        self.spawn_with_agent_inner(agent, name)?;
                    } else {
                        self.view.status = "No pending session.".into();
                    }
                }
                KeyCode::Char('f') => {
                    // Fix: git stash if suggested, then re-check.
                    if let Some(ws) = &self.popup.preflight_workspace {
                        let has_stash_suggestion = self
                            .popup
                            .preflight_result
                            .as_ref()
                            .map(|r| r.suggestions.iter().any(|s| s.contains("stash")))
                            .unwrap_or(false);
                        if has_stash_suggestion {
                            let _ = crate::app::git_cmd(ws, &["stash"]);
                        }
                        // Re-run preflight.
                        let result = crate::preflight::run_preflight(ws);
                        if result.has_warnings() {
                            self.popup.preflight_result = Some(result);
                            self.view.status = "Re-checked — still has warnings".into();
                        } else {
                            // All clear now — proceed.
                            let agent = self.popup.preflight_agent;
                            let name = self.popup.preflight_session_name.take();
                            self.popup.preflight_result = None;
                            self.popup.preflight_workspace = None;
                            self.popup.preflight_agent = None;
                            self.view.input_mode = InputMode::None;
                            if let Some(agent) = agent {
                                self.spawn_with_agent_inner(agent, name)?;
                            }
                        }
                    }
                }
                KeyCode::Esc => {
                    self.popup.preflight_result = None;
                    self.popup.preflight_workspace = None;
                    self.popup.preflight_agent = None;
                    self.popup.preflight_session_name = None;
                    self.view.input_mode = InputMode::None;
                    self.view.status = "Session start cancelled.".into();
                }
                _ => {}
            }
            return Ok(Action::Continue);
        }

        match key.code {
            KeyCode::Esc => {
                self.view.input_mode = InputMode::None;
                self.input_buffer.clear();
                self.rename_target = None;
                self.rename_workspace_target = None;
                self.view.status = "Cancelled.".into();
            }
            KeyCode::Enter => {
                self.confirm_input()?;
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
        Ok(Action::Continue)
    }

    pub(super) fn handle_mouse_click(&mut self, x: u16, y: u16) {
        let rect = self.view.tab_bar_rect;
        if self.ptys.ptys.is_empty() || rect.width == 0 || rect.height == 0 {
            return;
        }
        if y < rect.y || y >= rect.y + rect.height || x < rect.x || x >= rect.x + rect.width {
            return;
        }
        let local_x = x - rect.x;
        let tab_width = rect.width as usize / self.ptys.ptys.len();
        let num_tabs = self.ptys.ptys.len();
        let Some(tab_index) = super::ui::tab_index_from_x(local_x, tab_width, num_tabs) else {
            return;
        };
        if self.ptys.active_pty != Some(tab_index) {
            self.ptys.active_pty = Some(tab_index);
            if let Some(slot) = self.ptys.ptys.get(tab_index) {
                slot.handle.reset_scroll();
            }
            self.view.status = format!(
                "Switched to: {} ({}/{})",
                self.ptys.ptys[tab_index].info.title,
                tab_index + 1,
                self.ptys.ptys.len()
            );
            self.update_related_sessions();
        }
    }

    pub(super) fn handle_tab_close_click(&mut self, x: u16, y: u16) {
        let rect = self.view.tab_bar_rect;
        if self.ptys.ptys.is_empty() || rect.width == 0 || rect.height == 0 {
            return;
        }
        if y < rect.y || y >= rect.y + rect.height || x < rect.x || x >= rect.x + rect.width {
            return;
        }
        let local_x = x - rect.x;
        let tab_width = rect.width as usize / self.ptys.ptys.len();
        let num_tabs = self.ptys.ptys.len();
        let Some(tab_index) = super::ui::tab_index_from_x(local_x, tab_width, num_tabs) else {
            return;
        };
        // Close the tab (same logic as D on ActiveTab)
        let title = self
            .ptys
            .ptys
            .get(tab_index)
            .map(|s| s.info.title.clone())
            .unwrap_or_default();
        if let Some(slot) = self.ptys.ptys.get(tab_index) {
            self.unregister_pty(&slot.id);
        }
        self.ptys.ptys.remove(tab_index);
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
        self.view.status = format!("Closed tab: {}", title);
    }

    pub(super) fn handle_paste(&mut self, text: &str) -> Result<Action> {
        if self.view.input_mode == InputMode::ScrollbackSearch {
            self.view.scrollback_query.push_str(text);
            self.run_scrollback_search();
        } else if self.view.input_mode != InputMode::None {
            self.input_buffer.push_str(text);
        } else if self.view.focus == Focus::Chat
            && let Some(idx) = self.ptys.active_pty
            && let Some(slot) = self.ptys.ptys.get(idx)
        {
            let res = slot.handle.write_input(text.as_bytes());
            if let Err(e) = res {
                self.view.status = format!("Write error: {e}");
            }
        }
        Ok(Action::Continue)
    }

    /// Handle keys in ScrollbackSearch mode (f in PTY chat).
    fn handle_scrollback_search_key(&mut self, key: KeyEvent) -> Result<Action> {
        match key.code {
            KeyCode::Esc => {
                self.view.scrollback_query.clear();
                self.view.scrollback_matches.clear();
                self.view.scrollback_match_idx = 0;
                self.view.input_mode = InputMode::None;
            }
            KeyCode::Enter => {
                if !self.view.scrollback_matches.is_empty() {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        // Shift+Enter: previous match
                        self.view.scrollback_match_idx = if self.view.scrollback_match_idx == 0 {
                            self.view.scrollback_matches.len() - 1
                        } else {
                            self.view.scrollback_match_idx - 1
                        };
                    } else {
                        // Enter: next match
                        self.view.scrollback_match_idx =
                            (self.view.scrollback_match_idx + 1) % self.view.scrollback_matches.len();
                    }
                    self.scroll_to_current_match();
                }
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.view.scrollback_regex = !self.view.scrollback_regex;
                self.run_scrollback_search();
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.view.scrollback_case_sensitive = !self.view.scrollback_case_sensitive;
                self.run_scrollback_search();
            }
            KeyCode::Char(c) => {
                self.view.scrollback_query.push(c);
                self.run_scrollback_search();
            }
            KeyCode::Backspace => {
                self.view.scrollback_query.pop();
                self.run_scrollback_search();
            }
            _ => {}
        }
        Ok(Action::Continue)
    }

    /// Search the active PTY screen for the current scrollback query.
    fn run_scrollback_search(&mut self) {
        self.view.scrollback_matches.clear();
        self.view.scrollback_match_idx = 0;

        let query = self.view.scrollback_query.as_str();
        if query.is_empty() {
            return;
        }

        let Some(idx) = self.ptys.active_pty else { return };
        let Some(slot) = self.ptys.ptys.get(idx) else { return };

        let (term_rows, term_cols) = slot.handle.grid_size();

        // Build matcher: either regex or plain substring.
        let regex_ok = if self.view.scrollback_regex {
            regex::RegexBuilder::new(query)
                .case_insensitive(!self.view.scrollback_case_sensitive)
                .build()
                .ok()
        } else {
            None
        };

        for row in 0..term_rows {
            let mut line = String::new();
            for col in 0..term_cols {
                match slot.handle.cell_contents(row, col) {
                    Some(t) => line.push_str(&t),
                    None => line.push(' '),
                }
            }

            if let Some(ref re) = regex_ok {
                // Regex mode: find all matches
                for m in re.find_iter(&line) {
                    self.view.scrollback_matches.push((u16::try_from(row).unwrap_or(u16::MAX), u16::try_from(m.start()).unwrap_or(u16::MAX), m.end() - m.start()));
                }
            } else {
                // Plain substring mode (case-insensitive unless toggled)
                let haystack = if self.view.scrollback_case_sensitive {
                    line
                } else {
                    line.to_lowercase()
                };
                let needle = if self.view.scrollback_case_sensitive {
                    query.to_string()
                } else {
                    query.to_lowercase()
                };
                let mut start = 0;
                while let Some(pos) = haystack[start..].find(&needle) {
                    let abs_pos = start + pos;
                    self.view.scrollback_matches.push((u16::try_from(row).unwrap_or(u16::MAX), u16::try_from(abs_pos).unwrap_or(u16::MAX), needle.len()));
                    start = abs_pos + needle.len();
                    if start >= haystack.len() {
                        break;
                    }
                }
            }
        }

        if !self.view.scrollback_matches.is_empty() {
            self.scroll_to_current_match();
        }
    }

    /// Scroll the PTY to make the current match visible.
    fn scroll_to_current_match(&self) {
        if self.view.scrollback_matches.is_empty() {
            return;
        }
        let target_row = self.view.scrollback_matches[self.view.scrollback_match_idx].0;
        let Some(idx) = self.ptys.active_pty else { return };
        let Some(slot) = self.ptys.ptys.get(idx) else { return };

        let (term_rows, _) = slot.handle.grid_size();

        let page_size = self.view.last_chat_area.height.saturating_sub(2) as usize;
        let current_offset = slot.handle.scrollback_offset();
        let visible_start = term_rows.saturating_sub(current_offset + page_size);
        let visible_end = term_rows.saturating_sub(current_offset);

        let target = target_row as usize;
        if target >= visible_start && target < visible_end {
            // Already visible
            return;
        }

        // Scroll so target_row is near the top of visible area
        let desired_offset = term_rows.saturating_sub(target + page_size);
        let new_offset = desired_offset.min(term_rows.saturating_sub(1));
        slot.handle.set_scrollback(new_offset);
    }

    /// Handle mouse events for split divider drag.
    /// Returns true if the event was consumed as a split drag.
    pub(super) fn handle_split_drag(
        &mut self,
        kind: crossterm::event::MouseEventKind,
        column: u16,
    ) -> bool {
        match kind {
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                // Check if click is near the split boundary.
                // The sidebar occupies x=[0, split_x), the chat starts at split_x.
                let chat_area = self.view.last_chat_area;
                // When there's a terminal split, last_chat_area is the upper portion,
                // but the horizontal split position is still at chat_area.x.
                // The full-width split boundary x is at:
                //   frame_width * split_ratio / 100
                // We don't have frame_width stored, but we can compute from chat_area:
                //   chat_area.x ≈ frame_width * split_ratio / 100
                let split_x = chat_area.x;
                // Allow a tolerance of ±2 pixels
                if column.abs_diff(split_x) <= 2 {
                    self.view.dragging_split = true;
                    return true;
                }
                false
            }
            crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                if !self.view.dragging_split {
                    return false;
                }
                // Compute frame width: chat_area.x + chat_area.width
                let chat_area = self.view.last_chat_area;
                let frame_width = u16::try_from(chat_area.x as u32 + chat_area.width as u32).unwrap_or(u16::MAX);
                if frame_width == 0 {
                    return true;
                }
                let new_ratio = u16::try_from(column as u32 * 100 / u32::from(frame_width)).unwrap_or(u16::MAX);
                // Clamp to 20-50
                self.view.split_ratio = new_ratio.clamp(20, 50);
                true
            }
            crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                if self.view.dragging_split {
                    self.view.dragging_split = false;
                    return true;
                }
                false
            }
            _ => false,
        }
    }
}
