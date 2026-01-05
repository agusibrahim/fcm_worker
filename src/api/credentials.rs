use crate::api::AppState;
use crate::error::{AppError, AppResult};
use crate::models::{CreateCredentialRequest, Credential, CredentialResponse, UpdateCredentialRequest};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::{IntoParams, ToSchema};

/// Query parameters for listing credentials
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct ListQuery {
    /// Filter to show only active credentials
    #[serde(default)]
    pub active_only: bool,
}

/// Response containing list of credentials
#[derive(Debug, Serialize, ToSchema)]
pub struct ListCredentialsResponse {
    /// List of credentials
    pub credentials: Vec<CredentialResponse>,
    /// Total count
    pub total: usize,
}

/// List all credentials
#[utoipa::path(
    get,
    path = "/api/credentials",
    tag = "credentials",
    params(ListQuery),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "List of credentials", body = ListCredentialsResponse),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn list_credentials(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<ListCredentialsResponse>> {
    let credentials = state.repo.list_credentials(query.active_only).await?;
    let pool = state.listener_pool.read().await;
    
    let responses: Vec<CredentialResponse> = credentials
        .iter()
        .map(|c| {
            let is_listening = futures::executor::block_on(pool.is_running(&c.id));
            c.to_response(is_listening)
        })
        .collect();

    let total = responses.len();

    Ok(Json(ListCredentialsResponse {
        credentials: responses,
        total,
    }))
}

/// Get a single credential
#[utoipa::path(
    get,
    path = "/api/credentials/{id}",
    tag = "credentials",
    params(
        ("id" = String, Path, description = "Credential ID")
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Credential details", body = CredentialResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Credential not found")
    )
)]
pub async fn get_credential(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<CredentialResponse>> {
    let credential = state
        .repo
        .get_credential(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Credential {} not found", id)))?;

    let pool = state.listener_pool.read().await;
    let is_listening = pool.is_running(&id).await;

    Ok(Json(credential.to_response(is_listening)))
}

/// Response for credential creation
#[derive(Debug, Serialize, ToSchema)]
pub struct CreateCredentialResponse {
    /// Created credential
    pub credential: CredentialResponse,
    /// Status message
    pub message: String,
}

/// Create a new credential (does NOT auto-start, use /start endpoint)
#[utoipa::path(
    post,
    path = "/api/credentials",
    tag = "credentials",
    request_body = CreateCredentialRequest,
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Credential created (not started)", body = CreateCredentialResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn create_credential(
    State(state): State<AppState>,
    Json(req): Json<CreateCredentialRequest>,
) -> AppResult<Json<CreateCredentialResponse>> {
    // Validate webhook URL
    if !req.webhook_url.starts_with("http://") && !req.webhook_url.starts_with("https://") {
        return Err(AppError::BadRequest("Invalid webhook URL".to_string()));
    }

    let topics = req.topics.clone();
    let credential = Credential::new(req);
    
    // Save to database
    state.repo.create_credential(&credential).await?;

    // Save topics if provided
    if !topics.is_empty() {
        state.repo.set_credential_topics(&credential.id, &topics).await?;
    }

    // NOTE: Do NOT auto-start - user must call /start endpoint
    info!("Created credential: {} ({}) - use /start to begin listening", credential.name, credential.id);

    Ok(Json(CreateCredentialResponse {
        credential: credential.to_response(false),
        message: "Credential created. Use POST /api/credentials/{id}/start to begin listening.".to_string(),
    }))
}

/// Update a credential (restarts worker if running to apply changes)
#[utoipa::path(
    put,
    path = "/api/credentials/{id}",
    tag = "credentials",
    params(
        ("id" = String, Path, description = "Credential ID")
    ),
    request_body = UpdateCredentialRequest,
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Credential updated (worker restarted if running)", body = CredentialResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Credential not found")
    )
)]
pub async fn update_credential(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateCredentialRequest>,
) -> AppResult<Json<CredentialResponse>> {
    // Check if exists
    let _old_credential = state
        .repo
        .get_credential(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Credential {} not found", id)))?;

    // Validate webhook URL if provided
    if let Some(ref url) = req.webhook_url {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(AppError::BadRequest("Invalid webhook URL".to_string()));
        }
    }

    // Update in database
    let webhook_headers_json = req
        .webhook_headers
        .as_ref()
        .map(|h| serde_json::to_string(h).unwrap_or_default());

    state
        .repo
        .update_credential(
            &id,
            req.name.as_deref(),
            req.webhook_url.as_deref(),
            webhook_headers_json.as_deref(),
            req.is_active,
            req.api_key.as_deref(),
            req.app_id.as_deref(),
            req.project_id.as_deref(),
        )
        .await?;

    // Update topics if provided
    if let Some(topics) = &req.topics {
        state.repo.set_credential_topics(&id, topics).await?;
    }

    // Get updated credential
    let updated_credential = state.repo.get_credential(&id).await?.unwrap();
    let pool = state.listener_pool.read().await;

    // Check if worker was running - if so, restart to apply changes
    let was_running = pool.is_running(&id).await;
    if was_running {
        info!("Restarting worker to apply credential changes: {}", id);
        let _ = pool.restart_worker(&updated_credential).await;
    }

    // If is_active was set to false, stop the worker
    if req.is_active == Some(false) && was_running {
        let _ = pool.stop_worker(&id).await;
    }

    let is_listening = pool.is_running(&id).await;
    info!("Updated credential: {} (was_running={}, is_listening={})", id, was_running, is_listening);

    Ok(Json(updated_credential.to_response(is_listening)))
}

/// Delete a credential
#[utoipa::path(
    delete,
    path = "/api/credentials/{id}",
    tag = "credentials",
    params(
        ("id" = String, Path, description = "Credential ID")
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Credential deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Credential not found")
    )
)]
pub async fn delete_credential(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    // Check if exists
    let credential = state
        .repo
        .get_credential(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Credential {} not found", id)))?;

    // Stop listener first
    let pool = state.listener_pool.read().await;
    let _ = pool.stop_worker(&id).await;

    // Delete from database
    state.repo.delete_credential(&id).await?;

    info!("Deleted credential: {} ({})", credential.name, id);

    Ok(Json(serde_json::json!({
        "message": format!("Credential {} deleted", id),
        "id": id
    })))
}

/// Start listener for a credential
#[utoipa::path(
    post,
    path = "/api/credentials/{id}/start",
    tag = "credentials",
    params(
        ("id" = String, Path, description = "Credential ID")
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Listener started"),
        (status = 400, description = "Cannot start listener"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Credential not found")
    )
)]
pub async fn start_listener(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let credential = state
        .repo
        .get_credential(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Credential {} not found", id)))?;

    if !credential.is_active {
        return Err(AppError::BadRequest(
            "Cannot start listener for inactive credential".to_string(),
        ));
    }

    if credential.is_suspended {
        return Err(AppError::BadRequest(
            "Cannot start listener for suspended credential. Use /unsuspend first.".to_string(),
        ));
    }

    let pool = state.listener_pool.read().await;
    pool.start_worker(&credential).await?;

    info!("Started listener for: {}", credential.name);

    Ok(Json(serde_json::json!({
        "message": format!("Listener started for credential {}", id),
        "id": id
    })))
}

/// Stop listener for a credential
#[utoipa::path(
    post,
    path = "/api/credentials/{id}/stop",
    tag = "credentials",
    params(
        ("id" = String, Path, description = "Credential ID")
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Listener stopped"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Credential not found")
    )
)]
pub async fn stop_listener(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    // Check if exists
    state
        .repo
        .get_credential(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Credential {} not found", id)))?;

    let pool = state.listener_pool.read().await;
    pool.stop_worker(&id).await?;

    info!("Stopped listener for credential: {}", id);

    Ok(Json(serde_json::json!({
        "message": format!("Listener stopped for credential {}", id),
        "id": id
    })))
}

/// Restart listener for a credential
#[utoipa::path(
    post,
    path = "/api/credentials/{id}/restart",
    tag = "credentials",
    params(
        ("id" = String, Path, description = "Credential ID")
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Listener restarted"),
        (status = 400, description = "Cannot restart listener"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Credential not found")
    )
)]
pub async fn restart_listener(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let credential = state
        .repo
        .get_credential(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Credential {} not found", id)))?;

    if !credential.is_active {
        return Err(AppError::BadRequest(
            "Cannot restart listener for inactive credential".to_string(),
        ));
    }

    if credential.is_suspended {
        return Err(AppError::BadRequest(
            "Cannot restart listener for suspended credential. Use /unsuspend first.".to_string(),
        ));
    }

    let pool = state.listener_pool.read().await;
    pool.restart_worker(&credential).await?;

    info!("Restarted listener for: {}", credential.name);

    Ok(Json(serde_json::json!({
        "message": format!("Listener restarted for credential {}", id),
        "id": id
    })))
}

/// Suspend a credential (stops worker and prevents auto-start on server boot)
#[utoipa::path(
    post,
    path = "/api/credentials/{id}/suspend",
    tag = "credentials",
    params(
        ("id" = String, Path, description = "Credential ID")
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Credential suspended"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Credential not found")
    )
)]
pub async fn suspend_credential(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    // Check if exists
    let credential = state
        .repo
        .get_credential(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Credential {} not found", id)))?;

    // Stop worker if running
    let pool = state.listener_pool.read().await;
    let _ = pool.stop_worker(&id).await;

    // Mark as suspended
    state.repo.suspend_credential(&id).await?;

    info!("Suspended credential: {} ({})", credential.name, id);

    Ok(Json(serde_json::json!({
        "message": format!("Credential {} suspended. Worker stopped and won't auto-start on server boot.", id),
        "id": id,
        "is_suspended": true
    })))
}

/// Unsuspend a credential (allows auto-start on server boot)
#[utoipa::path(
    post,
    path = "/api/credentials/{id}/unsuspend",
    tag = "credentials",
    params(
        ("id" = String, Path, description = "Credential ID")
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    ),
    responses(
        (status = 200, description = "Credential unsuspended"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Credential not found")
    )
)]
pub async fn unsuspend_credential(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    // Check if exists
    let credential = state
        .repo
        .get_credential(&id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Credential {} not found", id)))?;

    // Mark as not suspended
    state.repo.unsuspend_credential(&id).await?;

    info!("Unsuspended credential: {} ({})", credential.name, id);

    Ok(Json(serde_json::json!({
        "message": format!("Credential {} unsuspended. Use /start to begin listening.", id),
        "id": id,
        "is_suspended": false
    })))
}
