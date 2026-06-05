use std::{fs, path::Path};

use crate::config::load_session_meta;
use crate::discovery::find_session_jsonl;
use crate::types::ClaudeRecord;
use crate::util::now_secs;

/// Parse GSD JSONL v3 session. First line: `{"type":"session","version":3,"id":"...","timestamp":"...","cwd":"..."}`
/// Title: prefer `custom_message` with `customType:"gsd-run"`, fallback to `message` with `role:"user"`.
pub fn parse_gsd_session(path: &Path) -> Option<(String, Option<String>, Option<String>)> {
    let content = fs::read_to_string(path).ok()?;
    let mut id = String::new();
    let mut cwd: Option<String> = None;
    let mut title: Option<String> = None;

    for line in content.lines() {
        let record: serde_json::Value = serde_json::from_str(line).ok()?;
        let r#type = record.get("type")?.as_str()?;

        match r#type {
            "session" => {
                id = record.get("id")?.as_str()?.to_string();
                cwd = record
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
            "custom_message" if title.is_none() => {
                if record.get("customType").and_then(|v| v.as_str()) == Some("gsd-run")
                    && let Some(t) = record
                        .get("message")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                {
                    let truncated: String = t.chars().take(50).collect();
                    title = Some(truncated);
                }
            }
            "message"
                if title.is_none()
                    && record.get("role").and_then(|v| v.as_str()) == Some("user") =>
            {
                let text = record
                    .get("message")
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .or_else(|| {
                        record
                            .get("message")
                            .and_then(|v| extract_text_from_content(v.clone()))
                    });
                if let Some(t) = text {
                    let truncated: String = t.chars().take(50).collect();
                    title = Some(truncated);
                }
            }
            _ => {}
        }

        // Early exit once we have everything
        if !id.is_empty() && title.is_some() {
            break;
        }
    }

    if id.is_empty() {
        return None;
    }
    Some((id, title, cwd))
}

pub(crate) fn extract_claude_title(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    for line in std::io::BufRead::lines(reader) {
        let line = line.ok()?;
        let record: ClaudeRecord = serde_json::from_str(&line).ok()?;
        if record.record_type.as_deref() != Some("user") {
            continue;
        }
        let msg = record.message?;
        if msg.role.as_deref() != Some("user") {
            continue;
        }
        let text = extract_text_from_content(msg.content?)?;
        let cleaned = clean_user_message(&text);
        if !cleaned.is_empty() {
            return Some(cleaned.chars().take(50).collect());
        }
    }
    None
}

pub fn parse_codex_session(path: &Path) -> Option<(String, Option<String>, String)> {
    let content = fs::read_to_string(path).ok()?;
    let mut id = String::new();
    let mut cwd = String::new();
    let mut first_user_msg: Option<String> = None;

    for line in content.lines() {
        let record: serde_json::Value = serde_json::from_str(line).ok()?;
        let r#type = record.get("type")?.as_str()?;

        match r#type {
            "session_meta" => {
                let p = record.get("payload")?;
                id = p.get("id")?.as_str()?.to_string();
                cwd = p
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            "user_message" if first_user_msg.is_none() => {
                let text = record.get("payload")?.get("text")?.as_str()?;
                let truncated: String = text.chars().take(50).collect();
                first_user_msg = Some(truncated);
            }
            _ => {}
        }

        if !id.is_empty() && first_user_msg.is_some() {
            break;
        }
    }

    if id.is_empty() {
        return None;
    }
    Some((id, first_user_msg, cwd))
}

pub fn clean_user_message(text: &str) -> String {
    let mut cleaned = text.to_string();

    if let Some(start) = cleaned.find("P>|")
        && let Some(end) = cleaned[start..].find('\\')
    {
        cleaned = format!("{}{}", &cleaned[..start], &cleaned[start + end + 1..]);
    }

    let noise_prefixes = ["\x1b", "P>|", "P<|"];
    for prefix in noise_prefixes {
        if cleaned.starts_with(prefix) {
            return String::new();
        }
    }

    cleaned.trim().to_string()
}

pub fn extract_text_from_content(content: serde_json::Value) -> Option<String> {
    match content {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Array(arr) => {
            let mut texts = Vec::new();
            for item in arr {
                if item.get("type").and_then(|v| v.as_str()) == Some("text")
                    && let Some(t) = item.get("text").and_then(|v| v.as_str())
                {
                    texts.push(t.to_string());
                }
            }
            if texts.is_empty() {
                None
            } else {
                Some(texts.join(" "))
            }
        }
        _ => None,
    }
}

/// Extract the last user message from a session JSONL file.
/// Returns the message text truncated to ~100 chars, or None if no user message found.
pub(crate) fn extract_last_user_message(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines().rev() {
        let record: serde_json::Value = serde_json::from_str(line).ok()?;
        let r#type = record.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let text = if r#type == "user" {
            extract_claude_message_text(&record)
        } else if r#type == "message" {
            if record
                .get("message")
                .and_then(|msg| msg.get("role"))
                .and_then(|v| v.as_str())
                .is_some_and(|r| r == "user")
            {
                extract_claude_message_text(&record)
            } else {
                String::new()
            }
        } else if r#type == "user_message" {
            record
                .get("payload")
                .and_then(|p| p.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        } else {
            continue;
        };
        if !text.is_empty() {
            let truncated: String = text.chars().take(100).collect();
            let suffix = if text.chars().count() > 100 {
                "..."
            } else {
                ""
            };
            return Some(format!("{truncated}{suffix}"));
        }
    }
    None
}

/// Preview entry: a role + truncated text.
#[derive(Clone, Debug)]
pub struct PreviewLine {
    pub role: String, // "user" or "assistant"
    pub text: String,
}

/// Read the last few user/assistant exchanges from a session JSONL file.
/// Returns up to `max_pairs` conversation pairs (user + assistant), newest first.
pub fn preview_session_content(path: &Path, max_pairs: usize) -> Option<Vec<PreviewLine>> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut messages: Vec<PreviewLine> = Vec::new();

    for line in content.lines().rev() {
        if messages.len() >= max_pairs * 2 {
            break;
        }
        let record: serde_json::Value = serde_json::from_str(line).ok()?;

        // Claude/Codex JSONL: {"type":"user","message":{"content":"..."}}
        // or {"type":"assistant","message":{"content":[{type:"text",text:"..."}]}}
        // GSD/OMP JSONL v3: {"type":"message","message":{"role":"user","content":"..."}}
        let r#type = record.get("type").and_then(|v| v.as_str()).unwrap_or("");

        let (role, text) = if r#type == "user" {
            ("user".to_string(), extract_claude_message_text(&record))
        } else if r#type == "assistant" {
            (
                "assistant".to_string(),
                extract_claude_message_text(&record),
            )
        } else if r#type == "message" {
            // GSD/OMP v3 format
            let msg = record.get("message")?;
            let role = msg
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = msg.get("content");
            let text = content
                .and_then(|c| extract_text_from_content(c.clone()))
                .unwrap_or_default();
            (role, text)
        } else if r#type == "user_message" {
            // Codex format
            let text = record
                .get("payload")
                .and_then(|p| p.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ("user".to_string(), text)
        } else {
            continue;
        };

        if !text.is_empty() {
            // Truncate to ~120 chars
            let truncated: String = text.chars().take(120).collect();
            let suffix = if text.chars().count() > 120 {
                "..."
            } else {
                ""
            };
            messages.push(PreviewLine {
                role,
                text: format!("{truncated}{suffix}"),
            });
        }
    }

    // Reverse back to chronological order
    messages.reverse();
    Some(messages)
}

fn extract_claude_message_text(record: &serde_json::Value) -> String {
    record
        .get("message")
        .and_then(|msg| msg.get("content"))
        .and_then(|content| extract_text_from_content(content.clone()))
        .unwrap_or_default()
}

/// Export a session JSONL file to markdown format.
/// Reads all user/assistant exchanges and writes them as markdown.
/// Returns the path to the exported file.
pub fn export_session_to_markdown(
    jsonl_path: &Path,
    session_title: &str,
    output_dir: &Path,
) -> std::io::Result<std::path::PathBuf> {
    let content = std::fs::read_to_string(jsonl_path)?;

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("# {session_title}"));
    lines.push(String::new());
    lines.push(format!("Exported: {}", chrono_now_or_fallback()));
    lines.push(String::new());
    lines.push("---".to_string());
    lines.push(String::new());

    for line in content.lines() {
        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let r#type = record.get("type").and_then(|v| v.as_str()).unwrap_or("");

        let (role, text) = if r#type == "user" {
            ("User".to_string(), extract_claude_message_text(&record))
        } else if r#type == "assistant" {
            (
                "Assistant".to_string(),
                extract_claude_message_text(&record),
            )
        } else if r#type == "message" {
            let msg = match record.get("message") {
                Some(m) => m,
                None => continue,
            };
            let role = msg
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = msg.get("content");
            let text = content
                .and_then(|c| extract_text_from_content(c.clone()))
                .unwrap_or_default();
            (role, text)
        } else if r#type == "user_message" {
            let text = record
                .get("payload")
                .and_then(|p| p.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ("user".to_string(), text)
        } else {
            continue;
        };

        if text.is_empty() {
            continue;
        }

        match role.as_str() {
            "user" | "User" => {
                lines.push("## User".to_string());
                lines.push(String::new());
                lines.push(text);
                lines.push(String::new());
            }
            "assistant" | "Assistant" => {
                lines.push("## Assistant".to_string());
                lines.push(String::new());
                lines.push(text);
                lines.push(String::new());
            }
            _ => {}
        }
    }

    // Write to output directory
    std::fs::create_dir_all(output_dir)?;
    let filename = jsonl_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("session");
    let export_path = output_dir.join(format!("{filename}.md"));
    std::fs::write(&export_path, lines.join("\n"))?;
    Ok(export_path)
}

fn chrono_now_or_fallback() -> String {
    // Simple timestamp without chrono dependency
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Rough ISO format from unix timestamp
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    // Years since 1970 approximation (not accounting for leap years precisely)
    let year = 1970 + u32::try_from(days / 365).unwrap_or(u32::MAX);
    let day_of_year = days % 365;
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;
    format!("{year}-{month:02}-{day:02} {hours:02}:{minutes:02}")
}

/// A single conversation turn for branch point selection.
#[derive(Clone, Debug)]
pub struct BranchPoint {
    /// 0-based index among all user messages.
    pub index: usize,
    /// Truncated user message text (first line, max 80 chars).
    pub summary: String,
    /// Full user message text.
    pub full_text: String,
    /// Number of preceding user messages (i.e. how much context before this point).
    pub context_count: usize,
}

/// Extract branch points (user messages) from a session JSONL file.
/// Returns user messages in chronological order, each with context count.
pub fn extract_branch_points(path: &Path) -> Option<Vec<BranchPoint>> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut points: Vec<BranchPoint> = Vec::new();
    let mut user_count = 0usize;

    for line in content.lines() {
        let record: serde_json::Value = serde_json::from_str(line).ok()?;
        let r#type = record.get("type").and_then(|v| v.as_str()).unwrap_or("");

        // Extract text and determine if this is a user message
        let (text, is_user) = if r#type == "user" {
            (extract_claude_message_text(&record), true)
        } else if r#type == "message" {
            let msg = record.get("message")?;
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
            let text = msg
                .get("content")
                .and_then(|c| extract_text_from_content(c.clone()))
                .unwrap_or_default();
            (text, role == "user")
        } else if r#type == "user_message" {
            let text = record
                .get("payload")
                .and_then(|p| p.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            (text, true)
        } else {
            continue;
        };

        if !is_user {
            continue;
        }

        if text.is_empty() {
            continue;
        }

        let summary: String = text.lines().next().unwrap_or("").chars().take(80).collect();
        points.push(BranchPoint {
            index: user_count,
            summary,
            full_text: text,
            context_count: user_count,
        });
        user_count += 1;
    }

    Some(points)
}

/// Build a context prompt from all user messages up to (and including) the given branch point index.
/// Returns a formatted string that can be used as the initial prompt for a new session.
pub fn extract_branch_context(path: &Path, branch_index: usize) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut user_msgs: Vec<String> = Vec::new();

    for line in content.lines() {
        let record: serde_json::Value = serde_json::from_str(line).ok()?;
        let r#type = record.get("type").and_then(|v| v.as_str()).unwrap_or("");

        let (text, is_user) = if r#type == "user" {
            (extract_claude_message_text(&record), true)
        } else if r#type == "assistant" {
            (extract_claude_message_text(&record), false)
        } else if r#type == "message" {
            let msg = record.get("message")?;
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
            let text = msg
                .get("content")
                .and_then(|c| extract_text_from_content(c.clone()))
                .unwrap_or_default();
            (text, role == "user")
        } else if r#type == "user_message" {
            let text = record
                .get("payload")
                .and_then(|p| p.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            (text, true)
        } else {
            continue;
        };

        if text.is_empty() {
            continue;
        }

        if is_user {
            user_msgs.push(text);
            if user_msgs.len() > branch_index + 1 {
                break;
            }
        }
    }

    if user_msgs.is_empty() {
        return None;
    }

    let mut ctx = String::from("[Session Branch Context]\nPrevious conversation context:\n\n");
    for (i, msg) in user_msgs.iter().enumerate() {
        ctx.push_str(&format!("User (turn {}): {}\n\n", i + 1, msg));
    }
    ctx.push_str("Continue from the above context. Take a different approach than before.\n");
    Some(ctx)
}

/// Extract the first user message from a session JSONL file.
/// Supports Claude, Codex, GSD/OMP formats.
/// Returns the raw user text (not truncated).
pub fn extract_first_user_message(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;

    for line in content.lines() {
        let record: serde_json::Value = serde_json::from_str(line).ok()?;
        let r#type = record.get("type").and_then(|v| v.as_str()).unwrap_or("");

        let text = if r#type == "user" {
            extract_claude_message_text(&record)
        } else if r#type == "message" {
            let msg = record.get("message")?;
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "user" {
                continue;
            }
            msg.get("content")
                .and_then(|c| extract_text_from_content(c.clone()))
                .unwrap_or_default()
        } else if r#type == "user_message" {
            record
                .get("payload")
                .and_then(|p| p.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        } else {
            continue;
        };

        let cleaned = clean_user_message(&text);
        if !cleaned.is_empty() {
            return Some(cleaned);
        }
    }

    None
}

/// Token usage extracted from a session JSONL file.
#[derive(Clone, Debug, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cost: f64,
}

/// Extract cumulative token usage from a session JSONL file.
/// Supports Claude, Codex, OMP/GSD formats.
/// Returns the highest `total_tokens` seen (cumulative) plus cost if available.
pub fn extract_token_usage(path: &Path) -> Option<TokenUsage> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut usage = TokenUsage::default();

    for line in content.lines() {
        let record: serde_json::Value = serde_json::from_str(line).ok()?;
        let r#type = record.get("type").and_then(|v| v.as_str()).unwrap_or("");

        // Claude format: type=assistant, message.usage.input_tokens/output_tokens
        if r#type == "assistant" {
            if let Some(u) = record.get("message").and_then(|m| m.get("usage")) {
                let input = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let output = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                usage.input_tokens += input;
                usage.output_tokens += output;
                usage.total_tokens = usage.input_tokens + usage.output_tokens;
            }
        }
        // OMP format: type=message, message.usage.input/output/totalTokens/cost.total
        else if r#type == "message" {
            if let Some(u) = record.get("message").and_then(|m| m.get("usage")) {
                let total = u.get("totalTokens").and_then(|v| v.as_u64()).unwrap_or(0);
                if total > usage.total_tokens {
                    usage.total_tokens = total;
                    usage.input_tokens = u
                        .get("input")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(usage.input_tokens);
                    usage.output_tokens = u
                        .get("output")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(usage.output_tokens);
                }
                if let Some(cost) = u
                    .get("cost")
                    .and_then(|c| c.get("total"))
                    .and_then(|v| v.as_f64())
                {
                    usage.cost += cost;
                }
            }
        }
        // Codex format: type=event_msg, payload.type=token_count
        else if r#type == "event_msg" {
            let payload_type = record
                .get("payload")
                .and_then(|p| p.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if payload_type == "token_count"
                && let Some(info) = record.get("payload").and_then(|p| p.get("info"))
                && let Some(total_usage) = info.get("total_token_usage")
            {
                let total = total_usage
                    .get("total_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if total > usage.total_tokens {
                    usage.total_tokens = total;
                    usage.input_tokens = total_usage
                        .get("input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    usage.output_tokens = total_usage
                        .get("output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                }
            }
        }
    }

    if usage.total_tokens == 0 && usage.input_tokens == 0 && usage.output_tokens == 0 {
        return None;
    }
    Some(usage)
}

/// Extract all assistant text content from a session JSONL file.
/// Returns a single concatenated string of assistant messages.
pub fn extract_session_output(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut output = String::new();

    for line in content.lines() {
        let record: serde_json::Value = serde_json::from_str(line).ok()?;
        let r#type = record.get("type").and_then(|v| v.as_str()).unwrap_or("");

        let text = if r#type == "assistant" {
            extract_claude_message_text(&record)
        } else if r#type == "message" {
            let msg = record.get("message")?;
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role != "assistant" {
                continue;
            }
            msg.get("content")
                .and_then(|c| extract_text_from_content(c.clone()))
                .unwrap_or_default()
        } else {
            continue;
        };

        if !text.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&text);
        }
    }

    if output.is_empty() {
        return None;
    }
    Some(output)
}

/// A single line in a diff view.
#[derive(Clone, Debug)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub content: String,
}

/// Kind of diff line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffKind {
    Context,
    LeftOnly,
    RightOnly,
}

/// Compute a simple line-level diff between two strings.
/// Returns lines tagged as Context (same), LeftOnly, or RightOnly.
pub fn compute_diff(left: &str, right: &str) -> Vec<DiffLine> {
    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();

    // LCS-based diff using a simple O(n*m) approach
    let n = left_lines.len();
    let m = right_lines.len();

    // Build LCS table
    let mut dp = vec![vec![0u16; m + 1]; n + 1];
    for i in 1..=n {
        for j in 1..=m {
            if left_lines[i - 1] == right_lines[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to find diff
    let mut result = Vec::new();
    let mut i = n;
    let mut j = m;

    // Collect in reverse, then reverse
    let mut ops: Vec<(DiffKind, String)> = Vec::new();
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && left_lines[i - 1] == right_lines[j - 1] {
            ops.push((DiffKind::Context, left_lines[i - 1].to_string()));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            ops.push((DiffKind::RightOnly, right_lines[j - 1].to_string()));
            j -= 1;
        } else {
            ops.push((DiffKind::LeftOnly, left_lines[i - 1].to_string()));
            i -= 1;
        }
    }
    ops.reverse();

    // Truncate to reasonable size for display
    const MAX_LINES: usize = 200;
    if ops.len() > MAX_LINES {
        ops.truncate(MAX_LINES);
        ops.push((DiffKind::Context, "... (truncated)".into()));
    }

    for (kind, content) in ops {
        result.push(DiffLine { kind, content });
    }

    result
}

/// Default paths to scan for session JSONL files on remote hosts.
const DEFAULT_REMOTE_SCAN_PATHS: &[&str] = &[
    "~/.claude/projects",
    "~/.codex/sessions",
    "~/.omp/agent/sessions",
];

/// Discover sessions on a remote host via SSH.
///
/// Uses the system `ssh` command with timeouts. Scans default agent session
/// directories plus any user-configured `agent_paths`. Returns tuples of
/// (display_title, agent_type). On SSH failure, returns a single entry with
/// an error indicator so the UI can display it.
pub fn discover_remote_sessions(host: &crate::types::RemoteHost) -> Vec<(String, String)> {
    let ssh_target = match &host.user {
        Some(user) => format!("{}@{}", user, host.host),
        None => host.host.clone(),
    };

    // Use custom agent_paths if configured, otherwise use defaults.
    let scan_dirs: Vec<String> = if host.agent_paths.is_empty() {
        DEFAULT_REMOTE_SCAN_PATHS
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    } else {
        host.agent_paths.clone()
    };

    // Use a portable approach: find + stat via a shell one-liner.
    // Tries GNU stat first, falls back to BSD stat, then to plain ls.
    // Output format: "mtime_epoch<TAB>fullpath" — one per line, newest first.
    let find_cmd = format!(
        "find {} -name '*.jsonl' 2>/dev/null | while IFS= read -r f; do \
         m=$(stat -c '%Y' \"$f\" 2>/dev/null || stat -f '%m' \"$f\" 2>/dev/null || echo 0); \
         printf '%s\\t%s\\n' \"$m\" \"$f\"; \
         done | sort -rn | head -50",
        scan_dirs.join(" ")
    );

    let port_str = host
        .port
        .map(|p| p.to_string())
        .unwrap_or_else(|| "22".to_string());

    let output = match std::process::Command::new("ssh")
        .args([
            "-p",
            &port_str,
            "-o",
            "ConnectTimeout=5",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            "BatchMode=yes",
            "-o",
            "ServerAliveInterval=5",
            "-o",
            "ServerAliveCountMax=2",
        ])
        .arg(&ssh_target)
        .arg(&find_cmd)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            return vec![(
                format!("SSH error for {}: {}", host.name, e),
                "Error".to_string(),
            )];
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = stderr.trim();
        let detail = if msg.is_empty() {
            format!("SSH to {} failed (exit {})", host.name, output.status)
        } else {
            // Truncate long error to first line
            let first_line = msg.lines().next().unwrap_or(msg);
            format!("SSH {}: {}", host.name, first_line)
        };
        return vec![(detail, "Error".to_string())];
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut sessions = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse "mtime_epoch<TAB>fullpath"
        let (mtime_str, filepath) = match line.split_once('\t') {
            Some(pair) => pair,
            None => {
                // Fallback: treat whole line as path (no mtime from server)
                ("0", line)
            }
        };

        let filepath = filepath.trim();
        if filepath.is_empty() {
            continue;
        }

        let path = std::path::Path::new(filepath);
        let filename = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        let agent = detect_agent_from_path(filepath);

        // Format mtime into a human-readable relative string
        let time_tag = format_mtime(mtime_str);

        sessions.push((
            format!("{} {} [{}]", filename, time_tag, host.name),
            agent.to_string(),
        ));
    }

    if sessions.is_empty() {
        sessions.push((
            format!("No sessions found on {}", host.name),
            "Info".to_string(),
        ));
    }

    sessions
}

/// Detect agent type from a session file path.
fn detect_agent_from_path(path: &str) -> &'static str {
    if path.contains(".claude/") {
        "Claude"
    } else if path.contains(".codex/") {
        "Codex"
    } else if path.contains(".omp/") {
        "OMP"
    } else {
        "Unknown"
    }
}

/// Format an mtime epoch string (e.g. "1717400000.1234567890") into a
/// human-readable relative time tag like "[2h ago]".
fn format_mtime(mtime_str: &str) -> String {
    let epoch: f64 = match mtime_str.parse() {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    let now = now_secs() as f64;
    let elapsed_secs = now - epoch;
    if elapsed_secs < 0.0 {
        return String::new();
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mins = (elapsed_secs / 60.0).round().clamp(0.0, u64::MAX as f64) as u64;
    if mins < 1 {
        "just now".to_string()
    } else if mins < 60 {
        format!("{mins}m ago")
    } else {
        let hours = mins / 60;
        if hours < 24 {
            format!("{hours}h ago")
        } else {
            let days = hours / 24;
            format!("{days}d ago")
        }
    }
}

/// A single timeline event from a session.
#[derive(Clone, Debug)]
pub struct TimelineEvent {
    pub timestamp: u64,
    pub agent: String,
    pub session_title: String,
    pub event_type: String,
    pub summary: String,
}

/// Extract timeline events from all session JSONL files in the workspaces.
/// Aggregates user and assistant messages chronologically.
pub fn extract_timeline(sessions: &[crate::types::Session]) -> Vec<TimelineEvent> {
    let mut events: Vec<TimelineEvent> = Vec::new();

    for session in sessions {
        let jsonl_path = find_session_jsonl(session);
        let Some(jsonl_path) = jsonl_path else {
            continue;
        };
        let content = match std::fs::read_to_string(&jsonl_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let agent_label = session.agent.label().to_string();
        let title = session.title.clone();

        for line in content.lines() {
            let record: serde_json::Value = match serde_json::from_str(line) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let r#type = record.get("type").and_then(|v| v.as_str()).unwrap_or("");

            let (event_type, summary, ts) = if r#type == "user" {
                let text = extract_claude_message_text(&record);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let ts = record
                    .get("timestamp")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0)
                    .round()
                    .clamp(0.0, u64::MAX as f64) as u64;
                ("user".into(), text.chars().take(80).collect::<String>(), ts)
            } else if r#type == "assistant" {
                let text = extract_claude_message_text(&record);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let ts = record
                    .get("timestamp")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0)
                    .round()
                    .clamp(0.0, u64::MAX as f64) as u64;
                (
                    "assistant".into(),
                    text.chars().take(80).collect::<String>(),
                    ts,
                )
            } else if r#type == "message" {
                let msg = match record.get("message") {
                    Some(m) => m,
                    None => continue,
                };
                let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
                let text = msg
                    .get("content")
                    .and_then(|c| extract_text_from_content(c.clone()))
                    .unwrap_or_default();
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let ts = record
                    .get("timestamp")
                    .or_else(|| record.get("createdAt"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0)
                    .round()
                    .clamp(0.0, u64::MAX as f64) as u64;
                (
                    role.to_string(),
                    text.chars().take(80).collect::<String>(),
                    ts,
                )
            } else if r#type == "user_message" {
                let text = record
                    .get("payload")
                    .and_then(|p| p.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let ts = record
                    .get("timestamp")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0)
                    .round()
                    .clamp(0.0, u64::MAX as f64) as u64;
                ("user".into(), text.chars().take(80).collect::<String>(), ts)
            } else {
                continue;
            };

            if ts > 0 && !summary.is_empty() {
                events.push(TimelineEvent {
                    timestamp: ts,
                    agent: agent_label.clone(),
                    session_title: title.clone(),
                    event_type,
                    summary,
                });
            }
        }
    }

    events.sort_by_key(|e| e.timestamp);
    // Truncate to last 100 events
    if events.len() > 100 {
        events = events.split_off(events.len() - 100);
    }
    events
}

/// Agent performance metrics for recommendation.
#[derive(Clone, Debug)]
pub struct AgentMetrics {
    pub agent: String,
    pub total_sessions: usize,
    pub completed_sessions: usize,
    pub avg_duration_secs: f64,
    pub success_rate: f64,
}

/// Analyze historical session data to recommend best agent.
pub fn compute_agent_recommendations(sessions: &[crate::types::Session]) -> Vec<AgentMetrics> {
    use std::collections::HashMap;
    let mut counts: HashMap<String, usize> = HashMap::new();

    for session in sessions {
        let key = session.agent.label().to_string();
        *counts.entry(key.clone()).or_insert(0) += 1;
    }

    let mut metrics: Vec<AgentMetrics> = counts
        .into_iter()
        .map(|(agent, total)| AgentMetrics {
            agent,
            total_sessions: total,
            completed_sessions: total,
            avg_duration_secs: 0.0,
            success_rate: 1.0,
        })
        .collect();

    // Sort by total sessions (most used agents ranked higher)
    metrics.sort_by(|a, b| b.total_sessions.cmp(&a.total_sessions));
    metrics
}

/// Generate a markdown summary from a completed session's JSONL.
pub fn generate_session_summary(session: &crate::types::Session) -> Option<String> {
    let jsonl_path = find_session_jsonl(session)?;
    let content = std::fs::read_to_string(&jsonl_path).ok()?;

    let mut user_msgs: Vec<String> = Vec::new();
    let mut assistant_msgs: Vec<String> = Vec::new();
    let mut file_paths: Vec<String> = Vec::new();

    let path_re = regex::Regex::new(r"(?:^|\s)([\w./-]+\.\w{1,10})(?::|\s|$)").ok()?;

    for line in content.lines() {
        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let r#type = record.get("type").and_then(|v| v.as_str()).unwrap_or("");

        if r#type == "user" || r#type == "user_message" {
            let text = if r#type == "user" {
                extract_claude_message_text(&record)
            } else {
                record
                    .get("payload")
                    .and_then(|p| p.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            if !text.is_empty() {
                let first_line = text
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(100)
                    .collect::<String>();
                user_msgs.push(first_line);
            }
        } else if r#type == "assistant" {
            let text = extract_claude_message_text(&record);
            if !text.is_empty() {
                let first_line = text
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(100)
                    .collect::<String>();
                assistant_msgs.push(first_line);
                // Extract file paths from assistant output
                for cap in path_re.captures_iter(&text) {
                    let p = cap[1].to_string();
                    if !file_paths.contains(&p) {
                        file_paths.push(p);
                    }
                }
            }
        } else if r#type == "message" {
            let msg = match record.get("message") {
                Some(m) => m,
                None => continue,
            };
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
            let text = msg
                .get("content")
                .and_then(|c| extract_text_from_content(c.clone()))
                .unwrap_or_default();
            if text.is_empty() {
                continue;
            }
            let first_line = text
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(100)
                .collect::<String>();
            if role == "user" {
                user_msgs.push(first_line);
            } else if role == "assistant" {
                assistant_msgs.push(first_line);
                for cap in path_re.captures_iter(&text) {
                    let p = cap[1].to_string();
                    if !file_paths.contains(&p) {
                        file_paths.push(p);
                    }
                }
            }
        }
    }

    let mut md = String::new();
    md.push_str(&format!("# Session: {}\n\n", session.title));
    md.push_str(&format!("**Agent:** {}\n", session.agent.label()));
    md.push_str(&format!("**ID:** {}\n\n", session.id));

    if !user_msgs.is_empty() {
        md.push_str("## User Messages\n\n");
        for msg in user_msgs.iter().take(10) {
            md.push_str(&format!("- {msg}\n"));
        }
        md.push('\n');
    }

    if !file_paths.is_empty() {
        md.push_str("## Files Touched\n\n");
        for f in file_paths.iter().take(20) {
            md.push_str(&format!("- `{f}`\n"));
        }
        md.push('\n');
    }

    if !assistant_msgs.is_empty() {
        md.push_str("## Key Responses\n\n");
        for msg in assistant_msgs.iter().take(5) {
            md.push_str(&format!("- {msg}\n"));
        }
    }

    Some(md)
}

/// Search result from cross-session full-text search.
#[derive(Clone, Debug)]
pub struct CrossSearchResult {
    pub session_id: String,
    pub session_title: String,
    pub agent: String,
    pub matches: Vec<String>,
}

/// Full-text search across all session JSONL files.
pub fn cross_session_search(
    sessions: &[crate::types::Session],
    query: &str,
) -> Vec<CrossSearchResult> {
    let query_lower = query.to_lowercase();
    let mut results: Vec<CrossSearchResult> = Vec::new();

    for session in sessions {
        let jsonl_path = match find_session_jsonl(session) {
            Some(p) => p,
            None => continue,
        };
        let content = match std::fs::read_to_string(&jsonl_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut matches: Vec<String> = Vec::new();
        for line in content.lines() {
            if line.to_lowercase().contains(&query_lower) {
                // Extract a readable snippet around the match
                if let Some(pos) = line.to_lowercase().find(&query_lower) {
                    let start = pos.saturating_sub(30);
                    let end = (pos + query_lower.len() + 30).min(line.len());
                    let snippet: String = line.chars().skip(start).take(end - start).collect();
                    let clean = snippet.chars().take(80).collect::<String>();
                    if !clean.is_empty() {
                        matches.push(clean);
                    }
                }
                if matches.len() >= 5 {
                    break;
                }
            }
        }

        // Also search session notes from meta files
        if let Some(meta) = load_session_meta(&session.id, None)
            && let Some(ref note) = meta.note
            && !note.is_empty()
            && note.to_lowercase().contains(&query_lower)
        {
            let snippet: String = note.chars().take(80).collect();
            matches.push(format!("[note] {snippet}"));
        }

        if !matches.is_empty() {
            results.push(CrossSearchResult {
                session_id: session.id.clone(),
                session_title: session.title.clone(),
                agent: session.agent.label().to_string(),
                matches,
            });
        }
    }

    results.sort_by_key(|b| std::cmp::Reverse(b.matches.len()));
    results.truncate(20);
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write JSON values as JSONL (one JSON object per line, no trailing blank line).
    fn write_jsonl(path: &Path, records: &[serde_json::Value]) {
        let jsonl: String = records
            .iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(path, jsonl).unwrap();
    }

    // ── clean_user_message ──────────────────────────────────────────

    #[test]
    fn clean_user_message_ansi_escape_stripped() {
        let input = "\x1b[32mgreen text\x1b[0m";
        // Starts with \x1b → noise prefix → empty
        assert_eq!(clean_user_message(input), "");
    }

    #[test]
    fn clean_user_message_leading_noise_returns_empty() {
        assert_eq!(clean_user_message("P>|something"), "");
        assert_eq!(clean_user_message("P<|something"), "");
    }

    #[test]
    fn clean_user_message_embedded_p_pipe_removed() {
        // "hello P>|world\"rest" → "hello rest"
        let input = "hello P>|world\\rest";
        let cleaned = clean_user_message(input);
        assert_eq!(cleaned, "hello rest");
    }

    #[test]
    fn clean_user_message_trims_whitespace() {
        assert_eq!(clean_user_message("  hello world  "), "hello world");
    }

    #[test]
    fn clean_user_message_plain_text_unchanged() {
        assert_eq!(clean_user_message("hello world"), "hello world");
    }

    #[test]
    fn clean_user_message_empty_string() {
        assert_eq!(clean_user_message(""), "");
    }

    // ── extract_text_from_content ───────────────────────────────────

    #[test]
    fn extract_text_from_string_value() {
        let v = serde_json::Value::String("hello".into());
        assert_eq!(extract_text_from_content(v), Some("hello".to_string()));
    }

    #[test]
    fn extract_text_from_array_of_text_blocks() {
        let v = serde_json::json!([
            {"type": "text", "text": "hello"},
            {"type": "text", "text": "world"}
        ]);
        assert_eq!(
            extract_text_from_content(v),
            Some("hello world".to_string())
        );
    }

    #[test]
    fn extract_text_from_array_ignores_non_text_blocks() {
        let v = serde_json::json!([
            {"type": "image", "url": "http://x"},
            {"type": "text", "text": "only this"}
        ]);
        assert_eq!(extract_text_from_content(v), Some("only this".to_string()));
    }

    #[test]
    fn extract_text_from_empty_array_returns_none() {
        let v = serde_json::json!([]);
        assert_eq!(extract_text_from_content(v), None);
    }

    #[test]
    fn extract_text_from_null_returns_none() {
        assert_eq!(extract_text_from_content(serde_json::Value::Null), None);
    }

    #[test]
    fn extract_text_from_object_without_text_returns_none() {
        let v = serde_json::json!({"key": "value"});
        assert_eq!(extract_text_from_content(v), None);
    }

    // ── compute_diff ────────────────────────────────────────────────

    #[test]
    fn compute_diff_identical_strings() {
        let diff = compute_diff("a\nb\nc", "a\nb\nc");
        assert!(diff.iter().all(|d| d.kind == DiffKind::Context));
        assert_eq!(diff.len(), 3);
    }

    #[test]
    fn compute_diff_added_lines() {
        let diff = compute_diff("", "new line");
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].kind, DiffKind::RightOnly);
        assert_eq!(diff[0].content, "new line");
    }

    #[test]
    fn compute_diff_removed_lines() {
        let diff = compute_diff("old line", "");
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].kind, DiffKind::LeftOnly);
        assert_eq!(diff[0].content, "old line");
    }

    #[test]
    fn compute_diff_both_empty() {
        let diff = compute_diff("", "");
        assert!(diff.is_empty());
    }

    #[test]
    fn compute_diff_mixed_changes() {
        let diff = compute_diff("a\nb\nc", "a\nx\nc");
        // Should have Context("a"), LeftOnly("b"), RightOnly("x"), Context("c")
        assert_eq!(diff.len(), 4);
        assert_eq!(diff[0].kind, DiffKind::Context);
        assert_eq!(diff[0].content, "a");
        assert_eq!(diff[1].kind, DiffKind::LeftOnly);
        assert_eq!(diff[1].content, "b");
        assert_eq!(diff[2].kind, DiffKind::RightOnly);
        assert_eq!(diff[2].content, "x");
        assert_eq!(diff[3].kind, DiffKind::Context);
        assert_eq!(diff[3].content, "c");
    }

    // ── detect_agent_from_path ──────────────────────────────────────

    #[test]
    fn detect_agent_claude() {
        assert_eq!(
            detect_agent_from_path("/home/user/.claude/projects/abc/session.jsonl"),
            "Claude"
        );
    }

    #[test]
    fn detect_agent_codex() {
        assert_eq!(
            detect_agent_from_path("/home/user/.codex/sessions/xyz.jsonl"),
            "Codex"
        );
    }

    #[test]
    fn detect_agent_omp() {
        assert_eq!(
            detect_agent_from_path("/home/user/.omp/sessions/def.jsonl"),
            "OMP"
        );
    }

    #[test]
    fn detect_agent_unknown() {
        assert_eq!(
            detect_agent_from_path("/tmp/random/session.jsonl"),
            "Unknown"
        );
    }

    // ── format_mtime ────────────────────────────────────────────────

    #[test]
    fn format_mtime_invalid_returns_empty() {
        assert_eq!(format_mtime("not-a-number"), "");
    }

    #[test]
    fn format_mtime_future_returns_empty() {
        // A far-future timestamp should produce negative elapsed → empty
        let future = format!("{}", u64::MAX);
        assert_eq!(format_mtime(&future), "");
    }

    #[test]
    fn format_mtime_recent_returns_just_now() {
        // 10 seconds ago — well under the 1-minute boundary
        let ts = format!("{}", now_secs() - 10);
        assert_eq!(format_mtime(&ts), "just now");
    }

    #[test]
    fn format_mtime_minutes_ago() {
        let ts = format!("{}", now_secs() - 300); // 5 min ago
        assert_eq!(format_mtime(&ts), "5m ago");
    }

    #[test]
    fn format_mtime_hours_ago() {
        let ts = format!("{}", now_secs() - 7200); // 2h ago
        assert_eq!(format_mtime(&ts), "2h ago");
    }

    #[test]
    fn format_mtime_days_ago() {
        let ts = format!("{}", now_secs() - 172_800); // 2 days ago
        assert_eq!(format_mtime(&ts), "2d ago");
    }

    // ── parse_gsd_session ───────────────────────────────────────────

    #[test]
    fn parse_gsd_session_basic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        write_jsonl(
            &path,
            &[
                serde_json::json!({"type":"session","version":3,"id":"gsd-123","timestamp":"2024-01-01T00:00:00Z","cwd":"/home/user"}),
                serde_json::json!({"type":"custom_message","customType":"gsd-run","message":"build the feature"}),
            ],
        );

        let result = parse_gsd_session(&path).unwrap();
        assert_eq!(result.0, "gsd-123");
        assert_eq!(result.1, Some("build the feature".to_string()));
        assert_eq!(result.2, Some("/home/user".to_string()));
    }

    #[test]
    fn parse_gsd_session_title_truncated_at_50_chars() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let long_msg = "x".repeat(80);
        write_jsonl(
            &path,
            &[
                serde_json::json!({"type":"session","version":3,"id":"gsd-456","timestamp":"2024-01-01T00:00:00Z","cwd":"/tmp"}),
                serde_json::json!({"type":"custom_message","customType":"gsd-run","message":long_msg}),
            ],
        );

        let result = parse_gsd_session(&path).unwrap();
        assert_eq!(result.1.unwrap().len(), 50);
    }

    #[test]
    fn parse_gsd_session_fallback_to_user_message() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        write_jsonl(
            &path,
            &[
                serde_json::json!({"type":"session","version":3,"id":"gsd-789","timestamp":"2024-01-01T00:00:00Z"}),
                serde_json::json!({"type":"message","role":"user","message":"hello from user"}),
            ],
        );

        let result = parse_gsd_session(&path).unwrap();
        assert_eq!(result.0, "gsd-789");
        assert_eq!(result.1, Some("hello from user".to_string()));
        assert_eq!(result.2, None);
    }

    #[test]
    fn parse_gsd_session_no_id_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        write_jsonl(
            &path,
            &[serde_json::json!({"type":"message","role":"user","message":"no session header"})],
        );
        assert!(parse_gsd_session(&path).is_none());
    }

    // ── parse_codex_session ─────────────────────────────────────────

    #[test]
    fn parse_codex_session_basic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        write_jsonl(
            &path,
            &[
                serde_json::json!({"type":"session_meta","payload":{"id":"codex-abc","cwd":"/work"}}),
                serde_json::json!({"type":"user_message","payload":{"text":"fix the bug"}}),
            ],
        );

        let result = parse_codex_session(&path).unwrap();
        assert_eq!(result.0, "codex-abc");
        assert_eq!(result.1, Some("fix the bug".to_string()));
        assert_eq!(result.2, "/work");
    }

    #[test]
    fn parse_codex_session_no_user_message() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        write_jsonl(
            &path,
            &[
                serde_json::json!({"type":"session_meta","payload":{"id":"codex-def","cwd":"/home"}}),
            ],
        );

        let result = parse_codex_session(&path).unwrap();
        assert_eq!(result.0, "codex-def");
        assert!(result.1.is_none());
    }

    #[test]
    fn parse_codex_session_empty_id_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        write_jsonl(
            &path,
            &[serde_json::json!({"type":"user_message","payload":{"text":"no meta"}})],
        );
        assert!(parse_codex_session(&path).is_none());
    }

    // ── extract_token_usage ─────────────────────────────────────────

    #[test]
    fn extract_token_usage_claude_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        write_jsonl(
            &path,
            &[
                serde_json::json!({"type":"assistant","message":{"usage":{"input_tokens":100,"output_tokens":50}}}),
                serde_json::json!({"type":"assistant","message":{"usage":{"input_tokens":200,"output_tokens":100}}}),
            ],
        );

        let usage = extract_token_usage(&path).unwrap();
        assert_eq!(usage.input_tokens, 300);
        assert_eq!(usage.output_tokens, 150);
        assert_eq!(usage.total_tokens, 450);
    }

    #[test]
    fn extract_token_usage_omp_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        write_jsonl(
            &path,
            &[
                serde_json::json!({"type":"message","message":{"usage":{"totalTokens":500,"input":200,"output":300,"cost":{"total":0.05}}}}),
                serde_json::json!({"type":"message","message":{"usage":{"totalTokens":800,"input":350,"output":450,"cost":{"total":0.08}}}}),
            ],
        );

        let usage = extract_token_usage(&path).unwrap();
        assert_eq!(usage.total_tokens, 800);
        assert_eq!(usage.input_tokens, 350);
        assert_eq!(usage.output_tokens, 450);
        assert!((usage.cost - 0.13).abs() < 0.001);
    }

    #[test]
    fn extract_token_usage_codex_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        write_jsonl(
            &path,
            &[
                serde_json::json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":600,"input_tokens":250,"output_tokens":350}}}}),
            ],
        );

        let usage = extract_token_usage(&path).unwrap();
        assert_eq!(usage.total_tokens, 600);
        assert_eq!(usage.input_tokens, 250);
        assert_eq!(usage.output_tokens, 350);
    }

    #[test]
    fn extract_token_usage_no_data_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        write_jsonl(
            &path,
            &[serde_json::json!({"type":"user","message":"no tokens here"})],
        );
        assert!(extract_token_usage(&path).is_none());
    }

    #[test]
    fn extract_token_usage_missing_file_returns_none() {
        assert!(extract_token_usage(Path::new("/nonexistent/file.jsonl")).is_none());
    }
}
