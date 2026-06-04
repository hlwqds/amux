use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line as AlacLine, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::pty::PtyState;
use crate::types::*;
use crate::util::{PARENT_DIR, SELECT_CURRENT, SELECT_VIRTUAL, centered_rect, relative_time};

fn ansi_to_ratatui_color(c: AnsiColor) -> Option<Color> {
    match c {
        AnsiColor::Spec(rgb) => Some(Color::Rgb(rgb.r, rgb.g, rgb.b)),
        AnsiColor::Indexed(i) => Some(Color::Indexed(i)),
        AnsiColor::Named(named) => named_to_ratatui_color(named),
    }
}

fn named_to_ratatui_color(n: NamedColor) -> Option<Color> {
    use NamedColor::*;
    match n {
        Foreground | Background | Cursor | DimForeground | BrightForeground => None,
        Black => Some(Color::Black),
        Red => Some(Color::Red),
        Green => Some(Color::Green),
        Yellow => Some(Color::Yellow),
        Blue => Some(Color::Blue),
        Magenta => Some(Color::Magenta),
        Cyan => Some(Color::Cyan),
        White => Some(Color::Gray),
        BrightBlack => Some(Color::DarkGray),
        BrightRed => Some(Color::LightRed),
        BrightGreen => Some(Color::LightGreen),
        BrightYellow => Some(Color::LightYellow),
        BrightBlue => Some(Color::LightBlue),
        BrightMagenta => Some(Color::LightMagenta),
        BrightCyan => Some(Color::LightCyan),
        BrightWhite => Some(Color::White),
        DimBlack => Some(Color::Black),
        DimRed => Some(Color::Red),
        DimGreen => Some(Color::Green),
        DimYellow => Some(Color::Yellow),
        DimBlue => Some(Color::Blue),
        DimMagenta => Some(Color::Magenta),
        DimCyan => Some(Color::Cyan),
        DimWhite => Some(Color::Gray),
    }
}
impl super::App {
    pub(super) fn chat_size(&self) -> (u16, u16) {
        (
            self.view.last_chat_area.width.saturating_sub(2),
            self.view.last_chat_area.height.saturating_sub(2),
        )
    }

    // ─── Rendering ────────────────────────────────────────

    pub(super) fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Length(1)])
            .split(area);

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(chunks[0]);

        self.render_sidebar(frame, cols[0]);
        self.view.last_chat_area = cols[1];
        self.render_chat(frame, cols[1]);
        self.render_status(frame, chunks[1]);

        if self.view.input_mode == InputMode::Help {
            self.render_help_popup(frame, area);
        } else if self.view.input_mode == InputMode::SessionPreview
            || self.view.input_mode == InputMode::SummaryPreview
        {
            self.render_session_preview(frame, area);
        } else if self.view.input_mode == InputMode::Settings {
            self.render_settings_popup(frame, area);
        } else if self.view.input_mode == InputMode::KeybindView {
            self.render_keybind_view(frame, area);
        } else if self.view.input_mode == InputMode::ThemeSelect {
            self.render_theme_select(frame, area);
        } else if self.view.input_mode == InputMode::TemplateSelect {
            self.render_template_select(frame, area);
        } else if self.view.input_mode == InputMode::AutomationSelect {
            self.render_automation_select(frame, area);
        } else if self.view.input_mode == InputMode::BranchSelect {
            self.render_branch_select(frame, area);
        } else if self.view.input_mode == InputMode::Stats {
            self.render_stats(frame, area);
        } else if self.view.input_mode == InputMode::TokenStats {
            self.render_token_stats(frame, area);
        } else if self.view.input_mode == InputMode::DiffView {
            self.render_diff_view(frame, area);
        } else if self.view.input_mode == InputMode::RemoteView {
            self.render_remote_view(frame, area);
        } else if self.view.input_mode == InputMode::PluginList {
            self.render_plugin_list(frame, area);
        } else if self.view.input_mode == InputMode::PluginOutput {
            self.render_plugin_output(frame, area);
        } else if self.view.input_mode == InputMode::Timeline {
            self.render_timeline(frame, area);
        } else if self.view.input_mode == InputMode::ConflictWarning
            || self.view.input_mode == InputMode::ConflictResolve
        {
            self.render_conflict_resolve(frame, area);
        } else if self.view.input_mode == InputMode::AgentRecommend {
            self.render_agent_recommend(frame, area);
        } else if self.view.input_mode == InputMode::CrossSearch {
            self.render_cross_search(frame, area);
        } else if self.view.input_mode == InputMode::BudgetWarning {
            self.render_budget_warning(frame, area);
        } else if self.view.input_mode == InputMode::ChainSelect {
            self.render_chain_select(frame, area);
        } else if self.view.input_mode == InputMode::RollbackConfirm {
            self.render_rollback_confirm(frame, area);
        } else if self.view.input_mode == InputMode::ConfirmDelete {
            self.render_confirm_delete(frame, area);
        } else if self.view.input_mode == InputMode::PreflightConfirm {
            self.render_preflight_confirm(frame, area);
        } else if self.view.input_mode == InputMode::SemanticSearch {
            self.render_semantic_search(frame, area);
        } else if self.view.input_mode != InputMode::None
            && self.view.input_mode != InputMode::ScrollbackSearch
        {
            self.render_input_popup(frame, area);
        }
    }

    fn render_sidebar(&mut self, frame: &mut Frame, area: Rect) {
        // Pre-compute pty state lookup to avoid borrowing self inside .map() closure.
        let pty_state_map: Vec<(String, PtyState)> = self
            .ptys
            .ptys
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                s.info
                    .session_id
                    .as_ref()
                    .map(|sid| (sid.clone(), self.pty_display_state(i)))
            })
            .collect();
        // Pre-compute active tab state for ActiveTab nodes.
        let active_tab_data: Vec<(
            PtyState,
            String,
            Agent,
            CheckStatus,
            DiffSummary,
            Option<crate::procfs::ProcessStats>,
        )> = self
            .ptys
            .ptys
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    self.pty_display_state(i),
                    s.info.title.clone(),
                    s.info.agent,
                    s.info.check_status.clone(),
                    s.info.diff_summary.clone(),
                    s.process_stats.clone(),
                )
            })
            .collect();

        let items: Vec<_> = self
            .sessions
            .tree
            .iter()
            .map(|node| match node {
                TreeNode::PinnedWorkspace => {
                    let count = self.sessions.sessions.iter().filter(|s| s.pinned).count();
                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(
                                "▼ 📌 ",
                                Style::default().fg(self.view.theme.sidebar_highlight),
                            ),
                            Span::styled(
                                "Pinned",
                                Style::default().fg(self.view.theme.sidebar_highlight),
                            ),
                        ]),
                        Line::from(format!("   {} pinned sessions", count))
                            .style(Style::default().fg(self.view.theme.sidebar_dim)),
                    ])
                }
                TreeNode::RecentWorkspace => {
                    // Count sessions in the Recent virtual workspace from the tree
                    let count = self
                        .sessions
                        .tree
                        .iter()
                        .skip_while(|n| !matches!(n, TreeNode::RecentWorkspace))
                        .skip(1)
                        .take_while(|n| matches!(n, TreeNode::Session(_, _)))
                        .count();
                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(
                                "▼ 🕐 ",
                                Style::default().fg(self.view.theme.sidebar_highlight),
                            ),
                            Span::styled(
                                "Recent",
                                Style::default().fg(self.view.theme.sidebar_highlight),
                            ),
                        ]),
                        Line::from(format!("   {} recent sessions", count))
                            .style(Style::default().fg(self.view.theme.sidebar_dim)),
                    ])
                }
                TreeNode::Workspace(wi) => {
                    let ws = &self.sessions.workspaces[*wi];
                    let icon = if ws.expanded { "\u{25bc}" } else { "\u{25b6}" };
                    let count = self
                        .sessions
                        .ws_session_map
                        .get(*wi)
                        .map(|v| v.len())
                        .unwrap_or(0);

                    let (binding_icon, binding_style, subtitle) = match &ws.path {
                        Some(p) => (
                            "\u{25c6}",
                            Style::default().fg(self.view.theme.sidebar_selected),
                            format!("   {} sessions \u{00b7} {}", count, p.display()),
                        ),
                        None => (
                            "\u{25c7}",
                            Style::default().fg(self.view.theme.sidebar_highlight),
                            format!("   {} sessions \u{00b7} virtual", count),
                        ),
                    };

                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(
                                format!("{} {} ", icon, binding_icon),
                                binding_style.add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                ws.name.clone(),
                                Style::default()
                                    .fg(self.view.theme.sidebar_selected)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]),
                        Line::from(subtitle)
                            .style(Style::default().fg(self.view.theme.sidebar_dim)),
                    ])
                }
                TreeNode::Session(_wi, si) => {
                    if let Some(session) = self.sessions.sessions.get(*si) {
                        let short_id = &session.id[..8.min(session.id.len())];
                        let pty_state = pty_state_map
                            .iter()
                            .find(|(sid, _)| sid == &session.id)
                            .map(|(_, s)| *s);

                        let agent_tag = Span::styled(
                            format!(" [{}]", session.agent.icon()),
                            Style::default().fg(match session.agent {
                                Agent::Claude => self.view.theme.agent_claude,
                                Agent::Codex => self.view.theme.agent_codex,
                                Agent::Omp => self.view.theme.agent_omp,
                            }),
                        );

                        let is_selected = self.view.selected_set.contains(si);
                        let check = if is_selected {
                            Span::styled(
                                "\u{2611} ",
                                Style::default().fg(self.view.theme.status_done),
                            ) // ☑
                        } else {
                            Span::raw("  ")
                        };

                        let (marker, state_label) = match pty_state {
                            Some(PtyState::Running) => (
                                Span::styled(
                                    " \u{258c}", // ▌ left block — running indicator
                                    Style::default().fg(self.view.theme.status_running),
                                ),
                                Some("running"),
                            ),
                            Some(PtyState::Completed) => (
                                Span::styled(
                                    " \u{258c}", // ▌ left block — done indicator
                                    Style::default().fg(self.view.theme.status_done),
                                ),
                                Some("done"),
                            ),
                            None => (
                                Span::styled(
                                    " \u{2591}", // ░ light shade — idle indicator
                                    Style::default().fg(self.view.theme.sidebar_dim),
                                ),
                                None,
                            ),
                        };
                        let note_present = crate::config::load_session_meta(&session.id, None)
                            .and_then(|meta| meta.note)
                            .is_some_and(|n| !n.is_empty());
                        let title = truncate_title(
                            &session.title,
                            area.width.saturating_sub(14).max(12) as usize,
                        );
                        let mut spans = vec![
                            check,
                            marker,
                            agent_tag.clone(),
                            Span::raw(" "),
                            Span::styled(title, Style::default().fg(self.view.theme.sidebar_text)),
                        ];
                        if session.pinned {
                            spans.push(Span::styled(
                                " 📌",
                                Style::default().fg(self.view.theme.sidebar_highlight),
                            ));
                        }
                        if note_present {
                            spans.push(Span::styled(
                                " \u{1f4dd}",
                                Style::default().fg(self.view.theme.accent),
                            ));
                        }
                        let diff_summary = if pty_state == Some(PtyState::Completed) {
                            self.ptys
                                .ptys
                                .iter()
                                .find(|s| s.info.session_id.as_deref() == Some(&session.id))
                                .map(|pty_slot| &pty_slot.info.diff_summary)
                        } else {
                            None
                        };
                        let detail_line = session_secondary_text(
                            &relative_time(session.last_active),
                            short_id,
                            state_label,
                            &session.tags,
                            diff_summary,
                            session.last_message.as_deref(),
                        );
                        let item_lines = vec![
                            Line::from(spans),
                            Line::from(detail_line)
                                .style(Style::default().fg(self.view.theme.sidebar_dim)),
                        ];
                        ListItem::new(item_lines)
                    } else {
                        ListItem::new(Line::from("   \u{25cf} ?"))
                    }
                }
                TreeNode::ActiveTab(pi) => {
                    let (state, title, agent, check, diff_summary, proc_stats) =
                        active_tab_data.get(*pi).cloned().unwrap_or((
                            PtyState::Running,
                            "New Session".into(),
                            Agent::Claude,
                            CheckStatus::Pending,
                            DiffSummary::default(),
                            None,
                        ));
                    let (dot_color, state_text) = match state {
                        PtyState::Running => (self.view.theme.status_running, " [running]".into()),
                        PtyState::Completed => match &check {
                            CheckStatus::Failed(e) => {
                                (self.view.theme.status_error, format!(" \u{26a0} {}", e))
                            }
                            CheckStatus::Running => (
                                self.view.theme.status_running,
                                " \u{23f3} checking...".into(),
                            ),
                            CheckStatus::Passed => {
                                (self.view.theme.status_done, " \u{2714} done".into())
                            }
                            CheckStatus::Pending => {
                                (self.view.theme.status_done, " \u{2714} done".into())
                            }
                        },
                    };
                    let state_color = dot_color;
                    let stats_span = if let Some(ref stats) = proc_stats {
                        let cpu = format!("{:.1}%", stats.cpu_percent);
                        let mem = crate::procfs::format_bytes(stats.mem_rss_kb * 1024);
                        Span::styled(
                            format!(" CPU:{} MEM:{}", cpu, mem),
                            Style::default().fg(self.view.theme.sidebar_dim),
                        )
                    } else {
                        Span::raw("")
                    };
                    let title_spans = vec![
                        Span::styled("  \u{258c} ", Style::default().fg(dot_color)),
                        Span::styled(title, Style::default().fg(self.view.theme.sidebar_text)),
                        Span::styled(state_text, Style::default().fg(state_color)),
                        Span::styled(
                            format!(" [{}]", agent.icon()),
                            Style::default().fg(match agent {
                                Agent::Claude => self.view.theme.agent_claude,
                                Agent::Codex => self.view.theme.agent_codex,
                                Agent::Omp => self.view.theme.agent_omp,
                            }),
                        ),
                        stats_span,
                    ];
                    let detail = if state == PtyState::Completed {
                        let ds = &diff_summary;
                        if ds.files_changed.is_empty() {
                            Line::from("     no changes detected")
                                .style(Style::default().fg(self.view.theme.sidebar_dim))
                        } else {
                            let info = format!(
                                "     +{}/-{} in {} file(s)",
                                ds.insertions,
                                ds.deletions,
                                ds.files_changed.len()
                            );
                            Line::from(vec![Span::styled(
                                info,
                                Style::default().fg(self.view.theme.sidebar_dim),
                            )])
                        }
                    } else {
                        Line::from("     waiting for session file...")
                            .style(Style::default().fg(self.view.theme.sidebar_dim))
                    };
                    ListItem::new(vec![Line::from(title_spans), detail])
                }
                TreeNode::WorkspaceWarning(_, msg) => {
                    ListItem::new(Line::from(vec![Span::styled(
                        format!("  \u{26a0} {}", msg),
                        Style::default().fg(self.view.theme.sidebar_highlight),
                    )]))
                }
                TreeNode::AgentHeader(agent) => ListItem::new(Line::from(vec![Span::styled(
                    format!("  \u{2500}\u{2500} {} \u{2500}\u{2500}", agent.label()),
                    Style::default()
                        .fg(match agent {
                            Agent::Claude => self.view.theme.agent_claude,
                            Agent::Codex => self.view.theme.agent_codex,
                            Agent::Omp => self.view.theme.agent_omp,
                        })
                        .add_modifier(Modifier::DIM),
                )])),
                TreeNode::ArchivedHeader => {
                    let count = self.sessions.archived_sessions.len();
                    ListItem::new(Line::from(vec![Span::styled(
                        format!("  \u{25b6} Archived ({})", count),
                        Style::default().fg(self.view.theme.sidebar_dim),
                    )]))
                }
                TreeNode::ArchivedSession(_wi, ai) => {
                    if let Some(session) = self.sessions.archived_sessions.get(*ai) {
                        let short_id = &session.id[..8.min(session.id.len())];
                        ListItem::new(vec![
                            Line::from(vec![
                                Span::styled(
                                    " \u{25cb} ",
                                    Style::default().fg(self.view.theme.sidebar_dim),
                                ),
                                Span::styled(
                                    relative_time(session.last_active),
                                    Style::default().fg(self.view.theme.sidebar_dim),
                                ),
                                Span::styled(
                                    format!(" ({})", short_id),
                                    Style::default().fg(self.view.theme.sidebar_dim),
                                ),
                                Span::styled(
                                    format!(" [{}]", session.agent.icon()),
                                    Style::default()
                                        .fg(self.view.theme.sidebar_dim)
                                        .add_modifier(Modifier::DIM),
                                ),
                            ]),
                            Line::from(format!("     {}", session.title))
                                .style(Style::default().fg(self.view.theme.sidebar_dim)),
                        ])
                    } else {
                        ListItem::new(Line::from("   \u{25cb} ?"))
                    }
                }
            })
            .collect();


        let is_searching = self.view.input_mode == InputMode::Search;
        let search_query = self.view.search_query.as_deref().unwrap_or("");

        let sort_label = self.view.sort_mode.label();

        let title = match (is_searching, &self.view.agent_filter, &self.view.tag_filter) {
            (true, Some(agent), Some(tag)) => {
                format!(
                    " [{}] [search: {}] [tag: {}] [sort: {}] ",
                    agent.label(),
                    search_query,
                    tag,
                    sort_label
                )
            }
            (true, Some(agent), None) => {
                format!(
                    " [{}] [search: {}] [sort: {}] ",
                    agent.label(),
                    search_query,
                    sort_label
                )
            }
            (true, None, Some(tag)) => {
                format!(
                    " [search: {}] [tag: {}] [sort: {}] ",
                    search_query, tag, sort_label
                )
            }
            (true, None, None) => format!(" [search: {}] [sort: {}] ", search_query, sort_label),
            (false, Some(agent), Some(tag)) => {
                format!(
                    " [{}] [tag: {}] Workspaces [sort: {}] ",
                    agent.label(),
                    tag,
                    sort_label
                )
            }
            (false, Some(agent), None) => {
                format!(" [{}] Workspaces [sort: {}] ", agent.label(), sort_label)
            }
            (false, None, Some(tag)) => {
                format!(" [tag: {}] Workspaces [sort: {}] ", tag, sort_label)
            }
            (false, None, None) => format!(" Workspaces [sort: {}] ", sort_label),
        };

        let block = Block::default()
            .borders(Borders::RIGHT)
            .title(title)
            .style(Style::default().bg(self.view.theme.sidebar_bg))
            .border_style(
                Style::default()
                    .fg(self.view.theme.chat_border)
                    .bg(self.view.theme.sidebar_bg),
            );

        if is_searching {
            // Split sidebar into tree area (top) and search prompt (bottom)
            let inner = block.inner(area);
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(1)])
                .split(inner);

            let list = List::new(items)
                .highlight_style(
                    Style::default()
                        .bg(self.view.theme.input_cursor)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("\u{258c} "); // ▌ left block

            frame.render_stateful_widget(list, chunks[0], &mut self.sessions.tree_state);

            let query = self.view.search_query.as_deref().unwrap_or("");
            let filter_count = self.sessions.tree.len();
            let filter_text = match filter_count {
                0 => "0 matches".to_string(),
                1 => "1 match".to_string(),
                n => format!("{} matches", n),
            };

            let search_line = Line::from(vec![
                Span::styled(
                    " search: ",
                    Style::default().fg(self.view.theme.accent).bold(),
                ),
                Span::styled(
                    query.to_string(),
                    Style::default().fg(self.view.theme.sidebar_text),
                ),
                Span::styled("|", Style::default().fg(self.view.theme.sidebar_dim)),
                Span::styled(
                    format!(" {}", filter_text),
                    Style::default().fg(self.view.theme.sidebar_dim),
                ),
            ]);
            frame.render_widget(Paragraph::new(search_line), chunks[1]);
            frame.render_widget(block, area);
        } else {
            let list = List::new(items)
                .block(block)
                .highlight_style(
                    Style::default()
                        .bg(self.view.theme.input_cursor)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("\u{258c} "); // ▌ left block

            frame.render_stateful_widget(list, area, &mut self.sessions.tree_state);
        }
    }

    fn render_chat(&mut self, frame: &mut Frame, area: Rect) {
        let border_color = if self.view.focus == Focus::Chat {
            self.view.theme.sidebar_highlight
        } else {
            self.view.theme.chat_border
        };

        let scroll_offset = self
            .ptys
            .active_pty
            .and_then(|idx| self.ptys.ptys.get(idx))
            .map(|s| s.handle.scrollback_offset())
            .unwrap_or(0);

        let title = if let Some(idx) = self.ptys.active_pty {
            if let Some(slot) = self.ptys.ptys.get(idx) {
                let scroll_hint = if scroll_offset > 0 {
                    format!(" [↑{} PgDn:bottom]", scroll_offset)
                } else {
                    String::new()
                };
                format!(
                    " {} [{}] ({}/{}){}",
                    slot.info.title,
                    slot.info.agent.label(),
                    idx + 1,
                    self.ptys.ptys.len(),
                    scroll_hint,
                )
            } else {
                " Agent ".to_string()
            }
        } else {
            " Agent ".to_string()
        };

        let block = Block::default()
            .borders(Borders::NONE)
            .title(title)
            .border_style(Style::default().fg(border_color));

        // When PTYs are active, split area into [tab_bar(1)] + [pty_content]
        if !self.ptys.ptys.is_empty() {
            let inner = block.inner(area);
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(inner);
            self.view.tab_bar_rect = chunks[0];
            // Render tab bar
            let tab_line = self.build_tab_bar(chunks[0].width as usize);
            frame.render_widget(Paragraph::new(tab_line), chunks[0]);

            let is_searching = self.view.input_mode == InputMode::ScrollbackSearch;
            // Split pty area: [search_bar(1)] + [pty] when searching
            let pty_area = if is_searching {
                let search_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(1)])
                    .split(chunks[1]);
                // Render search bar
                let query = &self.view.scrollback_query;
                let total = self.view.scrollback_matches.len();
                let current = if total == 0 {
                    0
                } else {
                    self.view.scrollback_match_idx + 1
                };
                let search_text = if query.is_empty() {
                    " Search: _".to_string()
                } else if total == 0 {
                    format!(" Search: {} (no matches)", query)
                } else {
                    format!(" Search: {} ({}/{})", query, current, total)
                };
                let search_bar = Paragraph::new(Line::from(vec![
                    Span::styled(
                        search_text,
                        Style::default()
                            .fg(Color::Black)
                            .bg(self.view.theme.status_running)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " ".repeat(search_chunks[0].width as usize),
                        Style::default().bg(self.view.theme.status_running),
                    ),
                ]))
                .style(Style::default().bg(self.view.theme.status_running));
                frame.render_widget(search_bar, search_chunks[0]);
                search_chunks[1]
            } else {
                chunks[1]
            };

            // Render PTY content
            if let Some(idx) = self.ptys.active_pty
                && let Some(slot) = self.ptys.ptys.get(idx)
            {
                slot.handle.resize((pty_area.width, pty_area.height));
                // Check if we're viewing a snapshot (alternate screen scrollback)
                if let Some(snapshot_rows) = slot.handle.scrolled_snapshot() {
                    let lines: Vec<Line<'_>> = snapshot_rows
                        .iter()
                        .take(pty_area.height as usize)
                        .map(|row| Line::from(row.clone()))
                        .collect();
                    let snap_widget = Paragraph::new(lines).style(
                        Style::default()
                            .fg(self.view.theme.sidebar_text)
                            .bg(self.view.theme.sidebar_bg),
                    );
                    frame.render_widget(Clear, pty_area);
                    frame.render_widget(snap_widget, pty_area);
                    // Show scroll indicator
                    let indicator = Paragraph::new(Line::from(vec![Span::styled(
                        " HIST ",
                        Style::default()
                            .fg(Color::Black)
                            .bg(self.view.theme.status_running)
                            .add_modifier(Modifier::BOLD),
                    )]));
                    frame.render_widget(
                        indicator,
                        Rect {
                            x: pty_area.x,
                            y: pty_area.y,
                            width: 5,
                            height: 1,
                        },
                    );
                } else {
                    let term = slot.handle.term();
                    let guard = term.lock();
                    let display_offset = guard.grid().display_offset();
                    let grid = guard.grid();
                    let screen_rows = guard.screen_lines() as u16;
                    let screen_cols = guard.columns() as u16;
                    let max_rows = pty_area.height.min(screen_rows);
                    let max_cols = pty_area.width.min(screen_cols);
                    for y in 0..max_rows {
                        let line_idx = y as i32 - display_offset as i32;
                        for x in 0..max_cols {
                            let p = Point::new(AlacLine(line_idx), Column(x as usize));
                            let cell = &grid[p];
                            // Skip spacer cells — they are the right half of a
                            // wide (CJK) character.  Rendering them as spaces
                            // would overwrite the wide glyph.
                            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                                continue;
                            }
                            let ch = if cell.c == '\0' { ' ' } else { cell.c };
                            let mut style = Style::default();
                            if let Some(c) = ansi_to_ratatui_color(cell.fg) {
                                style = style.fg(c);
                            }
                            if let Some(c) = ansi_to_ratatui_color(cell.bg) {
                                style = style.bg(c);
                            }
                            let flags = cell.flags;
                            if flags.contains(Flags::BOLD) {
                                style = style.add_modifier(Modifier::BOLD);
                            }
                            if flags.contains(Flags::ITALIC) {
                                style = style.add_modifier(Modifier::ITALIC);
                            }
                            if flags.intersects(Flags::ALL_UNDERLINES) {
                                style = style.add_modifier(Modifier::UNDERLINED);
                            }
                            if flags.contains(Flags::INVERSE) {
                                style = style.add_modifier(Modifier::REVERSED);
                            }
                            let target_x = pty_area.x + x;
                            let target_y = pty_area.y + y;
                            if let Some(buf_cell) =
                                frame.buffer_mut().cell_mut((target_x, target_y))
                            {
                                let mut tmp = [0u8; 4];
                                buf_cell.set_symbol(ch.encode_utf8(&mut tmp));
                                buf_cell.set_style(style);
                            }
                        }
                    }
                    drop(guard);
                    // Highlight scrollback search matches
                    if is_searching && !self.view.scrollback_matches.is_empty() {
                        let offset = slot.handle.scrollback_offset();
                        let (term_rows, _term_cols) = slot.handle.grid_size();
                        let page_height = pty_area.height as usize;
                        let vis_end = term_rows.saturating_sub(offset);
                        let vis_start = vis_end.saturating_sub(page_height);
                        for (mi, &(row, col, len)) in
                            self.view.scrollback_matches.iter().enumerate()
                        {
                            let row = row as usize;
                            if row < vis_start || row >= vis_end {
                                continue;
                            }
                            let screen_y = pty_area.y + (row - vis_start) as u16;
                            let screen_x = pty_area.x + col;
                            if screen_x + len as u16 > pty_area.x + pty_area.width {
                                continue;
                            }
                            let highlight_style = if mi == self.view.scrollback_match_idx {
                                Style::default()
                                    .bg(self.view.theme.accent)
                                    .fg(Color::Black)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default()
                                    .bg(self.view.theme.sidebar_dim)
                                    .fg(self.view.theme.sidebar_text)
                            };
                            let mut text = String::new();
                            for c in col..col + len as u16 {
                                match slot.handle.cell_contents(row, c as usize) {
                                    Some(t) => text.push_str(&t),
                                    None => text.push(' '),
                                }
                            }
                            let span = Span::styled(text, highlight_style);
                            let highlight_area = Rect {
                                x: screen_x,
                                y: screen_y,
                                width: len as u16,
                                height: 1,
                            };
                            frame.render_widget(
                                Paragraph::new(Line::from(vec![span])),
                                highlight_area,
                            );
                        }
                    }
                }
            }
            frame.render_widget(block, area);
            return;
        }

        // No PTYs — existing placeholder path
        if let Some(idx) = self.ptys.active_pty
            && let Some(slot) = self.ptys.ptys.get(idx)
        {
            let inner = block.inner(area);
            slot.handle.resize((inner.width, inner.height));

            let term_arc = slot.handle.term();
            let guard = term_arc.lock();
            let display_offset = guard.grid().display_offset();
            let grid = guard.grid();
            let screen_rows = guard.screen_lines() as u16;
            let screen_cols = guard.columns() as u16;
            let max_rows = inner.height.min(screen_rows);
            let max_cols = inner.width.min(screen_cols);
            for y in 0..max_rows {
                let line_idx = y as i32 - display_offset as i32;
                for x in 0..max_cols {
                    let p = Point::new(AlacLine(line_idx), Column(x as usize));
                    let cell = &grid[p];
                    if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                        continue;
                    }
                    let ch = if cell.c == '\0' { ' ' } else { cell.c };
                    let mut style = Style::default();
                    if let Some(c) = ansi_to_ratatui_color(cell.fg) {
                        style = style.fg(c);
                    }
                    if let Some(c) = ansi_to_ratatui_color(cell.bg) {
                        style = style.bg(c);
                    }
                    let flags = cell.flags;
                    if flags.contains(Flags::BOLD) {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    if flags.contains(Flags::ITALIC) {
                        style = style.add_modifier(Modifier::ITALIC);
                    }
                    if flags.intersects(Flags::ALL_UNDERLINES) {
                        style = style.add_modifier(Modifier::UNDERLINED);
                    }
                    if flags.contains(Flags::INVERSE) {
                        style = style.add_modifier(Modifier::REVERSED);
                    }
                    let target_x = inner.x + x;
                    let target_y = inner.y + y;
                    if let Some(buf_cell) = frame.buffer_mut().cell_mut((target_x, target_y)) {
                        let mut tmp = [0u8; 4];
                        buf_cell.set_symbol(ch.encode_utf8(&mut tmp));
                        buf_cell.set_style(style);
                    }
                }
            }
            drop(guard);
            frame.render_widget(block, area);
            return;
        }

        let lines = self.render_placeholder();
        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_placeholder(&self) -> Vec<Line<'static>> {
        let mut lines: Vec<Line> = Vec::new();

        match self.selected_node() {
            Some(TreeNode::Workspace(wi)) => {
                let ws = &self.sessions.workspaces[*wi];
                lines.push(
                    Line::from(ws.name.clone()).style(
                        Style::default()
                            .fg(self.view.theme.sidebar_selected)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                match &ws.path {
                    Some(p) => {
                        lines.push(
                            Line::from(format!("\u{25c6} {}", p.display()))
                                .style(Style::default().fg(self.view.theme.status_done)),
                        );
                    }
                    None => {
                        lines.push(
                            Line::from("\u{25c7} Virtual workspace (no directory)")
                                .style(Style::default().fg(self.view.theme.sidebar_highlight)),
                        );
                    }
                }
                lines.push(Line::from(""));
                lines.push(
                    Line::from("Press Enter to start a named Claude Code session")
                        .style(Style::default().fg(self.view.theme.sidebar_highlight)),
                );
            }
            Some(TreeNode::Session(_wi, si)) => {
                let session = &self.sessions.sessions[*si];
                lines.push(
                    Line::from(session.title.clone()).style(
                        Style::default()
                            .fg(self.view.theme.sidebar_selected)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                lines.push(
                    Line::from(format!("ID: {}", session.id))
                        .style(Style::default().fg(self.view.theme.sidebar_dim)),
                );
                lines.push(Line::from(format!(
                    "Last active: {}",
                    relative_time(session.last_active)
                )));
                if !session.tags.is_empty() {
                    lines.push(
                        Line::from(format!("Tags: {}", session.tags.join(", ")))
                            .style(Style::default().fg(self.view.theme.agent_omp)),
                    );
                }
                if self.pty_index_for_session(&session.id).is_some() {
                    lines.push(Line::from(""));
                    lines.push(
                        Line::from("This session is already running - Enter to switch to it")
                            .style(Style::default().fg(self.view.theme.status_done)),
                    );
                } else {
                    lines.push(Line::from(""));
                    lines.push(
                        Line::from("Press Enter to resume this session")
                            .style(Style::default().fg(self.view.theme.sidebar_highlight)),
                    );
                }
            }
            Some(&TreeNode::ActiveTab(pi)) => {
                let title = self
                    .ptys
                    .ptys
                    .get(pi)
                    .map(|s| s.info.title.clone())
                    .unwrap_or_else(|| "New Session".into());
                lines.push(
                    Line::from(title).style(
                        Style::default()
                            .fg(self.view.theme.status_done)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                lines.push(
                    Line::from("Session is running...")
                        .style(Style::default().fg(self.view.theme.sidebar_dim)),
                );
                lines.push(Line::from(""));
                lines.push(
                    Line::from("Press Enter to switch to this session")
                        .style(Style::default().fg(self.view.theme.sidebar_highlight)),
                );
            }
            Some(&TreeNode::PinnedWorkspace) => {
                let count = self.sessions.sessions.iter().filter(|s| s.pinned).count();
                lines.push(
                    Line::from("📌 Pinned Sessions").style(
                        Style::default()
                            .fg(self.view.theme.sidebar_highlight)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                lines.push(Line::from(format!(
                    "{} pinned session(s) from all workspaces",
                    count
                )));
                lines.push(Line::from(""));
                lines.push(
                    Line::from("Press ! on a session to unpin it")
                        .style(Style::default().fg(self.view.theme.sidebar_dim)),
                );
            }
            Some(&TreeNode::RecentWorkspace) => {
                let count = self
                    .sessions
                    .tree
                    .iter()
                    .skip_while(|n| !matches!(n, TreeNode::RecentWorkspace))
                    .skip(1)
                    .take_while(|n| matches!(n, TreeNode::Session(_, _)))
                    .count();
                lines.push(
                    Line::from("🕐 Recent Sessions").style(
                        Style::default()
                            .fg(self.view.theme.sidebar_highlight)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                lines.push(Line::from(format!(
                    "{} recent session(s) from all workspaces",
                    count
                )));
                lines.push(Line::from(
                    "Most recently active sessions from all workspaces",
                ));
                lines.push(Line::from(""));
                lines.push(
                    Line::from("Sessions are automatically sorted by last activity")
                        .style(Style::default().fg(self.view.theme.sidebar_dim)),
                );
            }
            Some(TreeNode::WorkspaceWarning(_, msg)) => {
                lines.push(
                    Line::from("Workspace Warning").style(
                        Style::default()
                            .fg(self.view.theme.sidebar_highlight)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                lines.push(Line::from(""));
                lines.push(
                    Line::from(msg.clone())
                        .style(Style::default().fg(self.view.theme.sidebar_highlight)),
                );
            }
            Some(&TreeNode::AgentHeader(agent)) => {
                let label = agent.label().to_string();
                let color = match agent {
                    Agent::Claude => self.view.theme.agent_claude,
                    Agent::Codex => self.view.theme.agent_codex,
                    Agent::Omp => self.view.theme.agent_omp,
                };
                lines.push(
                    Line::from(label)
                        .style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
                );
                lines.push(Line::from(""));
                lines.push(
                    Line::from("Agent group header")
                        .style(Style::default().fg(self.view.theme.sidebar_dim)),
                );
            }
            Some(&TreeNode::ArchivedHeader) => {
                lines.push(
                    Line::from(format!(
                        "Archived Sessions ({})",
                        self.sessions.archived_sessions.len()
                    ))
                    .style(Style::default().fg(self.view.theme.sidebar_dim)),
                );
                lines.push(Line::from(""));
                lines.push(
                    Line::from("Press G to toggle archived visibility")
                        .style(Style::default().fg(self.view.theme.sidebar_highlight)),
                );
            }
            Some(&TreeNode::ArchivedSession(_wi, ai)) => {
                if let Some(session) = self.sessions.archived_sessions.get(ai) {
                    lines.push(
                        Line::from(session.title.clone()).style(
                            Style::default()
                                .fg(self.view.theme.sidebar_dim)
                                .add_modifier(Modifier::BOLD),
                        ),
                    );
                    lines.push(
                        Line::from(format!("ID: {} (archived)", session.id))
                            .style(Style::default().fg(self.view.theme.sidebar_dim)),
                    );
                    lines.push(Line::from(format!(
                        "Last active: {}",
                        relative_time(session.last_active)
                    )));
                    lines.push(Line::from(""));
                    lines.push(
                        Line::from("Press Enter to unarchive and resume")
                            .style(Style::default().fg(self.view.theme.sidebar_highlight)),
                    );
                }
            }
            None => {
                lines.push(
                    Line::from("No selection")
                        .style(Style::default().fg(self.view.theme.sidebar_dim)),
                );
            }
        }

        lines.push(Line::from(""));
        lines.push(
            Line::from(format!(
                "── {} → Keybindings for full list ──",
                self.view.keybinds.settings.display()
            ))
            .style(Style::default().fg(self.view.theme.sidebar_dim)),
        );
        lines.push(Line::from(format!(
            "  {}     New / Resume / Switch",
            "Enter"
        )));
        lines.push(Line::from(format!(
            "  {}/{}  Navigate",
            self.view.keybinds.move_up.display(),
            self.view.keybinds.move_down.display()
        )));
        lines.push(Line::from(format!(
            "  {}     Quit",
            self.view.keybinds.quit.display()
        )));
        lines
    }

    fn render_input_popup(&mut self, frame: &mut Frame, area: Rect) {
        if self.view.input_mode == InputMode::BrowseDir {
            self.render_browse_popup(frame, area);
            return;
        }
        if self.view.input_mode == InputMode::SelectAgent {
            self.render_agent_popup(frame, area);
            return;
        }

        let popup = centered_rect(60, 20, area);
        frame.render_widget(Clear, popup);

        let (title, label) = match self.view.input_mode {
            InputMode::SessionName => (" New Session ", "Session name: "),
            InputMode::RenameSession => (" Rename Session ", "New name: "),
            InputMode::RenameWorkspace => (" Rename Workspace ", "New name: "),
            InputMode::NewWorkspaceName => (" New Workspace ", "Workspace name: "),
            InputMode::TagFilter => (" Tag Filter ", "Tag: "),
            InputMode::Search => (" Search ", "Search: "),
            _ => return,
        };

        let input = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(label, Style::default().fg(Color::Cyan).bold()),
                Span::styled(&self.input_buffer, Style::default().fg(Color::White)),
                Span::styled("_", Style::default().fg(Color::Gray)),
            ]),
            Line::from(""),
            Line::from("Enter to confirm \u{00b7} Esc to cancel")
                .style(Style::default().fg(Color::DarkGray)),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(title)
                .border_style(Style::default().fg(Color::Yellow)),
        );

        frame.render_widget(input, popup);
    }

    fn render_agent_popup(&mut self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(50, 30, area);
        frame.render_widget(Clear, popup);

        let items: Vec<ListItem> = self
            .available_agents
            .iter()
            .map(|agent| {
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(
                            format!(" [{}] ", agent.icon()),
                            Style::default()
                                .fg(agent.color())
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(agent.label(), Style::default().fg(Color::White)),
                    ]),
                    Line::from(format!("     {}", agent.cmd()))
                        .style(Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
            .title(" Select Agent ")
            .border_style(Style::default().fg(Color::Yellow));

        let inner = block.inner(popup);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(1)])
            .split(inner);

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(24, 36, 72))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("\u{203a}");
        frame.render_stateful_widget(list, chunks[0], &mut self.agent_state);

        let help = Line::from(" C:Claude  X:Codex  O:OMP  j/k:navigate  Enter:confirm  Esc:cancel")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(Paragraph::new(help), chunks[1]);
        frame.render_widget(block, popup);
    }

    fn render_browse_popup(&mut self, frame: &mut Frame, area: Rect) {
        let popup = centered_rect(70, 70, area);
        frame.render_widget(Clear, popup);

        let ws_name = self.new_workspace_name.as_deref().unwrap_or("?");
        let title = format!(" {} \u{2192} Select Directory ", ws_name);

        let items: Vec<ListItem> = self
            .browse_entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let is_select_current = entry.name == SELECT_CURRENT;
                let is_virtual = entry.name == SELECT_VIRTUAL;
                let is_parent = entry.name == PARENT_DIR;

                let style = if is_select_current {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else if is_virtual {
                    Style::default().fg(Color::Yellow)
                } else if is_parent {
                    Style::default().fg(Color::DarkGray)
                } else if i % 2 == 0 {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::Gray)
                };

                let display = if is_select_current {
                    format!("{} {}", entry.name, entry.path.display())
                } else if entry.is_dir && !is_parent && !is_virtual {
                    format!("  \u{25b8} {}", entry.name)
                } else {
                    format!("  {}", entry.name)
                };

                ListItem::new(Line::from(display)).style(style)
            })
            .collect();

        let path_line = Line::from(format!(" {}", self.browse_dir.display()))
            .style(Style::default().fg(Color::Cyan));

        let help_line = Line::from(" j/k:navigate  Enter:open/select  Backspace/h:up  Esc:cancel")
            .style(Style::default().fg(Color::DarkGray));

        let block = Block::default()
            .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
            .title(title)
            .border_style(Style::default().fg(Color::Yellow));

        let inner = block.inner(popup);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(3),
                Constraint::Length(1),
            ])
            .split(inner);

        frame.render_widget(Paragraph::new(path_line), chunks[0]);

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(24, 36, 72))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("\u{203a}");
        frame.render_stateful_widget(list, chunks[1], &mut self.browse_state);

        frame.render_widget(Paragraph::new(help_line), chunks[2]);
        frame.render_widget(block, popup);
    }

    fn build_tab_bar(&mut self, width: usize) -> Line<'static> {
        if self.ptys.ptys.is_empty() {
            return Line::raw("");
        }

        let n_tabs = self.ptys.ptys.len();
        let tab_width = width / n_tabs;

        let states: Vec<PtyState> = (0..n_tabs).map(|i| self.pty_display_state(i)).collect();

        let mut spans = Vec::with_capacity(n_tabs * 4);

        for (i, slot) in self.ptys.ptys.iter().enumerate() {
            let is_active = self.ptys.active_pty == Some(i);
            let state = states[i];

            let (state_char, state_color) = match state {
                PtyState::Running => ("\u{25cf}", self.view.theme.status_running),
                PtyState::Completed => {
                    let check = &slot.info.check_status;
                    if matches!(check, CheckStatus::Failed(_)) {
                        ("\u{26a0}", self.view.theme.status_error)
                    } else if check == &CheckStatus::Running {
                        ("\u{23f3}", self.view.theme.status_running)
                    } else {
                        let pt = slot.info.project_type;
                        if pt != crate::discovery::ProjectType::Rust
                            && pt != crate::discovery::ProjectType::Unknown
                        {
                            (pt.icon(), self.view.theme.status_done)
                        } else {
                            ("\u{2714}", self.view.theme.status_done)
                        }
                    }
                }
            };

            let fixed_overhead = 6;
            let max_title = tab_width.saturating_sub(fixed_overhead);
            let title = truncate_title(&slot.info.title, max_title);
            let agent = slot.info.agent;
            let agent_color = match agent {
                Agent::Claude => self.view.theme.agent_claude,
                Agent::Codex => self.view.theme.agent_codex,
                Agent::Omp => self.view.theme.agent_omp,
            };

            if is_active {
                // Active tab: highlighted background, bold
                let active_bg = self.view.theme.input_cursor;
                // Left rounded cap
                spans.push(Span::styled(
                    "\u{258c}", // ▌ left half block — visual left edge
                    Style::default().bg(active_bg).fg(active_bg),
                ));
                spans.push(Span::styled(
                    format!("{} ", agent.icon()),
                    Style::default()
                        .fg(agent_color)
                        .bg(active_bg)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    format!("{} ", title),
                    Style::default()
                        .fg(self.view.theme.sidebar_text)
                        .bg(active_bg)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    state_char.to_string(),
                    Style::default().fg(state_color).bg(active_bg),
                ));
                // Right rounded cap
                spans.push(Span::styled(
                    "\u{2590}", // ▐ right half block — visual right edge
                    Style::default().bg(active_bg).fg(active_bg),
                ));
            } else {
                // Inactive tab: dim, no background
                spans.push(Span::styled(
                    format!(" {} ", agent.icon()),
                    Style::default().fg(self.view.theme.sidebar_dim),
                ));
                spans.push(Span::styled(
                    format!("{} ", title),
                    Style::default().fg(self.view.theme.sidebar_dim),
                ));
                spans.push(Span::styled(
                    format!("{} ", state_char),
                    Style::default().fg(state_color),
                ));
                // Spacer between inactive tabs
                spans.push(Span::raw(" "));
            }
        }

        Line::from(spans)
    }

    fn render_status(&mut self, frame: &mut Frame, area: Rect) {
        let active_count = self.ptys.ptys.len();
        let pty_status = if active_count > 0 {
            let current = self
                .ptys
                .active_pty
                .map(|i| {
                    self.ptys
                        .ptys
                        .get(i)
                        .map(|s| s.info.title.as_str())
                        .unwrap_or("?")
                })
                .unwrap_or("none");
            // P2: Show idle time when agent is quiet
            let idle_info = self
                .ptys
                .active_pty
                .and_then(|i| {
                    self.ptys.ptys.get(i).and_then(|slot| {
                        let idle = slot.handle.idle_secs();
                        if idle >= crate::pty::IDLE_THRESHOLD_SECS {
                            Some(format!(" (quiet {}s)", idle))
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_default();
            Span::styled(
                format!(" [{} active: {}{}]", active_count, current, idle_info),
                Style::default().fg(self.view.theme.status_done),
            )
        } else {
            Span::raw("")
        };

        // Budget alert indicator in status bar
        let budget_span = if let Some(ref msg) = self.popup.budget_alert {
            self.popup.budget_flash_on = !self.popup.budget_flash_on;
            if self.popup.budget_flash_on {
                Span::styled(
                    format!(" {} ", msg),
                    Style::default()
                        .fg(Color::White)
                        .bg(self.view.theme.status_error)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(
                    format!(" {} ", msg),
                    Style::default()
                        .fg(self.view.theme.status_error)
                        .bg(Color::Black),
                )
            }
        } else {
            Span::raw("")
        };


        let chain_span = if let Some(ref chain) = self.chains.active_chain {
            let agent_label = if chain.current_step < chain.total_steps {
                self.chains
                    .chains
                    .iter()
                    .find(|c| c.name == chain.chain_name)
                    .and_then(|c| c.steps.get(chain.current_step))
                    .map(|s| format!(" ({})", s.agent.label()))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            Span::styled(
                format!(
                    " Chain: {}/{}{} ",
                    chain.current_step + 1,
                    chain.total_steps,
                    agent_label
                ),
                Style::default()
                    .fg(self.view.theme.agent_omp)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::raw("")
        };

        // Resource usage for the active PTY only
        let stats_span = {
            let stats = self
                .ptys
                .active_pty
                .and_then(|idx| self.ptys.ptys.get(idx))
                .and_then(|s| s.process_stats.clone());
            if let Some(ref stats) = stats {
                if stats.cpu_percent > 0.0 || stats.mem_rss_kb > 0 {
                    Span::styled(
                        format!(
                            " CPU:{:.1}% MEM:{}",
                            stats.cpu_percent,
                            crate::procfs::format_bytes(stats.mem_rss_kb * 1024)
                        ),
                        Style::default().fg(self.view.theme.accent),
                    )
                } else {
                    Span::raw("")
                }
            } else {
                Span::raw("")
            }
        };

        let mode_span = if self.view.focus == Focus::Chat
            && self.view.chat_mode == ChatMode::Passthrough
        {
            Span::styled(
                " RAW",
                Style::default()
                    .fg(self.view.theme.status_running),
            )
        } else {
            Span::raw("")
        };
        let line = Line::from(vec![
            mode_span,
            Span::styled(
                self.view.status.clone(),
                Style::default().fg(self.view.theme.sidebar_text),
            ),
            chain_span,
            pty_status,
            stats_span,
            budget_span,
            Span::raw("  "),
            Span::styled(
                self.view.keybinds.status_hint(),
                Style::default().fg(self.view.theme.sidebar_dim),
            ),
        ]);

        frame.render_widget(
            Paragraph::new(line)
                .style(Style::default().bg(self.view.theme.sidebar_bg)),
            area,
        );
    }

    fn render_budget_warning(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(60, 12, area);

        let msg = self
            .popup
            .budget_alert
            .as_deref()
            .unwrap_or("Budget exceeded");

        let lines = vec![
            Line::from(vec![Span::styled(
                "  TOKEN BUDGET ALERT",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(Span::styled(
                format!("  {}", msg),
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Token usage has exceeded your configured budget limit.",
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                "  Consider pausing active sessions or increasing the budget.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Press any key to dismiss",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .title(" Budget Warning ")
                        .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Red))
                        .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                )
                .wrap(Wrap { trim: true }),
            popup_area,
        );
    }

    fn render_help_popup(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(48, 24, area);
        let mut lines: Vec<Line> = vec![
            Line::from(vec![Span::styled(
                "  Sidebar Keybindings",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
        ];
        for (action, key) in self.view.keybinds.help_sidebar_pairs() {
            lines.push(Line::from(format!("  {:<14} {}", key, action)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Chat: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Tab=sidebar  Ctrl+J/K=switch  Ctrl+Q=kill",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Scroll: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "PgUp/Dn  Ctrl+B/F  Home/End",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Extra: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "s=sort  o=open dir  c/x/g/o=quick-agent  G=archived",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            format!(
                "  Full list: {} → Keybindings",
                self.view.keybinds.settings.display()
            ),
            Style::default().fg(Color::DarkGray),
        )]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "  Press any key to close",
            Style::default().fg(Color::Yellow),
        )]));
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                        .title(" Help ")
                        .title_style(
                            Style::default()
                                .fg(self.view.theme.popup_border)
                                .add_modifier(Modifier::BOLD),
                        )
                        .border_style(Style::default().fg(self.view.theme.popup_border)),
                )
                .wrap(Wrap { trim: false }),
            popup_area,
        );
    }

    fn render_settings_popup(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(55, 18, area);

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(vec![Span::styled(
            "  Workspaces",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));

        for (i, ws) in self.sessions.workspaces.iter().enumerate() {
            let path_str = ws
                .path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "virtual".into());
            let marker = if i == self.sessions.workspaces.len() - 1 {
                ">"
            } else {
                " "
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {} ", marker), Style::default().fg(Color::Yellow)),
                Span::styled(ws.name.clone(), Style::default().fg(Color::White)),
                Span::styled(
                    format!("  {}", path_str),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Actions: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "a",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("dd  "),
            Span::styled(
                "r",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("ename  "),
            Span::styled(
                "k",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("eybinds  "),
            Span::styled(
                "t",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("hemes  "),
            Span::styled(
                "b",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("udget  "),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" close"),
        ]));
        lines.push(Line::from(""));
        // Show current budget status
        let budget_status = if let Some(ref b) = self.token_budget {
            let mut parts = Vec::new();
            if let Some(dt) = b.daily_tokens {
                parts.push(format!("{}k daily tokens", dt / 1000));
            }
            if let Some(dc) = b.daily_cost {
                parts.push(format!("${:.2} daily cost", dc));
            }
            if let Some(wt) = b.weekly_tokens {
                parts.push(format!("{}k weekly tokens", wt / 1000));
            }
            if let Some(wc) = b.weekly_cost {
                parts.push(format!("${:.2} weekly cost", wc));
            }
            format!("Budget: {} (b to clear)", parts.join(", "))
        } else {
            "Budget: not set (b to set default 100k daily)".into()
        };
        lines.push(Line::from(vec![Span::styled(
            format!("  {} ", budget_status),
            Style::default().fg(Color::DarkGray),
        )]));
        lines.push(Line::from(vec![Span::styled(
            "  (targets last workspace in list)",
            Style::default().fg(Color::DarkGray),
        )]));

        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                        .title(" Settings ")
                        .title_style(
                            Style::default()
                                .fg(self.view.theme.popup_border)
                                .add_modifier(Modifier::BOLD),
                        )
                        .border_style(Style::default().fg(self.view.theme.popup_border)),
                )
                .wrap(Wrap { trim: false }),
            popup_area,
        );
    }

    fn render_theme_select(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(40, 16, area);

        let items: Vec<ListItem> = self
            .theme_list
            .iter()
            .map(|t| {
                let label = t.label();
                let is_current = *t == self.view.theme_name;
                let prefix = if is_current { ">> " } else { "   " };
                let style = if is_current {
                    Style::default()
                        .fg(self.view.theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.view.theme.popup_text)
                };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        prefix,
                        Style::default().fg(self.view.theme.sidebar_highlight),
                    ),
                    Span::styled(label, style),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Themes ")
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(Clear, popup_area);
        frame.render_stateful_widget(list, popup_area, &mut self.theme_list_state);
    }

    fn render_keybind_view(&mut self, frame: &mut Frame, area: Rect) {
        // Use 80% width, 80% height — big but not fullscreen
        let popup_area = centered_rect(80, 80, area);
        let kb = &self.view.keybinds;
        let mut lines: Vec<Line> = Vec::new();
        // Section: Configurable bindings
        lines.push(Line::from(Span::styled(
            "  Configurable (edit config.json to customize)",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        for line in kb.display_lines() {
            lines.push(Line::from(format!("  {}", line)));
        }
        // Section: Navigation
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Navigation",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from("  Tab              Sidebar ↔ Chat"));
        lines.push(Line::from("  Alt+h            Chat → Sidebar"));
        lines.push(Line::from("  Ctrl+J/K         Switch active PTY tab"));
        lines.push(Line::from("  Ctrl+Shift+J/K   Reorder PTY tabs"));
        lines.push(Line::from("  Alt+h/Alt+l      Cycle popup panels"));
        // Section: Sidebar extra
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Sidebar Extra",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(
            "  c/x/o            Quick create Claude/Codex/OMP",
        ));
        lines.push(Line::from("  1/2/3            Filter by agent type"));
        lines.push(Line::from("  Space            Mark/unmark session"));
        lines.push(Line::from("  s                Cycle sort mode"));
        lines.push(Line::from("  S                Semantic search (BM25)"));
        lines.push(Line::from("  o                Open workspace directory"));
        lines.push(Line::from("  !                Pin/unpin session"));
        lines.push(Line::from("  p                Template select"));
        lines.push(Line::from("  G                Toggle archived sessions"));
        // Section: Session Preview
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Session Preview",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from("  s                Toggle summary view"));
        lines.push(Line::from("  b                Rollback to snapshot"));
        lines.push(Line::from("  k                Toggle knowledge view"));
        lines.push(Line::from("  c                Clear knowledge / copy"));
        // Section: Chat/PTY
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Chat / PTY",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from("  Ctrl+Q           Kill current session"));
        lines.push(Line::from("  Ctrl+Y           Copy session title"));
        lines.push(Line::from("  PgUp/PgDn        Scroll PTY output"));
        lines.push(Line::from(
            "  Ctrl+B/F         Scroll page up/down (vi-style)",
        ));
        lines.push(Line::from("  Home/End         Scroll to top/bottom"));
        lines.push(Line::from(
            "  y                Copy visible screen (when scrolled)",
        ));
        // Section: Panels & info
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Panels & Info",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from("  Ctrl+S           Activity statistics"));
        lines.push(Line::from("  Ctrl+T           Token usage"));
        lines.push(Line::from("  Ctrl+G           Session timeline"));
        lines.push(Line::from("  Ctrl+W           Agent recommendations"));
        lines.push(Line::from("  Ctrl+F           Cross-session search"));
        lines.push(Line::from("  Ctrl+R           Remote sessions"));
        lines.push(Line::from("  Ctrl+P           Plugin list"));
        lines.push(Line::from("  Ctrl+A           Automation select"));
        lines.push(Line::from("  Ctrl+E           Chain select"));
        lines.push(Line::from("  B                Git branch"));
        lines.push(Line::from("  X                Diff view"));
        // Conflicts
        let conflicts = kb.validate();
        if !conflicts.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  ⚠ Conflicts:",
                Style::default().fg(Color::Red),
            )));
            for (a, b) in &conflicts {
                lines.push(Line::from(format!("    {} <-> {}", a, b)));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ↑/↓ or j/k scroll · Alt+h/Alt+l cycle panels · Esc close",
            Style::default().fg(Color::DarkGray),
        )));
        let total_lines = lines.len() as u16;
        let visible_height = popup_area.height.saturating_sub(2); // minus borders
        let max_scroll = total_lines.saturating_sub(visible_height);
        if self.popup.keybind_scroll > max_scroll {
            self.popup.keybind_scroll = max_scroll;
        }
        let scroll = self.popup.keybind_scroll;
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            Paragraph::new(lines)
                .scroll((scroll, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                        .title(format!(
                            " Keybindings ({}/{}) ",
                            scroll + visible_height.min(total_lines),
                            total_lines,
                        ))
                        .title_style(
                            Style::default()
                                .fg(self.view.theme.popup_border)
                                .add_modifier(Modifier::BOLD),
                        )
                        .border_style(Style::default().fg(self.view.theme.popup_border)),
                )
                .wrap(Wrap { trim: false }),
            popup_area,
        );
    }

    fn render_template_select(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(50, 12, area);

        let items: Vec<ratatui::widgets::ListItem<'static>> = self
            .templates
            .iter()
            .map(|t| {
                let agent_label = t.agent.label();
                let ws = t.workspace_id.as_deref().unwrap_or("current");
                ratatui::widgets::ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {} ", agent_label),
                        Style::default().fg(t.agent.color()),
                    ),
                    Span::styled(t.name.clone(), Style::default().fg(Color::White)),
                    Span::styled(
                        format!("  ws: {}", ws),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]))
            })
            .collect();

        let list = ratatui::widgets::List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Templates (p) ")
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(Clear, popup_area);
        frame.render_stateful_widget(list, popup_area, &mut self.template_state);
    }

    fn render_automation_select(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(50, 12, area);

        let items: Vec<ratatui::widgets::ListItem<'static>> = self
            .automations
            .iter()
            .map(|a| {
                ratatui::widgets::ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {} steps ", a.steps.len()),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(a.name.clone(), Style::default().fg(Color::White)),
                ]))
            })
            .collect();

        let list = ratatui::widgets::List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Automations (Ctrl+A) ")
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(Clear, popup_area);
        frame.render_stateful_widget(list, popup_area, &mut self.automation_state);
    }

    fn render_chain_select(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(50, 14, area);

        let items: Vec<ratatui::widgets::ListItem<'static>> = self
            .chains
            .chains
            .iter()
            .map(|c| {
                let steps_preview: Vec<String> = c
                    .steps
                    .iter()
                    .map(|s| s.agent.label().to_string())
                    .collect();
                let steps_str = steps_preview.join(" -> ");
                ratatui::widgets::ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(
                            format!(" {} steps ", c.steps.len()),
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::styled(c.name.clone(), Style::default().fg(Color::White)),
                    ]),
                    Line::from(format!("    {}", steps_str))
                        .style(Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect();

        let list = ratatui::widgets::List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Chains (Ctrl+E) ")
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(Clear, popup_area);
        frame.render_stateful_widget(list, popup_area, &mut self.chains.chain_state);
    }

    fn render_branch_select(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(60, 14, area);

        let items: Vec<ratatui::widgets::ListItem<'static>> = self
            .popup
            .branch_points
            .iter()
            .map(|bp| {
                ratatui::widgets::ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" #{} ", bp.index + 1),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(bp.summary.clone(), Style::default().fg(Color::White)),
                ]))
            })
            .collect();

        let list = ratatui::widgets::List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Branch Points (B) ")
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(Clear, popup_area);
        frame.render_stateful_widget(list, popup_area, &mut self.branch_state);
    }

    fn render_stats(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(60, 16, area);

        // Compute per-agent stats from sessions + ptys
        let mut stats: Vec<AgentStats> = self
            .available_agents
            .iter()
            .map(|&agent| {
                let total = self
                    .sessions
                    .sessions
                    .iter()
                    .filter(|s| s.agent == agent)
                    .count();
                let active = self
                    .ptys
                    .ptys
                    .iter()
                    .filter(|p| p.info.agent == agent && !p.info.completed)
                    .count();
                let completed = self
                    .ptys
                    .ptys
                    .iter()
                    .filter(|p| p.info.agent == agent && p.info.completed)
                    .count();
                AgentStats {
                    agent,
                    total_sessions: total,
                    active_sessions: active,
                    completed_sessions: completed,
                }
            })
            .collect();

        // Also count sessions from agents not in available_agents
        for s in &self.sessions.sessions {
            if !self.available_agents.contains(&s.agent) {
                if let Some(st) = stats.iter_mut().find(|st| st.agent == s.agent) {
                    st.total_sessions += 1;
                } else {
                    stats.push(AgentStats {
                        agent: s.agent,
                        total_sessions: 1,
                        active_sessions: 0,
                        completed_sessions: 0,
                    });
                }
            }
        }

        let total_all: usize = stats.iter().map(|s| s.total_sessions).sum();
        let active_all: usize = stats.iter().map(|s| s.active_sessions).sum();

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(" Total Sessions: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", total_all),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   Active: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", active_all),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            " Agent            Sessions   Active   Done",
            Style::default().fg(Color::DarkGray),
        )]));
        lines.push(Line::from(vec![Span::styled(
            " ─────────────────────────────────────────",
            Style::default().fg(Color::DarkGray),
        )]));

        for st in &stats {
            if st.total_sessions == 0 && st.active_sessions == 0 {
                continue;
            }
            let label = st.agent.label();
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {:<16} ", label),
                    Style::default().fg(st.agent.color()),
                ),
                Span::styled(
                    format!("{:>3}        ", st.total_sessions),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("{:>2}       ", st.active_sessions),
                    Style::default().fg(if st.active_sessions > 0 {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(
                    format!("{:>2}", st.completed_sessions),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            format!(
                " Workspaces: {}   Tabs: {}",
                self.sessions.workspaces.len(),
                self.ptys.ptys.len()
            ),
            Style::default().fg(Color::DarkGray),
        )]));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            paragraph.block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Activity Stats (Ctrl+S) ")
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            ),
            popup_area,
        );
    }

    fn render_token_stats(&self, frame: &mut Frame, area: Rect) {
        use crate::discovery::{extract_token_usage, find_session_jsonl};

        let popup_area = centered_rect(65, 20, area);

        // Aggregate token usage per agent across all sessions
        let mut agent_tokens: std::collections::HashMap<Agent, (u64, u64, u64, f64)> =
            std::collections::HashMap::new();

        for session in &self.sessions.sessions {
            if let Some(jsonl) = find_session_jsonl(session)
                && let Some(usage) = extract_token_usage(&jsonl)
            {
                let entry = agent_tokens.entry(session.agent).or_insert((0, 0, 0, 0.0));
                entry.0 += usage.input_tokens;
                entry.1 += usage.output_tokens;
                entry.2 += usage.total_tokens;
                entry.3 += usage.cost;
            }
        }

        let total_input: u64 = agent_tokens.values().map(|v| v.0).sum();
        let total_output: u64 = agent_tokens.values().map(|v| v.1).sum();
        let total_all: u64 = agent_tokens.values().map(|v| v.2).sum();
        let total_cost: f64 = agent_tokens.values().map(|v| v.3).sum();

        let fmt_tokens = |n: u64| -> String {
            if n >= 1_000_000 {
                format!("{:.1}M", n as f64 / 1_000_000.0)
            } else if n >= 1_000 {
                format!("{:.1}K", n as f64 / 1_000.0)
            } else {
                format!("{}", n)
            }
        };

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(" Total Tokens: ", Style::default().fg(Color::Gray)),
            Span::styled(
                fmt_tokens(total_all),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  In: ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_tokens(total_input), Style::default().fg(Color::Cyan)),
            Span::styled("  Out: ", Style::default().fg(Color::Gray)),
            Span::styled(fmt_tokens(total_output), Style::default().fg(Color::Yellow)),
        ]));
        if total_cost > 0.0 {
            lines.push(Line::from(vec![Span::styled(
                format!(" Total Cost: ${:.4}", total_cost),
                Style::default().fg(Color::Green),
            )]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            " Agent          Input        Output       Total",
            Style::default().fg(Color::DarkGray),
        )]));
        lines.push(Line::from(vec![Span::styled(
            " ───────────────────────────────────────────────",
            Style::default().fg(Color::DarkGray),
        )]));

        let mut agents: Vec<_> = agent_tokens.iter().collect();
        agents.sort_by_key(|b| std::cmp::Reverse(b.1.2));
        for (agent, (inp, out, total, cost)) in agents {
            let mut spans = vec![
                Span::styled(
                    format!(" {:<13}  ", agent.label()),
                    Style::default().fg(agent.color()),
                ),
                Span::styled(
                    format!("{:>9}    ", fmt_tokens(*inp)),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("{:>9}    ", fmt_tokens(*out)),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    format!("{:>9}", fmt_tokens(*total)),
                    Style::default().fg(Color::White),
                ),
            ];
            if *cost > 0.0 {
                spans.push(Span::styled(
                    format!("  ${:.3}", cost),
                    Style::default().fg(Color::Green),
                ));
            }
            lines.push(Line::from(spans));
        }

        if agent_tokens.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                " No token data found in sessions.",
                Style::default().fg(Color::DarkGray),
            )]));
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            paragraph.block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Token Usage (Ctrl+T) ")
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            ),
            popup_area,
        );
    }

    fn render_diff_view(&self, frame: &mut Frame, area: Rect) {
        use crate::discovery::DiffKind;

        let popup_area = centered_rect(75, 24, area);

        let lines: Vec<Line<'static>> = self
            .popup
            .diff_lines
            .iter()
            .map(|dl| {
                let (prefix, color) = match dl.kind {
                    DiffKind::Context => (" ", Color::DarkGray),
                    DiffKind::LeftOnly => ("-", Color::Red),
                    DiffKind::RightOnly => ("+", Color::Green),
                };
                let truncated: String = dl.content.chars().take(100).collect();
                Line::from(vec![
                    Span::styled(
                        prefix,
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(truncated, Style::default().fg(color)),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            paragraph.block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Session Diff (- left, + right) ")
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            ),
            popup_area,
        );
    }

    fn render_remote_view(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(70, 20, area);

        let lines: Vec<Line<'static>> = self
            .remote_sessions
            .iter()
            .map(|(name, agent)| {
                let color = match agent.as_str() {
                    "Claude" => Color::Magenta,
                    "Codex" => Color::Green,
                    "OMP" => Color::Blue,
                    "Error" => Color::Red,
                    "Info" => Color::Yellow,
                    _ => Color::DarkGray,
                };
                Line::from(vec![
                    Span::styled(format!(" {} ", agent), Style::default().fg(color)),
                    Span::styled(name.clone(), Style::default().fg(Color::White)),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            paragraph.block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Remote Sessions (Ctrl+R) ")
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            ),
            popup_area,
        );
    }

    fn render_plugin_list(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(50, 14, area);

        let items: Vec<ratatui::widgets::ListItem<'static>> = self
            .plugins
            .iter()
            .map(|p| {
                let key_label = p.key.map(|k| format!(" ({})", k)).unwrap_or_default();
                let hook_label = if p.hooks.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", p.hooks.join(","))
                };
                ratatui::widgets::ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {}{}{}", p.name, key_label, hook_label),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(
                        format!("  — {}", &p.command[..p.command.len().min(40)]),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]))
            })
            .collect();

        let list = ratatui::widgets::List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Plugins (Ctrl+P) ")
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(Clear, popup_area);
        frame.render_stateful_widget(list, popup_area, &mut self.plugin_state);
    }

    fn render_plugin_output(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(70, 20, area);
        let inner = {
            let b = Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .title(" Plugin Output (Esc/q=close, j/k=scroll) ")
                .title_style(
                    Style::default()
                        .fg(self.view.theme.popup_border)
                        .add_modifier(Modifier::BOLD),
                )
                .border_style(Style::default().fg(self.view.theme.popup_border));
            b.inner(popup_area)
        };

        let visible_height = inner.height as usize;
        let total = self.plugin_output.len();
        let max_scroll = total.saturating_sub(visible_height);
        let scroll = self.plugin_scroll.min(max_scroll);

        let lines: Vec<Line<'static>> = self
            .plugin_output
            .iter()
            .skip(scroll)
            .take(visible_height)
            .map(|line| {
                if line.starts_with("$ ") {
                    Line::from(vec![Span::styled(
                        line.clone(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )])
                } else if line.contains("\x1b[") {
                    // ANSI-colored output: parse into spans
                    ansi_to_spans(line)
                } else {
                    // Check for JSON action lines and highlight them
                    let trimmed = line.trim();
                    if trimmed.starts_with('{') && trimmed.contains("\"action\"") {
                        Line::from(vec![Span::styled(
                            line.clone(),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::ITALIC),
                        )])
                    } else {
                        Line::from(vec![Span::styled(
                            line.clone(),
                            Style::default().fg(Color::White),
                        )])
                    }
                }
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            paragraph.block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(format!(
                        " Plugin Output [{}/{}] (Esc/q=close, j/k=scroll) ",
                        scroll + 1,
                        total
                    ))
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            ),
            popup_area,
        );
    }

    fn render_timeline(&self, frame: &mut Frame, area: Rect) {
        use crate::util::relative_time;
        let popup_area = centered_rect(70, 22, area);
        let now = crate::util::now_secs();

        let lines: Vec<Line<'static>> = self
            .timeline_events
            .iter()
            .rev()
            .take(30)
            .rev()
            .map(|ev| {
                let agent_color = match ev.agent.as_str() {
                    "Claude" => Color::Magenta,
                    "OMP" => Color::Blue,
                    "Codex" => Color::Yellow,
                    _ => Color::DarkGray,
                };
                let type_icon = if ev.event_type == "user" {
                    "▸"
                } else {
                    "◂"
                };
                let time = if ev.timestamp > now {
                    "now".into()
                } else {
                    relative_time(now - ev.timestamp)
                };
                Line::from(vec![
                    Span::styled(format!(" {} ", time), Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{} ", ev.agent), Style::default().fg(agent_color)),
                    Span::styled(
                        type_icon,
                        Style::default().fg(if ev.event_type == "user" {
                            Color::Cyan
                        } else {
                            Color::Yellow
                        }),
                    ),
                    Span::styled(
                        format!(" {}", ev.summary),
                        Style::default().fg(Color::White),
                    ),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            paragraph.block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Timeline (Ctrl+G) ")
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            ),
            popup_area,
        );
    }

    fn render_conflict_resolve(&self, frame: &mut Frame, area: Rect) {
        let height = (self.popup.conflict_warnings.len() * 2 + 9).min(30) as u16;
        let popup_area = centered_rect(80, height, area);
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(vec![Span::styled(
            "  \u{26a0} File Conflict Detected",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));
        for w in &self.popup.conflict_warnings {
            lines.push(Line::from(vec![
                Span::styled("  \u{26a0} ", Style::default().fg(Color::Red)),
                Span::styled(w.clone(), Style::default().fg(Color::Yellow)),
            ]));
        }
        lines.push(Line::from(""));
        let git_ok = crate::worktree::git_available();
        if git_ok {
            lines.push(Line::from(vec![
                Span::styled(
                    "  [I] ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Isolate into git worktrees",
                    Style::default().fg(Color::White),
                ),
            ]));
        }
        lines.push(Line::from(vec![
            Span::styled(
                "  [D] ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Dismiss", Style::default().fg(Color::DarkGray)),
        ]));
        if !git_ok {
            lines.push(Line::from(Span::styled(
                "  Note: git not found — worktree isolation unavailable",
                Style::default().fg(Color::DarkGray),
            )));
        }
        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            paragraph.block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Conflict Detection ")
                    .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                    .border_style(Style::default().fg(Color::Red)),
            ),
            popup_area,
        );
    }

    fn render_agent_recommend(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(60, 16, area);
        let mut lines: Vec<Line<'static>> = vec![
            Line::from(vec![Span::styled(
                "  Agent Recommendations",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(Span::styled(
                "  Ranked by sessions:",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
        ];

        for (i, m) in self.agent_recommendations.iter().enumerate() {
            let medal = match i {
                0 => "\u{1f947}", // 🥇
                1 => "\u{1f948}", // 🥈
                2 => "\u{1f949}", // 🥉
                _ => "  ",
            };
            let agent_color = match m.agent.as_str() {
                "Claude" => Color::Magenta,
                "OMP" => Color::Blue,
                "Codex" => Color::Yellow,
                _ => Color::White,
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {} ", medal), Style::default()),
                Span::styled(
                    format!("{:<8}", m.agent),
                    Style::default()
                        .fg(agent_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {} sessions", m.total_sessions),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Press any key to close.",
            Style::default().fg(Color::DarkGray),
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            paragraph.block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Recommendations (Ctrl+W) ")
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            ),
            popup_area,
        );
    }

    fn render_cross_search(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(70, 22, area);
        let mut lines: Vec<Line<'static>> = vec![
            Line::from(vec![Span::styled(
                "  Cross-Session Search",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
        ];

        for result in &self.cross_search_results {
            let agent_color = match result.agent.as_str() {
                "Claude" => Color::Magenta,
                "OMP" => Color::Blue,
                "Codex" => Color::Yellow,
                _ => Color::White,
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", result.agent),
                    Style::default()
                        .fg(agent_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    result.session_title.clone(),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!(" ({} matches)", result.matches.len()),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            for m in result.matches.iter().take(2) {
                lines.push(Line::from(vec![
                    Span::styled("    ", Style::default()),
                    Span::styled(truncate_str(m, 70), Style::default().fg(Color::DarkGray)),
                ]));
            }
            lines.push(Line::from(""));
            if lines.len() > 25 {
                break;
            }
        }

        lines.push(Line::from(Span::styled(
            "  Press any key to close",
            Style::default().fg(Color::DarkGray),
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            paragraph.block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Search Results (Ctrl+F) ")
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            ),
            popup_area,
        );
    }

    fn render_semantic_search(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(70, 24, area);
        let mut lines: Vec<Line<'static>> = vec![
            Line::from(vec![Span::styled(
                "  BM25 Semantic Search",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
        ];

        if self.search_results.is_empty() {
            // Show input prompt
            lines.push(Line::from(Span::styled(
                format!("  Query: {}", self.input_buffer),
                Style::default().fg(Color::White),
            )));
            lines.push(Line::from(Span::styled(
                "  █",
                Style::default().fg(Color::White),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Type query and press Enter to search",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            // Show results with selection highlight
            let selected = self.search_result_state.selected();
            for (i, (session_id, score)) in self.search_results.iter().enumerate() {
                let is_sel = selected == Some(i);
                let short_id = &session_id[..8.min(session_id.len())];

                // Find session title
                let title = self
                    .sessions
                    .sessions
                    .iter()
                    .find(|s| s.id == *session_id)
                    .map(|s| s.title.as_str())
                    .unwrap_or("(unknown)");

                let pct = (score * 100.0).round() as u8;
                let prefix = if is_sel { " > " } else { "   " };
                let style = if is_sel {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                lines.push(Line::from(vec![
                    Span::styled(
                        prefix,
                        Style::default().fg(if is_sel {
                            Color::Yellow
                        } else {
                            Color::DarkGray
                        }),
                    ),
                    Span::styled(
                        format!("{} ", short_id),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(truncate_str(title, 45), style),
                    Span::styled(format!("  {}%", pct), Style::default().fg(Color::DarkGray)),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  j/k=navigate · Enter=select · Esc=cancel",
                Style::default().fg(Color::DarkGray),
            )));
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            paragraph.block(
                Block::default()
                    .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Semantic Search (Shift+S) ")
                    .title_style(
                        Style::default()
                            .fg(self.view.theme.popup_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(self.view.theme.popup_border)),
            ),
            popup_area,
        );
    }

    fn render_session_preview(&self, frame: &mut Frame, area: Rect) {
        let height_pct = 20;
        let popup_area = centered_rect(65, height_pct, area);
        let mut lines: Vec<Line<'static>> = Vec::new();
        let is_summary = self.popup.preview_show_summary;
        let is_auto = self.view.input_mode == InputMode::SummaryPreview;
        if self.popup.knowledge_view {
            lines.push(Line::from(vec![Span::styled(
                "  Knowledge Base",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )]));
        } else if is_summary {
            lines.push(Line::from(vec![Span::styled(
                "  Session Summary",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]));
        } else {
            lines.push(Line::from(vec![Span::styled(
                "  Session Content Preview",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]));
        }
        lines.push(Line::from(""));
        // Show tags if available
        if let Some(ref sid) = self.popup.preview_session_id
            && let Some(session) = self.sessions.sessions.iter().find(|s| s.id == *sid)
            && !session.tags.is_empty()
        {
            lines.push(Line::from(vec![
                Span::styled("  Tags: ", Style::default().fg(Color::DarkGray)),
                Span::styled(session.tags.join(", "), Style::default().fg(Color::Magenta)),
            ]));
            lines.push(Line::from(""));
        }
        if is_summary {
            // Render summary with simple markdown formatting
            if self.popup.preview_lines.is_empty() {
                lines.push(Line::from("  No summary available."));
            } else {
                let mut in_code_block = false;
                for entry in &self.popup.preview_lines {
                    let text = &entry.text;
                    if text.starts_with("```") {
                        in_code_block = !in_code_block;
                        lines.push(Line::from(""));
                        continue;
                    }
                    if in_code_block {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {}", text),
                            Style::default().fg(Color::DarkGray),
                        )]));
                    } else if let Some(stripped) = text.strip_prefix("# ") {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {}", stripped),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        )]));
                        lines.push(Line::from(""));
                    } else if let Some(stripped) = text.strip_prefix("## ") {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {}", stripped),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )]));
                        lines.push(Line::from(""));
                    } else if let Some(stripped) = text.strip_prefix("### ") {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {}", stripped),
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        )]));
                    } else if let Some(stripped) = text.strip_prefix("- ") {
                        lines.push(Line::from(vec![
                            Span::styled("    • ", Style::default().fg(Color::White)),
                            Span::styled(stripped.to_string(), Style::default().fg(Color::White)),
                        ]));
                    } else if text.trim().is_empty() {
                        lines.push(Line::from(""));
                    } else {
                        lines.push(Line::from(format!("  {}", text)));
                    }
                }
            }
        } else {
            // Original content preview rendering
            if self.popup.preview_lines.is_empty() {
                lines.push(Line::from("  (no text content found)"));
            } else {
                for entry in &self.popup.preview_lines {
                    let (label, color) = if entry.role == "user" {
                        ("You", Color::Yellow)
                    } else {
                        ("Bot", Color::Green)
                    };
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("  {} ", label),
                            Style::default().fg(color).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(entry.text.clone(), Style::default().fg(Color::White)),
                    ]));
                    lines.push(Line::from(""));
                }
            }
        }
        lines.push(Line::from(""));
        if is_auto {
            lines.push(Line::from(vec![Span::styled(
                "  Press any key to dismiss",
                Style::default().fg(Color::Yellow),
            )]));
        } else if self.popup.knowledge_view {
            lines.push(Line::from(vec![Span::styled(
                "  k=back  c=clear  any key=close",
                Style::default().fg(Color::Yellow),
            )]));
        } else if is_summary {
            lines.push(Line::from(vec![Span::styled(
                "  s=content  b=rollback  k=knowledge  any key=close",
                Style::default().fg(Color::Yellow),
            )]));
        } else {
            lines.push(Line::from(vec![Span::styled(
                "  s=summary  b=rollback  k=knowledge  any key=close",
                Style::default().fg(Color::Yellow),
            )]));
        }
        let title = if self.popup.knowledge_view {
            " Knowledge "
        } else if is_summary {
            " Summary "
        } else {
            " Preview (v) "
        };
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                        .title(title)
                        .title_style(
                            Style::default()
                                .fg(self.view.theme.popup_border)
                                .add_modifier(Modifier::BOLD),
                        )
                        .border_style(Style::default().fg(self.view.theme.popup_border)),
                )
                .wrap(Wrap { trim: true }),
            popup_area,
        );
    }

    fn render_preflight_confirm(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(65, 50, area);
        let mut lines: Vec<Line<'_>> = Vec::new();

        lines.push(Line::from(vec![Span::styled(
            "  \u{1f6ec} Pre-flight Check",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));

        if let Some(result) = &self.popup.preflight_result {
            for (label, status) in &result.checks {
                let (icon, color, msg) = match status {
                    crate::preflight::CheckStatus::Pass(m) => ("\u{2713}", Color::Green, m),
                    crate::preflight::CheckStatus::Warn(m) => ("\u{26a0}", Color::Yellow, m),
                    crate::preflight::CheckStatus::Fail(m) => ("\u{2717}", Color::Red, m),
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", icon), Style::default().fg(color)),
                    Span::styled(
                        format!("{:16}", format!("{}:", label)),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(msg.clone(), Style::default().fg(color)),
                ]));
            }

            if !result.suggestions.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "  Suggestions:",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )));
                for s in &result.suggestions {
                    lines.push(Line::from(vec![
                        Span::styled("    \u{2022} ", Style::default().fg(Color::DarkGray)),
                        Span::styled(s.clone(), Style::default().fg(Color::Yellow)),
                    ]));
                }
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                "  Enter/p",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("=proceed  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "f",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "=fix (stash+recheck)  ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "Esc",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("=cancel", Style::default().fg(Color::DarkGray)),
        ]));

        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                        .title(" Pre-flight ")
                        .title_style(
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        )
                        .border_style(Style::default().fg(Color::Cyan)),
                )
                .wrap(Wrap { trim: true }),
            popup_area,
        );
    }

    fn render_rollback_confirm(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(60, 20, area);
        let mut lines: Vec<Line<'_>> = Vec::new();

        lines.push(Line::from(vec![Span::styled(
            "  \u{26a0} Rollback Confirmation",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));

        if let Some(commit) = &self.popup.rollback_snapshot {
            let short = commit[..8.min(commit.len())].to_string();
            lines.push(Line::from(vec![
                Span::styled("  Reset to: ", Style::default().fg(Color::DarkGray)),
                Span::styled(short, Style::default().fg(Color::Yellow)),
            ]));
        }
        lines.push(Line::from(""));

        if self.popup.rollback_files.is_empty() {
            lines.push(Line::from("  No file changes detected."));
        } else {
            lines.push(Line::from(vec![Span::styled(
                format!(
                    "  {} file(s) will be reverted:",
                    self.popup.rollback_files.len()
                ),
                Style::default().fg(Color::White),
            )]));
            lines.push(Line::from(""));
            let max_show = 12;
            for (i, file) in self.popup.rollback_files.iter().enumerate() {
                if i >= max_show {
                    let remaining = self.popup.rollback_files.len() - max_show;
                    lines.push(Line::from(format!("    ... and {} more", remaining)));
                    break;
                }
                lines.push(Line::from(vec![
                    Span::styled("    \u{2022} ", Style::default().fg(Color::DarkGray)),
                    Span::styled(file.clone(), Style::default().fg(Color::White)),
                ]));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                "  y",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("=confirm rollback  ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "n",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("=cancel", Style::default().fg(Color::Yellow)),
        ]));

        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                        .title(" Rollback ")
                        .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                        .border_style(Style::default().fg(Color::Red)),
                )
                .wrap(Wrap { trim: true }),
            popup_area,
        );
    }

    fn render_confirm_delete(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(50, 16, area);
        let mut lines: Vec<Line<'_>> = Vec::new();

        lines.push(Line::from(vec![Span::styled(
            "  Delete Confirmation",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));

        if self.pending_batch_delete {
            lines.push(Line::from(vec![
                Span::styled("  Delete ", Style::default().fg(Color::White)),
                Span::styled(
                    format!("{}", self.view.selected_set.len()),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" marked session(s)?", Style::default().fg(Color::White)),
            ]));
        } else if let Some(node) = &self.pending_delete {
            match node {
                TreeNode::Workspace(wi) => {
                    let ws = &self.sessions.workspaces[*wi];
                    let count = self
                        .sessions
                        .ws_session_map
                        .get(*wi)
                        .map(|v| v.len())
                        .unwrap_or(0);
                    lines.push(Line::from(vec![
                        Span::styled("  Workspace: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(&ws.name, Style::default().fg(Color::Cyan)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("  Sessions: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("{}", count), Style::default().fg(Color::Yellow)),
                    ]));
                }
                TreeNode::Session(_, si) => {
                    if *si < self.sessions.sessions.len() {
                        let title = &self.sessions.sessions[*si].title;
                        lines.push(Line::from(vec![
                            Span::styled("  Session: ", Style::default().fg(Color::DarkGray)),
                            Span::styled(title.clone(), Style::default().fg(Color::White)),
                        ]));
                    }
                }
                _ => {
                    lines.push(Line::from("  Delete selected item?"));
                }
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                "  y",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("=delete  ", Style::default().fg(Color::Yellow)),
            Span::styled(
                "n",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("=cancel", Style::default().fg(Color::Yellow)),
        ]));

        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                        .title(" Delete ")
                        .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                        .border_style(Style::default().fg(Color::Red)),
                )
                .wrap(Wrap { trim: true }),
            popup_area,
        );
    }
}

/// Calculate tab index from a local x-coordinate within the tab bar.
/// Returns `None` if `tab_width` is 0 or `num_tabs` is 0.
pub(super) fn tab_index_from_x(local_x: u16, tab_width: usize, num_tabs: usize) -> Option<usize> {
    if tab_width == 0 || num_tabs == 0 {
        return None;
    }
    let idx = (local_x as usize) / tab_width;
    if idx < num_tabs { Some(idx) } else { None }
}

/// Truncate a title to `max_len` characters, appending "..." if truncated.
/// Returns the original string unchanged if max_len <= 3 or the title fits.
fn truncate_title(title: &str, max_len: usize) -> String {
    if max_len <= 3 || title.len() <= max_len {
        return title.to_string();
    }
    // Find the char boundary at or before max_len - 3
    let end = title
        .char_indices()
        .take_while(|(i, c)| *i + c.len_utf8() <= max_len - 3)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let mut s = title[..end].to_string();
    s.push_str("...");
    s
}

fn session_secondary_text(
    relative_time: &str,
    short_id: &str,
    state_label: Option<&str>,
    tags: &[String],
    diff_summary: Option<&DiffSummary>,
    last_message: Option<&str>,
) -> String {
    let mut parts = vec![format!("{relative_time} ({short_id})")];

    if let Some(state_label) = state_label {
        parts.push(state_label.to_string());
    }
    if !tags.is_empty() {
        parts.push(format!("tags: {}", tags.join(", ")));
    }
    if let Some(diff) = diff_summary
        && !diff.files_changed.is_empty()
    {
        parts.push(format!(
            "+{}/-{} {}f",
            diff.insertions,
            diff.deletions,
            diff.files_changed.len()
        ));
    }
    if let Some(last_message) = last_message
        && !last_message.is_empty()
    {
        parts.push(last_message.to_string());
    }

    format!("   {}", parts.join(" · "))
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    let end = s
        .char_indices()
        .take_while(|(i, c)| *i + c.len_utf8() <= max_len)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    format!("{}...", &s[..end])
}

/// Parse a string containing ANSI escape sequences into ratatui Spans.
/// Supports basic foreground colors (30-37, 90-97), bold (1), italic (3), and reset (0).
fn ansi_to_spans(input: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current_style = Style::default();
    let mut buf = String::new();
    let mut chars = input.chars().peekable();

    let flush = |buf: &mut String, style: Style, spans: &mut Vec<Span<'static>>| {
        if !buf.is_empty() {
            spans.push(Span::styled(buf.clone(), style));
            buf.clear();
        }
    };

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                flush(&mut buf, current_style, &mut spans);
                // Collect the sequence: digits and semicolons until a letter
                let mut seq = String::new();
                while let Some(&sc) = chars.peek() {
                    if sc.is_ascii_digit() || sc == ';' {
                        seq.push(chars.next().unwrap_or('\0'));
                    } else if sc.is_ascii_alphabetic() {
                        chars.next(); // consume the terminator (e.g. 'm')
                        break;
                    } else {
                        break;
                    }
                }
                // Parse the SGR sequence
                for code in seq.split(';').filter_map(|s| s.parse::<u8>().ok()) {
                    match code {
                        0 => current_style = Style::default(),
                        1 => current_style = current_style.add_modifier(Modifier::BOLD),
                        3 => current_style = current_style.add_modifier(Modifier::ITALIC),
                        4 => current_style = current_style.add_modifier(Modifier::UNDERLINED),
                        22 => current_style = current_style.remove_modifier(Modifier::BOLD),
                        30 => current_style = current_style.fg(Color::Black),
                        31 => current_style = current_style.fg(Color::Red),
                        32 => current_style = current_style.fg(Color::Green),
                        33 => current_style = current_style.fg(Color::Yellow),
                        34 => current_style = current_style.fg(Color::Blue),
                        35 => current_style = current_style.fg(Color::Magenta),
                        36 => current_style = current_style.fg(Color::Cyan),
                        37 => current_style = current_style.fg(Color::White),
                        90 => current_style = current_style.fg(Color::DarkGray),
                        91 => current_style = current_style.fg(Color::LightRed),
                        92 => current_style = current_style.fg(Color::LightGreen),
                        93 => current_style = current_style.fg(Color::LightYellow),
                        94 => current_style = current_style.fg(Color::LightBlue),
                        95 => current_style = current_style.fg(Color::LightMagenta),
                        96 => current_style = current_style.fg(Color::LightCyan),
                        97 => current_style = current_style.fg(Color::Gray),
                        _ => {}
                    }
                }
            } else {
                buf.push(c);
            }
        } else {
            buf.push(c);
        }
    }
    flush(&mut buf, current_style, &mut spans);
    if spans.is_empty() {
        spans.push(Span::raw(input.to_string()));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tab_bar_tests {
    use super::*;

    // ─── tab_index_from_x tests ───

    #[test]
    fn tab_index_click_first_tab() {
        // 4 tabs, width=20 each, click at x=0 → index 0
        assert_eq!(tab_index_from_x(0, 20, 4), Some(0));
    }

    #[test]
    fn tab_index_click_second_tab() {
        // 4 tabs, width=20 each, click at x=20 → index 1
        assert_eq!(tab_index_from_x(20, 20, 4), Some(1));
    }

    #[test]
    fn tab_index_click_last_pixel_of_second_tab() {
        // 4 tabs, width=20 each, x=39 → still tab 1
        assert_eq!(tab_index_from_x(39, 20, 4), Some(1));
    }

    #[test]
    fn tab_index_click_third_tab() {
        assert_eq!(tab_index_from_x(40, 20, 4), Some(2));
    }

    #[test]
    fn tab_index_click_beyond_last_tab() {
        // 4 tabs spanning 80px, click at local_x=80 → index 4 which is >= num_tabs → None
        assert_eq!(tab_index_from_x(80, 20, 4), None);
    }

    #[test]
    fn tab_index_click_on_current_tab_returns_valid_index() {
        // The "no switch" logic is in handle_mouse_click, not this helper.
        // This helper always returns the computed index.
        assert_eq!(tab_index_from_x(0, 20, 4), Some(0));
    }

    #[test]
    fn tab_index_zero_tab_width_returns_none() {
        assert_eq!(tab_index_from_x(10, 0, 4), None);
    }

    #[test]
    fn tab_index_zero_num_tabs_returns_none() {
        assert_eq!(tab_index_from_x(10, 20, 0), None);
    }

    #[test]
    fn tab_index_single_tab_always_zero() {
        assert_eq!(tab_index_from_x(0, 80, 1), Some(0));
        assert_eq!(tab_index_from_x(79, 80, 1), Some(0));
    }

    #[test]
    fn tab_index_with_narrow_tabs() {
        // 10 tabs in 80px → tab_width=8
        assert_eq!(tab_index_from_x(0, 8, 10), Some(0));
        assert_eq!(tab_index_from_x(7, 8, 10), Some(0));
        assert_eq!(tab_index_from_x(8, 8, 10), Some(1));
        assert_eq!(tab_index_from_x(72, 8, 10), Some(9));
        assert_eq!(tab_index_from_x(79, 8, 10), Some(9));
    }

    // ─── truncate_title tests ───

    #[test]
    fn truncate_title_fits_within_limit() {
        assert_eq!(truncate_title("hello", 10), "hello");
    }

    #[test]
    fn truncate_title_exact_fit() {
        assert_eq!(truncate_title("hello", 5), "hello");
    }

    #[test]
    fn truncate_title_truncates_long_title() {
        assert_eq!(truncate_title("hello world", 8), "hello...");
    }

    #[test]
    fn truncate_title_small_max_len() {
        // max_len <= 3 returns original
        assert_eq!(truncate_title("hello", 3), "hello");
    }

    #[test]
    fn truncate_title_zero_max_len() {
        assert_eq!(truncate_title("hello", 0), "hello");
    }

    #[test]
    fn truncate_title_empty_string() {
        assert_eq!(truncate_title("", 10), "");
        assert_eq!(truncate_title("", 0), "");
    }

    #[test]
    fn truncate_title_unicode_aware() {
        // Each Greek letter is 2 bytes; max_len=7 gives budget 4 for chars + "..."
        // α (2 bytes at 0) + β (2 bytes at 2) = 4 <= 4. γ at 4 + 2 = 6 > 4.
        assert_eq!(truncate_title("αβγδεζ", 7), "αβ...");
    }

    #[test]
    fn truncate_title_at_boundary() {
        // "hello" is exactly 5 bytes; max_len=5 → fits exactly
        assert_eq!(truncate_title("hello", 5), "hello");
        // max_len=6 still fits
        assert_eq!(truncate_title("hello", 6), "hello");
    }

    #[test]
    fn session_secondary_text_collects_low_priority_metadata() {
        let mut diff = DiffSummary::default();
        diff.insertions = 12;
        diff.deletions = 3;
        diff.files_changed = vec!["src/lib.rs".into(), "README.md".into()];

        let text = session_secondary_text(
            "2h ago",
            "abcdef12",
            Some("done"),
            &["bug".into(), "ui".into()],
            Some(&diff),
            Some("fixed the footer layout"),
        );

        assert_eq!(
            text,
            "   2h ago (abcdef12) · done · tags: bug, ui · +12/-3 2f · fixed the footer layout"
        );
    }

    #[test]
    fn session_secondary_text_omits_empty_parts() {
        let text = session_secondary_text("now", "1234", None, &[], None, None);

        assert_eq!(text, "   now (1234)");
    }

    // ─── tab bar hidden when empty ───

    #[test]
    fn tab_bar_hidden_when_no_ptys() {
        // When ptys is empty, build_tab_bar returns an empty Line
        let mut app = crate::app::tests::test_app(vec![], vec![]);
        let line = app.build_tab_bar(80);
        // An empty Line has no spans
        assert!(
            line.spans.is_empty(),
            "tab bar should be empty when no PTYs active"
        );
    }

    #[test]
    fn tab_bar_hidden_default_rect() {
        // Default Rect has width=0, height=0 — rendering should skip
        let rect = Rect::default();
        assert_eq!(rect.width, 0);
        assert_eq!(rect.height, 0);
    }

    #[test]
    fn handle_mouse_click_ignores_when_no_ptys() {
        let mut app = crate::app::tests::test_app(vec![], vec![]);
        app.view.tab_bar_rect = Rect::new(0, 0, 80, 1);
        app.handle_mouse_click(40, 0);
        assert_eq!(
            app.ptys.active_pty, None,
            "no active_pty when no PTYs exist"
        );
    }
}
