use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

// ansi_to_spans and truncate_str are associated functions on App (defined in super::ui), called as Self::*
use super::*;

impl App {
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
            lines.push(Line::from(format!("  {key:<14} {action}")));
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
                Span::styled(format!(" {marker} "), Style::default().fg(Color::Yellow)),
                Span::styled(ws.name.clone(), Style::default().fg(Color::White)),
                Span::styled(
                    format!("  {path_str}"),
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
        let budget_status = self.token_budget.as_ref().map_or_else(
            || "Budget: not set (b to set default 100k daily)".into(),
            |b| {
                let mut parts = Vec::new();
                if let Some(dt) = b.daily_tokens {
                    parts.push(format!("{}k daily tokens", dt / 1000));
                }
                if let Some(dc) = b.daily_cost {
                    parts.push(format!("${dc:.2} daily cost"));
                }
                if let Some(wt) = b.weekly_tokens {
                    parts.push(format!("{}k weekly tokens", wt / 1000));
                }
                if let Some(wc) = b.weekly_cost {
                    parts.push(format!("${wc:.2} weekly cost"));
                }
                format!("Budget: {} (b to clear)", parts.join(", "))
            },
        );
        lines.push(Line::from(vec![Span::styled(
            format!("  {budget_status} "),
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
            lines.push(Line::from(format!("  {line}")));
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
                lines.push(Line::from(format!("    {a} <-> {b}")));
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
                format!("{total_all}"),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   Active: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{active_all}"),
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
                    format!(" {label:<16} "),
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
                format!("{n}")
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
                format!(" Total Cost: ${total_cost:.4}"),
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
        agents.sort_by_key(|b| std::cmp::Reverse(b.1 .2));
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
                    format!("  ${cost:.3}"),
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
                    Span::styled(format!(" {agent} "), Style::default().fg(color)),
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
                    Span::styled(format!(" {time} "), Style::default().fg(Color::DarkGray)),
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
                            format!("  {text}"),
                            Style::default().fg(Color::DarkGray),
                        )]));
                    } else if let Some(stripped) = text.strip_prefix("# ") {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {stripped}"),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        )]));
                        lines.push(Line::from(""));
                    } else if let Some(stripped) = text.strip_prefix("## ") {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {stripped}"),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )]));
                        lines.push(Line::from(""));
                    } else if let Some(stripped) = text.strip_prefix("### ") {
                        lines.push(Line::from(vec![Span::styled(
                            format!("  {stripped}"),
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
                        lines.push(Line::from(format!("  {text}")));
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
                            format!("  {label} "),
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
                Span::styled(format!(" {medal} "), Style::default()),
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
                    Span::styled(Self::truncate_str(m, 70), Style::default().fg(Color::DarkGray)),
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

    pub(super) fn render_semantic_search(&self, frame: &mut Frame, area: Rect) {
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
                    Span::styled(format!("{short_id} "), Style::default().fg(Color::DarkGray)),
                    Span::styled(Self::truncate_str(title, 45), style),
                    Span::styled(format!("  {pct}%"), Style::default().fg(Color::DarkGray)),
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
}
