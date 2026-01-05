use axum::{
    extract::State,
    Json,
    http::StatusCode,
    Extension,
};
use crate::api::router::AppState;
use crate::db::schema::{AuditLog, User};
use crate::auth::rbac::{Permission, check_permission};

pub async fn list_audit_logs(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Json<Vec<AuditLog>>, StatusCode> {
    check_permission(&user, Permission::ViewAuditLogs)?;

    state.audit_service.query_logs().await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

use axum::response::{IntoResponse, Response};
use axum::http::header;

pub async fn export_audit_logs(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
) -> Result<Response, StatusCode> {
    check_permission(&user, Permission::ViewAuditLogs)?;

    let csv = state.audit_service.export_logs_csv().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = (
        [(header::CONTENT_TYPE, "text/csv"), (header::CONTENT_DISPOSITION, "attachment; filename=\"audit_logs.csv\"")],
        csv
    ).into_response();

    Ok(response)
}
