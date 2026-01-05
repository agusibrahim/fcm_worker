use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MessageLog {
    pub id: String,
    pub credential_id: String,
    pub fcm_message_id: Option<String>,
    pub payload: String,
    pub webhook_status: Option<i32>,
    pub webhook_response: Option<String>,
    pub received_at: DateTime<Utc>,
}

impl MessageLog {
    pub fn new(credential_id: String, fcm_message_id: Option<String>, payload: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            credential_id,
            fcm_message_id,
            payload,
            webhook_status: None,
            webhook_response: None,
            received_at: Utc::now(),
        }
    }

    /// Extract fcmMessageId from payload JSON
    pub fn extract_fcm_message_id(payload: &str) -> Option<String> {
        serde_json::from_str::<serde_json::Value>(payload)
            .ok()
            .and_then(|v| v.get("fcmMessageId").and_then(|id| id.as_str().map(|s| s.to_string())))
    }
}

/// Message log response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MessageLogResponse {
    /// Unique message ID
    pub id: String,
    /// Associated credential ID
    pub credential_id: String,
    /// FCM message ID for deduplication
    pub fcm_message_id: Option<String>,
    /// FCM message payload
    pub payload: serde_json::Value,
    /// HTTP status code from webhook delivery
    pub webhook_status: Option<i32>,
    /// Response body from webhook
    pub webhook_response: Option<String>,
    /// When the message was received
    pub received_at: DateTime<Utc>,
}

impl MessageLog {
    pub fn to_response(&self) -> MessageLogResponse {
        MessageLogResponse {
            id: self.id.clone(),
            credential_id: self.credential_id.clone(),
            fcm_message_id: self.fcm_message_id.clone(),
            payload: serde_json::from_str(&self.payload).unwrap_or(serde_json::json!({})),
            webhook_status: self.webhook_status,
            webhook_response: self.webhook_response.clone(),
            received_at: self.received_at,
        }
    }
}
