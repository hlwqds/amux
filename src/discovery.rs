use std::{
    env, fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::config::{data_dir, encode_project_path, load_session_title};
use crate::types::{Agent, ClaudeRecord, Session, Workspace};
use crate::util::now_secs;

pub fn discover_workspaces_from_fs() -> Vec<Workspace> {
    let roots = env::var_os("AGENT_WORKSPACES")
        .map(|v| env::split_paths(&v).collect())
        .unwrap_or_else(crate::config::default_roots);

    let mut ws: Vec<_> = roots
        .into_iter()
        .filter(|p| p.join(".git").exists())
        .map(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?").into();
            Workspace {
                id: crate::config::generate_id(),
                name,
                path: Some(p),
                created_at: now_secs(),
                expanded: true,
            }
        })
        .collect();
    ws.sort_by_key(|a| a.name.clone());
    ws
}

pub fn discover_sessions(workspaces: &[Workspace]) -> Vec<Session> {
    let mut sessions = Vec::new();
    discover_claude_sessions(workspaces, &mut sessions);
    discover_codex_sessions(workspaces, &mut sessions);
    discover_gsd_sessions(workspaces, &mut sessions);
    sessions.sort_by_key(|b| std::cmp::Reverse(b.last_active));
    sessions
}

pub fn find_session_jsonl(session: &Session) -> Option<PathBuf> {
    match session.agent {
        Agent::Claude => {
            let projects_dir = Agent::Claude.sessions_dir()?;
            let encoded = encode_project_path(&session.workspace_path);
            let path = projects_dir
                .join(encoded)
                .join(format!("{}.jsonl", session.id));
            if path.exists() { Some(path) } else { None }
        }
        Agent::Codex => {
            let sessions_root = Agent::Codex.sessions_dir()?;
            walk_codex_jsonl(&sessions_root, &session.id)
        }
        Agent::Gsd => {
            let sessions_root = Agent::Gsd.sessions_dir()?;
            walk_gsd_jsonl(&sessions_root, &session.id)
        }
    }
}

fn walk_gsd_jsonl(root: &Path, session_id: &str) -> Option<PathBuf> {
    // GSD sessions are stored in subdirs named after the encoded workspace path
    // e.g. ~/.gsd/sessions/-home-user-proj/<session-id>.jsonl
    if let Ok(subdirs) = fs::read_dir(root) {
        for subdir in subdirs.flatten() {
            if !subdir.path().is_dir() {
                continue;
            }
            if let Ok(files) = fs::read_dir(subdir.path()) {
                for file in files.flatten() {
                    let path = file.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                        continue;
                    }
                    if let Ok(content) = fs::read_to_string(&path)
                        && content.contains(session_id)
                    {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

fn walk_codex_jsonl(root: &Path, session_id: &str) -> Option<PathBuf> {
    if let Ok(years) = fs::read_dir(root) {
        for year in years.flatten() {
            if !year.path().is_dir() {
                continue;
            }
            if let Ok(months) = fs::read_dir(year.path()) {
                for month in months.flatten() {
                    if !month.path().is_dir() {
                        continue;
                    }
                    if let Ok(days) = fs::read_dir(month.path()) {
                        for day in days.flatten() {
                            if !day.path().is_dir() {
                                continue;
                            }
                            if let Ok(files) = fs::read_dir(day.path()) {
                                for file in files.flatten() {
                                    let path = file.path();
                                    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                                        continue;
                                    }
                                    if let Ok(content) = fs::read_to_string(&path)
                                        && content.contains(session_id)
                                    {
                                        return Some(path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn discover_gsd_sessions(workspaces: &[Workspace], out: &mut Vec<Session>) {
    let sessions_root = match Agent::Gsd.sessions_dir() {
        Some(d) => d,
        None => return,
    };

    let ws_paths: Vec<PathBuf> = workspaces
        .iter()
        .map(|ws| {
            ws.path.clone().unwrap_or_else(|| {
                let dir = data_dir().join("workspaces").join(&ws.id);
                let _ = fs::create_dir_all(&dir);
                dir
            })
        })
        .collect();

    if let Ok(subdirs) = fs::read_dir(&sessions_root) {
        for subdir in subdirs.flatten() {
            if !subdir.path().is_dir() {
                continue;
            }
            // Decode directory name back to workspace path (reverse of replace '/' with '-')
            let dir_name = subdir.file_name().to_string_lossy().into_owned();
            let decoded_ws_path = PathBuf::from(dir_name.replace('-', "/"));

            if let Ok(files) = fs::read_dir(subdir.path()) {
                for file in files.flatten() {
                    let path = file.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                        continue;
                    }

                    let meta = parse_gsd_session(&path);
                    let (id, title, cwd) = match meta {
                        Some(m) => m,
                        None => continue,
                    };

                    let ws_path = if let Some(ref cwd_str) = cwd {
                        ws_paths
                            .iter()
                            .find(|p| cwd_str == p.to_string_lossy().as_ref())
                            .cloned()
                            .unwrap_or_else(|| decoded_ws_path.clone())
                    } else {
                        decoded_ws_path.clone()
                    };

                    let last_active = fs::metadata(&path)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);

                    out.push(Session {
                        id,
                        workspace_path: ws_path,
                        title: title.unwrap_or_else(|| "GSD session".into()),
                        last_active,
                        agent: Agent::Gsd,
                    });
                }
            }
        }
    }
}

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
            "message" if title.is_none() => {
                if record.get("role").and_then(|v| v.as_str()) == Some("user") {
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

fn discover_claude_sessions(workspaces: &[Workspace], out: &mut Vec<Session>) {
    let projects_dir = match Agent::Claude.sessions_dir() {
        Some(d) => d,
        None => return,
    };

    for ws in workspaces {
        let ws_path = ws.path.clone().unwrap_or_else(|| {
            let dir = data_dir().join("workspaces").join(&ws.id);
            let _ = fs::create_dir_all(&dir);
            dir
        });
        let encoded = encode_project_path(&ws_path);
        let proj_dir = projects_dir.join(encoded);
        let entries = match fs::read_dir(&proj_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            let last_active = fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let title = load_session_title(&id, Some(&ws_path))
                .or_else(|| extract_claude_title(&path))
                .unwrap_or_else(|| format!("Session {}", &id[..8.min(id.len())]));

            out.push(Session {
                id,
                workspace_path: ws_path.clone(),
                title,
                last_active,
                agent: Agent::Claude,
            });
        }
    }
}

fn extract_claude_title(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let record: ClaudeRecord = serde_json::from_str(line).ok()?;
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

fn discover_codex_sessions(workspaces: &[Workspace], out: &mut Vec<Session>) {
    let sessions_root = match Agent::Codex.sessions_dir() {
        Some(d) => d,
        None => return,
    };

    let ws_paths: Vec<PathBuf> = workspaces
        .iter()
        .map(|ws| {
            ws.path.clone().unwrap_or_else(|| {
                let dir = data_dir().join("workspaces").join(&ws.id);
                let _ = fs::create_dir_all(&dir);
                dir
            })
        })
        .collect();

    if let Ok(years) = fs::read_dir(&sessions_root) {
        for year in years.flatten() {
            if !year.path().is_dir() {
                continue;
            }
            if let Ok(months) = fs::read_dir(year.path()) {
                for month in months.flatten() {
                    if !month.path().is_dir() {
                        continue;
                    }
                    if let Ok(days) = fs::read_dir(month.path()) {
                        for day in days.flatten() {
                            if !day.path().is_dir() {
                                continue;
                            }
                            if let Ok(files) = fs::read_dir(day.path()) {
                                for file in files.flatten() {
                                    let path = file.path();
                                    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                                        continue;
                                    }

                                    let meta = parse_codex_session(&path);
                                    let (id, title, cwd) = match meta {
                                        Some(m) => m,
                                        None => continue,
                                    };

                                    let ws_path = ws_paths
                                        .iter()
                                        .find(|p| cwd == p.to_string_lossy().as_ref())
                                        .cloned()
                                        .unwrap_or_else(|| {
                                            ws_paths.first().cloned().unwrap_or_default()
                                        });

                                    let last_active = fs::metadata(&path)
                                        .ok()
                                        .and_then(|m| m.modified().ok())
                                        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                                        .map(|d| d.as_secs())
                                        .unwrap_or(0);

                                    out.push(Session {
                                        id,
                                        workspace_path: ws_path,
                                        title: title.unwrap_or_else(|| "Codex session".into()),
                                        last_active,
                                        agent: Agent::Codex,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::config::encode_project_path;
    use serde_json;

    use super::*;

    #[test]
    fn clean_user_message_normal() {
        assert_eq!(clean_user_message("hello world"), "hello world");
    }

    #[test]
    fn clean_user_message_escapes() {
        assert_eq!(clean_user_message("\x1b[32m"), "");
    }

    #[test]
    fn clean_user_message_noise_prefix() {
        assert_eq!(clean_user_message("P>|stuff"), "");
        assert_eq!(clean_user_message("P<|stuff"), "");
    }

    #[test]
    fn clean_user_message_strips_whitespace() {
        assert_eq!(clean_user_message("  hello  "), "hello");
    }

    #[test]
    fn extract_text_from_string_content() {
        let val = serde_json::json!("hello");
        assert_eq!(extract_text_from_content(val), Some("hello".into()));
    }

    #[test]
    fn extract_text_from_array_content() {
        let val = serde_json::json!([
            {"type": "text", "text": "hello "},
            {"type": "text", "text": "world"}
        ]);
        assert_eq!(extract_text_from_content(val), Some("hello  world".into()));
    }

    #[test]
    fn extract_text_from_array_with_non_text() {
        let val = serde_json::json!([
            {"type": "image", "url": "http://example.com"},
            {"type": "text", "text": "visible"}
        ]);
        assert_eq!(extract_text_from_content(val), Some("visible".into()));
    }

    #[test]
    fn extract_text_from_empty_array() {
        let val = serde_json::json!([{"type": "image"}]);
        assert_eq!(extract_text_from_content(val), None);
    }

    #[test]
    fn extract_text_from_number() {
        let val = serde_json::json!(42);
        assert_eq!(extract_text_from_content(val), None);
    }

    #[test]
    fn parse_codex_session_valid() {
        let jsonl = r#"{"type":"session_meta","payload":{"id":"sess-123","cwd":"/home/user/proj"}}
{"type":"user_message","payload":{"text":"fix the bug"}}"#;
        let dir = std::env::temp_dir().join("agent-test-codex");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("rollout-20260115-sess-123.jsonl");
        std::fs::write(&path, jsonl).unwrap();

        let result = parse_codex_session(&path);
        assert!(result.is_some());
        let (id, title, cwd) = result.unwrap();
        assert_eq!(id, "sess-123");
        assert_eq!(title.unwrap(), "fix the bug");
        assert_eq!(cwd, "/home/user/proj");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_codex_session_invalid_json() {
        let dir = std::env::temp_dir().join("agent-test-codex-invalid");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("bad.jsonl");
        std::fs::write(&path, "not json").unwrap();

        let result = parse_codex_session(&path);
        assert!(result.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_gsd_session_valid_with_gsd_run_title() {
        let jsonl = r#"{"type":"session","version":3,"id":"gsd-sess-001","timestamp":"2026-06-02T10:00:00Z","cwd":"/home/user/proj"}
{"type":"custom_message","customType":"gsd-run","message":"implement the feature"}"#;
        let dir = std::env::temp_dir().join("agent-test-gsd");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("gsd-sess-001.jsonl");
        std::fs::write(&path, jsonl).unwrap();

        let result = parse_gsd_session(&path);
        assert!(result.is_some());
        let (id, title, cwd) = result.unwrap();
        assert_eq!(id, "gsd-sess-001");
        assert_eq!(title.unwrap(), "implement the feature");
        assert_eq!(cwd.unwrap(), "/home/user/proj");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_gsd_session_fallback_to_user_message() {
        let jsonl = r#"{"type":"session","version":3,"id":"gsd-sess-002","timestamp":"2026-06-02T10:00:00Z","cwd":"/home/user/proj"}
{"type":"message","role":"user","message":"hello from interactive"}"#;
        let dir = std::env::temp_dir().join("agent-test-gsd-user-msg");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("gsd-sess-002.jsonl");
        std::fs::write(&path, jsonl).unwrap();

        let result = parse_gsd_session(&path);
        assert!(result.is_some());
        let (id, title, _cwd) = result.unwrap();
        assert_eq!(id, "gsd-sess-002");
        assert_eq!(title.unwrap(), "hello from interactive");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_gsd_session_gsd_run_takes_priority() {
        let jsonl = r#"{"type":"session","version":3,"id":"gsd-sess-003","timestamp":"2026-06-02T10:00:00Z","cwd":"/home/user/proj"}
{"type":"custom_message","customType":"gsd-run","message":"auto-mode task"}
{"type":"message","role":"user","message":"user typed something"}"#;
        let dir = std::env::temp_dir().join("agent-test-gsd-priority");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("gsd-sess-003.jsonl");
        std::fs::write(&path, jsonl).unwrap();

        let result = parse_gsd_session(&path);
        assert!(result.is_some());
        let (_id, title, _cwd) = result.unwrap();
        assert_eq!(title.unwrap(), "auto-mode task");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_gsd_session_no_session_line() {
        let jsonl =
            r#"{"type":"custom_message","customType":"gsd-run","message":"no session header"}"#;
        let dir = std::env::temp_dir().join("agent-test-gsd-no-session");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("bad.jsonl");
        std::fs::write(&path, jsonl).unwrap();

        let result = parse_gsd_session(&path);
        assert!(result.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_gsd_session_title_truncated_to_50_chars() {
        let long_title = "a".repeat(100);
        let session_line = serde_json::json!({
            "type": "session",
            "version": 3,
            "id": "gsd-sess-004",
            "timestamp": "2026-06-02T10:00:00Z",
            "cwd": "/home/user/proj"
        });
        let msg_line = serde_json::json!({
            "type": "custom_message",
            "customType": "gsd-run",
            "message": long_title
        });
        let jsonl = format!("{}\n{}", session_line, msg_line);
        let dir = std::env::temp_dir().join("agent-test-gsd-truncate");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("gsd-sess-004.jsonl");
        std::fs::write(&path, jsonl).unwrap();

        let result = parse_gsd_session(&path);
        assert!(result.is_some());
        let (_id, title, _cwd) = result.unwrap();
        assert_eq!(title.unwrap().len(), 50);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_gsd_session_empty_file() {
        let dir = std::env::temp_dir().join("agent-test-gsd-empty");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("empty.jsonl");
        std::fs::write(&path, "").unwrap();

        let result = parse_gsd_session(&path);
        assert!(result.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_gsd_sessions_finds_by_workspace() {
        // Create a temp sessions dir simulating ~/.gsd/sessions/
        let tmp = std::env::temp_dir().join("agent-test-gsd-discover");
        let _ = std::fs::remove_dir_all(&tmp);
        let ws_path = Path::new("/tmp/fake-workspace-for-test");
        let encoded_ws = encode_project_path(ws_path);
        let session_dir = tmp.join(&encoded_ws);
        std::fs::create_dir_all(&session_dir).unwrap();

        let session_line = serde_json::json!({
            "type": "session",
            "version": 3,
            "id": "gsd-disc-001",
            "timestamp": "2026-06-02T10:00:00Z",
            "cwd": "/tmp/fake-workspace-for-test"
        });
        let msg_line = serde_json::json!({
            "type": "custom_message",
            "customType": "gsd-run",
            "message": "discovery test"
        });
        let jsonl = format!("{}\n{}", session_line, msg_line);
        std::fs::write(session_dir.join("gsd-disc-001.jsonl"), jsonl).unwrap();

        // Verify parse_gsd_session can read it
        let parsed = parse_gsd_session(&session_dir.join("gsd-disc-001.jsonl"));
        assert!(parsed.is_some());
        let (id, title, cwd) = parsed.unwrap();
        assert_eq!(id, "gsd-disc-001");
        assert_eq!(title.unwrap(), "discovery test");
        assert_eq!(cwd.unwrap(), "/tmp/fake-workspace-for-test");

        // Verify the encoded directory matches expected workspace
        assert_eq!(encoded_ws, "-tmp-fake-workspace-for-test");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
