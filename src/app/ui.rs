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
use crate::util::relative_time;

const fn ansi_to_ratatui_color(c: AnsiColor) -> Option<Color> {
    match c {
        AnsiColor::Spec(rgb) => Some(Color::Rgb(rgb.r, rgb.g, rgb.b)),
        AnsiColor::Indexed(i) => Some(Color::Indexed(i)),
        AnsiColor::Named(named) => named_to_ratatui_color(named),
    }
}

const fn named_to_ratatui_color(n: NamedColor) -> Option<Color> {
    use NamedColor::*;
    match n {
        Foreground | Background | Cursor | DimForeground | BrightForeground => None,
        Black | DimBlack => Some(Color::Black),
        Red | DimRed => Some(Color::Red),
        Green | DimGreen => Some(Color::Green),
        Yellow | DimYellow => Some(Color::Yellow),
        Blue | DimBlue => Some(Color::Blue),
        Magenta | DimMagenta => Some(Color::Magenta),
        Cyan | DimCyan => Some(Color::Cyan),
        White | DimWhite => Some(Color::Gray),
        BrightBlack => Some(Color::DarkGray),
        BrightRed => Some(Color::LightRed),
        BrightGreen => Some(Color::LightGreen),
        BrightYellow => Some(Color::LightYellow),
        BrightBlue => Some(Color::LightBlue),
        BrightMagenta => Some(Color::LightMagenta),
        BrightCyan => Some(Color::LightCyan),
        BrightWhite => Some(Color::White),
    }
}

/// Render an alacritty terminal grid directly into a ratatui frame buffer.
/// This bypasses ratatui's widget system for pixel-perfect terminal rendering.
fn render_grid_to_frame<T: alacritty_terminal::event::EventListener>(
    frame: &mut Frame,
    term: &alacritty_terminal::sync::FairMutex<alacritty_terminal::term::Term<T>>,
    area: Rect,
) {
    let guard = term.lock();
    let display_offset = guard.grid().display_offset();
    let grid = guard.grid();
    let screen_rows = u16::try_from(guard.screen_lines()).unwrap_or(u16::MAX);
    let screen_cols = u16::try_from(guard.columns()).unwrap_or(u16::MAX);
    let max_rows = area.height.min(screen_rows);
    let max_cols = area.width.min(screen_cols);
    for y in 0..max_rows {
        let line_idx = i32::from(y) - i32::try_from(display_offset).unwrap_or(i32::MAX);
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
            let target_x = area.x + x;
            let target_y = area.y + y;
            if let Some(buf_cell) = frame.buffer_mut().cell_mut((target_x, target_y)) {
                let mut tmp = [0u8; 4];
                buf_cell.set_symbol(ch.encode_utf8(&mut tmp));
                buf_cell.set_style(style);
            }
        }
    }
    drop(guard);
}
impl super::App {
    pub(super) const fn chat_size(&self) -> (u16, u16) {
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
            .constraints([
                Constraint::Percentage(self.view.split_ratio),
                Constraint::Percentage(100 - self.view.split_ratio),
            ])
            .split(chunks[0]);

        self.render_sidebar(frame, cols[0]);
        self.view.last_chat_area = cols[1];
        if self.terminal.is_some() {
            let term_height = (cols[1].height / 3).max(5);
            let chat_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(4), Constraint::Length(term_height)])
                .split(cols[1]);
            self.view.last_chat_area = chat_chunks[0];
            self.render_chat(frame, chat_chunks[0]);
            self.render_terminal(frame, chat_chunks[1]);
        } else {
            self.render_chat(frame, cols[1]);
        }
        self.render_status(frame, chunks[1]);

        match self.view.input_mode {
            InputMode::Help => self.render_help_popup(frame, area),
            InputMode::SessionPreview | InputMode::SummaryPreview => {
                self.render_session_preview(frame, area);
            }
            InputMode::Settings => self.render_settings_popup(frame, area),
            InputMode::KeybindView => self.render_keybind_view(frame, area),
            InputMode::ThemeSelect => self.render_theme_select(frame, area),
            InputMode::TemplateSelect => self.render_template_select(frame, area),
            InputMode::AutomationSelect => self.render_automation_select(frame, area),
            InputMode::BranchSelect => self.render_branch_select(frame, area),
            InputMode::Stats => self.render_stats(frame, area),
            InputMode::TokenStats => self.render_token_stats(frame, area),
            InputMode::DiffView => self.render_diff_view(frame, area),
            InputMode::RemoteView => self.render_remote_view(frame, area),
            InputMode::PluginList => self.render_plugin_list(frame, area),
            InputMode::PluginOutput => self.render_plugin_output(frame, area),
            InputMode::Timeline => self.render_timeline(frame, area),
            InputMode::ConflictWarning | InputMode::ConflictResolve => {
                self.render_conflict_resolve(frame, area);
            }
            InputMode::AgentRecommend => self.render_agent_recommend(frame, area),
            InputMode::CrossSearch => self.render_cross_search(frame, area),
            InputMode::BudgetWarning => self.render_budget_warning(frame, area),
            InputMode::ChainSelect => self.render_chain_select(frame, area),
            InputMode::RollbackConfirm => self.render_rollback_confirm(frame, area),
            InputMode::ConfirmDelete => self.render_confirm_delete(frame, area),
            InputMode::PreflightConfirm => self.render_preflight_confirm(frame, area),
            InputMode::SemanticSearch => self.render_semantic_search(frame, area),
            InputMode::SelectAgent
            | InputMode::SessionName
            | InputMode::NewWorkspaceName
            | InputMode::RenameSession
            | InputMode::RenameWorkspace
            | InputMode::BrowseDir
            | InputMode::Search
            | InputMode::TagFilter => self.render_input_popup(frame, area),
            InputMode::None | InputMode::ScrollbackSearch => {}
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
        let active_tab_data: Vec<(PtyState, String, Agent, CheckStatus, DiffSummary)> = self
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
                    let arrow = if self.sessions.pinned_expanded {
                        "▼"
                    } else {
                        "▶"
                    };
                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(
                                format!("{arrow} 📌 "),
                                Style::default().fg(self.view.theme.sidebar_highlight),
                            ),
                            Span::styled(
                                "Pinned",
                                Style::default().fg(self.view.theme.sidebar_highlight),
                            ),
                        ]),
                        Line::from(format!("   {count} pinned sessions"))
                            .style(Style::default().fg(self.view.theme.sidebar_dim)),
                    ])
                }
                TreeNode::RecentWorkspace => {
                    let count = self.sessions.recent_count;
                    let arrow = if self.sessions.recent_expanded {
                        "▼"
                    } else {
                        "▶"
                    };
                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(
                                format!("{arrow} 🕐 "),
                                Style::default().fg(self.view.theme.sidebar_highlight),
                            ),
                            Span::styled(
                                "Recent",
                                Style::default().fg(self.view.theme.sidebar_highlight),
                            ),
                        ]),
                        Line::from(format!("   {count} recent sessions"))
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
                            format!("   {count} sessions \u{00b7} virtual"),
                        ),
                    };

                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(
                                format!("{icon} {binding_icon} "),
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
                        let title = Self::truncate_title(
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
                        let detail_line = Self::session_secondary_text(
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
                    let (state, title, agent, check, diff_summary) =
                        active_tab_data.get(*pi).cloned().unwrap_or_else(|| {
                            (
                                PtyState::Running,
                                "New Session".into(),
                                Agent::Claude,
                                CheckStatus::Pending,
                                DiffSummary::default(),
                            )
                        });
                    let (dot_color, state_text) = match state {
                        PtyState::Running => (self.view.theme.status_running, " [running]".into()),
                        PtyState::Completed => match &check {
                            CheckStatus::Failed(e) => {
                                (self.view.theme.status_error, format!(" \u{26a0} {e}"))
                            }
                            CheckStatus::Running => (
                                self.view.theme.status_running,
                                " \u{23f3} checking...".into(),
                            ),
                            CheckStatus::Passed | CheckStatus::Pending => {
                                (self.view.theme.status_done, " \u{2714} done".into())
                            }
                        },
                    };
                    let state_color = dot_color;
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
                    ];
                    let detail = if state == PtyState::Completed {
                        let ds = &diff_summary;
                        if ds.files_changed.is_empty() {
                            Line::from("     no changes detected")
                                .style(Style::default().fg(self.view.theme.sidebar_dim))
                        } else {
                            Line::from(vec![
                                Span::raw("     "),
                                Span::styled(
                                    format!("+{}", ds.insertions),
                                    Style::default().fg(Color::Green),
                                ),
                                Span::raw("/"),
                                Span::styled(
                                    format!("-{}", ds.deletions),
                                    Style::default().fg(Color::Red),
                                ),
                                Span::styled(
                                    format!(" in {} file(s)", ds.files_changed.len()),
                                    Style::default().fg(self.view.theme.sidebar_dim),
                                ),
                            ])
                        }
                    } else {
                        Line::from("     waiting for session file...")
                            .style(Style::default().fg(self.view.theme.sidebar_dim))
                    };
                    ListItem::new(vec![Line::from(title_spans), detail])
                }
                TreeNode::WorkspaceWarning(_, msg) => {
                    ListItem::new(Line::from(vec![Span::styled(
                        format!("  \u{26a0} {msg}"),
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
                        format!("  \u{25b6} Archived ({count})"),
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
                                    format!(" ({short_id})"),
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
                format!(" [search: {search_query}] [tag: {tag}] [sort: {sort_label}] ")
            }
            (true, None, None) => format!(" [search: {search_query}] [sort: {sort_label}] "),
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
                format!(" [tag: {tag}] Workspaces [sort: {sort_label}] ")
            }
            (false, None, None) => format!(" Workspaces [sort: {sort_label}] "),
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
                n => format!("{n} matches"),
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
                    format!(" {filter_text}"),
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
                    format!(" [↑{scroll_offset} PgDn:bottom]")
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
                let mode_tags = format!(
                    "{}{}",
                    if self.view.scrollback_regex {
                        " [REGEX]"
                    } else {
                        ""
                    },
                    if self.view.scrollback_case_sensitive {
                        " [CASE]"
                    } else {
                        ""
                    },
                );
                let search_text = if query.is_empty() {
                    format!(" Search:_{mode_tags}")
                } else if total == 0 {
                    format!(" Search: {query} (no matches){mode_tags}")
                } else {
                    format!(" Search: {query} ({current}/{total}){mode_tags}")
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
                    render_grid_to_frame(frame, &slot.handle.term(), pty_area);
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
                            let screen_y =
                                pty_area.y + u16::try_from(row - vis_start).unwrap_or(u16::MAX);
                            let screen_x = pty_area.x + col;
                            if screen_x + u16::try_from(len).unwrap_or(u16::MAX)
                                > pty_area.x + pty_area.width
                            {
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
                            for c in col..col + u16::try_from(len).unwrap_or(u16::MAX) {
                                match slot.handle.cell_contents(row, c as usize) {
                                    Some(t) => text.push_str(&t),
                                    None => text.push(' '),
                                }
                            }
                            let span = Span::styled(text, highlight_style);
                            let highlight_area = Rect {
                                x: screen_x,
                                y: screen_y,
                                width: u16::try_from(len).unwrap_or(u16::MAX),
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

            render_grid_to_frame(frame, &slot.handle.term(), inner);
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

    fn render_terminal(&self, frame: &mut Frame, area: Rect) {
        let Some(slot) = &self.terminal else {
            return;
        };
        // Border with title
        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(self.view.theme.accent))
            .title(" Shell ")
            .title_style(
                Style::default()
                    .fg(self.view.theme.accent)
                    .add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(area);

        slot.handle.resize((inner.width, inner.height));
        let term_arc = slot.handle.term();
        let guard = term_arc.lock();
        let display_offset = guard.grid().display_offset();
        let grid = guard.grid();
        let screen_rows = u16::try_from(guard.screen_lines()).unwrap_or(u16::MAX);
        let screen_cols = u16::try_from(guard.columns()).unwrap_or(u16::MAX);
        let max_rows = inner.height.min(screen_rows);
        let max_cols = inner.width.min(screen_cols);
        for y in 0..max_rows {
            let line_idx = i32::from(y) - i32::try_from(display_offset).unwrap_or(i32::MAX);
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
                    Line::from("Press Enter to start a new session")
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
                    "{count} pinned session(s) from all workspaces"
                )));
                lines.push(Line::from(""));
                lines.push(
                    Line::from("Press ! on a session to unpin it")
                        .style(Style::default().fg(self.view.theme.sidebar_dim)),
                );
            }
            Some(&TreeNode::RecentWorkspace) => {
                let count = self.sessions.recent_count;
                lines.push(
                    Line::from("🕐 Recent Sessions").style(
                        Style::default()
                            .fg(self.view.theme.sidebar_highlight)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                lines.push(Line::from(format!(
                    "{count} recent session(s) from all workspaces"
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

    fn render_status(&mut self, frame: &mut Frame, area: Rect) {
        // Budget alert — flashing, keep when present
        let budget_span = if let Some(ref msg) = self.popup.budget_alert {
            self.popup.budget_flash_on = !self.popup.budget_flash_on;
            if self.popup.budget_flash_on {
                Span::styled(
                    format!(" {msg} "),
                    Style::default()
                        .fg(self.view.theme.bold_text)
                        .bg(self.view.theme.status_error)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(
                    format!(" {msg} "),
                    Style::default().fg(self.view.theme.status_error),
                )
            }
        } else {
            Span::raw("")
        };

        let chain_span = if let Some(ref chain) = self.chains.active_chain {
            Span::styled(
                format!(" {}/{} ", chain.current_step + 1, chain.total_steps,),
                Style::default().fg(self.view.theme.agent_omp),
            )
        } else {
            Span::raw("")
        };

        let stats_span = {
            let stats = if self.view.focus == Focus::Chat {
                self.ptys
                    .active_pty
                    .and_then(|idx| self.ptys.ptys.get(idx))
                    .and_then(|s| s.process_stats.clone())
            } else {
                None
            };
            if let Some(ref stats) = stats {
                if stats.cpu_percent > 0.0 || stats.mem_rss_kb > 0 {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let cpu_pct = stats.cpu_percent.round().clamp(0.0, u32::MAX as f64) as u32;
                    Span::styled(
                        format!(
                            " ⚡{}% 🧠{}",
                            cpu_pct,
                            crate::procfs::format_bytes(stats.mem_rss_kb * 1024)
                        ),
                        Style::default().fg(self.view.theme.status_text),
                    )
                } else {
                    Span::raw("")
                }
            } else {
                Span::raw("")
            }
        };

        // Left side: volatile content (status messages, alerts)
        // Auto-expire status after 3 seconds.
        // Detect when status string changes and record the timestamp.
        let status_text = {
            let elapsed = self.view.status_set_at.elapsed();
            if elapsed < std::time::Duration::from_secs(3) {
                self.view.status.clone()
            } else {
                String::new()
            }
        };
        let left = Line::from(vec![
            Span::styled(
                status_text,
                Style::default().fg(self.view.theme.status_text),
            ),
            budget_span,
        ]);

        // Focus indicator
        let focus_label = match self.view.focus {
            Focus::Sidebar => "◂ SIDEBAR",
            Focus::Chat => "CHAT ▸",
        };
        let focus_span = Span::styled(
            format!(" {focus_label} "),
            Style::default()
                .fg(Color::Black)
                .bg(self.view.theme.accent)
                .add_modifier(Modifier::BOLD),
        );
        // Right side: stable content (chain, resource usage, focus)
        let right = Line::from(vec![chain_span, stats_span, focus_span]);

        let base_style = Style::default()
            .fg(self.view.theme.status_text)
            .bg(self.view.theme.status_bg);

        frame.render_widget(
            Paragraph::new(right).style(base_style).right_aligned(),
            area,
        );
        // Render left on top (overwrites the left portion)
        frame.render_widget(Paragraph::new(left).style(base_style), area);
    }

    pub(super) fn build_tab_bar(&self, width: usize) -> Line<'static> {
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
            let title = Self::truncate_title(&slot.info.title, max_title);
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
                    format!("{title} "),
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
                    format!("{title} "),
                    Style::default().fg(self.view.theme.sidebar_dim),
                ));
                spans.push(Span::styled(
                    format!("{state_char} "),
                    Style::default().fg(state_color),
                ));
                // Spacer between inactive tabs
                spans.push(Span::raw(" "));
            }
        }

        Line::from(spans)
    }

    /// Calculate tab index from a local x-coordinate within the tab bar.
    /// Returns `None` if `tab_width` is 0 or `num_tabs` is 0.
    pub(super) const fn tab_index_from_x(
        local_x: u16,
        tab_width: usize,
        num_tabs: usize,
    ) -> Option<usize> {
        if tab_width == 0 || num_tabs == 0 {
            return None;
        }
        let idx = (local_x as usize) / tab_width;
        if idx < num_tabs { Some(idx) } else { None }
    }

    /// Truncate a title to `max_len` characters, appending "..." if truncated.
    /// Returns the original string unchanged if max_len <= 3 or the title fits.
    pub(super) fn truncate_title(title: &str, max_len: usize) -> String {
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

    pub(super) fn truncate_str(s: &str, max_len: usize) -> String {
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
    pub(super) fn ansi_to_spans(input: &str) -> Line<'static> {
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
}

#[cfg(test)]
mod tab_bar_tests {
    use super::*;
    use super::super::App;

    // ─── tab_index_from_x tests ───

    #[test]
    fn tab_index_click_first_tab() {
        // 4 tabs, width=20 each, click at x=0 → index 0
        assert_eq!(App::tab_index_from_x(0, 20, 4), Some(0));
    }

    #[test]
    fn tab_index_click_second_tab() {
        // 4 tabs, width=20 each, click at x=20 → index 1
        assert_eq!(App::tab_index_from_x(20, 20, 4), Some(1));
    }

    #[test]
    fn tab_index_click_last_pixel_of_second_tab() {
        // 4 tabs, width=20 each, x=39 → still tab 1
        assert_eq!(App::tab_index_from_x(39, 20, 4), Some(1));
    }

    #[test]
    fn tab_index_click_third_tab() {
        assert_eq!(App::tab_index_from_x(40, 20, 4), Some(2));
    }

    #[test]
    fn tab_index_click_beyond_last_tab() {
        // 4 tabs spanning 80px, click at local_x=80 → index 4 which is >= num_tabs → None
        assert_eq!(App::tab_index_from_x(80, 20, 4), None);
    }

    #[test]
    fn tab_index_click_on_current_tab_returns_valid_index() {
        // The "no switch" logic is in handle_mouse_click, not this helper.
        // This helper always returns the computed index.
        assert_eq!(App::tab_index_from_x(0, 20, 4), Some(0));
    }

    #[test]
    fn tab_index_zero_tab_width_returns_none() {
        assert_eq!(App::tab_index_from_x(10, 0, 4), None);
    }

    #[test]
    fn tab_index_zero_num_tabs_returns_none() {
        assert_eq!(App::tab_index_from_x(10, 20, 0), None);
    }

    #[test]
    fn tab_index_single_tab_always_zero() {
        assert_eq!(App::tab_index_from_x(0, 80, 1), Some(0));
        assert_eq!(App::tab_index_from_x(79, 80, 1), Some(0));
    }

    #[test]
    fn tab_index_with_narrow_tabs() {
        // 10 tabs in 80px → tab_width=8
        assert_eq!(App::tab_index_from_x(0, 8, 10), Some(0));
        assert_eq!(App::tab_index_from_x(7, 8, 10), Some(0));
        assert_eq!(App::tab_index_from_x(8, 8, 10), Some(1));
        assert_eq!(App::tab_index_from_x(72, 8, 10), Some(9));
        assert_eq!(App::tab_index_from_x(79, 8, 10), Some(9));
    }

    // ─── truncate_title tests ───

    #[test]
    fn truncate_title_fits_within_limit() {
        assert_eq!(App::truncate_title("hello", 10), "hello");
    }

    #[test]
    fn truncate_title_exact_fit() {
        assert_eq!(App::truncate_title("hello", 5), "hello");
    }

    #[test]
    fn truncate_title_truncates_long_title() {
        assert_eq!(App::truncate_title("hello world", 8), "hello...");
    }

    #[test]
    fn truncate_title_small_max_len() {
        // max_len <= 3 returns original
        assert_eq!(App::truncate_title("hello", 3), "hello");
    }

    #[test]
    fn truncate_title_zero_max_len() {
        assert_eq!(App::truncate_title("hello", 0), "hello");
    }

    #[test]
    fn truncate_title_empty_string() {
        assert_eq!(App::truncate_title("", 10), "");
        assert_eq!(App::truncate_title("", 0), "");
    }

    #[test]
    fn truncate_title_unicode_aware() {
        // Each Greek letter is 2 bytes; max_len=7 gives budget 4 for chars + "..."
        // α (2 bytes at 0) + β (2 bytes at 2) = 4 <= 4. γ at 4 + 2 = 6 > 4.
        assert_eq!(App::truncate_title("αβγδεζ", 7), "αβ...");
    }

    #[test]
    fn truncate_title_at_boundary() {
        // "hello" is exactly 5 bytes; max_len=5 → fits exactly
        assert_eq!(App::truncate_title("hello", 5), "hello");
        // max_len=6 still fits
        assert_eq!(App::truncate_title("hello", 6), "hello");
    }

    #[test]
    fn session_secondary_text_collects_low_priority_metadata() {
        let diff = DiffSummary {
            insertions: 12,
            deletions: 3,
            files_changed: vec!["src/lib.rs".into(), "README.md".into()],
            ..Default::default()
        };

        let text = App::session_secondary_text(
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
        let text = App::session_secondary_text("now", "1234", None, &[], None, None);

        assert_eq!(text, "   now (1234)");
    }

    // ─── tab bar hidden when empty ───

    #[test]
    fn tab_bar_hidden_when_no_ptys() {
        // When ptys is empty, build_tab_bar returns an empty Line
        let app = crate::app::tests::test_app(vec![], vec![]);
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
