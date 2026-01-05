use axum::{
    routing::{post, get, delete},
    Router,
    middleware,
    http::StatusCode,
};
use crate::auth::{service::AuthService, session::SessionService, handlers::login};
use crate::services::{device::DeviceService, pairing::PairingService, audit::AuditService, infrastructure::InfrastructureService, updates::UpdateService, dashboard::DashboardService, api_keys::ApiKeyService};
use crate::db::store::DbStore;
use crate::api::middleware::auth_middleware;
use crate::api::{devices, pairings, audit, infrastructure, updates, dashboard, api_keys, users};

#[derive(Clone)]
pub struct AppState {
    pub db: DbStore,
    pub auth_service: AuthService,
    pub session_service: SessionService,
    pub device_service: DeviceService,
    pub pairing_service: PairingService,
    pub audit_service: AuditService,
    pub infrastructure_service: InfrastructureService,
    pub update_service: UpdateService,
    pub dashboard_service: DashboardService,
    pub api_key_service: ApiKeyService,
}

use tower::limit::RateLimitLayer;
use tower::buffer::BufferLayer;
use tower::ServiceBuilder;

pub fn create_router(state: AppState) -> Router {
    // Auth Router
    let auth_router = Router::new()
        .route("/login", post(login))
        .route("/totp/setup", post(crate::auth::handlers::setup_totp))
        .route("/totp/verify", post(crate::auth::handlers::verify_totp))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        // .layer(ServiceBuilder::new()
        //     .layer(BufferLayer::new(1024))
        //     .layer(RateLimitLayer::new(5, Duration::from_secs(1)))
        // )
        .with_state(state.clone());

    // Protected Router
    let protected_router = Router::new()
        .route("/me", get(users::get_current_user))
        .route("/dashboard/stats", get(dashboard::get_stats))
        .route("/dashboard/metrics", get(dashboard::get_metrics))
        .route("/ws/dashboard", get(super::ws::ws_handler))
        .route("/devices", get(devices::list_devices))
        .route("/devices/:id", get(devices::get_device).delete(devices::delete_device).patch(devices::update_device))
        .route("/pairings", get(pairings::list_pairings))
        .route("/pairings/:id", get(pairings::get_pairing).delete(pairings::revoke_pairing))
        .route("/audit-logs", get(audit::list_audit_logs))
        .route("/audit-logs/export", get(audit::export_audit_logs))
        .route("/infrastructure/health", get(infrastructure::health_check))
        .route("/infrastructure/relays", get(infrastructure::list_relays).post(infrastructure::add_relay))
        .route("/infrastructure/dirnodes", get(infrastructure::list_dirnodes).post(infrastructure::add_dirnode))
        .route("/updates/channels", get(updates::list_channels))
        .route("/updates/releases", get(updates::list_releases).post(updates::publish_release))
        .route("/updates/status", get(updates::get_rollout_status))
        .route("/api-keys", get(api_keys::list_keys).post(api_keys::create_key))
        .route("/api-keys/:id", delete(api_keys::revoke_key))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        // .layer(ServiceBuilder::new()
        //     .layer(HandleErrorLayer::new(handle_error))
        //     .layer(BufferLayer::new(1024))
        //     .layer(RateLimitLayer::new(100, Duration::from_secs(1)))
        // )
        .with_state(state.clone());

    let api_router = Router::new()
        .nest("/auth", auth_router)
        .merge(protected_router);

    Router::new()
        .nest("/api", api_router)
        .route("/health", get(|| async { "OK" }))
}
