//! Session statistics: daily aggregation and chart rendering.

use std::collections::BTreeMap;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph, Wrap},
};

use crate::discovery::{extract_token_usage, find_session_jsonl};
use crate::types::Session;

// ── Data types ──────────────────────────────────────────────

/// One day's aggregated stats.
#[derive(Clone, Debug, Default)]
pub struct DailyStats {
    pub sessions: usize,
    pub total_tokens: u64,
    pub total_cost: f64,
}

/// All stats keyed by date string "YYYY-MM-DD".
pub type DailyStatsMap = BTreeMap<String, DailyStats>;

// ── Aggregation ─────────────────────────────────────────────

/// Aggregate sessions into daily stats, extracting token usage from JSONL.
pub fn aggregate_daily(sessions: &[Session]) -> DailyStatsMap {
    let mut map: DailyStatsMap = BTreeMap::new();

    for session in sessions {
        let date = epoch_to_date(session.last_active);
        let entry = map.entry(date).or_default();
        entry.sessions += 1;
        if let Some(jsonl) = find_session_jsonl(session)
            && let Some(usage) = extract_token_usage(&jsonl)
        {
            entry.total_tokens += usage.total_tokens;
            entry.total_cost += usage.cost;
        }
    }

    map
}

// ── Rendering ───────────────────────────────────────────────

/// Render a pass-rate line chart for the last N days (placeholder until
/// check_status is tracked per session; shows session count instead).
pub fn render_session_count_chart(frame: &mut Frame, area: Rect, stats: &DailyStatsMap) {
    let data = last_n_days_data(stats, 30, |d| d.sessions as f64);

    let datasets = vec![Dataset::default()
        .name("Sessions")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(Color::Green))
        .data(&data)];

    let max_val = data.iter().map(|p| p.1).fold(1.0_f64, f64::max);

    let chart = Chart::new(datasets)
        .block(Block::bordered().title("Sessions (30 days)"))
        .x_axis(
            Axis::default()
                .title("Day")
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, data.len() as f64]),
        )
        .y_axis(
            Axis::default()
                .title("Count")
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, max_val * 1.1])
                .labels(vec![
                    "0".into(),
                    format!("{:.0}", max_val / 2.0),
                    format!("{:.0}", max_val),
                ]),
        );
    frame.render_widget(chart, area);
}

/// Render a token usage bar chart for the last N days.
pub fn render_token_chart(frame: &mut Frame, area: Rect, stats: &DailyStatsMap) {
    let data = last_n_days_data(stats, 30, |d| d.total_tokens as f64 / 1000.0);

    let datasets = vec![Dataset::default()
        .name("Tokens (K)")
        .marker(symbols::Marker::HalfBlock)
        .graph_type(GraphType::Bar)
        .style(Style::default().fg(Color::Cyan))
        .data(&data)];

    let max_val = data.iter().map(|p| p.1).fold(1.0_f64, f64::max);

    let chart = Chart::new(datasets)
        .block(Block::bordered().title("Token Usage (30 days, K)"))
        .x_axis(
            Axis::default()
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, data.len() as f64]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Color::Gray))
                .bounds([0.0, max_val * 1.1])
                .labels(vec![
                    "0".into(),
                    format!("{:.0}K", max_val / 2.0),
                    format!("{:.0}K", max_val),
                ]));

    frame.render_widget(chart, area);
}

/// Render a session dashboard summary.
pub fn render_dashboard(frame: &mut Frame, area: Rect, stats: &DailyStatsMap) {
    let total: DailyStats = stats.values().fold(DailyStats::default(), |acc, d| DailyStats {
        sessions: acc.sessions + d.sessions,
        total_tokens: acc.total_tokens + d.total_tokens,
        total_cost: acc.total_cost + d.total_cost,
    });

    let lines = vec![
        Line::from(vec![
            Span::styled(
                "Total Sessions: ",
                Style::default().add_modifier(ratatui::style::Modifier::BOLD),
            ),
            Span::raw(format!("{}", total.sessions)),
        ]),
        Line::from(vec![
            Span::styled(
                "Total Tokens: ",
                Style::default().add_modifier(ratatui::style::Modifier::BOLD),
            ),
            Span::raw(format_tokens(total.total_tokens)),
        ]),
        Line::from(vec![
            Span::styled(
                "Total Cost: ",
                Style::default().add_modifier(ratatui::style::Modifier::BOLD),
            ),
            Span::raw(format!("${:.2}", total.total_cost)),
        ]),
        Line::from(vec![
            Span::styled(
                "Days Tracked: ",
                Style::default().add_modifier(ratatui::style::Modifier::BOLD),
            ),
            Span::raw(format!("{}", stats.len())),
        ]),
    ];
    let para = Paragraph::new(lines)
        .block(Block::bordered().title("Session Dashboard"))
        .wrap(Wrap { trim: true });

    frame.render_widget(para, area);
}

// ── Helpers ─────────────────────────────────────────────────

fn last_n_days_data(
    stats: &DailyStatsMap,
    n: usize,
    extract: impl Fn(&DailyStats) -> f64,
) -> Vec<(f64, f64)> {
    let entries: Vec<_> = stats.iter().rev().take(n).collect();
    let mut data = Vec::with_capacity(entries.len());
    for (i, (_, daily)) in entries.iter().enumerate() {
        data.push((i as f64, extract(daily)));
    }
    data.reverse();
    data
}

fn epoch_to_date(secs: u64) -> String {
    let days_since_epoch = secs / 86400;
    let mut year = 1970u64;
    let mut remaining = days_since_epoch;

    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }

    let month_days = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for &md in &month_days {
        if remaining < md {
            break;
        }
        remaining -= md;
        month += 1;
    }

    let day = remaining + 1;
    format!("{year:04}-{month:02}-{day:02}")
}

fn is_leap(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        format!("{tokens}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_to_date() {
        assert_eq!(epoch_to_date(0), "1970-01-01");
        assert_eq!(epoch_to_date(86400), "1970-01-02");
        assert_eq!(epoch_to_date(1704067200), "2024-01-01");
    }

    #[test]
    fn test_is_leap() {
        assert!(is_leap(2024));
        assert!(!is_leap(2023));
        assert!(!is_leap(1900));
        assert!(is_leap(2000));
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5K");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }
}
