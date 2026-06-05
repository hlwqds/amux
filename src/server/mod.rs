pub mod api;
pub mod auth;
pub mod ws;

use anyhow::{Context, Result};
use axum::Router;
use axum::middleware;
use axum::routing::{get, post};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::config;
use crate::pty::PtyHandle;
use crate::types::Agent;

/// Shared PTY map type used by both TUI and HTTP server.
pub type SharedPtyMap = dashmap::DashMap<String, RegisteredPty>;

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
pub struct AppState {
    pub config_dir: std::path::PathBuf,
    /// Shared PTY handles keyed by PTY id, with metadata.
    /// TUI app registers PTYs here for WebSocket/REST access.
    pub ptys: Arc<SharedPtyMap>,
}

/// Run the server with a pre-existing shared PTY state (used by TUI).
///
/// Returns the actual port bound (may differ from requested if auto-assigned).
/// Pass `port = 0` to let the OS pick a free port.
pub async fn run_server_with_state(
    port: u16,
    token: String,
    ptys: Arc<SharedPtyMap>,
) -> Result<(u16, tokio::task::JoinHandle<()>)> {
    let state = Arc::new(AppState {
        config_dir: config::data_dir(),
        ptys,
    });
    let app = make_router(state, &token);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind port {}", port))?;
    let actual_port = listener.local_addr()?.port();
    eprintln!("amux: server listening on http://localhost:{}", actual_port);
    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("amux: server error: {}", e);
        }
    });
    Ok((actual_port, handle))
}

/// Run the standalone server (no TUI, `amux serve` subcommand).
pub async fn run_server(port: u16, token: String) -> Result<()> {
    info!("starting web server on port {}", port);
    let state = Arc::new(AppState {
        config_dir: config::data_dir(),
        ptys: Arc::new(SharedPtyMap::new()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::util::ServiceExt;

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            config_dir: std::path::PathBuf::from("/tmp/amux-test"),
            ptys: Arc::new(SharedPtyMap::new()),
        })
    }

    #[tokio::test]
    async fn test_bind_port_with_state() {
        let ptys = Arc::new(SharedPtyMap::new());
        // port 0 = let OS pick a free port
        let (port, handle) = run_server_with_state(0, String::new(), ptys)
            .await
            .expect("server should bind");
        assert!(port > 0, "OS should assign a non-zero port");
        handle.abort();
    }

    #[tokio::test]
    async fn test_index_returns_200() {
        let state = test_state();
        let app = make_router(state, "");
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: axum::body::Body = resp.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();
        let html = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(
            html.contains("<title>amux</title>"),
            "should serve index HTML"
        );
    }

    #[tokio::test]
    async fn test_router_has_expected_routes() {
        let state = test_state();
        let app = make_router(state, "test-token");

        // Unauthenticated requests to API routes should return 401
        let api_routes = ["/api/sessions", "/api/workspaces", "/api/ptys"];
        for path in &api_routes {
            let req = Request::builder().uri(*path).body(Body::empty()).unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::UNAUTHORIZED,
                "unauthenticated GET {} should be 401",
                path
            );
        }

        // POST routes should also require auth
        let post_routes = ["/api/pty/abc/input", "/api/pty/abc/resize"];
        for path in &post_routes {
            let req = Request::builder()
                .method("POST")
                .uri(*path)
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::UNAUTHORIZED,
                "unauthenticated POST {} should be 401",
                path
            );
        }

        // Authenticated request should pass auth (returns 200 for GET /api/sessions)
        let req = Request::builder()
            .uri("/api/sessions")
            .header("authorization", "Bearer test-token")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "authenticated GET /api/sessions should be 200"
        );
    }
}
