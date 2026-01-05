use crate::db::Repository;
use crate::models::{Credential, MessageLog};
use crate::workers::{WebhookClient, DedupCache, get_dedup_ttl};
use fcm_receiver_rs::client::FcmClient;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

/// Registration result from FCM
struct FcmRegistration {
    fcm_token: String,
    gcm_token: String,
    android_id: u64,
    security_token: u64,
    private_key_b64: String,
    auth_secret_b64: String,
}

/// Individual FCM listener worker for a single credential
pub struct FcmWorker {
    credential: Credential,
    repo: Repository,
    webhook_client: WebhookClient,
    shutdown_rx: watch::Receiver<bool>,
    dedup_cache: DedupCache,
}

impl FcmWorker {
    pub fn new(
        credential: Credential,
        repo: Repository,
        webhook_client: WebhookClient,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        let dedup_ttl = get_dedup_ttl();
        info!("Dedup TTL: {} seconds", dedup_ttl);
        
        Self {
            credential,
            repo,
            webhook_client,
            shutdown_rx,
            dedup_cache: DedupCache::new(dedup_ttl),
        }
    }

    /// Main worker loop
    pub async fn run(mut self) {
        let cred_id = self.credential.id.clone();
        let cred_name = self.credential.name.clone();
        
        info!("Starting FCM worker for credential: {} ({})", cred_name, cred_id);

        let mut retry_count = 0;
        let max_retries = 10;
        let base_delay = Duration::from_secs(5);

        loop {
            // Check for shutdown
            if *self.shutdown_rx.borrow() {
                info!("Shutdown signal received for worker: {}", cred_name);
                break;
            }

            match self.run_listener().await {
                Ok(_) => {
                    info!("Listener exited normally for: {}", cred_name);
                    break;
                }
                Err(e) => {
                    error!("Listener error for {}: {}", cred_name, e);
                    retry_count += 1;

                    if retry_count > max_retries {
                        error!("Max retries ({}) reached for {}. Worker stopping.", max_retries, cred_name);
                        break;
                    }

                    let delay = base_delay * 2u32.pow((retry_count - 1).min(6));
                    warn!(
                        "Reconnecting {} in {:?} (attempt {}/{})",
                        cred_name, delay, retry_count, max_retries
                    );

                    // Wait with shutdown check
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {}
                        _ = self.shutdown_rx.changed() => {
                            if *self.shutdown_rx.borrow() {
                                info!("Shutdown during reconnect delay for: {}", cred_name);
                                break;
                            }
                        }
                    }
                }
            }
        }

        info!("FCM worker stopped for: {} ({})", cred_name, cred_id);
    }

    async fn run_listener(&mut self) -> anyhow::Result<()> {
        // Clone all needed values upfront
        let cred_id = self.credential.id.clone();
        let cred_name = self.credential.name.clone();
        let api_key = self.credential.api_key.clone();
        let app_id = self.credential.app_id.clone();
        let project_id = self.credential.project_id.clone();
        let has_fcm_token = self.credential.fcm_token.is_some() && self.credential.private_key_base64.is_some();
        
        // Clone existing credentials if we have them
        let existing_fcm_token = self.credential.fcm_token.clone();
        let existing_gcm_token = self.credential.gcm_token.clone();
        let existing_android_id = self.credential.android_id;
        let existing_security_token = self.credential.security_token;
        let existing_private_key = self.credential.private_key_base64.clone();
        let existing_auth_secret = self.credential.auth_secret_base64.clone();

        // Check if we need to register or load existing credentials
        if has_fcm_token {
            debug!("Loading existing FCM credentials for: {}", cred_name);
            
            // Run client setup and listening in blocking task
            let webhook_url = self.credential.webhook_url.clone();
            let webhook_headers = self.credential.get_webhook_headers();
            let repo = self.repo.clone();
            let webhook_client = self.webhook_client.clone();
            let dedup_cache = self.dedup_cache.clone();
            let topics = self.repo.get_credential_topics(&cred_id).await?;
            
            // Use spawn_blocking for FCM client operations
            let result = tokio::task::spawn_blocking(move || {
                Self::run_fcm_client_existing(
                    api_key,
                    app_id,
                    project_id,
                    existing_fcm_token.unwrap(),
                    existing_gcm_token,
                    existing_android_id.unwrap_or(0) as u64,
                    existing_security_token.unwrap_or(0) as u64,
                    existing_private_key.unwrap(),
                    existing_auth_secret.unwrap(),
                    cred_id,
                    cred_name,
                    webhook_url,
                    webhook_headers,
                    repo,
                    webhook_client,
                    dedup_cache,
                    topics,
                )
            }).await??;
            
            Ok(result)
        } else {
            // Register new device - this is blocking so use spawn_blocking
            info!("Registering new FCM device for: {}", cred_name);
            
            let api_key_clone = api_key.clone();
            let app_id_clone = app_id.clone();
            let project_id_clone = project_id.clone();
            
            let registration = tokio::task::spawn_blocking(move || -> anyhow::Result<FcmRegistration> {
                let mut client = FcmClient::new(api_key_clone, app_id_clone, project_id_clone)?;
                
                let (private_key_b64, auth_secret_b64) = client.create_new_keys()?;
                client.load_keys(&private_key_b64, &auth_secret_b64)?;
                
                let (fcm_token, gcm_token, android_id, security_token) = client.register()?;
                
                Ok(FcmRegistration {
                    fcm_token,
                    gcm_token,
                    android_id,
                    security_token,
                    private_key_b64,
                    auth_secret_b64,
                })
            }).await??;
            
            // Save registration to database
            self.repo
                .update_credential_registration(
                    &cred_id,
                    &registration.fcm_token,
                    &registration.gcm_token,
                    registration.android_id as i64,
                    registration.security_token as i64,
                    &registration.private_key_b64,
                    &registration.auth_secret_b64,
                )
                .await?;

            // Update local credential
            self.credential.fcm_token = Some(registration.fcm_token.clone());
            self.credential.gcm_token = Some(registration.gcm_token.clone());
            self.credential.android_id = Some(registration.android_id as i64);
            self.credential.security_token = Some(registration.security_token as i64);
            self.credential.private_key_base64 = Some(registration.private_key_b64.clone());
            self.credential.auth_secret_base64 = Some(registration.auth_secret_b64.clone());

            info!("FCM device registered successfully for: {}", cred_name);
            info!("FCM Token: {}", registration.fcm_token);

            // Now start listening with the registered credentials
            let webhook_url = self.credential.webhook_url.clone();
            let webhook_headers = self.credential.get_webhook_headers();
            let repo = self.repo.clone();
            let webhook_client = self.webhook_client.clone();
            let dedup_cache = self.dedup_cache.clone();
            let topics = self.repo.get_credential_topics(&cred_id).await?;
            
            let cred_id_for_listener = cred_id.clone();
            let cred_name_for_listener = cred_name.clone();
            
            // Use spawn_blocking for the listener
            tokio::task::spawn_blocking(move || {
                Self::run_fcm_client_existing(
                    api_key,
                    app_id,
                    project_id,
                    registration.fcm_token,
                    Some(registration.gcm_token),
                    registration.android_id,
                    registration.security_token,
                    registration.private_key_b64,
                    registration.auth_secret_b64,
                    cred_id_for_listener,
                    cred_name_for_listener,
                    webhook_url,
                    webhook_headers,
                    repo,
                    webhook_client,
                    dedup_cache,
                    topics,
                )
            }).await??;
            
            Ok(())
        }
    }

    /// Run FCM client with existing credentials (blocking function for spawn_blocking)
    fn run_fcm_client_existing(
        api_key: String,
        app_id: String,
        project_id: String,
        fcm_token: String,
        gcm_token: Option<String>,
        android_id: u64,
        security_token: u64,
        private_key_b64: String,
        auth_secret_b64: String,
        cred_id: String,
        cred_name: String,
        webhook_url: String,
        webhook_headers: Option<std::collections::HashMap<String, String>>,
        repo: Repository,
        webhook_client: WebhookClient,
        dedup_cache: DedupCache,
        topics: Vec<String>,
    ) -> anyhow::Result<()> {
        let mut client = FcmClient::new(api_key, app_id, project_id)?;
        
        // Load existing credentials
        client.fcm_token = Some(fcm_token);
        client.gcm_token = gcm_token;
        client.android_id = android_id;
        client.security_token = security_token;
        client.load_keys(&private_key_b64, &auth_secret_b64)?;

        // Subscribe to topics
        for topic in &topics {
            match client.subscribe_to_topic(topic) {
                Ok(_) => info!("Subscribed to topic '{}' for: {}", topic, cred_name),
                Err(e) => warn!("Failed to subscribe to topic '{}': {}", topic, e),
            }
        }

        // Set up message handler with dedup
        let cred_id_handler = cred_id.clone();
        let dedup_ttl = dedup_cache.ttl_seconds();
        let max_messages = crate::workers::get_max_messages_per_credential();
        
        client.on_data_message = Some(Arc::new(move |payload| {
            let text = String::from_utf8_lossy(&payload).to_string();
            let cred_id = cred_id_handler.clone();
            let webhook_url = webhook_url.clone();
            let webhook_headers = webhook_headers.clone();
            let repo = repo.clone();
            let webhook_client = webhook_client.clone();
            let dedup_cache = dedup_cache.clone();

            // Spawn async task for message handling
            tokio::spawn(async move {
                debug!("Received FCM message for credential {}: {}", cred_id, text);

                // Extract fcmMessageId for persistent dedup
                let fcm_message_id = MessageLog::extract_fcm_message_id(&text);

                // Check for duplicate using fcmMessageId (persistent in DB)
                if let Some(ref fcm_id) = fcm_message_id {
                    match repo.is_fcm_message_duplicate(&cred_id, fcm_id).await {
                        Ok(true) => {
                            debug!("Duplicate fcmMessageId detected: {}, skipping", fcm_id);
                            return;
                        }
                        Err(e) => {
                            error!("Failed to check fcmMessageId duplicate: {}", e);
                            // Continue processing anyway
                        }
                        _ => {}
                    }
                }

                // Also check for duplicate in memory (for rapid fire duplicates)
                if dedup_cache.is_duplicate(&text) {
                    warn!("Duplicate message detected in memory (within {} seconds), skipping", dedup_ttl);
                    return;
                }

                // Create message log with fcmMessageId
                let mut log = MessageLog::new(cred_id.clone(), fcm_message_id, text.clone());
                
                // Save to database
                if let Err(e) = repo.create_message_log(&log).await {
                    error!("Failed to save message log: {}", e);
                    return;
                }

                // Cleanup old messages to keep only max_messages
                if let Err(e) = repo.cleanup_old_messages(&cred_id, max_messages).await {
                    error!("Failed to cleanup old messages: {}", e);
                }

                // Send webhook
                if let Err(e) = webhook_client
                    .send(
                        &webhook_url,
                        &text,
                        webhook_headers.as_ref(),
                        &mut log,
                        &repo,
                    )
                    .await
                {
                    error!("Webhook delivery failed: {}", e);
                }
            });
        }));

        // Start listening (this blocks until connection drops)
        info!("Starting FCM listener for: {}", cred_name);
        client.start_listening()?;

        Ok(())
    }
}
