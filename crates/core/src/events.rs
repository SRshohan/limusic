//! Transport-agnostic replacement for Tauri's `AppHandle::emit` (openspec/changes/
//! signed-in-http-parity design.md's "Event bus" decision). `crates/server` subscribes and
//! forwards frames over `/ws`; nothing in this crate knows anything about HTTP or WebSockets.

use tokio::sync::broadcast;

const CHANNEL_CAPACITY: usize = 256;

#[derive(Clone)]
pub struct Broadcaster {
    tx: broadcast::Sender<(String, serde_json::Value)>,
}

impl Default for Broadcaster {
    fn default() -> Self {
        let (tx, _rx) = broadcast::channel(CHANNEL_CAPACITY);
        Broadcaster { tx }
    }
}

impl Broadcaster {
    /// Send an event to every current subscriber. Silently dropped if nobody is listening — same
    /// "best-effort" shape `AppHandle::emit`'s `let _ = ...` call sites already assumed.
    pub fn emit(&self, name: &str, payload: impl serde::Serialize) {
        let value = match serde_json::to_value(payload) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(event = name, error = %e, "failed to serialize event payload");
                return;
            }
        };
        let _ = self.tx.send((name.to_owned(), value));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<(String, serde_json::Value)> {
        self.tx.subscribe()
    }
}
