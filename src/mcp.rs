//! Lightweight MCP (Model Context Protocol) JSON-RPC server over stdio.
//!
//! Implements the minimal subset needed for an external LLM to discover and
//! control amux sessions:
//!   - `initialize`       – capability handshake
//!   - `tools/list`       – advertise `list_sessions`, `send_input`, `attach_pty`
//!   - `tools/call`       – dispatch to real PtyHandle / discovery logic
//!
//! No external MCP crate is used; the protocol is just line-delimited JSON-RPC
//! 2.0 over stdin/stdout.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::config;
use crate::pty::PtyHandle;
use crate::server::{AppState, SharedPtyMap};
use crate::types::Agent;

// ─── JSON-RPC framing ────────────────────────────────────────

#[derive(serde::Deserialize)]
struct JsonRpcRequest {
    #[expect(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

fn success(id: Value, result: Value) -> String {
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    }))
    .expect("serializing a json! value cannot fail")
}

fn error_resp(id: Value, code: i32, message: &str) -> String {
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    }))
    .expect("serializing a json! value cannot fail")
}

// ─── Tool definitions ────────────────────────────────────────

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "list_sessions",
            "description": "List all discovered amux sessions (both active and historical).",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": [],
            },
        }),
        json!({
            "name": "send_input",
            "description": "Send keystrokes / text to a running PTY session.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pty_id": {
                        "type": "string",
                        "description": "The PTY identifier (from list_sessions).",
                    },
                    "data": {
                        "type": "string",
                        "description": "Text to send to the PTY stdin.",
                    },
                },
                "required": ["pty_id", "data"],
            },
        }),
        json!({
            "name": "attach_pty",
            "description": "Create (spawn) a new PTY session for a given agent and workspace.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "agent": {
                        "type": "string",
                        "description": "Agent to run: claude, codex, or omp.",
                    },
                    "workspace": {
                        "type": "string",
                        "description": "Absolute path to the workspace directory.",
                    },
                    "name": {
                        "type": "string",
                        "description": "Optional human-readable name for the session.",
                    },
                },
                "required": ["agent"],
            },
        }),
    ]
}

// ─── Tool dispatch ───────────────────────────────────────────

fn parse_agent(label: &str) -> Option<Agent> {
    match label.to_lowercase().as_str() {
        "claude" => Some(Agent::Claude),
        "codex" => Some(Agent::Codex),
        "omp" => Some(Agent::Omp),
        _ => None,
    }
}

fn handle_list_sessions(state: &AppState) -> Value {
    let cfg = config::load_config().unwrap_or_else(|_| crate::types::Config::default());
    let sessions = crate::discovery::discover_sessions(&cfg.workspaces);

    let active_ids: std::collections::HashSet<String> = state
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
            let active = active_ids.contains(&s.id);
            json!({
                "id": s.id,
                "title": s.title,
                "agent": s.agent.label(),
                "workspace": s.workspace_path.to_string_lossy(),
                "last_active": s.last_active,
                "active": active,
            })
        })
        .collect();

    json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json!({ "sessions": list })).unwrap_or_default() }] })
}

fn handle_send_input(state: &AppState, params: &Value) -> Result<Value> {
    let pty_id = params["pty_id"]
        .as_str()
        .context("missing 'pty_id' parameter")?;
    let data = params["data"]
        .as_str()
        .context("missing 'data' parameter")?;

    let rp = state
        .ptys
        .get(pty_id)
        .context(format!("no PTY with id '{pty_id}'"))?;

    rp.handle.write_input(data.as_bytes()).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(json!({
        "content": [{ "type": "text", "text": format!("sent {} bytes to PTY {pty_id}", data.len()) }]
    }))
}

fn handle_attach_pty(state: &AppState, params: &Value) -> Result<Value> {
    let agent_label = params["agent"]
        .as_str()
        .context("missing 'agent' parameter")?;
    let agent = parse_agent(agent_label)
        .context(format!("unknown agent '{agent_label}'; supported: claude, codex, omp"))?;

    let workspace: PathBuf = params["workspace"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let name = params["name"].as_str();

    let project_config = config::load_project_config(&workspace);
    let handle = PtyHandle::spawn(agent, &workspace, None, name, (80, 24), &project_config.env, &[])?;

    // Generate a unique ID based on the current map size + a timestamp suffix.
    let id = format!("mcp-{}", state.ptys.len() + 1);
    state.ptys.insert(
        id.clone(),
        crate::server::RegisteredPty {
            handle: Arc::new(handle),
            title: name.unwrap_or(agent.label()).to_string(),
            agent,
            session_id: None,
            process_stats: None,
        },
    );

    Ok(json!({
        "content": [{ "type": "text", "text": format!("spawned {} in {} (pty_id={id})", agent.label(), workspace.display()) }]
    }))
}

fn dispatch_tool(state: &AppState, name: &str, params: &Value) -> Result<Value> {
    match name {
        "list_sessions" => Ok(handle_list_sessions(state)),
        "send_input" => handle_send_input(state, params),
        "attach_pty" => handle_attach_pty(state, params),
        other => anyhow::bail!("unknown tool: {other}"),
    }
}

// ─── Main loop ───────────────────────────────────────────────

/// Run the MCP stdio server.
///
/// Reads JSON-RPC requests from stdin (one per line) and writes responses to
/// stdout. Stops when stdin reaches EOF.
pub fn run() -> Result<()> {
    let state = Arc::new(AppState {
        config_dir: config::data_dir(),
        ptys: Arc::new(SharedPtyMap::new()),
    });

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    let mut lines = stdin.lock().lines();

    // Send an initial `notifications/initialized` is NOT needed per spec —
    // we wait for the client's `initialize` request.

    while let Some(line) = lines.next().transpose()? {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = error_resp(Value::Null, -32_700, &format!("parse error: {e}"));
                writeln!(stdout, "{resp}")?;
                stdout.flush()?;
                continue;
            }
        };

        let id = req.id.unwrap_or(Value::Null);

        let resp = match req.method.as_str() {
            "initialize" => success(
                id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "amux-mcp",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                }),
            ),

            "notifications/initialized" => {
                // Client notification — no response expected.
                continue;
            }

            "tools/list" => success(id, json!({ "tools": tool_definitions() })),

            "tools/call" => {
                let tool_name = req.params["name"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let tool_args = req.params.get("arguments").cloned().unwrap_or(json!({}));

                match dispatch_tool(&state, &tool_name, &tool_args) {
                    Ok(val) => success(id, val),
                    Err(e) => success(
                        id,
                        json!({
                            "content": [{ "type": "text", "text": format!("error: {e}") }],
                            "isError": true,
                        }),
                    ),
                }
            }

            other => error_resp(id, -32_601, &format!("method not found: {other}")),
        };

        writeln!(stdout, "{resp}")?;
        stdout.flush()?;
    }

    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definitions_have_required_fields() {
        let defs = tool_definitions();
        assert_eq!(defs.len(), 3);

        let names: Vec<&str> = defs
            .iter()
            .filter_map(|d| d["name"].as_str())
            .collect();
        assert!(names.contains(&"list_sessions"));
        assert!(names.contains(&"send_input"));
        assert!(names.contains(&"attach_pty"));

        for d in &defs {
            assert!(d["description"].is_string());
            assert!(d["inputSchema"]["type"].is_string());
        }
    }

    #[test]
    fn parse_agent_cases() {
        assert_eq!(parse_agent("claude"), Some(Agent::Claude));
        assert_eq!(parse_agent("Claude"), Some(Agent::Claude));
        assert_eq!(parse_agent("codex"), Some(Agent::Codex));
        assert_eq!(parse_agent("omp"), Some(Agent::Omp));
        assert_eq!(parse_agent("unknown"), None);
    }

    #[test]
    fn json_rpc_success_shape() {
        let s = success(json!(1), json!({"ok": true}));
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 1);
        assert_eq!(v["result"]["ok"], true);
        assert!(v.get("error").is_none());
    }

    #[test]
    fn json_rpc_error_shape() {
        let s = error_resp(json!(2), -32_600, "bad");
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 2);
        assert_eq!(v["error"]["code"], -32_600);
        assert!(v.get("result").is_none());
    }
}
