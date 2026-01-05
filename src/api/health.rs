use crate::api::AppState;
use crate::error::AppResult;
use axum::{extract::State, Json};
use serde::Serialize;
use utoipa::ToSchema;

/// Health check response
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Server status
    pub status: String,
    /// Server version
    pub version: String,
}

/// Basic health check
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Server is healthy", body = HealthResponse)
    )
)]
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Server statistics response
#[derive(Debug, Serialize, ToSchema)]
pub struct StatsResponse {
    /// Number of active FCM listeners
    pub active_listeners: usize,
    /// Total number of credentials
    pub total_credentials: i64,
    /// Number of active credentials
    pub active_credentials: i64,
    /// Total number of messages received
    pub total_messages: i64,
    /// Messages received in last 24 hours
    pub messages_last_24h: i64,
}

/// Get server statistics
#[utoipa::path(
    get,
    path = "/api/stats",
    tag = "health",
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Server statistics", body = StatsResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn get_stats(State(state): State<AppState>) -> AppResult<Json<StatsResponse>> {
    let pool = state.listener_pool.read().await;
    let active_listeners = pool.active_count().await;
    
    let all_credentials = state.repo.list_credentials(false).await?;
    let active_credentials = state.repo.list_credentials(true).await?;
    let total_messages = state.repo.count_message_logs(None).await?;

    // For messages in last 24h, we'd need a separate query
    // For now, just return total
    let messages_last_24h = total_messages;

    Ok(Json(StatsResponse {
        active_listeners,
        total_credentials: all_credentials.len() as i64,
        active_credentials: active_credentials.len() as i64,
        total_messages,
        messages_last_24h,
    }))
}
