use std::{
    env, fs, io,
    os::unix::io::AsRawFd,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result};

use crate::types::{Config, ProjectConfig};
use crate::util::now_secs;

pub fn data_dir() -> PathBuf {
    if let Some(p) = env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        return p.join("amux");
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home).join("Library/Application Support/amux");
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let home = env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join(".local/share/amux")
    }
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn ensure_data_dir() -> io::Result<()> {
    let dir = data_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    let sessions_dir = dir.join("sessions");
    if !sessions_dir.exists() {
        fs::create_dir_all(&sessions_dir)?;
    }
    Ok(())
}

pub fn load_config() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config {
            workspaces: Vec::new(),
            theme: crate::theme::ThemeName::default(),
            keybinds: crate::types::Keybinds::default(),
            templates: Vec::new(),
            automations: Vec::new(),
            archive_days: None,
            remote_hosts: Vec::new(),
            plugins: Vec::new(),
            serve_port: None,
            serve_token: None,
            check_command: None,
            token_budget: None,
            chains: Vec::new(),
        });
    }
    let content = fs::read_to_string(&path).context("failed to read config.json")?;
    let config: Config = serde_json::from_str(&content).context("failed to parse config.json")?;
    Ok(config)
}

/// Load per-project configuration from `.amux.json` in the workspace root.
/// Returns a default (empty) config if the file doesn't exist or can't be parsed.
pub fn load_project_config(workspace_path: &Path) -> ProjectConfig {
    let config_path = workspace_path.join(".amux.json");
    if !config_path.exists() {
        return ProjectConfig::default();
    }
    match fs::read_to_string(&config_path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => ProjectConfig::default(),
    }
}

pub fn save_config_file(config: &Config) -> Result<()> {
    ensure_data_dir().context("failed to create data directory")?;
    let path = config_path();
    let content = serde_json::to_string_pretty(config).context("failed to serialize config")?;

    // File-based locking to prevent concurrent writes from multiple amux instances
    let lock_path = data_dir().join("config.lock");
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .context("failed to open lock file")?;

    // Try to acquire exclusive flock (non-blocking)
    #[cfg(unix)]
    {
        let fd = lock_file;
        let result = unsafe { libc::flock(fd.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result == -1 {
            // Couldn't acquire lock — another instance holds it. Wait briefly and retry.
            let _ = unsafe { libc::flock(fd.as_raw_fd(), libc::LOCK_EX) };
        }
        // Lock acquired, write config
        fs::write(&path, &content).context("failed to write config.json")?;
        // Lock released on drop
    }
    #[cfg(not(unix))]
    {
        let _ = lock_file;
        fs::write(&path, &content).context("failed to write config.json")?;
    }

    // Notify other instances that config changed
    let _ = fs::write(data_dir().join(".config-updated"), now_secs().to_string());

    Ok(())
}

pub fn generate_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("ws-{}-{}", now_secs(), count)
}

pub fn encode_project_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    s.replace('/', "-")
}

pub fn default_roots() -> Vec<PathBuf> {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let parent = cwd.parent().unwrap_or(&cwd);
    fs::read_dir(parent)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}

pub fn title_override_path(session_id: &str) -> PathBuf {
    data_dir()
        .join("sessions")
        .join(format!("{session_id}.title"))
}

pub fn legacy_title_override_path(workspace_path: &Path, session_id: &str) -> PathBuf {
    let encoded = workspace_path.to_string_lossy().replace('/', "-");
    workspace_path
        .join(".claude")
        .join(format!("{encoded}-{session_id}.title"))
}

pub fn save_session_title(session_id: &str, title: &str) -> io::Result<()> {
    let existing = load_session_meta(session_id, None);
    let (tags, rating, note) = match existing {
        Some(m) => (m.tags, m.rating, m.note),
        None => (Vec::new(), None, None),
    };
    save_session_meta(session_id, title, &tags, rating, note.as_deref())
}

pub fn save_session_meta(
    session_id: &str,
    title: &str,
    tags: &[String],
    rating: Option<u8>,
    note: Option<&str>,
) -> io::Result<()> {
    let path = title_override_path(session_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if tags.is_empty() && rating.is_none() && note.is_none_or(|s| s.is_empty()) {
        fs::write(path, title)
    } else {
        let mut meta = serde_json::json!({"title": title});
        if !tags.is_empty() {
            meta["tags"] = serde_json::json!(tags);
        }
        if let Some(r) = rating {
            meta["rating"] = serde_json::json!(r);
        }
        if let Some(n) = note
            && !n.is_empty()
        {
            meta["note"] = serde_json::json!(n);
        }
        fs::write(path, meta.to_string())
    }
}

/// Save only the rating for a session, preserving existing title/tags/note.
pub fn save_session_rating(session_id: &str, rating: u8) -> io::Result<()> {
    let existing = load_session_meta(session_id, None);
    let (title, tags, note) = match existing {
        Some(m) => (m.title, m.tags, m.note),
        None => (session_id.to_string(), Vec::new(), None),
    };
    save_session_meta(
        session_id,
        &title,
        &tags,
        Some(rating.clamp(1, 5)),
        note.as_deref(),
    )
}

/// Save only the note for a session, preserving existing title/tags/rating.
pub fn save_session_note(session_id: &str, note: &str) -> io::Result<()> {
    let existing = load_session_meta(session_id, None);
    let (title, tags, rating) = match existing {
        Some(m) => (m.title, m.tags, m.rating),
        None => (session_id.to_string(), Vec::new(), None),
    };
    save_session_meta(session_id, &title, &tags, rating, Some(note))
}

/// Save the snapshot commit hash to a standalone file for the session.
pub fn save_snapshot_meta(session_id: &str, snapshot_commit: &str) -> io::Result<()> {
    let dir = data_dir().join("snapshots");
    fs::create_dir_all(&dir)?;
    let short_id = &session_id[..session_id.len().min(16)];
    fs::write(dir.join(short_id), snapshot_commit)
}

/// Load the snapshot commit hash for a session (if saved).
pub fn load_snapshot_meta(session_id: &str) -> Option<String> {
    let short_id = &session_id[..session_id.len().min(16)];
    let path = data_dir().join("snapshots").join(short_id);
    fs::read_to_string(&path).ok().map(|s| s.trim().to_string())
}

/// Session metadata loaded from the title override file.
pub struct SessionMeta {
    pub title: String,
    pub tags: Vec<String>,
    pub rating: Option<u8>,
    pub note: Option<String>,
}

pub fn load_session_meta(session_id: &str, workspace_path: Option<&Path>) -> Option<SessionMeta> {
    let path = title_override_path(session_id);
    if let Ok(raw) = fs::read_to_string(&path) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            // Try JSON format first
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(trimmed) {
                let title = obj
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or(trimmed)
                    .to_string();
                let tags = obj
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let rating = obj.get("rating").and_then(|v| v.as_u64()).and_then(|r| {
                    if (1..=5).contains(&r) {
                        Some(r as u8)
                    } else {
                        None
                    }
                });
                let note = obj.get("note").and_then(|v| v.as_str()).map(String::from);
                return Some(SessionMeta {
                    title,
                    tags,
                    rating,
                    note,
                });
            }
            // Fallback: plain text (backward compat)
            return Some(SessionMeta {
                title: trimmed.to_string(),
                tags: Vec::new(),
                rating: None,
                note: None,
            });
        }
    }

    // Legacy path
    if let Some(wp) = workspace_path {
        let legacy = legacy_title_override_path(wp, session_id);
        if let Ok(title) = fs::read_to_string(&legacy) {
            let title = title.trim().to_string();
            if !title.is_empty() {
                return Some(SessionMeta {
                    title,
                    tags: Vec::new(),
                    rating: None,
                    note: None,
                });
            }
        }
    }

    None
}

pub fn load_session_title(session_id: &str, workspace_path: Option<&Path>) -> Option<String> {
    load_session_meta(session_id, workspace_path).map(|m| m.title)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

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
    fn generate_id_is_unique() {
        let ids: Vec<String> = (0..100).map(|_| generate_id()).collect();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j], "duplicate id at indices {} and {}", i, j);
            }
        }
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
    fn load_project_config_missing_file_returns_default() {
        let dir = std::env::temp_dir().join("amux_test_no_amux_json");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = super::load_project_config(&dir);
        assert!(config.default_agent.is_none());
        assert!(config.env.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_project_config_parses_valid_file() {
        let dir = std::env::temp_dir().join("amux_test_with_amux_json");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".amux.json"),
            r#"{"default_agent":"claude","env":[["FOO","bar"]]}"#,
        )
        .unwrap();
        let config = super::load_project_config(&dir);
        assert_eq!(config.default_agent, Some("claude".to_string()));
        assert_eq!(config.env, vec![("FOO".into(), "bar".into())]);
        // auto_inject_knowledge defaults to true when absent from JSON
        assert!(config.auto_inject_knowledge);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_project_config_invalid_json_returns_default() {
        let dir = std::env::temp_dir().join("amux_test_bad_amux_json");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".amux.json"), "not valid json{{{").unwrap();
        let config = super::load_project_config(&dir);
        assert!(config.default_agent.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
