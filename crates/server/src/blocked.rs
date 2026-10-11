// Blocked-artists routes (openspec/changes/signed-in-http-parity, task 9.1). Anonymous — the
// original Tauri commands have no sign-in guard either: this is a local blocklist with no account
// concept at all (`blocked.rs`'s `list`/`block`/`unblock` take only `&Db`), not a per-account
// setting. Found live-testing 9.1 that an earlier draft of this route gated it behind
// `RequireSession` anyway; removed that, and corrected `specs/http-blocked-artists/spec.md` to
// match (see design.md for the full note).
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use limusic_core::blocked::BlockedArtist;
use serde::Deserialize;

use crate::{command_err, ApiError, ServerState};

async fn get_blocked_artists(State(state): State<ServerState>) -> Json<Vec<BlockedArtist>> {
    Json(limusic_core::commands::blocked::get_blocked_artists(&state.app))
}

#[derive(Deserialize)]
struct BlockBody {
    id: Option<String>,
    name: String,
}

async fn block_artist(
    State(state): State<ServerState>,
    Json(body): Json<BlockBody>,
) -> Result<Json<Vec<BlockedArtist>>, ApiError> {
    Ok(Json(
        limusic_core::commands::blocked::block_artist(&state.app, body.id, &body.name)
            .map_err(command_err)?,
    ))
}

async fn unblock_artist(
    State(state): State<ServerState>,
    Path(key): Path<String>,
) -> Json<Vec<BlockedArtist>> {
    Json(limusic_core::commands::blocked::unblock_artist(&state.app, &key))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/blocked-artists", get(get_blocked_artists).post(block_artist))
        .route("/api/blocked-artists/{key}", axum::routing::delete(unblock_artist))
}
