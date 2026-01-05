use axum::{
    extract::State,
    Json,
    http::StatusCode,
};
use crate::auth::models::{LoginRequest, LoginResponse};
use crate::api::router::AppState;
use chrono::{Utc, Duration};

// use std::sync::Arc;

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) ->  Result<Json<LoginResponse>, StatusCode> {
    match state.auth_service.authenticate(&payload.username, &payload.password).await {
        Ok(user) => {
            // Create real session
            let token = state.session_service.create_session(&user.id).await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            let expires_at = Utc::now() + Duration::hours(24);
            
            Ok(Json(LoginResponse {
                token,
                user,
                expires_at,
            }))
        }
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}
// ... login handler ...

use crate::auth::models::{TotpSetupResponse, TotpVerifyRequest};
use axum::extract::Extension;

pub async fn setup_totp(
    State(state): State<AppState>,
    Extension(user_id): Extension<String>, // From Auth middleware
) -> Result<Json<TotpSetupResponse>, StatusCode> {
    // Need username, fetch from DB or middleware could provide it
    // For now, let's just fetch user
    let user = state.auth_service.get_user(&user_id).await.map_err(|_| StatusCode::NOT_FOUND)?;
    
    match state.auth_service.generate_totp_secret(&user_id, &user.username).await {
        Ok((secret, qr_code)) => Ok(Json(TotpSetupResponse { secret, qr_code })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn verify_totp(
    State(state): State<AppState>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<TotpVerifyRequest>,
) -> Result<Json<bool>, StatusCode> {
    match state.auth_service.verify_and_enable_totp(&user_id, &payload.code).await {
        Ok(valid) if valid => Ok(Json(true)),
        Ok(_) => Err(StatusCode::BAD_REQUEST),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
