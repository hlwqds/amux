use serde::{Deserialize, Serialize};

/// Token budget configuration. All limits are optional — set whichever you want.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenBudget {
    /// Maximum tokens allowed per day (cumulative across all sessions).
    #[serde(default)]
    pub daily_tokens: Option<u64>,
    /// Maximum tokens allowed per week (cumulative across all sessions).
    #[serde(default)]
    pub weekly_tokens: Option<u64>,
    /// Maximum cost in USD allowed per day.
    #[serde(default)]
    pub daily_cost: Option<f64>,
    /// Maximum cost in USD allowed per week.
    #[serde(default)]
    pub weekly_cost: Option<f64>,
}

/// Result of a budget check.
#[derive(Clone, Debug)]
pub struct BudgetAlert {
    /// Which limit was exceeded.
    pub limit_kind: BudgetLimitKind,
    /// Human-readable alert message.
    pub message: String,
    /// Current cumulative usage value.
    pub current: f64,
    /// The budget limit that was exceeded.
    pub limit: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetLimitKind {
    DailyTokens,
    WeeklyTokens,
    DailyCost,
    WeeklyCost,
}

/// Usage aggregated across sessions for a time window.
#[derive(Default)]
struct WindowUsage {
    tokens: u64,
    cost: f64,
}

/// Check token usage from sessions against the budget.
///
/// `extract_fn` receives each session's JSONL path and returns its token usage
/// if available. Returns the *most severe* alert (first exceeded limit found).
pub fn check_budget(
    sessions: &[crate::types::Session],
    budget: &TokenBudget,
    now_secs: u64,
    extract_fn: impl Fn(&crate::types::Session) -> Option<crate::discovery::TokenUsage>,
) -> Option<BudgetAlert> {
    // Time boundaries
    let day_start = now_secs - (now_secs % 86400); // start of today (UTC)
    let week_start =
        day_start.saturating_sub((now_secs_of_weekday(now_secs).saturating_sub(1)) * 86400);

    let mut daily = WindowUsage::default();
    let mut weekly = WindowUsage::default();

    for session in sessions {
        let usage = match extract_fn(session) {
            Some(u) => u,
            None => continue,
        };

        if session.last_active >= day_start {
            daily.tokens += usage.total_tokens;
            daily.cost += usage.cost;
        }
        if session.last_active >= week_start {
            weekly.tokens += usage.total_tokens;
            weekly.cost += usage.cost;
        }
    }

    // Check limits in priority order: daily tokens, daily cost, weekly tokens, weekly cost
    if let Some(limit) = budget.daily_tokens
        && daily.tokens > limit
    {
        return Some(BudgetAlert {
            limit_kind: BudgetLimitKind::DailyTokens,
            message: format!(
                "⚠ DAILY TOKEN BUDGET EXCEEDED: {} / {} tokens",
                daily.tokens, limit
            ),
            current: daily.tokens as f64,
            limit: limit as f64,
        });
    }
    if let Some(limit) = budget.daily_cost
        && daily.cost > limit
    {
        return Some(BudgetAlert {
            limit_kind: BudgetLimitKind::DailyCost,
            message: format!(
                "⚠ DAILY COST BUDGET EXCEEDED: ${:.2} / ${:.2}",
                daily.cost, limit
            ),
            current: daily.cost,
            limit,
        });
    }
    if let Some(limit) = budget.weekly_tokens
        && weekly.tokens > limit
    {
        return Some(BudgetAlert {
            limit_kind: BudgetLimitKind::WeeklyTokens,
            message: format!(
                "⚠ WEEKLY TOKEN BUDGET EXCEEDED: {} / {} tokens",
                weekly.tokens, limit
            ),
            current: weekly.tokens as f64,
            limit: limit as f64,
        });
    }
    if let Some(limit) = budget.weekly_cost
        && weekly.cost > limit
    {
        return Some(BudgetAlert {
            limit_kind: BudgetLimitKind::WeeklyCost,
            message: format!(
                "⚠ WEEKLY COST BUDGET EXCEEDED: ${:.2} / ${:.2}",
                weekly.cost, limit
            ),
            current: weekly.cost,
            limit,
        });
    }

    None
}

/// Returns what day-of-week (1=Mon, 7=Sun) the timestamp falls on in UTC.
fn now_secs_of_weekday(secs: u64) -> u64 {
    // 1970-01-01 was a Thursday (day 4). We want Monday=1.
    ((secs / 86400 + 3) % 7) + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Agent, Session};
    use std::path::PathBuf;

    fn mock_session(id: &str, last_active: u64) -> Session {
        Session {
            id: id.to_string(),
            workspace_path: PathBuf::from("/tmp/ws"),
            title: id.to_string(),
            last_active,
            agent: Agent::Claude,
            tags: vec![],
        }
    }

    fn mock_usage(total_tokens: u64, cost: f64) -> crate::discovery::TokenUsage {
        crate::discovery::TokenUsage {
            input_tokens: total_tokens / 2,
            output_tokens: total_tokens / 2,
            total_tokens,
            cost,
        }
    }

    #[test]
    fn test_no_budget_no_alert() {
        let sessions = vec![mock_session("s1", 1000)];
        let budget = TokenBudget {
            daily_tokens: None,
            weekly_tokens: None,
            daily_cost: None,
            weekly_cost: None,
        };
        assert!(check_budget(&sessions, &budget, 1000, |_| Some(mock_usage(1000, 1.0))).is_none());
    }

    #[test]
    fn test_daily_token_budget_exceeded() {
        let now: u64 = 86400 * 10; // Day 10, 00:00 UTC
        let sessions = vec![mock_session("s1", now + 100)];
        let budget = TokenBudget {
            daily_tokens: Some(500),
            weekly_tokens: None,
            daily_cost: None,
            weekly_cost: None,
        };
        let alert = check_budget(&sessions, &budget, now + 200, |_| {
            Some(mock_usage(600, 0.0))
        });
        assert!(alert.is_some());
        let alert = alert.unwrap();
        assert_eq!(alert.limit_kind, BudgetLimitKind::DailyTokens);
        assert!(alert.message.contains("DAILY TOKEN BUDGET EXCEEDED"));
    }

    #[test]
    fn test_daily_cost_budget_exceeded() {
        let now: u64 = 86400 * 10;
        let sessions = vec![mock_session("s1", now + 100)];
        let budget = TokenBudget {
            daily_tokens: None,
            weekly_tokens: None,
            daily_cost: Some(5.0),
            weekly_cost: None,
        };
        let alert = check_budget(&sessions, &budget, now + 200, |_| {
            Some(mock_usage(100, 10.0))
        });
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().limit_kind, BudgetLimitKind::DailyCost);
    }

    #[test]
    fn test_within_budget_no_alert() {
        let now: u64 = 86400 * 10;
        let sessions = vec![mock_session("s1", now + 100)];
        let budget = TokenBudget {
            daily_tokens: Some(1000),
            weekly_tokens: None,
            daily_cost: None,
            weekly_cost: None,
        };
        assert!(
            check_budget(&sessions, &budget, now + 200, |_| Some(mock_usage(
                500, 0.0
            )))
            .is_none()
        );
    }

    #[test]
    fn test_old_session_not_counted_for_daily() {
        let now: u64 = 86400 * 10;
        // Session from yesterday
        let sessions = vec![mock_session("s1", now - 100)];
        let budget = TokenBudget {
            daily_tokens: Some(100),
            weekly_tokens: None,
            daily_cost: None,
            weekly_cost: None,
        };
        assert!(
            check_budget(&sessions, &budget, now + 100, |_| Some(mock_usage(
                500, 0.0
            )))
            .is_none()
        );
    }

    #[test]
    fn test_weekly_token_budget_exceeded() {
        // Day 10, at end of day — sessions from earlier this week
        let now: u64 = 86400 * 10 + 100;
        let sessions = vec![mock_session("s1", 86400 * 8)];
        let budget = TokenBudget {
            daily_tokens: None,
            weekly_tokens: Some(100),
            daily_cost: None,
            weekly_cost: None,
        };
        let alert = check_budget(&sessions, &budget, now, |_| Some(mock_usage(200, 0.0)));
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().limit_kind, BudgetLimitKind::WeeklyTokens);
    }

    #[test]
    fn test_cumulative_daily_usage() {
        let now: u64 = 86400 * 10;
        let sessions = vec![mock_session("s1", now + 100), mock_session("s2", now + 200)];
        let budget = TokenBudget {
            daily_tokens: Some(150),
            weekly_tokens: None,
            daily_cost: None,
            weekly_cost: None,
        };
        // Each session uses 100 tokens -> 200 total, exceeds 150
        let alert = check_budget(&sessions, &budget, now + 300, |_| {
            Some(mock_usage(100, 0.0))
        });
        assert!(alert.is_some());
    }
}
