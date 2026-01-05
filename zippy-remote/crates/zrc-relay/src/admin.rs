//! Admin API for relay management

use std::sync::Arc;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, Router},
    Json,
};
use serde::Serialize;
use tracing::{info, warn};

use crate::allocation::{AllocationManager, AllocationInfo};
use crate::metrics::AllocationMetrics;
use crate::security::SecurityControls;

#[derive(Clone)]
pub struct AdminState {
    pub allocation_mgr: Arc<AllocationManager>,
    pub metrics: Arc<AllocationMetrics>,
    pub security: Arc<SecurityControls>,
    pub admin_token: String,
}

/// Admin API server
pub struct AdminApi {
    state: AdminState,
}

impl AdminApi {
    pub fn new(
        allocation_mgr: Arc<AllocationManager>,
        metrics: Arc<AllocationMetrics>,
        security: Arc<SecurityControls>,
        admin_token: String,
    ) -> Self {
        Self {
            state: AdminState {
                allocation_mgr,
                metrics,
                security,
                admin_token,
            },
        }
    }

    /// Create admin API router
    pub fn router(&self) -> Router {
        Router::new()
            .route("/admin/allocations", get(list_allocations))
            .route("/admin/allocations/:id", delete(terminate_allocation))
            .route("/admin/stats", get(get_stats))
            .with_state(self.state.clone())
    }
}

/// Check admin authentication
fn check_auth(headers: &HeaderMap, expected_token: &str) -> bool {
    let auth_header = headers.get("authorization");
    let token = auth_header
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .unwrap_or("");

    !token.is_empty() && token == expected_token
}

/// List active allocations
async fn list_allocations(
    State(state): State<AdminState>,
    headers: HeaderMap,
) -> Result<Json<ListAllocationsResponse>, StatusCode> {
    if !check_auth(&headers, &state.admin_token) {
        warn!("Admin API authentication failed");
        return Err(StatusCode::UNAUTHORIZED);
    }
    let allocations = state.allocation_mgr.list();
    let total = allocations.len();
    
    tracing::info!("Admin API: List allocations ({} active)", total);
    
    Ok(Json(ListAllocationsResponse {
        allocations,
        total,
    }))
}

/// Terminate an allocation
async fn terminate_allocation(
    State(state): State<AdminState>,
    Path(id_hex): Path<String>,
    headers: HeaderMap,
) -> Result<Json<TerminateResponse>, StatusCode> {
    if !check_auth(&headers, &state.admin_token) {
        warn!("Admin API authentication failed");
        return Err(StatusCode::UNAUTHORIZED);
    }
    // Parse allocation ID
    let id_bytes = hex::decode(&id_hex)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    if id_bytes.len() != 16 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut allocation_id = [0u8; 16];
    allocation_id.copy_from_slice(&id_bytes[..16]);

    // Terminate allocation
    state.allocation_mgr.terminate(
        &allocation_id,
        crate::allocation::TerminateReason::ExplicitRelease,
    );

    info!("Admin API: Terminated allocation {}", id_hex);
    
    Ok(Json(TerminateResponse {
        success: true,
        message: format!("Allocation {} terminated", id_hex),
    }))
}

/// Get detailed statistics
async fn get_stats(
    State(state): State<AdminState>,
    headers: HeaderMap,
) -> Result<Json<RelayStats>, StatusCode> {
    if !check_auth(&headers, &state.admin_token) {
        warn!("Admin API authentication failed");
        return Err(StatusCode::UNAUTHORIZED);
    }
    tracing::info!("Admin API: Get stats");

    let allocations = state.allocation_mgr.list();
    let active_count = allocations.len();
    let total_alloc = state.metrics.total_allocations() as u64;
    let bytes_fwd = state.metrics.bytes_forwarded() as u64;
    let packets_fwd = state.metrics.packets_forwarded() as u64;

    Ok(Json(RelayStats {
        active_allocations: active_count,
        total_allocations: total_alloc,
        bytes_forwarded: bytes_fwd,
        packets_forwarded: packets_fwd,
        uptime_seconds: state.metrics.uptime().as_secs(),
        bandwidth_usage: BandwidthStats {
            current_bps: state.metrics.current_bandwidth(),
            peak_bps: state.metrics.peak_bandwidth(),
            average_bps: state.metrics.average_bandwidth(),
        },
    }))
}

#[derive(Debug, Serialize)]
pub struct ListAllocationsResponse {
    pub allocations: Vec<AllocationInfo>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct TerminateResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct RelayStats {
    pub active_allocations: usize,
    pub total_allocations: u64,
    pub bytes_forwarded: u64,
    pub packets_forwarded: u64,
    pub uptime_seconds: u64,
    pub bandwidth_usage: BandwidthStats,
}

#[derive(Debug, Serialize)]
pub struct BandwidthStats {
    pub current_bps: u64,
    pub peak_bps: u64,
    pub average_bps: u64,
}
