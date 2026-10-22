// Real-time event channel (openspec/changes/signed-in-http-parity, task 2.5). Not used for Listen
// Together — that's a direct browser connection to the separate `crates/sync-server` relay, see
// design.md's dedicated decision.
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::session::RequireSession;
use crate::ServerState;

async fn upgrade(
    ws: WebSocketUpgrade,
    State(state): State<ServerState>,
    _session: RequireSession,
) -> Response {
    let mut rx = state.shared.events.subscribe();
    ws.on_upgrade(move |socket| async move { forward(socket, &mut rx).await })
}

async fn forward(
    mut socket: WebSocket,
    rx: &mut tokio::sync::broadcast::Receiver<(String, serde_json::Value)>,
) {
    loop {
        tokio::select! {
            event = rx.recv() => {
                let (name, data) = match event {
                    Ok(event) => event,
                    // A slow consumer missed some frames — tell it to resync rather than silently
                    // drop messages it will never know it lost. The connection stays open.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                };
                let frame = serde_json::json!({ "type": name, "data": data });
                if socket.send(Message::Text(frame.to_string().into())).await.is_err() {
                    return; // the client went away
                }
            }
            incoming = socket.recv() => {
                // This channel is server→client only; a close frame (or any error) ends it. Any
                // other frame the client sends is simply ignored rather than rejected.
                if matches!(incoming, None | Some(Err(_)) | Some(Ok(Message::Close(_)))) {
                    return;
                }
            }
        }
    }
}

pub fn router() -> Router<ServerState> {
    Router::new().route("/ws", get(upgrade))
}
