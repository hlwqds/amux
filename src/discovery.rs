use std::{
    env, fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::config::{data_dir, encode_project_path, load_session_meta, load_session_title};
use crate::types::{Agent, Session, Workspace};
use crate::util::now_secs;

/// Detected project build system type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProjectType {
    Rust,
    Node,
    Python,
    Go,
    Make,
    #[default]
    Unknown,
}
impl ProjectType {
    /// Detect project type from marker files in the given directory.
    pub fn detect(path: &Path) -> Self {
        if path.join("Cargo.toml").exists() {
            return Self::Rust;
        }
        if path.join("package.json").exists() {
            return Self::Node;
        }
        if path.join("pyproject.toml").exists() || path.join("setup.py").exists() {
            return Self::Python;
        }
        if path.join("go.mod").exists() {
            return Self::Go;
        }
        if path.join("Makefile").exists() {
            return Self::Make;
        }
        Self::Unknown
    }
    /// Return the check commands for this project type.
    /// Each entry is (program, args).
    pub fn check_commands(&self) -> Vec<(&'static str, Vec<&'static str>)> {
        match self {
            Self::Rust => vec![
                ("cargo", vec!["test", "--quiet"]),
                ("cargo", vec!["clippy", "--quiet"]),
            ],
            Self::Node => vec![("npm", vec!["test"])],
            Self::Python => vec![("pytest", vec!["-q"])],
            Self::Go => vec![("go", vec!["test", "./..."])],
            Self::Make => vec![("make", vec!["test"])],
            Self::Unknown => vec![],
        }
    }
    /// A short unicode icon for the project type.
    pub const fn icon(&self) -> &'static str {
        match self {
            Self::Rust => "\u{e7a8}",    //
            Self::Node => "\u{2b21}",    // ⬡
            Self::Python => "\u{1f40d}", // 🐍
            Self::Go => "\u{1f535}",     // 🔵
            Self::Make => "\u{2699}",    // ⚙
            Self::Unknown => "",
        }
    }
}

/// Scan filesystem roots for git repositories and return them as workspaces.
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

/// Cache for incremental session discovery — maps file path to (mtime, Session).
pub type SessionCache = std::collections::HashMap<PathBuf, (SystemTime, Session)>;

/// Discover all sessions across the given workspaces (no caching).
pub fn discover_sessions(workspaces: &[Workspace]) -> Vec<Session> {
    discover_sessions_cached(workspaces, &mut SessionCache::new())
}

/// Discover sessions with mtime-based caching. Skips re-parsing files whose mtime
/// hasn't changed since the last scan. Updates `cache` in place.
pub fn discover_sessions_cached(
    workspaces: &[Workspace],
    cache: &mut SessionCache,
) -> Vec<Session> {
    let mut jsonl_files: Vec<PathBuf> = Vec::new();
    collect_claude_jsonl(workspaces, &mut jsonl_files);
    collect_codex_jsonl(workspaces, &mut jsonl_files);
    collect_omp_jsonl(workspaces, &mut jsonl_files);
    let jsonl_set: std::collections::HashSet<_> = jsonl_files.iter().cloned().collect();
    cache.retain(|path, _| jsonl_set.contains(path));

    let mut sessions: Vec<Session> = Vec::with_capacity(jsonl_files.len());
    use rayon::prelude::*;

    let parsed: Vec<(PathBuf, Option<SystemTime>, Option<Session>)> = jsonl_files
        .par_iter()
        .map(|path| {
            let mtime = fs::metadata(path).ok().and_then(|m| m.modified().ok());
            if let Some(mt) = mtime
                && let Some((cached_mt, cached_session)) = cache.get(path)
                && *cached_mt == mt
            {
                return (path.clone(), mtime, Some(cached_session.clone()));
            }
            let session = parse_session_from_path(path, workspaces);
            (path.clone(), mtime, session)
        })
        .collect();

    for (path, mtime, session) in parsed {
        if let Some(session) = session {
            if let Some(mt) = mtime {
                cache.insert(path, (mt, session.clone()));
            }
            sessions.push(session);
        }
    }
    // Reload title/pinned/tags from override files (may have changed since cache)
    for session in &mut sessions {
        if let Some(meta) = load_session_meta(&session.id, Some(&session.workspace_path)) {
            if !meta.tags.is_empty() {
                session.tags = meta.tags;
            }
            session.title = meta.title;
            session.pinned = meta.pinned;
        }
    }
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
        Agent::Omp => {
            let sessions_root = Agent::Omp.sessions_dir()?;
            walk_omp_jsonl(&sessions_root, &session.id)
        }
    }
}

fn walk_codex_jsonl(root: &Path, session_id: &str) -> Option<PathBuf> {
    // Codex sessions: <year>/<month>/<day>/<timestamp>_<sessionId>.jsonl
    // Match by checking if the file stem ends with _<sessionId> or equals sessionId
    let suffix = format!("_{session_id}");
    let exact = format!("{session_id}.jsonl");
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
                                    let name = file.file_name();
                                    let name_str = name.to_string_lossy();
                                    if name_str == exact.as_str() {
                                        return Some(file.path());
                                    }
                                    if name_str.ends_with(&suffix) && name_str.ends_with(".jsonl") {
                                        return Some(file.path());
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

fn walk_omp_jsonl(root: &Path, session_id: &str) -> Option<PathBuf> {
    // OMP sessions: --<cwd-encoded>--/<timestamp>_<sessionId>.jsonl
    if let Ok(subdirs) = fs::read_dir(root) {
        for subdir in subdirs.flatten() {
            if !subdir.path().is_dir() {
                continue;
            }
            // Try direct filename match first: <timestamp>_<sessionId>.jsonl
            let expected = format!("{session_id}.jsonl");
            if let Ok(files) = fs::read_dir(subdir.path()) {
                for file in files.flatten() {
                    if file.file_name() == expected.as_str() {
                        return Some(file.path());
                    }
                }
            }
        }
    }
    None
}

/// Collect all Claude JSONL file paths from workspace project directories.
fn collect_claude_jsonl(workspaces: &[Workspace], out: &mut Vec<PathBuf>) {
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
        if let Ok(entries) = fs::read_dir(&proj_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    out.push(path);
                }
            }
        }
    }
}

/// Collect all Codex JSONL file paths from year/month/day subdirectories.
fn collect_codex_jsonl(_workspaces: &[Workspace], out: &mut Vec<PathBuf>) {
    let sessions_root = match Agent::Codex.sessions_dir() {
        Some(d) => d,
        None => return,
    };
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
                                    if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                                        out.push(path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Collect all OMP JSONL file paths from session subdirectories.
fn collect_omp_jsonl(_workspaces: &[Workspace], out: &mut Vec<PathBuf>) {
    let sessions_root = match Agent::Omp.sessions_dir() {
        Some(d) => d,
        None => return,
    };
    if let Ok(subdirs) = fs::read_dir(&sessions_root) {
        for subdir in subdirs.flatten() {
            if !subdir.path().is_dir() {
                continue;
            }
            if let Ok(files) = fs::read_dir(subdir.path()) {
                for file in files.flatten() {
                    let path = file.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                        out.push(path);
                    }
                }
            }
        }
    }
}

/// Parse a session from a JSONL file path, determining the agent type from the path structure.
fn parse_session_from_path(path: &Path, workspaces: &[Workspace]) -> Option<Session> {
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

    // Determine agent type from the path
    let claude_dir = Agent::Claude.sessions_dir();
    let omp_dir = Agent::Omp.sessions_dir();
    let codex_dir = Agent::Codex.sessions_dir();

    let is_claude = claude_dir.as_ref().is_some_and(|d| path.starts_with(d));
    let is_omp = omp_dir.as_ref().is_some_and(|d| path.starts_with(d));
    let is_codex = codex_dir.as_ref().is_some_and(|d| path.starts_with(d));

    let last_active = fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if is_claude {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();
        // Determine workspace from the path structure: projects_dir / encoded / id.jsonl
        let ws_path = claude_dir
            .as_ref()
            .and_then(|projects_dir| {
                path.parent().and_then(|parent| {
                    parent.parent().and_then(|_| {
                        // Reconstruct ws_path by checking all workspaces
                        ws_paths
                            .iter()
                            .find(|ws| {
                                let encoded = encode_project_path(ws);
                                *parent == projects_dir.join(encoded)
                            })
                            .cloned()
                    })
                })
            })
            .unwrap_or_else(|| ws_paths.first().cloned().unwrap_or_default());

        let title = load_session_title(&id, Some(&ws_path))
            .or_else(|| extract_claude_title(path))
            .unwrap_or_else(|| format!("Session {}", &id[..8.min(id.len())]));

        let pinned = crate::config::load_session_meta(&id, Some(&ws_path))
            .map(|m| m.pinned)
            .unwrap_or(false);
        Some(Session {
            id,
            workspace_path: ws_path,
            title,
            last_active,
            agent: Agent::Claude,
            tags: Vec::new(),
            pinned,
            last_message: extract_last_user_message(path),
        })
    } else if is_omp {
        let (id, title, cwd) = parse_gsd_session(path)?;
        let ws_path = match cwd {
            Some(ref cwd_str) => ws_paths
                .iter()
                .find(|p| cwd_str == p.to_string_lossy().as_ref())
                .cloned()
                .unwrap_or_else(|| PathBuf::from(cwd_str)),
            None => return None,
        };
        let pinned = crate::config::load_session_meta(&id, Some(&ws_path))
            .map(|m| m.pinned)
            .unwrap_or(false);
        Some(Session {
            id,
            workspace_path: ws_path,
            title: title.unwrap_or_else(|| "OMP session".into()),
            last_active,
            agent: Agent::Omp,
            tags: Vec::new(),
            pinned,
            last_message: extract_last_user_message(path),
        })
    } else if is_codex {
        let (id, title, cwd) = parse_codex_session(path)?;
        let ws_path = ws_paths
            .iter()
            .find(|p| cwd == p.to_string_lossy().as_ref())
            .cloned()
            .unwrap_or_else(|| ws_paths.first().cloned().unwrap_or_default());
        let pinned = crate::config::load_session_meta(&id, Some(&ws_path))
            .map(|m| m.pinned)
            .unwrap_or(false);
        Some(Session {
            id,
            workspace_path: ws_path,
            title: title.unwrap_or_else(|| "Codex session".into()),
            last_active,
            agent: Agent::Codex,
            tags: Vec::new(),
            pinned,
            last_message: extract_last_user_message(path),
        })
    } else {
        None
    }
}

pub use crate::extraction::*;

#[cfg(test)]
mod tests {
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
        let jsonl = format!("{session_line}\n{msg_line}");
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
        let jsonl = format!("{session_line}\n{msg_line}");
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
    fn test_session_cache_retain_evicts_stale() {
        // Directly test the cache retain behavior that discover_sessions_cached relies on.
        let mut cache = SessionCache::new();

        let keep = std::path::PathBuf::from("/keep/this.jsonl");
        let evict = std::path::PathBuf::from("/evict/this.jsonl");
        let session = crate::types::Session {
            id: "s".into(),
            workspace_path: std::path::PathBuf::from("/ws"),
            title: "T".into(),
            last_active: 0,
            agent: crate::types::Agent::Claude,
            tags: vec![],
            pinned: false,
            last_message: None,
        };
        let t = std::time::SystemTime::UNIX_EPOCH;
        cache.insert(keep.clone(), (t, session.clone()));
        cache.insert(evict.clone(), (t, session));
        assert_eq!(cache.len(), 2);

        // Retain only entries whose path is in the jsonl set
        let jsonl_set: std::collections::HashSet<_> = [keep.clone()].into_iter().collect();
        cache.retain(|path, _| jsonl_set.contains(path));

        assert_eq!(cache.len(), 1, "only the kept path should survive");
        assert!(cache.contains_key(&keep));
        assert!(!cache.contains_key(&evict));
    }
}
