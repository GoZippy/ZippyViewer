//! Web UI for token management (optional feature)

#[cfg(feature = "web-ui")]
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post, Router},
};
#[cfg(feature = "web-ui")]
use std::sync::Arc;
#[cfg(feature = "web-ui")]
use std::collections::HashMap;

#[cfg(feature = "web-ui")]
use crate::discovery::DiscoveryManager;
#[cfg(feature = "web-ui")]
use crate::access::AccessController;
#[cfg(feature = "web-ui")]
use std::time::Duration;

#[cfg(feature = "web-ui")]
#[derive(Clone)]
pub struct WebUIState {
    pub discovery_mgr: Arc<DiscoveryManager>,
    pub access_ctrl: Arc<AccessController>,
}

#[cfg(feature = "web-ui")]
/// Create Web UI router
pub fn create_router(
    discovery_mgr: Arc<DiscoveryManager>,
    access_ctrl: Arc<AccessController>,
) -> Router {
    let state = WebUIState {
        discovery_mgr,
        access_ctrl,
    };

    Router::new()
        .route("/ui", get(dashboard_handler))
        .route("/ui/tokens", get(tokens_handler))
        .route("/ui/tokens/create", post(create_token_handler))
        .route("/ui/tokens/:token_id/revoke", post(revoke_token_handler))
        .route("/ui/tokens/:token_id/qr", get(qr_code_handler))
        .with_state(state.clone())
}

#[cfg(feature = "web-ui")]
async fn dashboard_handler() -> Html<&'static str> {
    Html(r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>ZRC Directory Node</title>
        <style>
            body { font-family: sans-serif; max-width: 800px; margin: 40px auto; }
            h1 { color: #333; }
            .section { margin: 20px 0; padding: 20px; border: 1px solid #ddd; border-radius: 5px; }
            button { padding: 10px 20px; background: #007bff; color: white; border: none; border-radius: 3px; cursor: pointer; }
            button:hover { background: #0056b3; }
        </style>
    </head>
    <body>
        <h1>ZRC Directory Node</h1>
        <div class="section">
            <h2>Dashboard</h2>
            <p>Directory node is running.</p>
            <a href="/ui/tokens"><button>Manage Tokens</button></a>
        </div>
    </body>
    </html>
    "#)
}

#[cfg(feature = "web-ui")]
async fn tokens_handler(State(_state): State<WebUIState>) -> Response {
    // Get list of active tokens (simplified - show all for now)
    // In a real implementation, you'd want to track and display tokens per subject
    let html = r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Token Management - ZRC Directory Node</title>
        <style>
            body { font-family: sans-serif; max-width: 800px; margin: 40px auto; }
            h1 { color: #333; }
            .section { margin: 20px 0; padding: 20px; border: 1px solid #ddd; border-radius: 5px; }
            form { margin: 20px 0; }
            label { display: block; margin: 10px 0 5px 0; }
            input, select { width: 100%; padding: 8px; margin-bottom: 10px; }
            button { padding: 10px 20px; background: #007bff; color: white; border: none; border-radius: 3px; cursor: pointer; margin: 5px; }
            button:hover { background: #0056b3; }
            .danger { background: #dc3545; }
            .danger:hover { background: #c82333; }
            table { width: 100%; border-collapse: collapse; margin: 20px 0; }
            th, td { padding: 10px; text-align: left; border-bottom: 1px solid #ddd; }
            th { background: #f5f5f5; }
        </style>
    </head>
    <body>
        <h1>Token Management</h1>
        <div class="section">
            <h2>Create Discovery Token</h2>
            <form action="/ui/tokens/create" method="post">
                <label>Subject ID (hex):</label>
                <input type="text" name="subject_id" required placeholder="64 hex characters">
                <label>TTL (seconds):</label>
                <input type="number" name="ttl_seconds" value="600" min="60" max="3600">
                <label>Scope:</label>
                <select name="scope">
                    <option value="pairing_only">Pairing Only</option>
                    <option value="session_only">Session Only</option>
                    <option value="full">Full</option>
                </select>
                <button type="submit">Create Token</button>
            </form>
        </div>
        <div class="section">
            <h2>Active Tokens</h2>
            <p><em>Note: Token listing and revocation UI is available via API endpoints.</em></p>
            <p>To revoke a token, use: <code>POST /ui/tokens/{token_id_hex}/revoke</code></p>
            <p>To view QR code, use: <code>GET /ui/tokens/{token_id_hex}/qr</code></p>
        </div>
        <div class="section">
            <a href="/ui"><button>Back to Dashboard</button></a>
        </div>
    </body>
    </html>
    "#;
    Html(html).into_response()
}

#[cfg(feature = "web-ui")]
async fn create_token_handler(
    State(state): State<WebUIState>,
    axum::extract::Form(form): axum::extract::Form<HashMap<String, String>>,
) -> Response {
    let subject_id_hex = form.get("subject_id").map(|s| s.as_str()).unwrap_or("");
    let ttl_seconds = form.get("ttl_seconds")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(600);
    let scope_str = form.get("scope").map(|s| s.as_str()).unwrap_or("pairing_only");

    // Parse subject_id
    let subject_id = match hex::decode(subject_id_hex) {
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
    let scope = match scope_str {
        "pairing_only" => zrc_proto::v1::DiscoveryScopeV1::PairingOnly as i32,
        "session_only" => zrc_proto::v1::DiscoveryScopeV1::SessionOnly as i32,
        "full" => zrc_proto::v1::DiscoveryScopeV1::Full as i32,
        _ => zrc_proto::v1::DiscoveryScopeV1::PairingOnly as i32,
    };

    let ttl = std::time::Duration::from_secs(ttl_seconds);

    match state.discovery_mgr.create(subject_id, ttl, scope, None) {
        Ok(token) => {
            let token_id_hex = hex::encode(&token.token_id);
            
            // Generate QR code if qrcode feature is available
            let qr_code_html = if let Ok(qr_svg) = generate_qr_code(&token_id_hex) {
                format!(r#"
                <div style="margin: 20px 0;">
                    <h3>QR Code</h3>
                    <div style="display: inline-block;">
                        {}
                    </div>
                    <p><small>Scan to get token ID: <code>{}</code></small></p>
                </div>
                "#, qr_svg, token_id_hex)
            } else {
                String::new()
            };
            
            let html_content = format!(r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Token Created - ZRC Directory Node</title>
                <style>
                    body {{ font-family: sans-serif; max-width: 800px; margin: 40px auto; }}
                    .token {{ background: #f5f5f5; padding: 20px; border-radius: 5px; margin: 20px 0; }}
                    code {{ background: #e9e9e9; padding: 2px 6px; border-radius: 3px; }}
                    button {{ padding: 10px 20px; background: #007bff; color: white; border: none; border-radius: 3px; cursor: pointer; margin: 5px; }}
                    button:hover {{ background: #0056b3; }}
                </style>
            </head>
            <body>
                <h1>Discovery Token Created</h1>
                <div class="token">
                    <p><strong>Token ID:</strong> <code>{}</code></p>
                    <p><strong>Subject ID:</strong> <code>{}</code></p>
                    <p><strong>Expires At:</strong> {}</p>
                    {}
                </div>
                <div>
                    <a href="/ui/tokens/{}?qr=1"><button>View QR Code</button></a>
                    <a href="/ui/tokens"><button>Back to Tokens</button></a>
                </div>
            </body>
            </html>
            "#, token_id_hex, subject_id_hex, token.expires_at, qr_code_html, token_id_hex);
            Html(html_content).into_response()
        }
        Err(e) => {
            (StatusCode::BAD_REQUEST, format!("Error creating token: {}", e)).into_response()
        }
    }
}

#[cfg(feature = "web-ui")]
fn generate_qr_code(data: &str) -> Result<String, Box<dyn std::error::Error>> {
    use qrcode::QrCode;
    use qrcode::render::svg;
    
    let code = QrCode::new(data.as_bytes())?;
    let image = code.render()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#FFFFFF"))
        .build();
    
    // Return SVG string directly
    Ok(image)
}

#[cfg(not(feature = "web-ui"))]
fn generate_qr_code(_data: &str) -> Result<String, Box<dyn std::error::Error>> {
    Err("QR code feature not enabled".into())
}

#[cfg(feature = "web-ui")]
async fn qr_code_handler(
    State(_state): State<WebUIState>,
    Path(token_id_hex): Path<String>,
) -> Response {
    match generate_qr_code(&token_id_hex) {
        Ok(svg) => {
            let html = format!(r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>QR Code - ZRC Directory Node</title>
                <style>
                    body {{ font-family: sans-serif; max-width: 800px; margin: 40px auto; text-align: center; }}
                    .qr-container {{ margin: 20px 0; }}
                    button {{ padding: 10px 20px; background: #007bff; color: white; border: none; border-radius: 3px; cursor: pointer; }}
                </style>
            </head>
            <body>
                <h1>QR Code for Token</h1>
                <div class="qr-container">
                    {}
                </div>
                <p><code>{}</code></p>
                <a href="/ui/tokens"><button>Back to Tokens</button></a>
            </body>
            </html>
            "#, svg, token_id_hex);
            Html(html).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate QR code").into_response()
    }
}

#[cfg(feature = "web-ui")]
async fn revoke_token_handler(
    State(state): State<WebUIState>,
    Path(token_id_hex): Path<String>,
) -> Response {
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
            let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Token Revoked - ZRC Directory Node</title>
                <style>
                    body { font-family: sans-serif; max-width: 800px; margin: 40px auto; text-align: center; }
                    .success { background: #d4edda; color: #155724; padding: 20px; border-radius: 5px; margin: 20px 0; }
                    button { padding: 10px 20px; background: #007bff; color: white; border: none; border-radius: 3px; cursor: pointer; }
                </style>
            </head>
            <body>
                <h1>Token Revoked</h1>
                <div class="success">
                    <p>Token has been successfully revoked.</p>
                </div>
                <a href="/ui/tokens"><button>Back to Tokens</button></a>
            </body>
            </html>
            "#;
            Html(html).into_response()
        }
        Err(e) => {
            (StatusCode::BAD_REQUEST, format!("Error revoking token: {}", e)).into_response()
        }
    }
}

#[cfg(not(feature = "web-ui"))]
use axum::Router;
#[cfg(not(feature = "web-ui"))]
use std::sync::Arc;
#[cfg(not(feature = "web-ui"))]
use crate::discovery::DiscoveryManager;
#[cfg(not(feature = "web-ui"))]
use crate::access::AccessController;

#[cfg(not(feature = "web-ui"))]
pub fn create_router(
    _discovery_mgr: Arc<DiscoveryManager>,
    _access_ctrl: Arc<AccessController>,
) -> Router {
    Router::new()
}
