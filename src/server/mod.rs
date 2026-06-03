pub mod api;
pub mod auth;
pub mod ws;

use anyhow::Result;
use axum::Router;
use axum::middleware;
use axum::routing::{get, post};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

use crate::config;
use crate::pty::PtyHandle;
use crate::types::Agent;

/// Metadata for a PTY registered in the shared state.
#[derive(Clone)]
pub struct RegisteredPty {
    pub handle: Arc<PtyHandle>,
    pub title: String,
    pub agent: Agent,
    pub session_id: Option<String>,
    pub process_stats: Option<crate::procfs::ProcessStats>,
}

/// Shared application state for the HTTP server.
/// `ptys` is the same Arc shared with the TUI's App struct.
#[derive(Clone)]
pub struct AppState {
    pub config_dir: std::path::PathBuf,
    /// Shared PTY handles keyed by PTY id, with metadata.
    /// TUI app registers PTYs here for WebSocket/REST access.
    pub ptys: Arc<Mutex<HashMap<String, RegisteredPty>>>,
}

/// Run the server with a pre-existing shared PTY state (used by TUI).
pub async fn run_server_with_state(
    port: u16,
    token: String,
    ptys: Arc<Mutex<HashMap<String, RegisteredPty>>>,
) -> Result<()> {
    let state = Arc::new(AppState {
        config_dir: config::data_dir(),
        ptys,
    });

    let app = make_router(state, &token);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    eprintln!("amux: server listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// Run the standalone server (no TUI, `amux serve` subcommand).
pub async fn run_server(port: u16, token: String) -> Result<()> {
    let state = Arc::new(AppState {
        config_dir: config::data_dir(),
        ptys: Arc::new(Mutex::new(HashMap::new())),
    });

    let app = make_router(state, &token);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    if token.is_empty() {
        eprintln!("amux serve: listening on http://{} (no auth)", addr);
    } else {
        eprintln!("amux serve: listening on http://{} (auth enabled)", addr);
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn make_router(state: Arc<AppState>, token: &str) -> Router {
    let token_for_mw = token.to_string();
    Router::new()
        .route("/", get(index_handler))
        .route(
            "/api/sessions",
            get(api::list_sessions).post(api::create_session),
        )
        .route("/api/workspaces", get(api::list_workspaces))
        .route("/api/ptys", get(api::list_ptys))
        .route("/api/pty/{id}/input", post(api::pty_input))
        .route("/api/pty/{id}/resize", post(api::pty_resize))
        .route("/ws/pty/{session_id}", get(ws::pty_ws_handler))
        .layer(middleware::from_fn(move |req, next| {
            let t = token_for_mw.clone();
            auth::auth_middleware(t, req, next)
        }))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn index_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("static/index.html"))
}
