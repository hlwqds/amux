use std::{
    env, fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result};

use crate::types::Config;
use crate::util::now_secs;

pub fn data_dir() -> PathBuf {
    let xdg = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var("HOME").unwrap_or_default()).join(".local/share")
        });
    xdg.join("amux")
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
        });
    }
    let content = fs::read_to_string(&path).context("failed to read config.json")?;
    let config: Config = serde_json::from_str(&content).context("failed to parse config.json")?;
    Ok(config)
}

pub fn save_config_file(config: &Config) -> Result<()> {
    ensure_data_dir().context("failed to create data directory")?;
    let path = config_path();
    let content = serde_json::to_string_pretty(config).context("failed to serialize config")?;
    fs::write(path, content).context("failed to write config.json")?;
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
    data_dir().join("sessions").join(format!("{session_id}.title"))
}

pub fn legacy_title_override_path(workspace_path: &Path, session_id: &str) -> PathBuf {
    let encoded = workspace_path.to_string_lossy().replace('/', "-");
    workspace_path
        .join(".claude")
        .join(format!("{encoded}-{session_id}.title"))
}

pub fn save_session_title(session_id: &str, title: &str) -> io::Result<()> {
    let path = title_override_path(session_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, title)
}

pub fn load_session_title(session_id: &str, workspace_path: Option<&Path>) -> Option<String> {
    let path = title_override_path(session_id);
    if let Ok(title) = fs::read_to_string(&path) {
        let title = title.trim().to_string();
        if !title.is_empty() {
            return Some(title);
        }
    }

    if let Some(wp) = workspace_path {
        let legacy = legacy_title_override_path(wp, session_id);
        if let Ok(title) = fs::read_to_string(&legacy) {
            let title = title.trim().to_string();
            if !title.is_empty() {
                return Some(title);
            }
        }
    }

    None
}
