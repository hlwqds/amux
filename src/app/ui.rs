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
            })
            .collect();

        let border_color = if self.focus == Focus::Sidebar {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Workspaces ")
            .border_style(Style::default().fg(border_color));

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
