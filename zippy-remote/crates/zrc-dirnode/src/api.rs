//! HTTP API endpoints

use std::sync::Arc;
use std::net::SocketAddr;
use std::time::Duration;
use axum::{
    extract::{Path, State, ConnectInfo},
    http::{HeaderMap, StatusCode, HeaderValue},
    response::{IntoResponse, Response},
    routing::{delete, get, post, Router},
    Json, body::Bytes,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use prost::Message;
use zrc_proto::v1::DirRecordV1;

use crate::records::{RecordManager, RecordError};
use crate::access::AccessController;
use crate::discovery::{DiscoveryManager, DiscoveryError};
use crate::search_protection::SearchProtection;

#[derive(Clone)]
pub struct ApiState {
    pub record_mgr: Arc<RecordManager>,
    pub access_ctrl: Arc<AccessController>,
    pub discovery_mgr: Arc<DiscoveryManager>,
    pub protection: Arc<SearchProtection>,
}

/// Create API router
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        .route("/v1/records", post(post_record))
        .route("/v1/records/:subject_id_hex", get(get_record))
        .route("/v1/records/batch", post(get_batch))
        .route("/v1/discovery/tokens", post(create_discovery_token))
        .route("/v1/discovery/tokens/:token_id_hex", delete(revoke_discovery_token))
        .route("/health", get(health_handler))
        .with_state(state)
}

/// POST /v1/records - Store directory record
async fn post_record(
    State(state): State<ApiState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    body: Bytes,
) -> Response {
    // Parse DirRecordV1 from body
    let record = match Message::decode(&body[..]) {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to parse DirRecordV1: {}", e);
            return (StatusCode::BAD_REQUEST, "Invalid record format").into_response();
        }
    };

    // Check record size
    if body.len() > 4 * 1024 {
        return (StatusCode::PAYLOAD_TOO_LARGE, "Record too large").into_response();
    }

    // Store record
    match state.record_mgr.store(record).await {
        Ok(()) => {
            info!("Stored record from {}", addr.ip());
            StatusCode::CREATED.into_response()
        }
        Err(RecordError::InvalidSignature) | Err(RecordError::SubjectMismatch) => {
            warn!("Signature verification failed from {}", addr.ip());
            (StatusCode::FORBIDDEN, "Signature verification failed").into_response()
        }
        Err(RecordError::RecordTooLarge) => {
            (StatusCode::PAYLOAD_TOO_LARGE, "Record too large").into_response()
        }
        Err(RecordError::TTLTooLong) => {
            (StatusCode::BAD_REQUEST, "TTL too long").into_response()
        }
        Err(e) => {
            error!("Store error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Storage error").into_response()
        }
    }
}

/// GET /v1/records/{subject_id_hex} - Get directory record
async fn get_record(
    State(state): State<ApiState>,
    Path(subject_id_hex): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    // Check search protection
    if let Err(e) = state.protection.check_lookup(addr.ip()) {
        warn!("Search protection triggered for {}: {}", addr.ip(), e);
        // Return 404 for timing-safe response (same as not found)
        return (StatusCode::NOT_FOUND, "Record not found").into_response();
    }

    // Parse subject_id
    let subject_id = match hex::decode(&subject_id_hex) {
        Ok(id) if id.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&id);
            arr
        }
        _ => {
            return (StatusCode::BAD_REQUEST, "Invalid subject_id").into_response();
        }
    };

    // Extract bearer token
    let token = headers.get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));

    // Check if discoverable
    let is_discoverable = state.discovery_mgr.is_discoverable(&subject_id);

    // Check authorization
    if let Err(_) = state.access_ctrl.authorize_lookup(&subject_id, token, is_discoverable) {
        // Return 404 for timing-safe response (same as not found)
        return (StatusCode::NOT_FOUND, "Record not found").into_response();
    }

    // Get record
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    match state.record_mgr.get(&subject_id, now).await {
        Ok(Some(record)) => {
            // Calculate expiration
            let expires_at = record.timestamp.saturating_add(record.ttl_seconds as u64);
            
            // Encode record
            let mut record_bytes = Vec::new();
            if Message::encode(&record, &mut record_bytes).is_err() {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Encoding error").into_response();
            }
            
            // Build response with headers
            let mut headers = HeaderMap::new();
            headers.insert("Content-Type", HeaderValue::from_static("application/octet-stream"));
            if let Ok(expires_header) = HeaderValue::from_str(&expires_at.to_string()) {
                headers.insert("X-Record-Expires", expires_header);
            }
            headers.insert("X-Signature-Verified", HeaderValue::from_static("true"));
            
            (StatusCode::OK, headers, record_bytes).into_response()
        }
        Ok(None) => {
            (StatusCode::NOT_FOUND, "Record not found").into_response()
        }
        Err(e) => {
            error!("Get record error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Error retrieving record").into_response()
        }
    }
}

/// POST /v1/records/batch - Batch record lookup
#[derive(Deserialize)]
struct BatchRequest {
    subject_ids: Vec<String>,
}

#[derive(Serialize)]
struct BatchResponse {
    records: Vec<serde_json::Value>,
    not_found: Vec<String>,
}

async fn get_batch(
    State(state): State<ApiState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<BatchRequest>,
) -> Response {
    // Check search protection
    if let Err(_) = state.protection.check_lookup(addr.ip()) {
        return (StatusCode::NOT_FOUND, "Record not found").into_response();
    }

    // Extract bearer token
    let token = headers.get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));

    // Parse subject IDs
    let mut subject_ids = Vec::new();
    let mut invalid_ids = Vec::new();

    for id_hex in request.subject_ids {
        match hex::decode(&id_hex) {
            Ok(id) if id.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&id);
                subject_ids.push((id_hex, arr));
            }
            _ => {
                invalid_ids.push(id_hex);
            }
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut records = Vec::new();
    let mut not_found = invalid_ids;

    for (id_hex, subject_id) in subject_ids {
        // Check authorization
        let is_discoverable = state.discovery_mgr.is_discoverable(&subject_id);
        if state.access_ctrl.authorize_lookup(&subject_id, token, is_discoverable).is_ok() {
            if let Ok(Some(record)) = state.record_mgr.get(&subject_id, now).await {
                let mut record_bytes = Vec::new();
                Message::encode(&record, &mut record_bytes).ok();
                records.push(serde_json::json!({
                    "subject_id": id_hex,
                    "record": hex::encode(record_bytes),
                }));
            } else {
                not_found.push(id_hex);
            }
        } else {
            not_found.push(id_hex);
        }
    }

    Json(BatchResponse { records, not_found }).into_response()
}

/// POST /v1/discovery/tokens - Create discovery token
#[derive(Deserialize)]
struct CreateTokenRequest {
    subject_id: String,
    ttl_seconds: Option<u64>,
    scope: Option<String>,
}

#[derive(Serialize)]
struct CreateTokenResponse {
    token_id: String,
    expires_at: u64,
}

async fn create_discovery_token(
    State(state): State<ApiState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<CreateTokenRequest>,
) -> Response {
    // Check admin authorization
    let token = headers.get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));

    if let Some(token) = token {
        if state.access_ctrl.authorize_admin(token).is_err() {
            return (StatusCode::FORBIDDEN, "Admin authorization required").into_response();
        }
    } else {
        return (StatusCode::UNAUTHORIZED, "Admin token required").into_response();
    }

    // Parse subject_id
    let subject_id = match hex::decode(&request.subject_id) {
        Ok(id) if id.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&id);
            arr
        }
        _ => {
            return (StatusCode::BAD_REQUEST, "Invalid subject_id").into_response();
        }
    };

    // Parse scope
    let scope = match request.scope.as_deref() {
        Some("pairing_only") => zrc_proto::v1::DiscoveryScopeV1::PairingOnly as i32,
        Some("session_only") => zrc_proto::v1::DiscoveryScopeV1::SessionOnly as i32,
        Some("full") => zrc_proto::v1::DiscoveryScopeV1::Full as i32,
        _ => zrc_proto::v1::DiscoveryScopeV1::PairingOnly as i32,
    };

    let ttl = request.ttl_seconds
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(600)); // 10 minutes default

    match state.discovery_mgr.create(
        subject_id,
        ttl,
        scope,
        None, // No signing key for now
    ) {
        Ok(token) => {
            info!("Created discovery token for {} from {}", request.subject_id, addr.ip());
            Json(CreateTokenResponse {
                token_id: hex::encode(&token.token_id),
                expires_at: token.expires_at,
            }).into_response()
        }
        Err(DiscoveryError::TokenLimitExceeded) => {
            (StatusCode::BAD_REQUEST, "Token limit exceeded").into_response()
        }
        Err(DiscoveryError::TTLTooLong) => {
            (StatusCode::BAD_REQUEST, "TTL too long").into_response()
        }
        Err(e) => {
            error!("Discovery token creation error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Error creating token").into_response()
        }
    }
}

/// DELETE /v1/discovery/tokens/{token_id_hex} - Revoke discovery token
async fn revoke_discovery_token(
    State(state): State<ApiState>,
    Path(token_id_hex): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    // Check admin authorization
    let token = headers.get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));

    if let Some(token) = token {
        if state.access_ctrl.authorize_admin(token).is_err() {
            return (StatusCode::FORBIDDEN, "Admin authorization required").into_response();
        }
    } else {
        return (StatusCode::UNAUTHORIZED, "Admin token required").into_response();
    }

    // Parse token_id
    let token_id = match hex::decode(&token_id_hex) {
        Ok(id) if id.len() == 16 => {
            let mut arr = [0u8; 16];
            arr.copy_from_slice(&id);
            arr
        }
        _ => {
            return (StatusCode::BAD_REQUEST, "Invalid token_id").into_response();
        }
    };

    match state.discovery_mgr.revoke(&token_id) {
        Ok(()) => {
            info!("Revoked discovery token {} from {}", token_id_hex, addr.ip());
            StatusCode::NO_CONTENT.into_response()
        }
        Err(DiscoveryError::NotFound) => {
            (StatusCode::NOT_FOUND, "Token not found").into_response()
        }
        Err(e) => {
            error!("Revoke token error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Error revoking token").into_response()
        }
    }
}

/// GET /health - Health check
async fn health_handler() -> StatusCode {
    StatusCode::OK
}
