// Search routes (openspec/changes/http-search-vertical-slice, http-browse-vertical-slice).

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use innertube::{BrowseItem, SearchResults, SearchSuggestions, SongItem};
use serde::Deserialize;

use crate::{metadata_client, require_query, ApiError, ServerState};

#[derive(Deserialize)]
struct QueryParam {
    q: Option<String>,
}

async fn search(
    State(state): State<ServerState>,
    Query(params): Query<QueryParam>,
) -> Result<Json<Vec<SongItem>>, ApiError> {
    let query = require_query(params.q)?;
    let client = metadata_client(&state.app)?;

    // No session to record history against yet (anonymous only) — see design.md.
    let result = state
        .app.it
        .search_songs(client, &query, false)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    Ok(Json(result.items))
}

async fn search_all(
    State(state): State<ServerState>,
    Query(params): Query<QueryParam>,
) -> Result<Json<SearchResults>, ApiError> {
    let query = require_query(params.q)?;
    let client = metadata_client(&state.app)?;

    let result = state
        .app.it
        .search_all(client, &query, false)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    Ok(Json(result))
}

async fn search_suggestions(
    State(state): State<ServerState>,
    Query(params): Query<QueryParam>,
) -> Result<Json<SearchSuggestions>, ApiError> {
    // Empty input has nothing to suggest — a successful empty list, not the `400` a submitted
    // search gets (see specs/search-http-api/spec.md).
    let query = params.q.map(|q| q.trim().to_owned()).unwrap_or_default();
    if query.is_empty() {
        return Ok(Json(SearchSuggestions::default()));
    }

    let client = metadata_client(&state.app)?;
    let result = state
        .app.it
        .search_suggestions(client, &query)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    Ok(Json(result))
}

#[derive(Deserialize)]
struct CardsParams {
    q: Option<String>,
    category: Option<String>,
}

async fn search_cards(
    State(state): State<ServerState>,
    Query(params): Query<CardsParams>,
) -> Result<Json<Vec<BrowseItem>>, ApiError> {
    let query = require_query(params.q)?;
    let category = params
        .category
        .ok_or((StatusCode::BAD_REQUEST, "missing `category` query parameter".to_owned()))?;
    let client = metadata_client(&state.app)?;

    let result = state.app.it.search_cards(client, &query, &category).await.map_err(|e| match e {
        // `search_cards` validates `category` itself before any network call — distinguishable
        // from an upstream failure, so it gets its own `400` instead of the generic `502` below
        // (see design.md's "Error mapping" decision).
        innertube::Error::Other(msg) if msg.starts_with("unknown search category:") => {
            (StatusCode::BAD_REQUEST, msg)
        }
        other => (StatusCode::BAD_GATEWAY, other.to_string()),
    })?;

    Ok(Json(result))
}

async fn search_videos(
    State(state): State<ServerState>,
    Query(params): Query<QueryParam>,
) -> Result<Json<Vec<SongItem>>, ApiError> {
    let query = require_query(params.q)?;
    let client = metadata_client(&state.app)?;

    let result = state
        .app.it
        .search_videos(client, &query)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    Ok(Json(result.items))
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/search", get(search))
        .route("/api/search/all", get(search_all))
        .route("/api/search/suggestions", get(search_suggestions))
        .route("/api/search/cards", get(search_cards))
        .route("/api/search/videos", get(search_videos))
}
