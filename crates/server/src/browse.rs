// Browse routes (openspec/changes/http-browse-vertical-slice).
use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use innertube::{AlbumPage, ArtistPage, BrowseItem, HomePage, MoodSection, PlaylistContinuation, PlaylistPage, PlaylistSort};
use serde::Deserialize;

use crate::{metadata_client, ApiError, ServerState};

// Mirrors src-tauri/src/local.rs's `ALBUM_PREFIX`/`ARTIST_PREFIX` and state.rs's `ON_REPEAT_ID`/
// `LOCAL_PLAYLIST_PREFIX` — duplicated as plain string literals, not imported (src-tauri is a
// binary crate, not a library dependency, and these ids carry no logic of their own to share, just
// a shape to reject). See design.md's "Desktop-local ids rejected with 400" decision.
const LOCAL_ALBUM_PREFIX: &str = "LOCALALBUM:";
const LOCAL_ARTIST_PREFIX: &str = "LOCALARTIST:";
const ON_REPEAT_ID: &str = "LIMUSIC_ON_REPEAT";
const LOCAL_PLAYLIST_PREFIX: &str = "LOCALPLAYLIST:";

/// The browseId every Moods & Genres tile opens with its own `params` — mirrors
/// `src-tauri/src/commands.rs`'s `MOODS_CATEGORY_ID`.
const MOODS_CATEGORY_ID: &str = "FEmusic_moods_and_genres_category";

fn desktop_local_id(id: &str) -> ApiError {
    (StatusCode::BAD_REQUEST, format!("{id}: a desktop-local id, not reachable from this server"))
}

fn require_token(token: Option<String>) -> Result<String, ApiError> {
    token
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
        .ok_or((StatusCode::BAD_REQUEST, "missing or empty `token` query parameter".to_owned()))
}

#[derive(Deserialize)]
struct HomeParams {
    params: Option<String>,
}

async fn get_home(
    State(state): State<ServerState>,
    Query(params): Query<HomeParams>,
) -> Result<Json<HomePage>, ApiError> {
    let client = metadata_client(&state.app)?;
    let page = state
        .app.it
        .home(client, params.params.as_deref())
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(page))
}

#[derive(Deserialize)]
struct TokenParams {
    token: Option<String>,
}

async fn get_home_more(
    State(state): State<ServerState>,
    Query(params): Query<TokenParams>,
) -> Result<Json<HomePage>, ApiError> {
    let token = require_token(params.token)?;
    let client = metadata_client(&state.app)?;
    let page = state
        .app.it
        .home_continuation(client, &token)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(page))
}

async fn get_album(
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> Result<Json<AlbumPage>, ApiError> {
    if id.starts_with(LOCAL_ALBUM_PREFIX) || id.starts_with(LOCAL_ARTIST_PREFIX) {
        return Err(desktop_local_id(&id));
    }
    let client = metadata_client(&state.app)?;
    let page =
        state.app.it.album(client, &id).await.map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(page))
}

async fn get_artist(
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> Result<Json<ArtistPage>, ApiError> {
    let client = metadata_client(&state.app)?;
    let page =
        state.app.it.artist(client, &id).await.map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(page))
}

#[derive(Deserialize)]
struct PlaylistParams {
    sort: Option<PlaylistSort>,
    desc: Option<bool>,
}

async fn get_playlist(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Query(params): Query<PlaylistParams>,
) -> Result<Json<PlaylistPage>, ApiError> {
    if id == ON_REPEAT_ID || id.starts_with(LOCAL_PLAYLIST_PREFIX) {
        return Err(desktop_local_id(&id));
    }
    let client = metadata_client(&state.app)?;
    let sort = params.sort.map(|s| (s, params.desc.unwrap_or(false)));
    let page = state
        .app.it
        .playlist(client, &id, sort)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(page))
}

async fn get_playlist_more(
    State(state): State<ServerState>,
    Query(params): Query<TokenParams>,
) -> Result<Json<PlaylistContinuation>, ApiError> {
    let token = require_token(params.token)?;
    let client = metadata_client(&state.app)?;
    let page = state
        .app.it
        .playlist_continuation(client, &token)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(page))
}

async fn get_moods(
    State(state): State<ServerState>,
) -> Result<Json<Vec<MoodSection>>, ApiError> {
    let client = metadata_client(&state.app)?;
    let sections =
        state.app.it.moods(client).await.map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(sections))
}

/// A cover for each Moods & Genres tile, which YouTube draws as a coloured button with no art: the
/// first playlist in that tile's category. A tile whose category fails or comes back empty is
/// left out, not treated as an error for the whole request — mirrors
/// `src-tauri/src/commands.rs`'s `get_mood_art`, sequential here rather than concurrent (no
/// `futures_util` dependency in this crate yet; six-at-a-time buffering is a perf nicety, not a
/// behavior this slice's spec requires).
async fn get_mood_art(
    State(state): State<ServerState>,
    Json(params): Json<Vec<String>>,
) -> Result<Json<HashMap<String, String>>, ApiError> {
    let client = metadata_client(&state.app)?;
    let mut art = HashMap::new();
    for p in params {
        if let Ok(items) = state.app.it.browse_grid(client, MOODS_CATEGORY_ID, Some(&p)).await {
            if let Some(thumbnail) = items.into_iter().find_map(|i| i.thumbnail) {
                art.insert(p, thumbnail);
            }
        }
    }
    Ok(Json(art))
}

#[derive(Deserialize)]
struct BrowseGridParams {
    params: Option<String>,
}

async fn get_browse_grid(
    State(state): State<ServerState>,
    Path(id): Path<String>,
    Query(params): Query<BrowseGridParams>,
) -> Result<Json<Vec<BrowseItem>>, ApiError> {
    let client = metadata_client(&state.app)?;
    let items = state
        .app.it
        .browse_grid(client, &id, params.params.as_deref())
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(items))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/home", get(get_home))
        .route("/api/home/more", get(get_home_more))
        .route("/api/album/{id}", get(get_album))
        .route("/api/artist/{id}", get(get_artist))
        // Registered before the `{id}` route below: axum/matchit prioritizes a static segment
        // over a dynamic one regardless of registration order, but the explicit ordering here
        // keeps the routing table readable for whoever extends it next.
        .route("/api/playlist/more", get(get_playlist_more))
        .route("/api/playlist/{id}", get(get_playlist))
        .route("/api/moods", get(get_moods))
        .route("/api/moods/art", post(get_mood_art))
        .route("/api/browse/{id}", get(get_browse_grid))
}
