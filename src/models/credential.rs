use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Credential {
    pub id: String,
    pub name: String,
    pub api_key: String,
    pub app_id: String,
    pub project_id: String,
    pub fcm_token: Option<String>,
    pub gcm_token: Option<String>,
    pub android_id: Option<i64>,
    pub security_token: Option<i64>,
    pub private_key_base64: Option<String>,
    pub auth_secret_base64: Option<String>,
    pub webhook_url: String,
    pub webhook_headers: Option<String>,
    pub is_active: bool,
    pub is_suspended: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new FCM credential
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateCredentialRequest {
    /// Display name for this credential
    #[schema(example = "My App FCM")]
    pub name: String,
    /// Firebase API key
    #[schema(example = "AIzaSy...")]
    pub api_key: String,
    /// Firebase App ID
    #[schema(example = "1:123456789:android:abc123")]
    pub app_id: String,
    /// Firebase Project ID
    #[schema(example = "my-project-id")]
    pub project_id: String,
    /// Webhook URL to call when messages arrive
    #[schema(example = "https://webhook.site/xxx")]
    pub webhook_url: String,
    /// Optional custom headers for webhook requests
    #[serde(default)]
    pub webhook_headers: Option<HashMap<String, String>>,
    /// Topics to subscribe to
    #[serde(default)]
    #[schema(example = json!(["notifications", "promotions"]))]
    pub topics: Vec<String>,
}

/// Request to update an existing credential
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateCredentialRequest {
    /// New display name
    pub name: Option<String>,
    /// New webhook URL
    pub webhook_url: Option<String>,
    /// New custom headers
    pub webhook_headers: Option<HashMap<String, String>>,
    /// Set active status
    pub is_active: Option<bool>,
    /// New topics to subscribe to
    pub topics: Option<Vec<String>>,
    /// Firebase API key (update)
    pub api_key: Option<String>,
    /// Firebase App ID (update)  
    pub app_id: Option<String>,
    /// Firebase Project ID (update)
    pub project_id: Option<String>,
}

/// Credential response with status
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CredentialResponse {
    /// Unique credential ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Firebase API key
    pub api_key: String,
    /// Firebase App ID
    pub app_id: String,
    /// Firebase Project ID
    pub project_id: String,
    /// FCM token (generated after registration)
    pub fcm_token: Option<String>,
    /// Android ID (generated after registration)
    pub android_id: Option<i64>,
    /// Webhook URL
    pub webhook_url: String,
    /// Custom webhook headers
    pub webhook_headers: Option<HashMap<String, String>>,
    /// Whether credential is active
    pub is_active: bool,
    /// Whether worker is suspended (won't auto-start on server boot)
    pub is_suspended: bool,
    /// Whether FCM listener is currently running
    pub is_listening: bool,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl Credential {
    pub fn new(req: CreateCredentialRequest) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name: req.name,
            api_key: req.api_key,
            app_id: req.app_id,
            project_id: req.project_id,
            fcm_token: None,
            gcm_token: None,
            android_id: None,
            security_token: None,
            private_key_base64: None,
            auth_secret_base64: None,
            webhook_url: req.webhook_url,
            webhook_headers: req
                .webhook_headers
                .map(|h| serde_json::to_string(&h).unwrap_or_default()),
            is_active: true,
            is_suspended: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn get_webhook_headers(&self) -> Option<HashMap<String, String>> {
        self.webhook_headers
            .as_ref()
            .and_then(|h| serde_json::from_str(h).ok())
    }

    pub fn to_response(&self, is_listening: bool) -> CredentialResponse {
        CredentialResponse {
            id: self.id.clone(),
            name: self.name.clone(),
            api_key: self.api_key.clone(),
            app_id: self.app_id.clone(),
            project_id: self.project_id.clone(),
            fcm_token: self.fcm_token.clone(),
            android_id: self.android_id,
            webhook_url: self.webhook_url.clone(),
            webhook_headers: self.get_webhook_headers(),
            is_active: self.is_active,
            is_suspended: self.is_suspended,
            is_listening,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    /// Check if worker can be started (active and not suspended)
    #[allow(dead_code)]
    pub fn can_start(&self) -> bool {
        self.is_active && !self.is_suspended
    }
}
