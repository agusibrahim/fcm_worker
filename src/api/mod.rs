pub mod credentials;
pub mod health;
pub mod messages;

use crate::db::Repository;
use crate::middleware::ApiKeyConfig;
use crate::workers::ListenerPool;
use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

/// OpenAPI Documentation
#[derive(OpenApi)]
#[openapi(
    info(
        title = "FCM Multi-Credential Receiver API",
        version = "0.1.0",
        description = "REST API for managing multiple FCM credentials and receiving push notifications via webhooks",
        license(name = "MIT")
    ),
    servers(
        (url = "http://localhost:3000", description = "Local development server")
    ),
    tags(
        (name = "health", description = "Health check and statistics"),
        (name = "credentials", description = "FCM credential management"),
        (name = "messages", description = "Message log operations")
    ),
    paths(
        health::health_check,
        health::get_stats,
        credentials::list_credentials,
        credentials::create_credential,
        credentials::get_credential,
        credentials::update_credential,
        credentials::delete_credential,
        credentials::start_listener,
        credentials::stop_listener,
        credentials::restart_listener,
        credentials::suspend_credential,
        credentials::unsuspend_credential,
        messages::list_messages,
        messages::get_message,
        messages::retry_webhook,
        messages::clear_messages,
    ),
    components(
        schemas(
            health::HealthResponse,
            health::StatsResponse,
            credentials::ListCredentialsResponse,
            credentials::CreateCredentialResponse,
            credentials::ListQuery,
            crate::models::CreateCredentialRequest,
            crate::models::UpdateCredentialRequest,
            crate::models::CredentialResponse,
            messages::ListMessagesQuery,
            messages::ListMessagesResponse,
            messages::RetryWebhookResponse,
            messages::ClearMessagesResponse,
            crate::models::MessageLogResponse,
        )
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "api_key",
                utoipa::openapi::security::SecurityScheme::ApiKey(
                    utoipa::openapi::security::ApiKey::Header(
                        utoipa::openapi::security::ApiKeyValue::new("X-API-Key"),
                    ),
                ),
            );
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Bearer,
                    ),
                ),
            );
        }
    }
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub repo: Repository,
    pub listener_pool: Arc<RwLock<ListenerPool>>,
}

impl AppState {
    pub fn new(repo: Repository, listener_pool: ListenerPool) -> Self {
        Self {
            repo,
            listener_pool: Arc::new(RwLock::new(listener_pool)),
        }
    }
}

/// Build the API router
pub fn create_router(state: AppState, api_key_config: ApiKeyConfig) -> Router {
    // CORS must be the outermost layer (applied last, runs first)
    // This ensures OPTIONS preflight requests get CORS headers before hitting auth
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = Router::new()
        // Health endpoints
        .route("/health", get(health::health_check))
        .route("/api/stats", get(health::get_stats))
        // Credential endpoints
        .route("/api/credentials", get(credentials::list_credentials))
        .route("/api/credentials", post(credentials::create_credential))
        .route("/api/credentials/:id", get(credentials::get_credential))
        .route("/api/credentials/:id", put(credentials::update_credential))
        .route("/api/credentials/:id", delete(credentials::delete_credential))
        .route("/api/credentials/:id/start", post(credentials::start_listener))
        .route("/api/credentials/:id/stop", post(credentials::stop_listener))
        .route("/api/credentials/:id/restart", post(credentials::restart_listener))
        .route("/api/credentials/:id/suspend", post(credentials::suspend_credential))
        .route("/api/credentials/:id/unsuspend", post(credentials::unsuspend_credential))
        .route("/api/credentials/:id/messages", delete(messages::clear_messages))
        // Message endpoints
        .route("/api/messages", get(messages::list_messages))
        .route("/api/messages/:id", get(messages::get_message))
        .route("/api/messages/:id/retry", post(messages::retry_webhook))
        // Layers: order matters! Applied in reverse (last applied runs first)
        // 1. Auth middleware with state (runs after CORS)
        .layer(middleware::from_fn_with_state(
            api_key_config,
            crate::middleware::api_key_auth,
        ))
        // 2. Tracing
        .layer(TraceLayer::new_for_http())
        // 3. CORS (runs first - handles preflight before auth)
        .layer(cors)
        .with_state(state);

    // Merge with Swagger UI
    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(api_routes)
}
