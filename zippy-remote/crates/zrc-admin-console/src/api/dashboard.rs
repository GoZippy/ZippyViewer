use axum::{
    extract::State,
    Json,
    http::StatusCode,
    Extension,
};
use crate::api::router::AppState;
use crate::services::dashboard::DashboardStats;
use crate::db::schema::User;
use crate::auth::rbac::{Permission, check_permission};

pub async fn get_stats(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Json<DashboardStats>, StatusCode> {
    check_permission(&user, Permission::ViewDashboard)?;

    state.dashboard_service.get_stats().await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_metrics(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Json<crate::services::dashboard::SystemMetrics>, StatusCode> {
    check_permission(&user, Permission::ViewDashboard)?;
    
    state.dashboard_service.get_metrics().await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
