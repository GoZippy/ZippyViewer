use axum::{
    extract::{State, Path},
    Json,
    http::StatusCode,
    Extension,
};
use crate::api::router::AppState;
use crate::db::schema::{ApiKey, User};
use crate::auth::rbac::{Permission, check_permission};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub permissions: Vec<String>, // Array of permission strings
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct CreateApiKeyResponse {
    pub api_key: ApiKey,
    pub plaintext_key: String, // Only returned once
}

pub async fn list_keys(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Json<Vec<ApiKey>>, StatusCode> {
    // Users can view their own keys. 
    // If we want admins to view all, we need logic. 
    // For now, let's assume personal management or strict permission.
    // Let's implement Self-management for now.
    
    // Simplification: ANY user can have keys? Yes.
    
    state.api_key_service.list_keys(&user.id).await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn create_key(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> Result<Json<CreateApiKeyResponse>, StatusCode> {
    
    // Check if user is allowed to generate keys?
    // Maybe checking `Permission::ManageApiKeys` implies global management.
    // Let's say anyone logged in can create a key for themselves (common pattern).
    
    // Validate permissions requested don't exceed user's own permissions? 
    // That's complex logic. For MVP, we trust the input string or simply store it.
    // The validation middleware using the key needs to check if the Key has permission X.
    // AND if the User associated with the key has permission X.
    
    let perms_json = serde_json::to_string(&payload.permissions)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let (api_key, plaintext_key) = state.api_key_service.create_key(
        &user.id,
        &payload.name,
        &perms_json,
        payload.expires_at
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(CreateApiKeyResponse {
        api_key,
        plaintext_key,
    }))
}

pub async fn revoke_key(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state.api_key_service.revoke_key(&id, &user.id).await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
