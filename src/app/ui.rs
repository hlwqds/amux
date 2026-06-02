use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use tui_term::widget::PseudoTerminal;

use crate::pty::PtyState;
use crate::types::*;
use crate::util::{PARENT_DIR, SELECT_CURRENT, SELECT_VIRTUAL, centered_rect, relative_time};

impl super::App {
    pub(super) fn chat_size(&self) -> (u16, u16) {
        (
            self.last_chat_area.width.saturating_sub(2),
            self.last_chat_area.height.saturating_sub(2),
        )
    }

    // ─── Rendering ────────────────────────────────────────

    pub(super) fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Length(3)])
            .split(area);

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(chunks[0]);

        self.render_sidebar(frame, cols[0]);
        self.last_chat_area = cols[1];
        self.render_chat(frame, cols[1]);
        self.render_status(frame, chunks[1]);

        if self.input_mode != InputMode::None {
            self.render_input_popup(frame, area);
        }
    }

    fn render_sidebar(&mut self, frame: &mut Frame, area: Rect) {
        let pty_states: Vec<(String, PtyState)> = self
            .ptys
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    s.info.session_id.clone().unwrap_or_default(),
                    self.pty_display_state(i),
                )
            })
            .collect();
        let active_tab_states: Vec<(PtyState, Agent)> = self
            .ptys
            .iter()
            .enumerate()
            .map(|(i, s)| (self.pty_display_state(i), s.info.agent))
            .collect();

        let items: Vec<ListItem> = self
            .tree
            .iter()
            .map(|node| match node {
                TreeNode::Workspace(wi) => {
                    let ws = &self.workspaces[*wi];
                    let icon = if ws.expanded { "\u{25bc}" } else { "\u{25b6}" };
                    let count = self.ws_session_map.get(*wi).map(|v| v.len()).unwrap_or(0);

                    let (binding_icon, binding_style, subtitle) = match &ws.path {
                        Some(p) => (
                            "\u{25c6}",
                            Style::default().fg(Color::Cyan),
                            format!("   {} sessions \u{00b7} {}", count, p.display()),
                        ),
                        None => (
                            "\u{25c7}",
                            Style::default().fg(Color::Yellow),
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
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]),
                        Line::from(subtitle).style(Style::default().fg(Color::DarkGray)),
                    ])
                }
                TreeNode::Session(_wi, si) => {
                    if let Some(session) = self.sessions.get(*si) {
                        let short_id = &session.id[..8.min(session.id.len())];
                        let pty_info = pty_states.iter().find(|(sid, _)| sid == &session.id);
                        let pty_state = pty_info.map(|(_, s)| *s);

                        let agent_tag = Span::styled(
                            format!(" [{}]", session.agent.icon()),
                            Style::default().fg(session.agent.color()),
                        );

                        let (marker, state_tag) = match pty_state {
                            Some(PtyState::Running) => (
                                Span::styled("   \u{25cf} ", Style::default().fg(Color::Yellow)),
                                Span::styled(" [running]", Style::default().fg(Color::Yellow)),
                            ),
                            Some(PtyState::Completed) => (
                                Span::styled("   \u{25cf} ", Style::default().fg(Color::Green)),
                                Span::styled(" \u{2714} done", Style::default().fg(Color::Green)),
                            ),
                            None => (
                                Span::styled("   \u{25cb} ", Style::default().fg(Color::DarkGray)),
                                Span::raw(""),
                            ),
                        };
                        let mut spans = vec![
                            marker,
                            Span::styled(
                                relative_time(session.last_active),
                                Style::default().fg(Color::White),
                            ),
                            Span::styled(
                                format!(" ({})", short_id),
                                Style::default().fg(Color::DarkGray),
                            ),
                            state_tag,
                        ];
                        spans.push(agent_tag);
                        ListItem::new(vec![
                            Line::from(spans),
                            Line::from(format!("     {}", session.title))
                                .style(Style::default().fg(Color::Gray)),
                        ])
                    } else {
                        ListItem::new(Line::from("   \u{25cf} ?"))
                    }
                }
                TreeNode::ActiveTab(pi) => {
                    let title = self
                        .ptys
                        .get(*pi)
                        .map(|s| s.info.title.as_str())
                        .unwrap_or("New Session");
                    let info = active_tab_states.get(*pi);
                    let state = info.map(|(s, _)| *s).unwrap_or(PtyState::Running);
                    let agent = info.map(|(_, a)| *a).unwrap_or(Agent::Claude);
                    let (dot_color, state_text) = match state {
                        PtyState::Running => (Color::Yellow, " [running]"),
                        PtyState::Completed => (Color::Green, " \u{2714} done"),
                    };
                    let title_spans = vec![
                        Span::styled("   \u{25cf} ", Style::default().fg(dot_color)),
                        Span::styled(title, Style::default().fg(Color::White)),
                        Span::styled(state_text, Style::default().fg(Color::Green)),
                        Span::styled(
                            format!(" [{}]", agent.icon()),
                            Style::default().fg(agent.color()),
                        ),
                    ];
                    ListItem::new(vec![
                        Line::from(title_spans),
                        Line::from("     waiting for session file...")
                            .style(Style::default().fg(Color::DarkGray)),
                    ])
                }
                TreeNode::AgentHeader(agent) => ListItem::new(Line::from(vec![Span::styled(
                    format!("  \u{2500}\u{2500} {} \u{2500}\u{2500}", agent.label()),
                    Style::default()
                        .fg(agent.color())
                        .add_modifier(Modifier::DIM),
                )])),
            })
            .collect();

        let border_color = if self.focus == Focus::Sidebar {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        let is_searching = self.input_mode == InputMode::Search;
        let search_query = self.search_query.as_deref().unwrap_or("");

        let sort_label = self.sort_mode.label();

        let title = match (is_searching, &self.agent_filter) {
            (true, Some(agent)) => {
                format!(
                    " [{}] [search: {}] [sort: {}] ",
                    agent.label(),
                    search_query,
                    sort_label
                )
            }
            (true, None) => format!(" [search: {}] [sort: {}] ", search_query, sort_label),
            (false, Some(agent)) => {
                format!(" [{}] Workspaces [sort: {}] ", agent.label(), sort_label)
            }
            (false, None) => format!(" Workspaces [sort: {}] ", sort_label),
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color));

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
                        .bg(Color::Rgb(24, 36, 72))
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("\u{203a}");

            frame.render_stateful_widget(list, chunks[0], &mut self.tree_state);

            let query = self.search_query.as_deref().unwrap_or("");
            let filter_count = self.tree.len();
            let filter_text = match filter_count {
                0 => "0 matches".to_string(),
                1 => "1 match".to_string(),
                n => format!("{} matches", n),
            };

            let search_line = Line::from(vec![
                Span::styled(" search: ", Style::default().fg(Color::Cyan).bold()),
                Span::styled(query.to_string(), Style::default().fg(Color::White)),
                Span::styled("|", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!(" {}", filter_text),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            frame.render_widget(Paragraph::new(search_line), chunks[1]);
            frame.render_widget(block, area);
        } else {
            let list = List::new(items)
                .block(block)
                .highlight_style(
                    Style::default()
                        .bg(Color::Rgb(24, 36, 72))
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("\u{203a}");

            frame.render_stateful_widget(list, area, &mut self.tree_state);
        }
    }

    fn render_chat(&mut self, frame: &mut Frame, area: Rect) {
        let border_color = if self.focus == Focus::Chat {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        let scroll_offset = self
            .active_pty
            .and_then(|idx| self.ptys.get(idx))
            .map(|s| s.handle.scrollback_offset())
            .unwrap_or(0);

        let title = if let Some(idx) = self.active_pty {
            if let Some(slot) = self.ptys.get(idx) {
                let scroll_hint = if scroll_offset > 0 {
                    format!(" [↑{} PgDn:bottom]", scroll_offset)
                } else {
                    String::new()
                };
                format!(
                    " {} [{}] ({}/{}){} ",
                    slot.info.title,
                    slot.info.agent.label(),
                    idx + 1,
                    self.ptys.len(),
                    scroll_hint,
                )
            } else {
                " Agent ".into()
            }
        } else {
            " Agent ".into()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color));

        // When PTYs are active, split inner area into [tab_bar(1)] + [pty_content]
        if !self.ptys.is_empty() {
            let inner = block.inner(area);
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(inner);

            self.tab_bar_rect = chunks[0];

            // Render tab bar
            let tab_line = self.build_tab_bar(chunks[0].width as usize);
            frame.render_widget(
                Paragraph::new(tab_line),
                chunks[0],
            );

            // Render active PTY content
            if let Some(idx) = self.active_pty
                && let Some(slot) = self.ptys.get(idx)
            {
                slot.handle.resize((chunks[1].width, chunks[1].height));

                let parser = slot.handle.screen();
                let screen = parser.read().unwrap().screen().clone();
                let term = PseudoTerminal::new(&screen);
                frame.render_widget(term, chunks[1]);
            }

            frame.render_widget(block, area);
            return;
        }

        // No PTYs — existing placeholder path
        if let Some(idx) = self.active_pty
            && let Some(slot) = self.ptys.get(idx)
        {
            let inner = block.inner(area);
            slot.handle.resize((inner.width, inner.height));

            let parser = slot.handle.screen();
            let screen = parser.read().unwrap().screen().clone();
            let term = PseudoTerminal::new(&screen).block(block);
            frame.render_widget(term, area);
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
                let ws = &self.workspaces[*wi];
                lines.push(
                    Line::from(ws.name.clone()).style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                match &ws.path {
                    Some(p) => {
                        lines.push(
                            Line::from(format!("\u{25c6} {}", p.display()))
                                .style(Style::default().fg(Color::Green)),
                        );
                    }
                    None => {
                        lines.push(
                            Line::from("\u{25c7} Virtual workspace (no directory)")
                                .style(Style::default().fg(Color::Yellow)),
                        );
                    }
                }
                lines.push(Line::from(""));
                lines.push(
                    Line::from("Press Enter to start a named Claude Code session")
                        .style(Style::default().fg(Color::Yellow)),
                );
            }
            Some(TreeNode::Session(_wi, si)) => {
                let session = &self.sessions[*si];
                lines.push(
                    Line::from(session.title.clone()).style(
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                lines.push(
                    Line::from(format!("ID: {}", session.id))
                        .style(Style::default().fg(Color::DarkGray)),
                );
                lines.push(Line::from(format!(
                    "Last active: {}",
                    relative_time(session.last_active)
                )));
                if self.pty_index_for_session(&session.id).is_some() {
                    lines.push(Line::from(""));
                    lines.push(
                        Line::from("This session is already running - Enter to switch to it")
                            .style(Style::default().fg(Color::Green)),
                    );
                } else {
                    lines.push(Line::from(""));
                    lines.push(
                        Line::from("Press Enter to resume this session")
                            .style(Style::default().fg(Color::Yellow)),
                    );
                }
            }
            Some(&TreeNode::ActiveTab(pi)) => {
                let title = self
                    .ptys
                    .get(pi)
                    .map(|s| s.info.title.clone())
                    .unwrap_or_else(|| "New Session".into());
                lines.push(
                    Line::from(title).style(
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                );
                lines.push(
                    Line::from("Session is running...").style(Style::default().fg(Color::DarkGray)),
                );
                lines.push(Line::from(""));
                lines.push(
                    Line::from("Press Enter to switch to this session")
                        .style(Style::default().fg(Color::Yellow)),
                );
            }
            Some(&TreeNode::AgentHeader(agent)) => {
                let label = agent.label().to_string();
                let color = agent.color();
                lines.push(
                    Line::from(label)
                        .style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
                );
                lines.push(Line::from(""));
                lines.push(
                    Line::from("Agent group header").style(Style::default().fg(Color::DarkGray)),
                );
            }
            None => {
                lines.push(Line::from("No selection").style(Style::default().fg(Color::DarkGray)));
            }
        }

        lines.push(Line::from(""));
        lines.push(
            Line::from("\u{2500}\u{2500} Keybindings \u{2500}\u{2500}")
                .style(Style::default().fg(Color::DarkGray)),
        );
        lines.push(Line::from("Enter        New (with name) / Resume / Switch"));
        lines.push(Line::from("e            Expand / collapse workspace"));
        lines.push(Line::from("j/k \u{2191}\u{2193}     Navigate tree"));
        lines.push(Line::from("r            Refresh sessions"));
        lines.push(Line::from("R            Rename selected session"));
        lines.push(Line::from("N            New workspace"));
        lines.push(Line::from("D            Delete workspace"));
        lines.push(Line::from("Tab          Toggle sidebar/chat"));
        lines.push(Line::from("Ctrl+J/K     Switch between active sessions"));
        lines.push(Line::from("Ctrl+Q       Kill current session"));
        lines.push(Line::from("q / Esc      Quit"));

        lines
    }

    fn render_input_popup(&mut self, frame: &mut Frame, area: Rect) {
        if self.input_mode == InputMode::BrowseDir {
            self.render_browse_popup(frame, area);
            return;
        }
        if self.input_mode == InputMode::SelectAgent {
            self.render_agent_popup(frame, area);
            return;
        }

        let popup = centered_rect(60, 20, area);
        frame.render_widget(Clear, popup);

        let (title, label) = match self.input_mode {
            InputMode::SessionName => (" New Session ", "Session name: "),
            InputMode::RenameSession => (" Rename Session ", "New name: "),
            InputMode::RenameWorkspace => (" Rename Workspace ", "New name: "),
            InputMode::NewWorkspaceName => (" New Workspace ", "Workspace name: "),
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

        let help = Line::from(" C:Claude  X:Codex  G:GSD  j/k:navigate  Enter:confirm  Esc:cancel")
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

    fn build_tab_bar(&self, width: usize) -> Line<'static> {
        if self.ptys.is_empty() {
            return Line::raw("");
        }

        let n_tabs = self.ptys.len();
        let sep = "\u{2502}"; // │
        let sep_width = sep.len();
        // Each tab gets equal share, minus separators
        let total_sep_width = n_tabs.saturating_sub(1) * sep_width;
        let tab_width = if width > total_sep_width {
            (width - total_sep_width) / n_tabs
        } else {
            // Extremely narrow: just show agent icons
            2
        };

        let mut spans: Vec<Span<'static>> = Vec::new();

        for (i, slot) in self.ptys.iter().enumerate() {
            let is_active = self.active_pty == Some(i);
            let state = self.pty_display_state(i);

            // State indicator
            let (state_char, state_color) = match state {
                PtyState::Running => ("\u{25cf}", Color::Yellow),  // ●
                PtyState::Completed => ("\u{2714}", Color::Green), // ✔
            };

            // Calculate available space for title
            // Format: " [icon] title... state "
            // Fixed parts: " [C] " (4) + " " (1) + state + " " (1) = ~8 chars
            let fixed_overhead = 4 + state_char.len() + 2;
            let max_title = tab_width.saturating_sub(fixed_overhead);

            let title = truncate_title(&slot.info.title, max_title);

            let agent = slot.info.agent;

            if is_active {
                // Active tab: highlighted background
                let active_bg = Color::Rgb(24, 36, 72);
                spans.push(Span::styled(
                    format!(" [{}] ", agent.icon()),
                    Style::default().fg(agent.color()).bg(active_bg).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    format!("{} ", title),
                    Style::default().fg(Color::White).bg(active_bg),
                ));
                spans.push(Span::styled(
                    format!("{} ", state_char),
                    Style::default().fg(state_color).bg(active_bg),
                ));
            } else {
                // Inactive tab: dimmed
                spans.push(Span::styled(
                    format!(" [{}] ", agent.icon()),
                    Style::default().fg(Color::DarkGray),
                ));
                spans.push(Span::styled(
                    format!("{} ", title),
                    Style::default().fg(Color::DarkGray),
                ));
                spans.push(Span::styled(
                    format!("{} ", state_char),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            // Separator between tabs (not after last)
            if i < n_tabs - 1 {
                spans.push(Span::styled(
                    sep.to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }

        Line::from(spans)
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let active_count = self.ptys.len();
        let pty_status = if active_count > 0 {
            let current = self
                .active_pty
                .map(|i| {
                    self.ptys
                        .get(i)
                        .map(|s| s.info.title.as_str())
                        .unwrap_or("?")
                })
                .unwrap_or("none");
            Span::styled(
                format!(" [{} active: {}]", active_count, current),
                Style::default().fg(Color::Green),
            )
        } else {
            Span::raw("")
        };

        let line = Line::from(vec![
            Span::styled(self.status.clone(), Style::default().fg(Color::White)),
            pty_status,
            Span::raw("  "),
            Span::styled(
                "Enter:new/resume e:expand r:refresh R:rename N:new-ws D:del-ws Tab:toggle Ctrl+J/K:switch Ctrl+Q:kill q:quit",
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        frame.render_widget(
            Paragraph::new(line).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            ),
            area,
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
    if idx < num_tabs {
        Some(idx)
    } else {
        None
    }
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

    // ─── tab bar hidden when empty ───

    #[test]
    fn tab_bar_hidden_when_no_ptys() {
        // When ptys is empty, build_tab_bar returns an empty Line
        let mut app = crate::app::tests::test_app(vec![], vec![]);
        let line = app.build_tab_bar(80);
        // An empty Line has no spans
        assert!(line.spans.is_empty(), "tab bar should be empty when no PTYs active");
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
        app.tab_bar_rect = Rect::new(0, 0, 80, 1);
        app.handle_mouse_click(40, 0);
        assert_eq!(app.active_pty, None, "no active_pty when no PTYs exist");
    }
}
