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
}
