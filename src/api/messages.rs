use crate::api::AppState;
use crate::error::{AppError, AppResult};
use crate::models::MessageLogResponse;
use crate::workers::WebhookClient;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::{IntoParams, ToSchema};

/// Query parameters for listing messages
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct ListMessagesQuery {
    /// Filter by credential ID
    pub credential_id: Option<String>,
    /// Number of messages to return (default: 50)
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// Offset for pagination
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

/// Response containing list of messages
#[derive(Debug, Serialize, ToSchema)]
pub struct ListMessagesResponse {
    /// List of messages
    pub messages: Vec<MessageLogResponse>,
    /// Total count
    pub total: i64,
    /// Limit used
    pub limit: i64,
    /// Offset used
    pub offset: i64,
}

/// List message logs with pagination
#[utoipa::path(
    get,
    path = "/api/messages",
    tag = "messages",
    params(ListMessagesQuery),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "List of messages", body = ListMessagesResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_messages(
    State(state): State<AppState>,
    Query(query): Query<ListMessagesQuery>,
) -> AppResult<Json<ListMessagesResponse>> {
    let messages = state
        .repo
        .list_message_logs(query.credential_id.as_deref(), query.limit, query.offset)
        .await?;

    let total = state
        .repo
        .count_message_logs(query.credential_id.as_deref())
        .await?;

    let responses: Vec<MessageLogResponse> = messages.iter().map(|m| m.to_response()).collect();

    Ok(Json(ListMessagesResponse {
        messages: responses,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

/// Get a single message
#[utoipa::path(
    get,
    path = "/api/messages/{id}",
    tag = "messages",
    params(
        ("id" = String, Path, description = "Message ID")
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Message details", body = MessageLogResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Message not found")
    )
)]
pub async fn get_message(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<MessageLogResponse>> {
    let messages = state.repo.list_message_logs(None, 1, 0).await?;
    
    // Find the specific message (we need to query by ID)
    let message = messages
        .into_iter()
        .find(|m| m.id == id)
        .ok_or_else(|| AppError::NotFound(format!("Message {} not found", id)))?;

    Ok(Json(message.to_response()))
}

/// Response for webhook retry
#[derive(Debug, Serialize, ToSchema)]
pub struct RetryWebhookResponse {
    /// Status message
    pub message: String,
    /// HTTP status code from webhook (if available)
    pub status: Option<i32>,
}

/// Retry webhook delivery for a failed message
#[utoipa::path(
    post,
    path = "/api/messages/{id}/retry",
    tag = "messages",
    params(
        ("id" = String, Path, description = "Message ID")
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Webhook retry completed", body = RetryWebhookResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Message not found")
    )
)]
pub async fn retry_webhook(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<RetryWebhookResponse>> {
    // Get the message log
    let messages = state.repo.list_message_logs(None, 1000, 0).await?;
    let mut message = messages
        .into_iter()
        .find(|m| m.id == id)
        .ok_or_else(|| AppError::NotFound(format!("Message {} not found", id)))?;

    // Get the credential for webhook URL
    let credential = state
        .repo
        .get_credential(&message.credential_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Credential {} not found for message",
                message.credential_id
            ))
        })?;

    // Retry the webhook
    let webhook_client = WebhookClient::new();
    webhook_client
        .retry_message(
            &mut message,
            &credential.webhook_url,
            credential.get_webhook_headers().as_ref(),
            &state.repo,
        )
        .await?;

    info!("Retried webhook for message: {}", id);

    Ok(Json(RetryWebhookResponse {
        message: format!("Webhook retry completed for message {}", id),
        status: message.webhook_status,
    }))
}

/// Response for clear messages
#[derive(Debug, Serialize, ToSchema)]
pub struct ClearMessagesResponse {
    /// Credential ID
    pub credential_id: String,
    /// Number of messages deleted
    pub deleted: u64,
    /// Status message
    pub message: String,
}

/// Clear all messages for a credential
#[utoipa::path(
    delete,
    path = "/api/credentials/{id}/messages",
    tag = "messages",
    params(
        ("id" = String, Path, description = "Credential ID")
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Messages cleared", body = ClearMessagesResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Credential not found")
    )
)]
pub async fn clear_messages(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ClearMessagesResponse>> {
    // Check credential exists
    state
        .repo
        .get_credential(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Credential {} not found", id)))?;

    // Clear messages
    let deleted = state.repo.clear_credential_messages(&id).await?;

    info!("Cleared {} messages for credential: {}", deleted, id);

    Ok(Json(ClearMessagesResponse {
        credential_id: id.clone(),
        deleted,
        message: format!("{} messages cleared for credential {}", deleted, id),
    }))
}
