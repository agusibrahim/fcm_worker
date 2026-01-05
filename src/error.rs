use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::fmt;

/// Application-wide error types
#[derive(Debug)]
#[allow(dead_code)]
pub enum AppError {
    // Database errors
    Database(String),

    // FCM client errors
    FcmRegistration(String),
    FcmConnection(String),
    FcmDecryption(String),

    // Webhook errors
    WebhookRequest(String),
    WebhookTimeout(String),
    WebhookInvalidUrl(String),

    // API errors
    NotFound(String),
    BadRequest(String),
    Conflict(String),
    Internal(String),

    // Worker errors
    WorkerNotRunning(String),
    WorkerAlreadyRunning(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Database(msg) => write!(f, "Database error: {}", msg),
            AppError::FcmRegistration(msg) => write!(f, "FCM registration error: {}", msg),
            AppError::FcmConnection(msg) => write!(f, "FCM connection error: {}", msg),
            AppError::FcmDecryption(msg) => write!(f, "FCM decryption error: {}", msg),
            AppError::WebhookRequest(msg) => write!(f, "Webhook request error: {}", msg),
            AppError::WebhookTimeout(msg) => write!(f, "Webhook timeout: {}", msg),
            AppError::WebhookInvalidUrl(msg) => write!(f, "Invalid webhook URL: {}", msg),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            AppError::Conflict(msg) => write!(f, "Conflict: {}", msg),
            AppError::Internal(msg) => write!(f, "Internal error: {}", msg),
            AppError::WorkerNotRunning(msg) => write!(f, "Worker not running: {}", msg),
            AppError::WorkerAlreadyRunning(msg) => write!(f, "Worker already running: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match &self {
            AppError::Database(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "database_error", msg.clone()),
            AppError::FcmRegistration(msg) => (StatusCode::BAD_GATEWAY, "fcm_registration_error", msg.clone()),
            AppError::FcmConnection(msg) => (StatusCode::BAD_GATEWAY, "fcm_connection_error", msg.clone()),
            AppError::FcmDecryption(msg) => (StatusCode::BAD_GATEWAY, "fcm_decryption_error", msg.clone()),
            AppError::WebhookRequest(msg) => (StatusCode::BAD_GATEWAY, "webhook_error", msg.clone()),
            AppError::WebhookTimeout(msg) => (StatusCode::GATEWAY_TIMEOUT, "webhook_timeout", msg.clone()),
            AppError::WebhookInvalidUrl(msg) => (StatusCode::BAD_REQUEST, "invalid_webhook_url", msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg.clone()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", msg.clone()),
            AppError::WorkerNotRunning(msg) => (StatusCode::BAD_REQUEST, "worker_not_running", msg.clone()),
            AppError::WorkerAlreadyRunning(msg) => (StatusCode::CONFLICT, "worker_already_running", msg.clone()),
        };

        let body = Json(json!({
            "error": {
                "type": error_type,
                "message": message,
            }
        }));

        (status, body).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Database(err.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            AppError::WebhookTimeout(err.to_string())
        } else {
            AppError::WebhookRequest(err.to_string())
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
