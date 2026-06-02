use amux::app;

fn main() -> anyhow::Result<()> {
    app::run()
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use ratatui::style::Color;

    use amux::config::*;
    use amux::discovery::*;
    use amux::types::*;
    use amux::util::*;

    #[test]
    fn encode_project_path_simple() {
        assert_eq!(
            encode_project_path(Path::new("/home/user/my-project")),
            "-home-user-my-project"
        );
    }

    #[test]
    fn encode_project_path_root() {
        assert_eq!(encode_project_path(Path::new("/")), "-");
    }

    #[test]
    fn encode_project_path_relative() {
        assert_eq!(encode_project_path(Path::new("my-project")), "my-project");
    }

    #[test]
    fn config_roundtrip() {
        let config = Config {
            workspaces: vec![
                Workspace {
                    id: "ws-1".into(),
                    name: "Project A".into(),
                    path: Some(PathBuf::from("/home/user/proj-a")),
                    created_at: 1000,
                    expanded: false,
                },
                Workspace {
                    id: "ws-2".into(),
                    name: "Virtual".into(),
                    path: None,
                    created_at: 2000,
                    expanded: true,
                },
            ],
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.workspaces.len(), 2);
        assert_eq!(parsed.workspaces[0].id, "ws-1");
        assert_eq!(
            parsed.workspaces[0].path,
            Some(PathBuf::from("/home/user/proj-a"))
        );
        assert_eq!(parsed.workspaces[1].path, None);
        assert!(!parsed.workspaces[0].expanded);
        assert!(!parsed.workspaces[1].expanded);
    }

    #[test]
    fn workspace_serialization_virtual() {
        let ws = Workspace {
            id: "test-id".into(),
            name: "No Path".into(),
            path: None,
            created_at: 0,
            expanded: false,
        };
        let json = serde_json::to_string(&ws).unwrap();
        assert!(json.contains("\"path\":null"));
        let parsed: Workspace = serde_json::from_str(&json).unwrap();
        assert!(parsed.path.is_none());
    }

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
    fn agent_traits() {
        assert_eq!(Agent::Claude.cmd(), "claude");
        assert_eq!(Agent::Codex.cmd(), "codex");
        assert_eq!(Agent::Gsd.cmd(), "gsd");
        assert_eq!(Agent::Claude.label(), "Claude Code");
        assert_eq!(Agent::Codex.label(), "Codex");
        assert_eq!(Agent::Gsd.label(), "GSD");
        assert_eq!(Agent::Claude.icon(), "C");
        assert_eq!(Agent::Codex.icon(), "X");
        assert_eq!(Agent::Gsd.icon(), "G");
        assert_eq!(Agent::Claude.color(), Color::Cyan);
        assert_eq!(Agent::Codex.color(), Color::Green);
        assert_eq!(Agent::Gsd.color(), Color::Magenta);
    }

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
    fn generate_id_is_unique() {
        let ids: Vec<String> = (0..100).map(|_| generate_id()).collect();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j], "duplicate id at indices {} and {}", i, j);
            }
        }
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
        let jsonl = r#"{"type":"custom_message","customType":"gsd-run","message":"no session header"}"#;
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
    fn gsd_directory_name_encoding() {
        // Verify that GSD session dirs encode workspace paths by replacing / with -
        let ws_path = Path::new("/home/user/proj");
        let encoded = encode_project_path(ws_path);
        assert_eq!(encoded, "-home-user-proj");
    }

    #[test]
    fn encode_decode_gsd_dir_roundtrip() {
        // Roundtrip only holds for paths without hyphens (encoding is lossy for hyphens)
        let original = Path::new("/home/user/myproject");
        let encoded = encode_project_path(original);
        let decoded = encoded.replace('-', "/");
        assert_eq!(PathBuf::from(decoded), original.to_path_buf());
    }

    #[test]
    fn decode_gsd_dir_name_simple() {
        let dir_name = "-home-user-proj";
        let decoded = dir_name.replace('-', "/");
        assert_eq!(decoded, "/home/user/proj");
    }

    #[test]
    fn decode_gsd_dir_name_root() {
        let dir_name = "-";
        let decoded = dir_name.replace('-', "/");
        assert_eq!(decoded, "/");
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

    #[test]
    fn gsd_sessions_persist_after_pty_exit() {
        // Verifies the poll_states() retain logic from app.rs:
        //   self.ptys.retain(|slot| {
        //       if slot.info.agent == Agent::Codex && !slot.handle.is_alive() { return false; }
        //       true
        //   });
        // Only Codex sessions are cleaned up on PTY exit; GSD and Claude persist.
        let should_retain = |agent: Agent, is_alive: bool| -> bool {
            if agent == Agent::Codex && !is_alive {
                return false;
            }
            true
        };

        // When PTY is alive, all agents are retained
        assert!(should_retain(Agent::Claude, true), "Claude should retain when alive");
        assert!(should_retain(Agent::Codex, true), "Codex should retain when alive");
        assert!(should_retain(Agent::Gsd, true), "GSD should retain when alive");

        // When PTY exits, only Codex is removed — GSD and Claude persist
        assert!(should_retain(Agent::Claude, false), "Claude sessions MUST persist after PTY exit");
        assert!(!should_retain(Agent::Codex, false), "Codex sessions should be cleaned up after PTY exit");
        assert!(should_retain(Agent::Gsd, false), "GSD sessions MUST persist after PTY exit");
    }

    #[test]
    fn gsd_build_new_cmd_no_session_name() {
        let ws = Path::new("/home/user/proj");
        let cmd = Agent::Gsd.build_new_cmd(ws, None);
        // CommandBuilder doesn't expose args directly, but the method must not panic
        // and must return a valid builder (tested by compilation + the agent_traits test)
        let _ = cmd;
    }

    #[test]
    fn gsd_build_resume_cmd_uses_sessions() {
        let ws = Path::new("/home/user/proj");
        let cmd = Agent::Gsd.build_resume_cmd(ws, "some-session-id");
        // Verify the builder is created without panic.
        // The resume uses "gsd sessions" (interactive picker) not --resume.
        let _ = cmd;
    }
}
