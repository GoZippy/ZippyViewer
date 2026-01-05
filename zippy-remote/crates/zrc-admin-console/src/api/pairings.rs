use axum::{
    extract::{State, Path},
    Json,
    http::StatusCode,
    Extension,
};
use crate::api::router::AppState;
use crate::db::schema::{Pairing, User};
use crate::auth::rbac::{Permission, check_permission};

pub async fn list_pairings(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Json<Vec<Pairing>>, StatusCode> {
    check_permission(&user, Permission::ViewPairings)?;

    state.pairing_service.list_pairings().await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_pairing(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
) -> Result<Json<Pairing>, StatusCode> {
    check_permission(&user, Permission::ViewPairings)?;

    let pairing = state.pairing_service.get_pairing(&id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
        
    Ok(Json(pairing))
}

pub async fn revoke_pairing(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    check_permission(&user, Permission::RevokePairing)?;

    state.pairing_service.revoke_pairing(&id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
    Ok(StatusCode::NO_CONTENT)
}
