use std::{
    env, fs, io,
    os::unix::io::AsRawFd,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result};

use crate::types::{Config, ProjectConfig, Workspace};
use crate::util::now_secs;

/// Current config schema version. Increment when making breaking changes.
pub const CONFIG_VERSION: u32 = 1;

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

/// Try to parse config.json leniently by extracting known fields individually.
/// This handles cases where new fields were added but the serde struct hasn't
/// been updated yet, or where unknown fields cause strict parsing to fail.
fn lenient_config_parse(content: &str) -> Option<Config> {
    let value: serde_json::Value = serde_json::from_str(content).ok()?;
    // Try to parse the full struct with `deny_unknown_fields` disabled
    // (it's already disabled by default, but this is the safe path)
    if let Ok(config) = serde_json::from_value::<Config>(value.clone()) {
        return Some(config);
    }
    // If that still fails, try field-by-field extraction
    let mut config = Config::default();
    if let Some(ws) = value.get("workspaces")
        && let Ok(parsed) = serde_json::from_value::<Vec<Workspace>>(ws.clone())
    {
        config.workspaces = parsed;
    }
    if let Some(theme) = value.get("theme")
        && let Ok(parsed) = serde_json::from_value(theme.clone())
    {
        config.theme = parsed;
    }
    if let Some(keybinds) = value.get("keybinds")
        && let Ok(parsed) = serde_json::from_value(keybinds.clone())
    {
        config.keybinds = parsed;
    }
    if let Some(templates) = value.get("templates")
        && let Ok(parsed) = serde_json::from_value(templates.clone())
    {
        config.templates = parsed;
    }
    if let Some(automations) = value.get("automations")
        && let Ok(parsed) = serde_json::from_value(automations.clone())
    {
        config.automations = parsed;
    }
    if let Some(archive_days) = value.get("archive_days")
        && let Ok(parsed) = serde_json::from_value(archive_days.clone())
    {
        config.archive_days = parsed;
    }
    if let Some(remote_hosts) = value.get("remote_hosts")
        && let Ok(parsed) = serde_json::from_value(remote_hosts.clone())
    {
        config.remote_hosts = parsed;
    }
    if let Some(plugins) = value.get("plugins")
        && let Ok(parsed) = serde_json::from_value(plugins.clone())
    {
        config.plugins = parsed;
    }
    if let Some(serve_port) = value.get("serve_port")
        && let Ok(parsed) = serde_json::from_value(serve_port.clone())
    {
        config.serve_port = parsed;
    }
    if let Some(serve_token) = value.get("serve_token")
        && let Ok(parsed) = serde_json::from_value(serve_token.clone())
    {
        config.serve_token = parsed;
    }
    if let Some(check_command) = value.get("check_command")
        && let Ok(parsed) = serde_json::from_value(check_command.clone())
    {
        config.check_command = parsed;
    }
    if let Some(token_budget) = value.get("token_budget")
        && let Ok(parsed) = serde_json::from_value(token_budget.clone())
    {
        config.token_budget = parsed;
    }
    if let Some(chains) = value.get("chains")
        && let Ok(parsed) = serde_json::from_value(chains.clone())
    {
        config.chains = parsed;
    }
    if let Some(unset_env) = value.get("unset_env")
        && let Ok(parsed) = serde_json::from_value(unset_env.clone())
    {
        config.unset_env = parsed;
    }
    if let Some(recent_expanded) = value.get("recent_expanded")
        && let Ok(parsed) = serde_json::from_value(recent_expanded.clone())
    {
        config.recent_expanded = parsed;
    }
    if let Some(pinned_expanded) = value.get("pinned_expanded")
        && let Ok(parsed) = serde_json::from_value(pinned_expanded.clone())
    {
        config.pinned_expanded = parsed;
    }
    tracing::warn!(
        "Lenient parse succeeded — recovered {} workspaces",
        config.workspaces.len()
    );
    Some(config)
}

/// Load the global config with lenient parsing and config.d overlay support.
/// Uses lenient parsing: unknown fields are ignored, missing fields use defaults.
/// Returns a default config only if the file doesn't exist.
/// **Never overwrites the file on disk due to a parse error.**
pub fn load_config() -> Result<Config> {
    let path = config_path();
    let mut config: Config = if path.exists() {
        let content = fs::read_to_string(&path).context("failed to read config.json")?;
        match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                // Parse error — try lenient fallback: parse what we can
                tracing::error!("Failed to parse config.json: {e}");
                tracing::error!("Attempting lenient parse to preserve user data...");
                lenient_config_parse(&content).unwrap_or_else(|| {
                    tracing::error!(
                        "Lenient parse also failed — using defaults but NOT overwriting file"
                    );
                    Config {
                        workspaces: Vec::new(),
                        ..Default::default()
                    }
                })
            }
        }
    } else {
        Config {
            workspaces: Vec::new(),
            ..Default::default()
        }
    };

    let config_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    apply_config_overlays(&mut config, config_dir);
    migrate_config(&mut config);

    Ok(config)
}
/// Apply config migrations to bring an older config up to the current schema version.
fn migrate_config(config: &mut Config) {
    let version = config.config_version;
    if version < 1 {
        // Migration: ensure all workspaces have session_ids field
        for _ws in &mut config.workspaces {}
    }
    config.config_version = CONFIG_VERSION;
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
        apply_single_overlay(config, &entry.path());
    }
}

/// Try to load and merge a single config.d overlay file.
fn apply_single_overlay(config: &mut Config, path: &Path) {
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        tracing::warn!("Failed to parse {}", path.display());
        return;
    };
    match serde_json::from_value::<Config>(value) {
        Ok(overlay) => {
            merge_config(config, &overlay);
            tracing::info!("Loaded config overlay: {}", path.display());
        }
        Err(e) => tracing::warn!("Invalid overlay {}: {e}", path.display()),
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
    fs::read_to_string(&config_path).map_or_else(
        |_| ProjectConfig::default(),
        |content| match serde_json::from_str(&content) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!(
                    "Failed to parse {}: {e} — using defaults",
                    config_path.display()
                );
                ProjectConfig::default()
            }
        },
    )
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
                assert_ne!(ids[i], ids[j], "duplicate id at indices {i} and {j}");
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

// --- Comprehensive config tests (second module to keep original untouched) ---
#[cfg(test)]
mod config_comprehensive_tests {
    use super::*;
    use crate::types::*;

    /// Helper: write a JSON string to a file and return its parent dir (as TempDir stand-in).
    struct TempConfig {
        dir: PathBuf,
    }

    impl TempConfig {
        fn new(prefix: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("amux_test_{prefix}_{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self { dir }
        }

        fn config_path(&self) -> PathBuf {
            self.dir.join("config.json")
        }
    }

    impl Drop for TempConfig {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn test_save_load_roundtrip() {
        let tmp = TempConfig::new("roundtrip");
        let config_path = tmp.config_path();

        // Build a non-trivial Config
        let ws = Workspace {
            id: "ws-001".into(),
            name: "my-project".into(),
            path: Some(PathBuf::from("/home/user/my-project")),
            created_at: 1717000000,
            session_ids: vec!["sess-a".into(), "sess-b".into()],
            expanded: true,
        };
        let keybinds = Keybinds {
            quit: KeyBinding::ctrl("q"),
            ..Default::default()
        };

        let original = Config {
            config_version: super::CONFIG_VERSION,
            workspaces: vec![ws],
            theme: crate::theme::ThemeName::Mocha,
            keybinds: keybinds.clone(),
            templates: vec![],
            automations: vec![],
            archive_days: Some(30),
            remote_hosts: vec![],
            plugins: vec![],
            serve_port: Some(9999),
            serve_token: Some("secret-token-123".into()),
            check_command: Some("cargo test --all".into()),
            token_budget: None,
            chains: vec![],
            unset_env: vec!["KITTY_WINDOW_ID".into(), "WEZTERM_PANE".into()],
            recent_expanded: true,
            pinned_expanded: true,
        };

        // Write via save_config_file logic (manual since save_config_file writes to data_dir).
        let json = serde_json::to_string_pretty(&original).unwrap();
        std::fs::write(&config_path, &json).unwrap();

        // Load back via serde_json
        let content = std::fs::read_to_string(&config_path).unwrap();
        let loaded: Config = serde_json::from_str(&content).unwrap();

        // Assert every field
        assert_eq!(loaded.workspaces.len(), 1);
        let lws = &loaded.workspaces[0];
        assert_eq!(lws.id, "ws-001");
        assert_eq!(lws.name, "my-project");
        assert_eq!(lws.path, Some(PathBuf::from("/home/user/my-project")));
        assert_eq!(lws.created_at, 1717000000);
        assert_eq!(lws.session_ids, vec!["sess-a", "sess-b"]);
        assert!(lws.expanded);

        assert_eq!(loaded.theme, crate::theme::ThemeName::Mocha);
        assert_eq!(loaded.keybinds.quit, keybinds.quit);
        assert_eq!(loaded.archive_days, Some(30));
        assert_eq!(loaded.serve_port, Some(9999));
        assert_eq!(loaded.serve_token, Some("secret-token-123".into()));
        assert_eq!(loaded.check_command, Some("cargo test --all".into()));
        assert_eq!(loaded.unset_env, vec!["KITTY_WINDOW_ID", "WEZTERM_PANE"]);
        assert!(loaded.recent_expanded);
        assert!(loaded.pinned_expanded);
    }

    #[test]
    fn test_forward_compat_unknown_fields() {
        let json = r#"{
            "workspaces": [
                {"id":"w1","name":"alpha","path":"/tmp/a","created_at":100,"session_ids":[],"expanded":false}
            ],
            "theme": "Light",
            "future_field": "some value",
            "new_section": {"a": 1, "b": [true, false]},
            "another_unknown": 42,
            "unset_env": ["FOO"],
            "recent_expanded": false,
            "pinned_expanded": true
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();

        assert_eq!(config.workspaces.len(), 1);
        assert_eq!(config.workspaces[0].name, "alpha");
        assert_eq!(config.theme, crate::theme::ThemeName::Light);
        assert_eq!(config.unset_env, vec!["FOO"]);
        assert!(!config.recent_expanded);
        assert!(config.pinned_expanded);
    }

    #[test]
    fn test_forward_compat_missing_new_fields() {
        let json = r#"{"workspaces": []}"#;
        let config: Config = serde_json::from_str(json).unwrap();

        // All new fields should default correctly
        assert!(config.workspaces.is_empty());
        assert_eq!(config.theme, crate::theme::ThemeName::default());
        assert_eq!(config.keybinds, Keybinds::default());
        assert!(config.templates.is_empty());
        assert!(config.automations.is_empty());
        assert!(config.archive_days.is_none());
        assert!(config.remote_hosts.is_empty());
        assert!(config.plugins.is_empty());
        assert!(config.serve_port.is_none());
        assert!(config.serve_token.is_none());
        assert!(config.check_command.is_none());
        assert!(config.unset_env.is_empty());
        assert!(!config.recent_expanded);
        assert!(!config.pinned_expanded);
    }

    #[test]
    fn test_lenient_parse_recovers_workspaces() {
        // "theme": 123 is an invalid type — serde would reject full parsing,
        // but lenient_config_parse should still recover the workspaces.
        let json = r#"{
            "workspaces": [
                {"id":"w1","name":"beta","path":"/tmp/b","created_at":200,"session_ids":["s1"],"expanded":true}
            ],
            "theme": 123,
            "keybinds": "not_an_object"
        }"#;

        let config = lenient_config_parse(json).expect("lenient parse should succeed");

        assert_eq!(config.workspaces.len(), 1);
        assert_eq!(config.workspaces[0].name, "beta");
        assert_eq!(config.workspaces[0].session_ids, vec!["s1"]);
        assert!(config.workspaces[0].expanded);
        // theme and keybinds fall back to defaults since they were invalid
        assert_eq!(config.theme, crate::theme::ThemeName::default());
        assert_eq!(config.keybinds, Keybinds::default());
    }

    #[test]
    fn test_workspace_session_ids_persist() {
        let ws = Workspace {
            id: "ws-sid".into(),
            name: "session-test".into(),
            path: Some(PathBuf::from("/home/user/sess-test")),
            created_at: 1718000000,
            session_ids: vec!["sess-001".into(), "sess-002".into(), "sess-003".into()],
            expanded: false,
        };

        let json = serde_json::to_string(&ws).unwrap();
        let loaded: Workspace = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.session_ids, ws.session_ids);
        assert_eq!(loaded.session_ids.len(), 3);
        assert_eq!(loaded.session_ids[0], "sess-001");
        assert_eq!(loaded.session_ids[2], "sess-003");

        // Also verify within a full Config roundtrip
        let config = Config {
            workspaces: vec![ws],
            ..Config::default()
        };
        let config_json = serde_json::to_string(&config).unwrap();
        let config_loaded: Config = serde_json::from_str(&config_json).unwrap();
        assert_eq!(
            config_loaded.workspaces[0].session_ids,
            vec!["sess-001", "sess-002", "sess-003"]
        );
    }
}
