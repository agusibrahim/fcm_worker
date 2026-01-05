use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use serde_json::json;
use std::sync::Arc;

/// API Key configuration
#[derive(Clone)]
pub struct ApiKeyConfig {
    pub api_key: Arc<String>,
}

impl ApiKeyConfig {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key: Arc::new(api_key),
        }
    }
}

/// Middleware to validate API key (using State extractor)
pub async fn api_key_auth(
    State(config): State<ApiKeyConfig>,
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Skip auth for health check and swagger endpoints
    let path = request.uri().path();
    if path == "/health" 
        || path.starts_with("/swagger-ui") 
        || path.starts_with("/api-docs") 
    {
        return Ok(next.run(request).await);
    }

    let expected_key = &config.api_key;

    // Check Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    // Also check X-API-Key header
    let x_api_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok());

    let provided_key = match (auth_header, x_api_key) {
        (Some(auth), _) if auth.starts_with("Bearer ") => {
            Some(auth.trim_start_matches("Bearer ").to_string())
        }
        (_, Some(key)) => Some(key.to_string()),
        _ => None,
    };

    match provided_key {
        Some(key) if key == **expected_key => Ok(next.run(request).await),
        Some(_) => Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "type": "unauthorized",
                    "message": "Invalid API key"
                }
            })),
        )),
        None => Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "type": "unauthorized",
                    "message": "API key required. Use 'Authorization: Bearer <key>' or 'X-API-Key: <key>' header"
                }
            })),
        )),
    }
}

/// Generate a random API key
pub fn generate_api_key() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}
