use axum::{
    extract::{State, Query},
    Json,
    http::StatusCode,
    Extension,
};
use crate::api::router::AppState;
use crate::db::schema::{UpdateChannel, Release, User};
use crate::auth::rbac::{Permission, check_permission};
use crate::services::updates::ChannelRolloutStatus;
use serde::Deserialize;
use chrono::Utc;

#[derive(Deserialize)]
pub struct ListReleasesParams {
    pub channel_id: Option<String>,
}

#[derive(Deserialize)]
pub struct PublishReleaseRequest {
    pub version: String,
    pub channel_id: String,
    pub url: String,
    pub checksum: String,
    pub changelog: Option<String>,
}

pub async fn list_channels(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Json<Vec<UpdateChannel>>, StatusCode> {
    check_permission(&user, Permission::ViewInfrastructure)?; // Reusing permission or add new? 
    // Let's use ViewInfrastructure for now as updates are infra-related
    
    state.update_service.list_channels().await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn list_releases(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Query(params): Query<ListReleasesParams>,
) -> Result<Json<Vec<Release>>, StatusCode> {
    check_permission(&user, Permission::ViewInfrastructure)?;

    state.update_service.list_releases(params.channel_id.as_deref()).await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn publish_release(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Json(payload): Json<PublishReleaseRequest>,
) -> Result<Json<Release>, StatusCode> {
    check_permission(&user, Permission::ManageInfrastructure)?;

    let release = Release {
        version: payload.version,
        channel_id: payload.channel_id,
        url: payload.url,
        checksum: payload.checksum,
        changelog: payload.changelog,
        published_at: Utc::now(),
        is_active: true,
    };

    state.update_service.publish_release(release.clone()).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
    Ok(Json(release))
}

pub async fn get_rollout_status(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Json<Vec<ChannelRolloutStatus>>, StatusCode> {
    check_permission(&user, Permission::ViewInfrastructure)?;

    state.update_service.get_rollout_status().await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
