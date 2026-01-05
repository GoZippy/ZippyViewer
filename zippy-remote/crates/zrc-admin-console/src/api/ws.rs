use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::Response,
};
use crate::api::router::AppState;
use std::time::Duration;
use tokio::time::sleep;

// Handler to upgrade connection
#[utoipa::path(
    get,
    path = "/ws/dashboard",
    tag = "zrc-admin",
    responses(
        (status = 101, description = "WebSocket upgrade")
    )
)]
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    loop {
        // Fetch stats
        if let Ok(stats) = state.dashboard_service.get_stats().await {
            if let Ok(json) = serde_json::to_string(&stats) {
                if socket.send(Message::Text(json)).await.is_err() {
                    break; 
                }
            }
        }
        
        sleep(Duration::from_secs(5)).await;
    }
}
