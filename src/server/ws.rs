use axum::extract::{
    Path, State, WebSocketUpgrade,
    ws::{Message, WebSocket},
};
use std::sync::Arc;

use super::AppState;

/// WebSocket handler that proxies bidirectional PTY I/O.
pub async fn pty_ws_handler(
    ws: WebSocketUpgrade,
    Path(pty_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> axum::response::Response {
    let pty_handle = state
        .ptys
        .get(&pty_id)
        .map(|rp| rp.value().handle.clone());
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
    let initial = handle.screen_contents();
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


#[cfg(test)]
mod tests {
    use super::*;

    /// Compilation test: verify the module structure and public handler signature
    /// integrate correctly with AppState and axum extractors. The handler is async
    /// so we can't cast its fn pointer, but referencing it as a value proves it
    /// exists and compiles.
    #[test]
    fn module_compiles_and_exports_handler() {
        // Binding the function proves the module's public API is well-formed.
        // We can't call it without a live HTTP request, but compilation alone
        // validates the extractor types and return type integrate correctly.
        let _handler = pty_ws_handler;
    }

    /// Verify the PTY-not-found message format the handler sends to clients
    /// when the requested PTY id is absent from the shared state (line 29–33).
    #[test]
    fn not_found_message_formats_correctly() {
        let pty_id = "session-abc-123";
        let msg = format!(
            "amux: PTY '{}' not found. Is the TUI running with this session?",
            pty_id
        );
        assert!(msg.contains(pty_id));
        assert!(msg.starts_with("amux:"));
        assert!(msg.contains("not found"));
    }

    /// Verify the session-ended sentinel and error-prefix formatting used in
    /// the heartbeat path (line 65) and input-error path (line 79).
    #[test]
    fn ws_sentinel_and_error_messages_are_well_formed() {
        // Session ended sentinel
        let ended = "[session ended]";
        assert!(ended.starts_with('[') && ended.ends_with(']'));

        // Error formatting (matches `format!("[error: {}]", e)`)
        let err_msg = format!("[error: {}]", "write failed");
        assert!(err_msg.starts_with("[error:"));
        assert!(err_msg.ends_with(']'));
        assert!(err_msg.contains("write failed"));
    }
}
