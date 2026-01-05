use axum::{
    extract::{State, Path},
    Json,
    http::StatusCode,
    Extension,
};
use crate::api::router::AppState;
use crate::db::schema::{Device, User};
use crate::auth::rbac::{Permission, check_permission};

pub async fn list_devices(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Json<Vec<Device>>, StatusCode> {
    check_permission(&user, Permission::ViewDevices)?;
    
    state.device_service.list_devices().await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_device(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<Device>, StatusCode> {
    check_permission(&user, Permission::ViewDevices)?;

    let device = state.device_service.get_device(&id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
        
    Ok(Json(device))
}

pub async fn delete_device(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    check_permission(&user, Permission::ManageDevices)?;

    state.device_service.delete_device(&id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
pub struct UpdateDeviceRequest {
    pub group_name: Option<String>,
    pub tags: Option<Vec<String>>,
    pub channel_id: Option<String>,
}

pub async fn update_device(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateDeviceRequest>,
) -> Result<Json<Device>, StatusCode> {
    check_permission(&user, Permission::ManageDevices)?;

    state.device_service.update_device(&id, payload.group_name, payload.tags, payload.channel_id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let updated = state.device_service.get_device(&id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(updated))
}
