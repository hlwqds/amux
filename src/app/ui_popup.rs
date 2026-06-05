use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use super::ui::{ansi_to_spans, truncate_str, truncate_title};
use super::*;

impl App {
    pub(super) fn render_input_popup(&mut self, frame: &mut Frame, area: Rect) {
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

    pub(super) fn render_agent_popup(&mut self, frame: &mut Frame, area: Rect) {
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

    pub(super) fn render_browse_popup(&mut self, frame: &mut Frame, area: Rect) {
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

    pub(super) fn build_tab_bar(&mut self, width: usize) -> Line<'static> {
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

    pub(super) fn render_budget_warning(&self, frame: &mut Frame, area: Rect) {
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

    pub(super) fn render_help_popup(&self, frame: &mut Frame, area: Rect) {
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
                "PgUp/Dn  b/f  Home/End",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Extra: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "s=sort  o=open dir  c/x/g/o=quick-agent",
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

    pub(super) fn render_settings_popup(&self, frame: &mut Frame, area: Rect) {
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

    pub(super) fn render_theme_select(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(40, 16, area);

        let items: Vec<ListItem> = self
            .theme_list
            .iter()
            .filter(|t| {
                self.view.picker_query.is_empty()
                    || code_fuzzy_match::fuzzy_match(t.label(), &self.view.picker_query)
                        .is_some_and(|s| s > 0)
            })
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

        let title = if self.view.picker_query.is_empty() {
            " Themes ".into()
        } else {
            format!(" Themes [{}] ", self.view.picker_query)
        };

        let list = List::new(items)
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
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(Clear, popup_area);
        frame.render_stateful_widget(list, popup_area, &mut self.theme_list_state);
    }

    pub(super) fn render_keybind_view(&mut self, frame: &mut Frame, area: Rect) {
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
        // Section: Sidebar extra
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Sidebar Extra",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(
            "  c/x/o            Quick select agent (in agent picker)",
        ));
        lines.push(Line::from("  Space            Mark/unmark session"));
        lines.push(Line::from("  s                Cycle sort mode"));
        lines.push(Line::from("  S                Semantic search (BM25)"));
        lines.push(Line::from("  o                Open workspace directory"));
        lines.push(Line::from("  !                Pin/unpin session"));
        lines.push(Line::from("  p                Template select"));
        lines.push(Line::from("  Alt+Shift+P      Plugin list"));
        lines.push(Line::from("  Alt+Shift+A      Automation select"));
        lines.push(Line::from("  B                Git branch"));
        lines.push(Line::from("  Alt+Shift+G      Toggle archived sessions"));
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
        lines.push(Line::from(
            "  F12              Toggle Amux ↔ Passthrough mode",
        ));
        lines.push(Line::from(Span::styled(
            "    Amux (command)  — letters are actions (b/f/t/s/e/g/w/r/x/y)",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "    Passthrough     — all keys forwarded to PTY (normal typing)",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from("  Ctrl+Q           Kill current session"));
        lines.push(Line::from("  Ctrl+Y           Copy session title"));
        lines.push(Line::from("  Ctrl+J/K         Switch active PTY tab"));
        lines.push(Line::from("  Ctrl+Shift+J/K   Reorder PTY tabs"));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Amux Mode Keys",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from("  b                Scrollback page up"));
        lines.push(Line::from("  f                Scrollback search"));
        lines.push(Line::from("  t                Token usage"));
        lines.push(Line::from("  s                Activity stats"));
        lines.push(Line::from("  e                Chain select"));
        lines.push(Line::from("  g                Session timeline"));
        lines.push(Line::from("  w                Agent recommendations"));
        lines.push(Line::from("  r                Remote sessions"));
        lines.push(Line::from("  x                Diff view"));
        lines.push(Line::from("  y                Copy screen (when scrolled)"));
        lines.push(Line::from("  PgUp/PgDn        Scroll PTY output"));
        lines.push(Line::from("  Home/End         Scroll to top/bottom"));
        lines.push(Line::from(Span::styled(
            "  (Ctrl/Alt/Shift modified keys still forward to PTY)",
            Style::default().fg(Color::DarkGray),
        )));
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
        let total_lines = u16::try_from(lines.len()).unwrap_or(u16::MAX);
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

    pub(super) fn render_template_select(&mut self, frame: &mut Frame, area: Rect) {
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

    pub(super) fn render_automation_select(&mut self, frame: &mut Frame, area: Rect) {
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
                    .title(" Automations (a) ")
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

    pub(super) fn render_chain_select(&mut self, frame: &mut Frame, area: Rect) {
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
                    .title(" Chains (e) ")
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

    pub(super) fn render_branch_select(&mut self, frame: &mut Frame, area: Rect) {
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

    pub(super) fn render_stats(&self, frame: &mut Frame, area: Rect) {
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
                    .title(" Activity Stats (s) ")
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

    pub(super) fn render_token_stats(&self, frame: &mut Frame, area: Rect) {
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
                    .title(" Token Usage (t) ")
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

    pub(super) fn render_diff_view(&self, frame: &mut Frame, area: Rect) {
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

    pub(super) fn render_remote_view(&self, frame: &mut Frame, area: Rect) {
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
                    .title(" Remote Sessions (r) ")
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

    pub(super) fn render_plugin_list(&mut self, frame: &mut Frame, area: Rect) {
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
                    .title(" Plugins (Alt+Shift+P) ")
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

    pub(super) fn render_plugin_output(&self, frame: &mut Frame, area: Rect) {
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

    pub(super) fn render_timeline(&self, frame: &mut Frame, area: Rect) {
        use crate::util::relative_time;
        let popup_area = centered_rect(70, 24, area);
        let now = crate::util::now_secs();

        let mut lines: Vec<Line<'static>> = Vec::new();

        // Git log section
        if let Some(idx) = self.ptys.active_pty
            && let Some(slot) = self.ptys.ptys.get(idx)
            && let Ok(output) = std::process::Command::new("git")
                .args([
                    "log",
                    "--graph",
                    "--oneline",
                    "--decorate",
                    "--color=always",
                    "-15",
                ])
                .current_dir(&slot.info.workspace_path)
                .output()
            && output.status.success()
        {
            let git_out = String::from_utf8_lossy(&output.stdout);
            lines.push(Line::from(Span::styled(
                " Git Log",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            for line in git_out.lines().take(10) {
                lines.push(Line::from(line.to_string()));
            }
            lines.push(Line::from(""));
        }

        // Timeline events
        let timeline_lines: Vec<Line<'static>> = self
            .timeline_events
            .iter()
            .rev()
            .take(20)
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
        lines.extend(timeline_lines);

        let paragraph = Paragraph::new(lines);
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            paragraph.block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .title(" Timeline (g) ")
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

    pub(super) fn render_conflict_resolve(&self, frame: &mut Frame, area: Rect) {
        let height =
            u16::try_from((self.popup.conflict_warnings.len() * 2 + 9).min(30)).unwrap_or(u16::MAX);
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

    pub(super) fn render_agent_recommend(&self, frame: &mut Frame, area: Rect) {
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
                    .title(" Recommendations (w) ")
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

    pub(super) fn render_cross_search(&self, frame: &mut Frame, area: Rect) {
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
                    .title(" Search Results (f) ")
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

    pub(super) fn render_semantic_search(&mut self, frame: &mut Frame, area: Rect) {
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

                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let pct = (score * 100.0).round().clamp(0.0, 255.0) as u8;
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

    pub(super) fn render_session_preview(&self, frame: &mut Frame, area: Rect) {
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

    pub(super) fn render_preflight_confirm(&self, frame: &mut Frame, area: Rect) {
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

    pub(super) fn render_rollback_confirm(&self, frame: &mut Frame, area: Rect) {
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

    pub(super) fn render_confirm_delete(&self, frame: &mut Frame, area: Rect) {
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::test_app;
    use crate::budget::TokenBudget;
    use crate::util::centered_rect;

    // ── 1. Help view content: help_sidebar_pairs returns expected pairs ──
    #[test]
    fn help_sidebar_pairs_contains_all_actions() {
        let app = test_app(vec![], vec![]);
        let pairs = app.view.keybinds.help_sidebar_pairs();
        // Default Keybinds has 11 entries (move_up/down through quit)
        assert_eq!(pairs.len(), 11, "expected 11 sidebar pairs");
        assert_eq!(pairs[0].0, "Move selection");
        assert_eq!(pairs.last().unwrap().0, "Quit");
        for (action, key) in &pairs {
            assert!(!key.is_empty(), "key for '{}' must not be empty", action);
        }
    }

    // ── 2. Settings display content: budget line reflects token_budget ──
    #[test]
    fn settings_budget_line_with_token_budget() {
        let mut app = test_app(vec![], vec![]);
        assert!(app.token_budget.is_none());
        app.token_budget = Some(TokenBudget {
            daily_tokens: Some(200_000),
            daily_cost: Some(5.50),
            weekly_tokens: None,
            weekly_cost: None,
        });
        let b = app.token_budget.as_ref().unwrap();
        let mut parts = Vec::new();
        if let Some(dt) = b.daily_tokens {
            parts.push(format!("{}k daily tokens", dt / 1000));
        }
        if let Some(dc) = b.daily_cost {
            parts.push(format!("${:.2} daily cost", dc));
        }
        let expected = format!("Budget: {}", parts.join(", "));
        assert_eq!(expected, "Budget: 200k daily tokens, $5.50 daily cost");
    }

    #[test]
    fn settings_budget_line_without_budget() {
        let app = test_app(vec![], vec![]);
        assert!(app.token_budget.is_none());
    }

    // ── 3. Keybind view content: display_lines covers all configurable bindings ──
    #[test]
    fn keybind_display_lines_count_and_format() {
        let app = test_app(vec![], vec![]);
        let lines = app.view.keybinds.display_lines();
        assert_eq!(lines.len(), 17);
        for line in &lines {
            assert!(
                line.starts_with("  "),
                "line should be indented: {:?}",
                line
            );
            assert!(line.contains(": "), "line should contain ': ': {:?}", line);
        }
        assert!(lines[0].contains("move_up"));
        assert!(lines.last().unwrap().contains("quit"));
    }

    // ── 4. centered_rect produces correct areas ──
    #[test]
    fn centered_rect_percentages() {
        let area = Rect::new(0, 0, 100, 50);
        let r = centered_rect(50, 50, area);
        assert_eq!(r.width, 50);
        assert_eq!(r.height, 25);
        assert_eq!(r.x, 25);
        assert_eq!(r.y, 13);
    }

    #[test]
    fn centered_rect_100_percent_is_full_area() {
        let area = Rect::new(0, 0, 80, 24);
        let r = centered_rect(100, 100, area);
        assert_eq!(r, area);
    }

    #[test]
    fn centered_rect_clamped_at_minimum() {
        let area = Rect::new(0, 0, 80, 24);
        let r = centered_rect(0, 0, area);
        assert_eq!(r.width, 0);
        assert_eq!(r.height, 0);
    }

    // ── 5. Popup area calculations match expected sizes ──
    #[test]
    fn help_popup_area_48x24() {
        let area = Rect::new(0, 0, 80, 24);
        let popup = centered_rect(48, 24, area);
        assert_eq!(popup.width, 38);
        assert_eq!(popup.height, 6);
        assert_eq!(popup.x, 21);
        assert_eq!(popup.y, 9);
    }

    #[test]
    fn settings_popup_area_55x18() {
        let area = Rect::new(0, 0, 80, 24);
        let popup = centered_rect(55, 18, area);
        assert_eq!(popup.width, 44);
        assert_eq!(popup.height, 4);
    }

    #[test]
    fn keybind_popup_area_80x80() {
        let area = Rect::new(0, 0, 100, 50);
        let popup = centered_rect(80, 80, area);
        assert_eq!(popup.width, 80);
        assert_eq!(popup.height, 40);
        assert_eq!(popup.x, 10);
        assert_eq!(popup.y, 5);
    }

    // ── Render smoke tests using TestBackend ──
    #[test]
    fn render_help_popup_no_panic() {
        let app = test_app(vec![], vec![]);
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                app.render_help_popup(f, f.area());
            })
            .unwrap();
    }

    #[test]
    fn render_settings_popup_no_panic() {
        let app = test_app(vec![], vec![]);
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                app.render_settings_popup(f, f.area());
            })
            .unwrap();
    }

    #[test]
    fn render_keybind_view_no_panic() {
        let mut app = test_app(vec![], vec![]);
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                app.render_keybind_view(f, f.area());
            })
            .unwrap();
    }

    #[test]
    fn keybind_scroll_clamped_by_render() {
        let mut app = test_app(vec![], vec![]);
        app.popup.keybind_scroll = 9999;
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                app.render_keybind_view(f, f.area());
            })
            .unwrap();
        assert!(
            app.popup.keybind_scroll < 9999,
            "scroll should be clamped, got {}",
            app.popup.keybind_scroll,
        );
    }
}
