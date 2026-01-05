use axum::{
    extract::State,
    Json,
    http::StatusCode,
    Extension,
};
use crate::api::router::AppState;
use crate::db::schema::{Relay, Dirnode, User};
use crate::auth::rbac::{Permission, check_permission};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AddRelayRequest {
    pub url: String,
    pub region: Option<String>,
}

#[derive(Deserialize)]
pub struct AddDirnodeRequest {
    pub url: String,
    pub region: Option<String>,
    pub public_key: Option<String>,
}

pub async fn list_relays(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Json<Vec<Relay>>, StatusCode> {
    check_permission(&user, Permission::ViewInfrastructure)?;

    state.infrastructure_service.list_relays().await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn add_relay(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Json(payload): Json<AddRelayRequest>,
) -> Result<Json<Relay>, StatusCode> {
    check_permission(&user, Permission::ManageInfrastructure)?;

    state.infrastructure_service.add_relay(&payload.url, payload.region.as_deref()).await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn list_dirnodes(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Json<Vec<Dirnode>>, StatusCode> {
    check_permission(&user, Permission::ViewInfrastructure)?;

    state.infrastructure_service.list_dirnodes().await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn add_dirnode(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Json(payload): Json<AddDirnodeRequest>,
) -> Result<Json<Dirnode>, StatusCode> {
    check_permission(&user, Permission::ManageInfrastructure)?;

    state.infrastructure_service.add_dirnode(&payload.url, payload.region.as_deref(), payload.public_key.as_deref()).await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn health_check(
    State(state): State<AppState>,
    Extension(_user): Extension<User>,
) -> Result<Json<String>, StatusCode> {
    // Permission check? Maybe view infra? Or public?
    // Users usually check health.
    // check_permission(&user, Permission::ViewInfrastructure)?; 
    // Usually health check is public for loadbalancers, but this is an authenticated check inside admin console app.
    // Assuming authenticated for now due to router structure.
    
    let status = state.infrastructure_service.health_check().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
    Ok(Json(status))
}
