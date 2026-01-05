mod api;
mod db;
mod error;
mod middleware;
mod models;
mod workers;

use api::{create_router, AppState};
use db::Repository;
use middleware::{generate_api_key, ApiKeyConfig};
use std::net::SocketAddr;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use workers::ListenerPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if exists
    dotenv::dotenv().ok();

    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "fcm_recv=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("FCM Multi-Credential Receiver Server v{}", env!("CARGO_PKG_VERSION"));

    // Get configuration from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:fcm_receiver.db?mode=rwc".to_string());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a number");

    // Get or generate API key
    let api_key = std::env::var("API_KEY").unwrap_or_else(|_| {
        let key = generate_api_key();
        warn!("API_KEY not set in environment. Generated temporary key: {}", key);
        warn!("Add API_KEY={} to your .env file to persist this key", key);
        key
    });

    info!("API Key configured (use 'Authorization: Bearer <key>' or 'X-API-Key: <key>')");

    info!("Connecting to database: {}", database_url);

    // Initialize repository
    let repo = Repository::new(&database_url).await?;
    info!("Database connected and migrations applied");

    // Initialize listener pool
    let listener_pool = ListenerPool::new(repo.clone());
    
    // Start all active listeners
    info!("Starting active credential listeners...");
    if let Err(e) = listener_pool.start_all_active().await {
        error!("Failed to start some listeners: {}", e);
    }

    // Create app state and API key config
    let state = AppState::new(repo, listener_pool);
    let api_key_config = ApiKeyConfig::new(api_key);
    let pool_ref = state.listener_pool.clone();

    // Create router
    let app = create_router(state, api_key_config);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Starting HTTP server on http://{}", addr);
    info!("Swagger UI available at http://{}/swagger-ui/", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Run with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(pool_ref))
        .await?;

    info!("Server stopped");
    Ok(())
}

async fn shutdown_signal(pool: std::sync::Arc<tokio::sync::RwLock<ListenerPool>>) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, stopping listeners...");
    
    // Shutdown all listeners gracefully
    let pool = pool.read().await;
    pool.shutdown_all().await;
}
