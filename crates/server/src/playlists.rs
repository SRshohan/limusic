// Playlist write routes (openspec/changes/signed-in-http-parity, tasks 4.1-4.3).
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, post, put},
    Json, Router,
};
use innertube::PlaylistSort;
use serde::Deserialize;

use crate::session::RequireSession;
use crate::{command_err, ApiError, ServerState};

#[derive(Deserialize)]
struct CreateBody {
    title: String,
}

async fn create_playlist(
    State(state): State<ServerState>,
    _session: RequireSession,
    Json(body): Json<CreateBody>,
) -> Result<Json<String>, ApiError> {
    Ok(Json(
        limusic_core::commands::playlists::create_playlist(&state.app, &body.title)
            .await
            .map_err(command_err)?,
    ))
}

async fn delete_playlist(
    State(state): State<ServerState>,
    _session: RequireSession,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    limusic_core::commands::playlists::delete_playlist(&state.app, &id)
        .await
        .map_err(command_err)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct EditBody {
    name: Option<String>,
    description: Option<String>,
    public: Option<bool>,
}

async fn edit_playlist_details(
    State(state): State<ServerState>,
    _session: RequireSession,
    Path(id): Path<String>,
    Json(body): Json<EditBody>,
) -> Result<StatusCode, ApiError> {
    limusic_core::commands::playlists::edit_playlist_details(
        &state.app,
        &id,
        body.name.as_deref(),
        body.description.as_deref(),
        body.public,
    )
    .await
    .map_err(command_err)?;
    Ok(StatusCode::NO_CONTENT)
}

/// A non-empty body sets the cover (raw image bytes, sniffed by magic number — see
/// `commands::playlists::set_playlist_cover`'s doc comment); an empty body clears it.
async fn set_playlist_cover(
    State(state): State<ServerState>,
    _session: RequireSession,
    Path(id): Path<String>,
    bytes: Bytes,
) -> Result<Json<Option<String>>, ApiError> {
    let image = (!bytes.is_empty()).then(|| bytes.to_vec());
    Ok(Json(
        limusic_core::commands::playlists::set_playlist_cover(&state.app, &id, image)
            .await
            .map_err(command_err)?,
    ))
}

#[derive(Deserialize)]
struct AddTrackBody {
    #[serde(rename = "videoId")]
    video_id: String,
    #[serde(rename = "allowDuplicates", default)]
    allow_duplicates: bool,
}

async fn add_to_playlist(
    State(state): State<ServerState>,
    _session: RequireSession,
    Path(id): Path<String>,
    Json(body): Json<AddTrackBody>,
) -> Result<Json<bool>, ApiError> {
    Ok(Json(
        limusic_core::commands::playlists::add_to_playlist(
            &state.app,
            &id,
            &body.video_id,
            body.allow_duplicates,
        )
        .await
        .map_err(command_err)?,
    ))
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RemoveTracksBody {
    One { #[serde(rename = "videoId")] video_id: String, #[serde(rename = "setVideoId")] set_video_id: String },
    Many { tracks: Vec<(String, String)> },
}

async fn remove_from_playlist(
    State(state): State<ServerState>,
    _session: RequireSession,
    Path(id): Path<String>,
    Json(body): Json<RemoveTracksBody>,
) -> Result<StatusCode, ApiError> {
    match body {
        RemoveTracksBody::One { video_id, set_video_id } => {
            limusic_core::commands::playlists::remove_from_playlist(
                &state.app,
                &id,
                &video_id,
                &set_video_id,
            )
            .await
            .map_err(command_err)?;
        }
        RemoveTracksBody::Many { tracks } => {
            limusic_core::commands::playlists::remove_many_from_playlist(&state.app, &id, &tracks)
                .await
                .map_err(command_err)?;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SortBody {
    sort: PlaylistSort,
}

async fn set_playlist_sort(
    State(state): State<ServerState>,
    _session: RequireSession,
    Path(id): Path<String>,
    Json(body): Json<SortBody>,
) -> Result<StatusCode, ApiError> {
    limusic_core::commands::playlists::set_playlist_sort(&state.app, &id, body.sort)
        .await
        .map_err(command_err)?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<ServerState> {
    Router::new()
        .route("/api/playlists", post(create_playlist))
        .route("/api/playlists/{id}", delete(delete_playlist).patch(edit_playlist_details))
        .route("/api/playlists/{id}/cover", put(set_playlist_cover))
        .route("/api/playlists/{id}/tracks", post(add_to_playlist).delete(remove_from_playlist))
        .route("/api/playlists/{id}/sort", put(set_playlist_sort))
}
