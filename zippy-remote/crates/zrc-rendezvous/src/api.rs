use axum::{
    body::Bytes,
    extract::{Path, Query, State, ConnectInfo},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Instant};
use tokio::time::Duration;

use crate::{
    auth::{extract_bearer_token, AuthConfig},
    mailbox::{MailboxError, MailboxMap},
    metrics::MailboxMetrics,
    rate_limit::RateLimiter,
};

#[derive(Clone)]
pub struct AppState {
    pub mailboxes: MailboxMap,
    pub rate_limiter: RateLimiter,
    pub auth: AuthConfig,
    pub metrics: Arc<MailboxMetrics>,
    pub config: crate::config::ServerConfig,
    pub shutdown: tokio::sync::watch::Receiver<bool>,
}

// POST /v1/mailbox/{recipient_id_hex}
pub async fn post_mailbox(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>, // Extract IP
    Path(rid_hex): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let start = Instant::now();

    let ip = addr.ip();

    // Check rate limit
    match state.rate_limiter.check_post(ip).await {
        Ok(()) => {}
        Err(retry_after) => {
            state.metrics.rate_limit_hits.inc();
            let mut response = (StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded").into_response();
            response.headers_mut().insert(
                "Retry-After",
                HeaderValue::from_str(&retry_after.to_string()).unwrap(),
            );
            return response;
        }
    }

    // Check authentication
    let token = extract_bearer_token(headers.get("authorization"));
    if let Err(e) = state.auth.validate(token, None) {
        state.metrics.error_counts.inc();
        return match e {
            crate::auth::AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "missing token").into_response(),
            crate::auth::AuthError::InvalidToken => (StatusCode::FORBIDDEN, "invalid token").into_response(),
            _ => (StatusCode::BAD_REQUEST, "auth error").into_response(),
        };
    }

    // Parse recipient ID
    let rid = match hex::decode(&rid_hex) {
        Ok(b) => b,
        Err(_) => {
            state.metrics.error_counts.inc();
            return (StatusCode::BAD_REQUEST, "bad recipient id hex").into_response();
        }
    };

    if rid.len() != 32 {
        state.metrics.error_counts.inc();
        return (StatusCode::BAD_REQUEST, "recipient id must be 32 bytes").into_response();
    }

    // Post message
    let result = {
        let mut mailbox_entry = state.mailboxes.entry(rid.clone()).or_insert_with(crate::mailbox::Mailbox::new);
        mailbox_entry.value_mut().post(body, state.config.max_queue_length, state.config.max_message_size)
    };
    
    match result {
        Ok(_sequence) => {
            state.metrics.messages_posted.inc();
            state.metrics.messages_posted.inc();
            {
                state.metrics.active_mailboxes.set(state.mailboxes.len() as f64);
                let total: usize = state.mailboxes.iter().map(|e| e.value().queue_length()).sum();
                state.metrics.total_messages.set(total as f64);
            }
            
            let latency = start.elapsed().as_secs_f64();
            state.metrics.request_latency.observe(latency);
            
            (StatusCode::ACCEPTED, "ok").into_response()
        }
        Err(MailboxError::MessageTooLarge) => {
            state.metrics.error_counts.inc();
            (StatusCode::PAYLOAD_TOO_LARGE, "message too large").into_response()
        }
        Err(MailboxError::QueueFull) => {
            state.metrics.error_counts.inc();
            (StatusCode::INSUFFICIENT_STORAGE, "queue full").into_response()
        }
    }
}

// GET /v1/mailbox/{recipient_id_hex}?wait_ms=25000
pub async fn get_mailbox(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>, // Extract IP
    Path(rid_hex): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let start = Instant::now();

    // Check shutdown
    if *state.shutdown.borrow() {
        return (StatusCode::SERVICE_UNAVAILABLE, "server shutting down").into_response();
    }

    let ip = addr.ip();

    // Check rate limit
    match state.rate_limiter.check_get(ip).await {
        Ok(()) => {}
        Err(retry_after) => {
            state.metrics.rate_limit_hits.inc();
            let mut response = (StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded").into_response();
            response.headers_mut().insert(
                "Retry-After",
                HeaderValue::from_str(&retry_after.to_string()).unwrap(),
            );
            return response;
        }
    }

    // Parse wait_ms
    let wait_ms: u64 = params
        .get("wait_ms")
        .and_then(|s| s.parse().ok())
        .unwrap_or(25_000)
        .min(60_000);

    // Parse recipient ID
    let rid = match hex::decode(&rid_hex) {
        Ok(b) => b,
        Err(_) => {
            state.metrics.error_counts.inc();
            return (StatusCode::BAD_REQUEST, "bad recipient id hex").into_response();
        }
    };

    if rid.len() != 32 {
        state.metrics.error_counts.inc();
        return (StatusCode::BAD_REQUEST, "recipient id must be 32 bytes").into_response();
    }

    // Check authentication
    let token = extract_bearer_token(headers.get("authorization"));
    if let Err(e) = state.auth.validate(token, Some(&rid)) {
        state.metrics.error_counts.inc();
        return match e {
            crate::auth::AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "missing token").into_response(),
            crate::auth::AuthError::InvalidToken => (StatusCode::FORBIDDEN, "invalid token").into_response(),
            _ => (StatusCode::BAD_REQUEST, "auth error").into_response(),
        };
    }

    // Try immediate get
    let immediate_result: Option<(Bytes, u64, usize)> = { // Removed 'total' from tuple here to avoid deadlock
        if let Some(mut mailbox_entry) = state.mailboxes.get_mut(&rid) {
            let mailbox = mailbox_entry.value_mut();
            if let Some(message) = mailbox.get() {
                let queue_len = mailbox.queue_length();
                Some((message.data.clone(), message.sequence, queue_len))
            } else {
                None
            }
        } else {
            None
        }
    };
    
    if let Some((data, sequence, queue_len)) = immediate_result {
        // Calculate total outside the lock
        let total: usize = state.mailboxes.iter().map(|e| e.value().queue_length()).sum();

        state.metrics.messages_delivered.inc();
        state.metrics.total_messages.set(total as f64);
        
        let latency = start.elapsed().as_secs_f64();
        state.metrics.request_latency.observe(latency);
        
        let mut response = (StatusCode::OK, data).into_response();
        response.headers_mut().insert(
            "X-Message-Sequence",
            HeaderValue::from_str(&sequence.to_string()).unwrap(),
        );
        response.headers_mut().insert(
            "X-Queue-Length",
            HeaderValue::from_str(&queue_len.to_string()).unwrap(),
        );
        return response;
    }

    // Long poll
    if wait_ms > 0 {
        let notify = {
            let notify = match state.mailboxes.entry(rid.clone()) {
                dashmap::mapref::entry::Entry::Occupied(o) => o.get().notify.clone(),
                dashmap::mapref::entry::Entry::Vacant(v) => {
                    let mailbox = crate::mailbox::Mailbox::new();
                    let notify = mailbox.notify.clone();
                    v.insert(mailbox);
                    notify
                }
            };
            notify
        };

        let notified = notify.notified();
        let timeout = tokio::time::sleep(Duration::from_millis(wait_ms));
        tokio::pin!(timeout);

        tokio::select! {
            _ = notified => {
                // Check shutdown
                if *state.shutdown.borrow() {
                    return (StatusCode::SERVICE_UNAVAILABLE, "server shutting down").into_response();
                }
                
                let result: Option<(Bytes, u64, usize)> = {
                    if let Some(mut mailbox_entry) = state.mailboxes.get_mut(&rid) {
                        let mailbox = mailbox_entry.value_mut();
                        if let Some(message) = mailbox.get() {
                            let queue_len = mailbox.queue_length();
                            Some((message.data.clone(), message.sequence, queue_len))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                
                if let Some((data, sequence, queue_len)) = result {
                    // Calculate total outside lock
                    let total: usize = state.mailboxes.iter().map(|e| e.value().queue_length()).sum();
                    state.metrics.messages_delivered.inc();
                    state.metrics.total_messages.set(total as f64);
                    
                    let latency = start.elapsed().as_secs_f64();
                    state.metrics.request_latency.observe(latency);
                    
                    let mut response = (StatusCode::OK, data).into_response();
                    response.headers_mut().insert(
                        "X-Message-Sequence",
                        HeaderValue::from_str(&sequence.to_string()).unwrap(),
                    );
                    response.headers_mut().insert(
                        "X-Queue-Length",
                        HeaderValue::from_str(&queue_len.to_string()).unwrap(),
                    );
                    return response;
                }
            }
            _ = &mut timeout => {}
            _ = async {
                let mut rx = state.shutdown.clone();
                let _ = rx.changed().await;
            } => {
                if *state.shutdown.borrow() {
                    return (StatusCode::SERVICE_UNAVAILABLE, "server shutting down").into_response();
                }
            }
        }
    }

    // No content
    let latency = start.elapsed().as_secs_f64();
    state.metrics.request_latency.observe(latency);
    (StatusCode::NO_CONTENT, Bytes::new()).into_response()
}

// GET /health
pub async fn get_health(State(_state): State<AppState>) -> Response {
    use serde_json::json;
    
    // For now, use a simple uptime calculation
    // In production, you'd track this in metrics
    let response = json!({
        "status": "healthy",
        "uptime_seconds": 0.0,
        "version": env!("CARGO_PKG_VERSION"),
    });
    
    (StatusCode::OK, axum::Json(response)).into_response()
}

// GET /metrics
pub async fn get_metrics(State(state): State<AppState>) -> Response {
    let prometheus = state.metrics.export_prometheus();
    (
        StatusCode::OK,
        [("Content-Type", "text/plain; version=0.0.4")],
        prometheus,
    )
        .into_response()
}
