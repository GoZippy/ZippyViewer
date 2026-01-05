mod api;
mod auth;
mod db;
mod services;
mod assets;

use axum::routing::get;

use std::net::SocketAddr;
use tracing::info;
use crate::db::store::DbStore;
use crate::auth::session::SessionService;
use crate::auth::service::AuthService;
use crate::services::{device::DeviceService, pairing::PairingService, audit::AuditService, infrastructure::InfrastructureService, updates::UpdateService, dashboard::DashboardService, api_keys::ApiKeyService};
use crate::api::router::AppState;
use crate::db::schema::UserRole;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Configuration (Env vars)
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://admin.db".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3001".to_string()).parse::<u16>()?;

    info!("Initializing ZRC Admin Console on port {}...", port);
    
    // Database
    let db = DbStore::new(&database_url).await?;
    db.run_migrations().await?;
    
    // Services
    let auth_service = AuthService::new(db.clone());
    let session_service = SessionService::new(db.clone());
    let device_service = DeviceService::new(db.clone());
    let pairing_service = PairingService::new(db.clone());
    let audit_service = AuditService::new(db.clone());
    let infrastructure_service = InfrastructureService::new(db.clone());
    let update_service = UpdateService::new(db.clone());
    let dashboard_service = DashboardService::new(db.clone());
    let api_key_service = ApiKeyService::new(db.clone());
    
    // Seed default admin if not exists (Basic check)
    // In production we'd use a CLI command, for MVP we check on startup
    if let Err(_) = auth_service.authenticate("admin", "admin123").await {
         // Could check if *any* user exists, but for now specific check
         // Actually authenticate fails if bad password too. 
         // Let's rely on a specific check method or just try to create and ignore error?
         // For now, let's create if auth fails? No, that resets password.
         // Let's add a `get_user_by_username` to AuthService later.
         // For MVP, lets just try to create. 
         // Unique constraint will fail if exists.
         let _ = auth_service.create_user("admin", "admin123", UserRole::SuperAdmin).await;
    }

    // Router
    let state = AppState {
        db,
        auth_service,
        session_service,
        device_service,
        pairing_service,
        audit_service,
        infrastructure_service,
        update_service,
        dashboard_service,
        api_key_service,
    };
    
    use utoipa::OpenApi;
    use utoipa_swagger_ui::SwaggerUi;

    #[derive(OpenApi)]
    #[openapi(
        paths(),
        components(),
        tags(
            (name = "zrc-admin", description = "ZRC Admin Console API")
        )
    )]
    struct ApiDoc;
    
    // Router
    let api_router = api::router::create_router(state)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));
    
    // Serve embedded assets (SPA Support)
    let app = axum::Router::new()
        .merge(api_router)
        .fallback(get(assets::static_handler));

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    
    let tls_cert = std::env::var("TLS_CERT_PATH").ok();
    let tls_key = std::env::var("TLS_KEY_PATH").ok();

    if let (Some(cert_path), Some(key_path)) = (tls_cert, tls_key) {
        info!("Starting ZRC Admin Console on https://{} (TLS Enabled)", addr);
        use axum_server::tls_rustls::RustlsConfig;

        let config = RustlsConfig::from_pem_file(
            std::path::PathBuf::from(cert_path),
            std::path::PathBuf::from(key_path),
        )
        .await?;

        axum_server::bind_rustls(addr, config)
            .serve(app.into_make_service())
            .await?;
    } else {
        info!("Starting ZRC Admin Console on http://{}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
    }

    Ok(())
}
