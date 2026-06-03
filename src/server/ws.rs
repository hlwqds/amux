use axum::extract::{
    Path, State, WebSocketUpgrade,
    ws::{Message, WebSocket},
};
use std::sync::Arc;

use super::AppState;

pub async fn pty_ws_handler(
    ws: WebSocketUpgrade,
    Path(pty_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> axum::response::Response {
    let pty_handle = {
        let ptys = state.ptys.lock().await;
        ptys.get(&pty_id).map(|rp| rp.handle.clone())
    };
    ws.on_upgrade(move |socket| handle_pty_ws(socket, pty_id, pty_handle))
}

async fn handle_pty_ws(
    mut socket: WebSocket,
    pty_id: String,
    pty_handle: Option<Arc<crate::pty::PtyHandle>>,
) {
    let Some(handle) = pty_handle else {
        let _ = socket
            .send(Message::Text(format!(
                "amux: PTY '{}' not found. Is the TUI running with this session?",
                pty_id
            )))
            .await;
        return;
    };

    // Send initial screen content as Binary to match subsequent updates
    let initial = {
        let parser = handle.screen();
        let guard = parser.read();
        guard.screen().contents()
    };
    let _ = socket.send(Message::Binary(initial.into())).await;

    // Drain any raw output that arrived between spawn and now
    let _ = handle.take_raw_output();

    // Event-driven loop: Notify fires on new PTY output, 5s heartbeat as fallback
    let notify = handle.output_notify();
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(5));
    heartbeat.tick().await; // consume the immediate first tick
    loop {
        tokio::select! {
            _ = notify.notified() => {
                // Drain and send all buffered raw ANSI bytes
                loop {
                    let raw = handle.take_raw_output();
                    if raw.is_empty() {
                        break;
                    }
                    if socket.send(Message::Binary(raw)).await.is_err() {
                        return;
                    }
                }
            }
            _ = heartbeat.tick() => {
                // Fallback: check alive, drain any buffered data
                if !handle.is_alive() {
                    let _ = socket.send(Message::Text("[session ended]".into())).await;
                    return;
                }
                let raw = handle.take_raw_output();
                if !raw.is_empty()
                    && socket.send(Message::Binary(raw)).await.is_err()
                {
                    return;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Err(e) = handle.write_input(text.as_bytes()) {
                            let _ = socket.send(Message::Text(format!("[error: {}]", e))).await;
                        }
                    }
                    Some(Ok(Message::Binary(data))) => {
                        if let Err(e) = handle.write_input(&data) {
                            let _ = socket.send(Message::Text(format!("[error: {}]", e))).await;
                        }
                    }
                    _ => return,
                }
            }
        }
    }
}
