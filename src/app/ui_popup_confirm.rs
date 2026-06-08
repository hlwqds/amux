use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

// ansi_to_spans is an associated function on App (defined in super::ui), called as Self::*
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
                format!("  {msg}"),
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
                    Span::styled(format!("  {icon} "), Style::default().fg(color)),
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
                    lines.push(Line::from(format!("    ... and {remaining} more")));
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
                        Span::styled(format!("{count}"), Style::default().fg(Color::Yellow)),
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

    pub(super) fn render_plugin_list(&mut self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(50, 14, area);

        let items: Vec<ratatui::widgets::ListItem<'static>> = self
            .plugins
            .iter()
            .map(|p| {
                let key_label = p.key.map(|k| format!(" ({k})")).unwrap_or_default();
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
                    Self::ansi_to_spans(line)
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
}
