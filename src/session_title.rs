use std::{fs, path::Path};

use crate::extraction::{clean_user_message, extract_claude_message_text, extract_text_from_content};
use crate::types::ClaudeRecord;

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
