use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use super::*;

impl App {
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
        let title = format!(" {ws_name} \u{2192} Select Directory ");

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
                        format!(" {agent_label} "),
                        Style::default().fg(t.agent.color()),
                    ),
                    Span::styled(t.name.clone(), Style::default().fg(Color::White)),
                    Span::styled(format!("  ws: {ws}"), Style::default().fg(Color::DarkGray)),
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
                    Line::from(format!("    {steps_str}"))
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
}
