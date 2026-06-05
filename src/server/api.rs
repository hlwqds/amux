use axum::Json as AxumJson;
use axum::extract::{Path, State};
use axum::response::Json;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use super::AppState;

fn fallback_config() -> crate::types::Config {
    use crate::types::*;
    Config {
        workspaces: Vec::new(),
        ..Default::default()
    }
}

/// List all discovered sessions with their active status.
pub async fn list_sessions(State(state): State<Arc<AppState>>) -> Json<Value> {
    let config = crate::config::load_config().unwrap_or_else(|_| fallback_config());
    let sessions = crate::discovery::discover_sessions(&config.workspaces);

    // Build set of session_ids that are currently active (have a running PTY)
    let active_session_ids: std::collections::HashSet<String> = state
        .ptys
        .iter()
        .filter_map(|rp| {
            if rp.value().handle.is_alive() {
                rp.value().session_id.clone()
            } else {
                None
            }
        })
        .collect();

    let list: Vec<Value> = sessions
        .iter()
        .map(|s| {
            let active = active_session_ids.contains(&s.id);
            json!({
                "id": s.id,
                "title": s.title,
                "agent": s.agent.label(),
                "workspace": s.workspace_path.to_string_lossy(),
                "last_active": s.last_active,
                "tags": s.tags,
                "active": active,
            })
        })
        .collect();

    Json(json!({ "sessions": list }))
}

/// List all configured workspaces.
pub async fn list_workspaces(State(_state): State<Arc<AppState>>) -> Json<Value> {
    let config = crate::config::load_config().unwrap_or_else(|_| fallback_config());
    let list: Vec<Value> = config
        .workspaces
        .iter()
        .map(|w| {
            json!({
                "name": w.name,
                "path": w.path.as_ref().map(|p| p.display().to_string()),
            })
        })
        .collect();
    Json(json!({ "workspaces": list }))
}

/// List all active PTY sessions registered by the TUI.
pub async fn list_ptys(State(state): State<Arc<AppState>>) -> Json<Value> {
    let list: Vec<Value> = state
        .ptys
        .iter()
        .map(|ref_multi| {
            let (id, rp) = (ref_multi.key(), ref_multi.value());
            let mut obj = json!({
                "id": id,
                "alive": rp.handle.is_alive(),
                "title": rp.title,
                "agent": rp.agent.label(),
                "session_id": rp.session_id,
            });
            if let Some(stats) = &rp.process_stats {
                obj["cpu_percent"] = json!(stats.cpu_percent);
                obj["mem_rss_kb"] = json!(stats.mem_rss_kb);
                obj["mem_virt_kb"] = json!(stats.mem_virt_kb);
                obj["read_bytes"] = json!(stats.read_bytes);
                obj["write_bytes"] = json!(stats.write_bytes);
                obj["threads"] = json!(stats.threads);
            }
            obj
        })
        .collect();
    Json(json!({ "ptys": list }))
}

#[derive(Deserialize)]
pub struct PtyInputRequest {
    pub data: String,
}

/// Send input to a PTY by its ID.
pub async fn pty_input(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    AxumJson(body): AxumJson<PtyInputRequest>,
) -> Json<Value> {
    let Some(rp) = state.ptys.get(&id) else {
        return Json(json!({ "error": format!("PTY '{}' not found", id) }));
    };
    match rp.value().handle.write_input(body.data.as_bytes()) {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "status": "error", "message": e })),
    }
}

#[derive(Deserialize)]
pub struct PtyResizeRequest {
    pub cols: u16,
    pub rows: u16,
}

/// Resize a PTY by its ID.
pub async fn pty_resize(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    AxumJson(body): AxumJson<PtyResizeRequest>,
) -> Json<Value> {
    let Some(rp) = state.ptys.get(&id) else {
        return Json(json!({ "error": format!("PTY '{}' not found", id) }));
    };
    rp.value().handle.resize((body.cols, body.rows));
    Json(json!({ "status": "ok" }))
}

#[derive(Deserialize)]
pub struct CreateSessionRequest {
    pub agent: String,
    pub workspace: Option<String>,
    pub name: Option<String>,
}

/// Spawn a new agent session from an HTTP request.
pub async fn create_session(AxumJson(body): AxumJson<CreateSessionRequest>) -> Json<Value> {
    // Parse agent
    let agent = match body.agent.to_lowercase().as_str() {
        "claude" => crate::types::Agent::Claude,
        "codex" => crate::types::Agent::Codex,
        "omp" => crate::types::Agent::Omp,
        _ => {
            return Json(json!({
                "error": format!("Unknown agent: {}. Supported: claude, codex, omp", body.agent)
            }));
        }
    };

    // Resolve workspace path
    let workspace_path = body
        .workspace
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let chat_size = (80, 24);
    let project_config = crate::config::load_project_config(&workspace_path);
    let env = project_config.env;
    match crate::pty::PtyHandle::spawn(
        agent,
        &workspace_path,
        None,
        body.name.as_deref(),
        chat_size,
        &env,
        &[],
    ) {
        Ok(_handle) => Json(json!({
            "status": "started",
            "agent": agent.label(),
            "workspace": workspace_path.to_string_lossy(),
        })),
        Err(e) => Json(json!({
            "error": format!("Failed to spawn {}: {}", agent.label(), e)
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            config_dir: std::path::PathBuf::new(),
            ptys: Arc::new(dashmap::DashMap::new()),
        })
    }

    #[tokio::test]
    async fn list_sessions_returns_valid_json_structure() {
        let state = test_state();
        let Json(value) = list_sessions(State(state)).await;
        assert!(value.is_object(), "response should be a JSON object");
        assert!(value.get("sessions").is_some(), "should contain 'sessions' key");
        assert!(
            value["sessions"].is_array(),
            "'sessions' should be an array"
        );
    }

    #[tokio::test]
    async fn create_session_rejects_unknown_agent() {
        let req = AxumJson(CreateSessionRequest {
            agent: "unknown_agent".into(),
            workspace: None,
            name: None,
        });
        let Json(value) = create_session(req).await;
        assert!(
            value.get("error").is_some(),
            "unknown agent should return an error"
        );
        let msg = value["error"].as_str().unwrap();
        assert!(
            msg.contains("Unknown agent"),
            "error message should mention unknown agent, got: {msg}"
        );
    }

    #[tokio::test]
    async fn pty_input_returns_error_for_missing_pty() {
        let state = test_state();
        let path = Path("nonexistent-id".into());
        let body = AxumJson(PtyInputRequest {
            data: "hello".into(),
        });
        let Json(value) = pty_input(State(state), path, body).await;
        assert!(
            value.get("error").is_some(),
            "missing PTY should return an error"
        );
        let msg = value["error"].as_str().unwrap();
        assert!(
            msg.contains("not found"),
            "error should mention 'not found', got: {msg}"
        );
    }
}
