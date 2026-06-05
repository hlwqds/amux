use std::{
    env, fs, io,
    os::unix::io::AsRawFd,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result};

use crate::types::{Config, ProjectConfig};
use crate::util::now_secs;

/// Return the amux data directory, respecting `$XDG_DATA_HOME`.
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

/// Path to the global `config.json` file.
pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

/// Ensure the data directory and sessions subdirectory exist.
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

/// Load the global config, overlaying any `config.d/*.json` drop-ins.
pub fn load_config() -> Result<Config> {
    let path = config_path();
    let mut config: Config = if path.exists() {
        let content = fs::read_to_string(&path).context("failed to read config.json")?;
        serde_json::from_str(&content).context("failed to parse config.json")?
    } else {
        Config {
            workspaces: Vec::new(),
            ..Default::default()
        }
    };

    let config_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    apply_config_overlays(&mut config, config_dir);

    Ok(config)
}

/// Overlay config.d/*.json drop-in files on top of the base config.
fn apply_config_overlays(config: &mut Config, config_dir: &Path) {
    let config_d = config_dir.join("config.d");
    if !config_d.is_dir() {
        return;
    }

    let mut entries: Vec<_> = match fs::read_dir(&config_d) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(e) => {
            tracing::warn!("Failed to read config.d: {e}");
            return;
        }
    };
    entries.retain(|e| e.path().extension().is_some_and(|ext| ext == "json"));
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let overlay_path = entry.path();
        match fs::read_to_string(&overlay_path) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(value) => {
                    if let Ok(overlay) = serde_json::from_value::<Config>(value) {
                        merge_config(config, &overlay);
                        tracing::info!("Loaded config overlay: {}", overlay_path.display());
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to parse {}: {e}", overlay_path.display());
                }
            },
            Err(e) => {
                tracing::warn!("Failed to read {}: {e}", overlay_path.display());
            }
        }
    }
}

/// Merge overlay config into base. Only replaces non-default fields.
fn merge_config(base: &mut Config, overlay: &Config) {
    if !overlay.workspaces.is_empty() {
        base.workspaces = overlay.workspaces.clone();
    }
    if !overlay.chains.is_empty() {
        base.chains = overlay.chains.clone();
    }
    if !overlay.templates.is_empty() {
        base.templates = overlay.templates.clone();
    }
    if !overlay.automations.is_empty() {
        base.automations = overlay.automations.clone();
    }
    if !overlay.plugins.is_empty() {
        base.plugins = overlay.plugins.clone();
    }
    if !overlay.remote_hosts.is_empty() {
        base.remote_hosts = overlay.remote_hosts.clone();
    }
    if !overlay.unset_env.is_empty() {
        base.unset_env = overlay.unset_env.clone();
    }
    if let Some(ref token) = overlay.serve_token {
        base.serve_token = Some(token.clone());
    }
    if let Some(ref budget) = overlay.token_budget {
        base.token_budget = Some(budget.clone());
    }
    if let Some(ref cmd) = overlay.check_command {
        base.check_command = Some(cmd.clone());
    }
}

/// Load per-project configuration from `.amux.json` in the workspace root.
/// Returns a default (empty) config if the file doesn't exist or can't be parsed.
pub fn load_project_config(workspace_path: &Path) -> ProjectConfig {
    let config_path = workspace_path.join(".amux.json");
    if !config_path.exists() {
        return ProjectConfig::default();
    }
    match fs::read_to_string(&config_path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!(
                    "Failed to parse {}: {e} — using defaults",
                    config_path.display()
                );
                ProjectConfig::default()
            }
        },
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

/// Generate a unique workspace ID from the current timestamp and a counter.
pub fn generate_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("ws-{}-{}", now_secs(), count)
}

/// Encode a filesystem path by replacing `/` with `-`.
pub fn encode_project_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    s.replace('/', "-")
}

/// Return default workspace root directories (siblings of the current working directory).
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

/// Path to the title override file for a session.
pub fn title_override_path(session_id: &str) -> PathBuf {
    data_dir()
        .join("sessions")
        .join(format!("{session_id}.title"))
}

/// Path to the legacy (pre-v0.3) title override file inside `.claude/`.
pub fn legacy_title_override_path(workspace_path: &Path, session_id: &str) -> PathBuf {
    let encoded = workspace_path.to_string_lossy().replace('/', "-");
    workspace_path
        .join(".claude")
        .join(format!("{encoded}-{session_id}.title"))
}

/// Save the session title, preserving existing tags/note/pinned state.
pub fn save_session_title(session_id: &str, title: &str) -> io::Result<()> {
    let existing = load_session_meta(session_id, None);
    let (tags, note, pinned) = match existing {
        Some(m) => (m.tags, m.note, m.pinned),
        None => (Vec::new(), None, false),
    };
    save_session_meta(session_id, title, &tags, note.as_deref(), pinned)
}

/// Persist full session metadata (title, tags, note, pinned) to the override file.
pub fn save_session_meta(
    session_id: &str,
    title: &str,
    tags: &[String],
    note: Option<&str>,
    pinned: bool,
) -> io::Result<()> {
    let path = title_override_path(session_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if tags.is_empty() && note.is_none_or(|s| s.is_empty()) && !pinned {
        fs::write(path, title)
    } else {
        let mut meta = serde_json::json!({"title": title});
        if !tags.is_empty() {
            meta["tags"] = serde_json::json!(tags);
        }
        if let Some(n) = note
            && !n.is_empty()
        {
            meta["note"] = serde_json::json!(n);
        }
        if pinned {
            meta["pinned"] = serde_json::json!(true);
        }
        fs::write(path, meta.to_string())
    }
}

/// Save only the note for a session, preserving existing title/tags.
pub fn save_session_note(session_id: &str, note: &str) -> io::Result<()> {
    let existing = load_session_meta(session_id, None);
    let (title, tags, pinned) = match existing {
        Some(m) => (m.title, m.tags, m.pinned),
        None => (session_id.to_string(), Vec::new(), false),
    };
    save_session_meta(session_id, &title, &tags, Some(note), pinned)
}

/// Toggle the pinned state for a session, preserving existing title/tags/note.
pub fn save_session_pinned(session_id: &str, pinned: bool) -> io::Result<()> {
    let existing = load_session_meta(session_id, None);
    let (title, tags, note) = match existing {
        Some(m) => (m.title, m.tags, m.note),
        None => (session_id.to_string(), Vec::new(), None),
    };
    save_session_meta(session_id, &title, &tags, note.as_deref(), pinned)
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
    pub note: Option<String>,
    pub pinned: bool,
}

pub fn load_session_meta(session_id: &str, workspace_path: Option<&Path>) -> Option<SessionMeta> {
    let path = title_override_path(session_id);
    if let Ok(raw) = fs::read_to_string(&path) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            // Try JSON format first
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(trimmed) {
                let title = match obj.get("title").and_then(|v| v.as_str()) {
                    Some(t) if !t.is_empty() => t.to_string(),
                    _ => {
                        // JSON exists but no valid title — let discovery extract from JSONL
                        return None;
                    }
                };
                let tags = obj
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let note = obj.get("note").and_then(|v| v.as_str()).map(String::from);
                let pinned = obj.get("pinned").and_then(|v| v.as_bool()).unwrap_or(false);
                return Some(SessionMeta {
                    title,
                    tags,
                    note,
                    pinned,
                });
            }
            // Fallback: plain text (backward compat)
            return Some(SessionMeta {
                title: trimmed.to_string(),
                tags: Vec::new(),
                note: None,
                pinned: false,
            });
        }
    }

    // Legacy path
    if let Some(wp) = workspace_path {
        let legacy = legacy_title_override_path(wp, session_id);
        if let Ok(raw) = fs::read_to_string(&legacy) {
            let title = raw.trim().to_string();
            if !title.is_empty() {
                return Some(SessionMeta {
                    title,
                    tags: Vec::new(),
                    note: None,
                    pinned: false,
                });
            }
        }
    }

    None
}

/// Load just the session title, if a title override exists.
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
