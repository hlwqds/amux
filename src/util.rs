use std::{
    env, io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
};

use crate::types::Agent;

pub const SELECT_CURRENT: &str = "\u{2713} Select this directory";
pub const SELECT_VIRTUAL: &str = "\u{25cb} Virtual (no directory)";
pub const PARENT_DIR: &str = "..";

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn relative_time(secs: u64) -> String {
    let diff = now_secs().saturating_sub(secs);
    match diff {
        0..=60 => "just now".into(),
        61..=3600 => format!("{}m ago", diff / 60),
        3601..=86400 => format!("{}h ago", diff / 3600),
        _ => format!("{}d ago", diff / 86400),
    }
}

pub fn which(cmd: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).find_map(|dir| {
            let full = dir.join(cmd);
            full.is_file().then_some(full)
        })
    })
}

pub fn detect_agents() -> Vec<Agent> {
    Agent::ALL
        .iter()
        .filter(|a| which(a.cmd()).is_some())
        .copied()
        .collect()
}

pub fn key_to_bytes(key: &KeyEvent) -> Vec<u8> {
    match key.code {
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Delete => vec![27, 91, 51, 126],
        KeyCode::Up => vec![27, 91, 65],
        KeyCode::Down => vec![27, 91, 66],
        KeyCode::Right => vec![27, 91, 67],
        KeyCode::Left => vec![27, 91, 68],
        KeyCode::Home => vec![27, 91, 72],
        KeyCode::End => vec![27, 91, 70],
        KeyCode::PageUp => vec![27, 91, 53, 126],
        KeyCode::PageDown => vec![27, 91, 54, 126],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Esc => vec![27],
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let code = c.to_ascii_lowercase();
                if code.is_ascii_lowercase() {
                    vec![(code as u8) - b'a' + 1]
                } else {
                    c.to_string().into_bytes()
                }
            } else {
                c.to_string().into_bytes()
            }
        }
        KeyCode::F(n) => match n {
            1 => vec![27, 79, 80],
            2 => vec![27, 79, 81],
            3 => vec![27, 79, 82],
            4 => vec![27, 79, 83],
            5 => vec![27, 91, 49, 53, 126],
            6 => vec![27, 91, 49, 55, 126],
            7 => vec![27, 91, 49, 56, 126],
            8 => vec![27, 91, 49, 57, 126],
            9 => vec![27, 91, 50, 48, 126],
            10 => vec![27, 91, 50, 49, 126],
            11 => vec![27, 91, 50, 51, 126],
            12 => vec![27, 91, 50, 52, 126],
            _ => vec![],
        },
        _ => vec![],
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

pub fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    Terminal::new(CrosstermBackend::new(stdout)).context("failed to initialize terminal")
}

pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Parsed search query: optional date filter + text query.
pub struct ParsedSearch {
    pub text: Option<String>,
    pub min_last_active: Option<u64>, // unix timestamp, sessions older than this are excluded
}

/// Parse a search query that may contain a date prefix like `>7d`, `>1h`, `>30m`.
/// Returns the remaining text query and the minimum last_active timestamp.
/// Examples: `>7d fix bug` → text="fix bug", min_last_active=7 days ago
///           `fix bug` → text="fix bug", min_last_active=None
///           `>1h` → text=None, min_last_active=1 hour ago
pub fn parse_search_query(query: &str) -> ParsedSearch {
    let now = now_secs();
    let trimmed = query.trim();

    // Try to match date prefix: >Nd, >Nh, >Nm
    let re = date_regex();
    if let Some(caps) = re.captures(trimmed) {
        let full_match = caps.get(0).unwrap().as_str();
        let amount: u64 = caps.get(1).unwrap().as_str().parse().unwrap_or(1);
        let unit = caps.get(2).unwrap().as_str();

        let cutoff = match unit {
            "d" => now.saturating_sub(amount * 86400),
            "h" => now.saturating_sub(amount * 3600),
            "m" => now.saturating_sub(amount * 60),
            _ => now,
        };

        let remaining = trimmed[full_match.len()..].trim().to_string();
        ParsedSearch {
            text: if remaining.is_empty() {
                None
            } else {
                Some(remaining)
            },
            min_last_active: Some(cutoff),
        }
    } else {
        ParsedSearch {
            text: if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            },
            min_last_active: None,
        }
    }
}

fn date_regex() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^>(\d+)([dhm])\s*").unwrap())
}

/// Copy text to the system clipboard. Uses arboard first, falls back to
/// platform-specific CLI tools (pbcopy on macOS, xclip/wl-copy on Linux).
pub fn clipboard_copy(text: &str) -> Result<(), String> {
    // Try arboard first
    if let Ok(mut cb) = arboard::Clipboard::new()
        && cb.set_text(text).is_ok()
    {
        return Ok(());
    }

    // Fallback: try pbcopy (macOS)
    if std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .ok()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .map(|stdin| stdin.write_all(text.as_bytes()))
                .transpose()
                .ok()
        })
        .is_some()
    {
        return Ok(());
    }

    // Fallback: try xclip (X11)
    if std::process::Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .ok()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .map(|stdin| stdin.write_all(text.as_bytes()))
                .transpose()
                .ok()
        })
        .is_some()
    {
        return Ok(());
    }

    // Fallback: try wl-copy (Wayland)
    if std::process::Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .ok()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .map(|stdin| stdin.write_all(text.as_bytes()))
                .transpose()
                .ok()
        })
        .is_some()
    {
        return Ok(());
    }

    Err("No clipboard available (tried arboard, pbcopy, xclip, wl-copy)".into())
}

/// Extract file paths from terminal output text.
/// Matches common patterns: `/path/to/file.ext`, `./relative/path.ext`, `src/file.rs:line`.
/// Deduplicates and returns at most `max` paths.
pub fn extract_file_paths(text: &str, max: usize) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Match absolute paths and ./ relative paths with file extensions
    // Also match patterns like `file.rs:42` (file:line)
    let re = regex_lazy();

    for cap in re.find_iter(text) {
        let path = cap.as_str();
        // Strip trailing :line:col or :line suffix for display
        let cleaned = path.trim_end_matches(|c: char| c.is_ascii_digit() || c == ':');
        let cleaned = cleaned.trim_end_matches(':');
        if seen.insert(cleaned.to_string()) {
            paths.push(cleaned.to_string());
            if paths.len() >= max {
                break;
            }
        }
    }

    paths
}

/// Extract file paths from terminal output text, preserving optional line numbers.
/// Returns tuples of (path, optional_line_number).
/// Matches `src/main.rs:42` → ("src/main.rs", Some(42)),
///         `src/main.rs`     → ("src/main.rs", None).
/// Deduplicates by path and returns at most `max` results.
pub fn extract_file_paths_with_lines(text: &str, max: usize) -> Vec<(String, Option<u32>)> {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        // Same as regex_lazy but captures optional :line suffix
        regex::Regex::new(
            r"(?:/[\w./\-]+/[\w.\-]+\.[\w\-]+|\.?(?:src|lib|test|pkg|cmd|internal|crates|apps)/[\w./\-]+\.[\w\-]+)(?::(\d+))?"
        ).unwrap()
    });

    let mut paths: Vec<(String, Option<u32>)> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for cap in re.captures_iter(text) {
        let full = cap.get(0).unwrap().as_str();
        let line_num: Option<u32> = cap.get(1).and_then(|m| m.as_str().parse().ok());
        // Strip trailing :line for the path itself
        let path = if line_num.is_some() {
            &full[..full.rfind(':').unwrap_or(full.len())]
        } else {
            full
        };
        if seen.insert(path.to_string()) {
            paths.push((path.to_string(), line_num));
            if paths.len() >= max {
                break;
            }
        }
    }

    paths
}

fn regex_lazy() -> &'static regex::Regex {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Match absolute paths and ./ relative paths
        // Require at least one path separator and a file extension
        regex::Regex::new(
            r"(?:/[\w./\-]+/[\w.\-]+\.[\w\-]+|\.?(?:src|lib|test|pkg|cmd|internal|crates|apps)/[\w./\-]+\.[\w\-]+)"
        ).unwrap()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_time_formatting() {
        let now = now_secs();
        assert_eq!(relative_time(now), "just now");
        assert_eq!(relative_time(now - 30), "just now");
        assert_eq!(relative_time(now - 120), "2m ago");
        assert_eq!(relative_time(now - 7200), "2h ago");
        assert_eq!(relative_time(now - 172800), "2d ago");
    }

    #[test]
    fn extract_paths_absolute() {
        let text = "Modified /home/user/project/src/main.rs:42";
        let paths = extract_file_paths(text, 5);
        assert_eq!(paths, vec!["/home/user/project/src/main.rs"]);
    }

    #[test]
    fn extract_paths_relative() {
        let text = "Editing src/app/ui.rs and lib/core.py";
        let paths = extract_file_paths(text, 5);
        assert!(paths.contains(&"src/app/ui.rs".to_string()));
        assert!(paths.contains(&"lib/core.py".to_string()));
    }

    #[test]
    fn extract_paths_dedup() {
        let text = "src/main.rs src/main.rs src/main.rs";
        let paths = extract_file_paths(text, 5);
        assert_eq!(paths.len(), 1);
    }

    #[test]
    fn extract_paths_max_limit() {
        let text = "src/a.rs src/b.rs src/c.rs src/d.rs src/e.rs src/f.rs";
        let paths = extract_file_paths(text, 3);
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn extract_paths_no_match() {
        let text = "hello world no paths here";
        let paths = extract_file_paths(text, 5);
        assert!(paths.is_empty());
    }

    #[test]
    fn search_date_filter_days() {
        let parsed = parse_search_query(">7d fix bug");
        assert_eq!(parsed.text.as_deref(), Some("fix bug"));
        assert!(parsed.min_last_active.is_some());
    }

    #[test]
    fn search_date_filter_hours() {
        let parsed = parse_search_query(">1h");
        assert!(parsed.text.is_none());
        assert!(parsed.min_last_active.is_some());
    }

    #[test]
    fn search_date_filter_minutes() {
        let parsed = parse_search_query(">30m deploy");
        assert_eq!(parsed.text.as_deref(), Some("deploy"));
        assert!(parsed.min_last_active.is_some());
    }

    #[test]
    fn search_no_date_filter() {
        let parsed = parse_search_query("fix bug");
        assert_eq!(parsed.text.as_deref(), Some("fix bug"));
        assert!(parsed.min_last_active.is_none());
    }

    #[test]
    fn search_empty() {
        let parsed = parse_search_query("");
        assert!(parsed.text.is_none());
        assert!(parsed.min_last_active.is_none());
    }
}
