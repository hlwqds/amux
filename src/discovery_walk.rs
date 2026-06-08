use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::config::{data_dir, encode_project_path};
use crate::types::{Agent, Session, Workspace};

use crate::discovery::parse_session_from_path;

/// Find a session JSONL file by session ID, searching all agent session directories.
pub(super) fn find_session_jsonl_by_id(session_id: &str, _ws_path: &Path) -> Option<PathBuf> {
    for agent in &[Agent::Claude, Agent::Codex, Agent::Omp] {
        let sessions_dir = agent.sessions_dir()?;
        match agent {
            Agent::Claude => {
                // Claude stores as <encoded-project-path>/<id>.jsonl
                // Scan all project subdirectories for the ID
                if let Some(p) = walk_claude_jsonl(&sessions_dir, session_id) {
                    return Some(p);
                }
            }
            Agent::Codex => {
                if let Some(p) = walk_codex_jsonl(&sessions_dir, session_id) {
                    return Some(p);
                }
            }
            Agent::Omp => {
                if let Some(p) = walk_omp_jsonl(&sessions_dir, session_id) {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Walk Claude project directories to find a JSONL by session ID.
pub(super) fn walk_claude_jsonl(root: &Path, session_id: &str) -> Option<PathBuf> {
    let expected = format!("{session_id}.jsonl");
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            if let Ok(files) = fs::read_dir(entry.path()) {
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

pub(super) fn walk_codex_jsonl(root: &Path, session_id: &str) -> Option<PathBuf> {
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

pub(super) fn walk_omp_jsonl(root: &Path, session_id: &str) -> Option<PathBuf> {
    // OMP sessions: --<cwd-encoded>--/<timestamp>_<sessionId>.jsonl
    if let Ok(subdirs) = fs::read_dir(root) {
        for subdir in subdirs.flatten() {
            if !subdir.path().is_dir() {
                continue;
            }
            // OMP files are named: <timestamp>_<sessionId>.jsonl
            let suffix = format!("_{session_id}.jsonl");
            if let Ok(files) = fs::read_dir(subdir.path()) {
                for file in files.flatten() {
                    let name = file.file_name();
                    let name_str = name.to_string_lossy();
                    // Match both <sessionId>.jsonl and <timestamp>_<sessionId>.jsonl
                    if name_str == format!("{session_id}.jsonl")
                        || (name_str.ends_with(&suffix) && name_str.ends_with(".jsonl"))
                    {
                        return Some(file.path());
                    }
                }
            }
        }
    }
    None
}

/// Collect all Claude JSONL file paths from workspace project directories.
pub(super) fn collect_claude_jsonl(workspaces: &[Workspace], out: &mut Vec<PathBuf>) {
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
pub(super) fn collect_codex_jsonl(_workspaces: &[Workspace], out: &mut Vec<PathBuf>) {
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
pub(super) fn collect_omp_jsonl(_workspaces: &[Workspace], out: &mut Vec<PathBuf>) {
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

/// Find the most recent session JSONL for a given workspace path and time range.
/// Used to match a newly completed PTY to its session file (targeted, not full scan).
pub fn find_recent_session_for_workspace(ws_path: &Path, started_at: u64) -> Option<Session> {
    let workspaces = vec![Workspace {
        id: String::new(),
        name: String::new(),
        path: Some(ws_path.to_path_buf()),
        created_at: 0,
        session_ids: Vec::new(),
        expanded: false,
    }];
    let mut jsonl_files: Vec<PathBuf> = Vec::new();
    collect_claude_jsonl(&workspaces, &mut jsonl_files);
    collect_codex_jsonl(&workspaces, &mut jsonl_files);
    collect_omp_jsonl(&workspaces, &mut jsonl_files);
    // Find JSONL files with mtime >= started_at
    let mut candidates: Vec<Session> = Vec::new();
    for path in &jsonl_files {
        let mtime = fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if mtime >= started_at {
            if let Some(session) = parse_session_from_path(path, &workspaces) {
                candidates.push(session);
            }
        }
    }
    // Return the most recent one
    candidates.sort_by_key(|c| std::cmp::Reverse(c.last_active));
    candidates.into_iter().next()
}
