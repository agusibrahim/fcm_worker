use crate::db::Repository;
use crate::error::{AppError, AppResult};
use crate::models::Credential;
use crate::workers::{FcmWorker, WebhookClient};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

/// Manages a pool of FCM listener workers
pub struct ListenerPool {
    repo: Repository,
    webhook_client: WebhookClient,
    workers: Arc<RwLock<HashMap<String, WorkerHandle>>>,
    global_shutdown_tx: watch::Sender<bool>,
}

struct WorkerHandle {
    handle: JoinHandle<()>,
    shutdown_tx: watch::Sender<bool>,
    credential_name: String,
}

impl ListenerPool {
    pub fn new(repo: Repository) -> Self {
        let (global_shutdown_tx, _) = watch::channel(false);
        
        Self {
            repo,
            webhook_client: WebhookClient::new(),
            workers: Arc::new(RwLock::new(HashMap::new())),
            global_shutdown_tx,
        }
    }

    /// Start all runnable credentials on server boot (active and not suspended)
    pub async fn start_all_active(&self) -> AppResult<()> {
        let credentials = self.repo.list_runnable_credentials().await?;
        info!("Starting {} runnable credential listeners (active and not suspended)", credentials.len());

        for cred in credentials {
            if let Err(e) = self.start_worker(&cred).await {
                error!("Failed to start worker for {}: {}", cred.name, e);
            }
        }

        Ok(())
    }

    /// Start a worker for a specific credential
    pub async fn start_worker(&self, credential: &Credential) -> AppResult<()> {
        let cred_id = &credential.id;
        
        // Check if already running
        {
            let workers = self.workers.read().await;
            if workers.contains_key(cred_id) {
                return Err(AppError::WorkerAlreadyRunning(format!(
                    "Worker for credential {} is already running",
                    credential.name
                )));
            }
        }

        // Create shutdown channel for this worker
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // Create and spawn worker
        let worker = FcmWorker::new(
            credential.clone(),
            self.repo.clone(),
            self.webhook_client.clone(),
            shutdown_rx,
        );

        let cred_name = credential.name.clone();
        let handle = tokio::spawn(async move {
            worker.run().await;
        });

        // Store handle
        {
            let mut workers = self.workers.write().await;
            workers.insert(
                cred_id.clone(),
                WorkerHandle {
                    handle,
                    shutdown_tx,
                    credential_name: cred_name.clone(),
                },
            );
        }

        info!("Worker started for credential: {} ({})", cred_name, cred_id);
        Ok(())
    }

    /// Stop a specific worker
    pub async fn stop_worker(&self, credential_id: &str) -> AppResult<()> {
        let handle = {
            let mut workers = self.workers.write().await;
            workers.remove(credential_id)
        };

        match handle {
            Some(worker_handle) => {
                info!("Stopping worker for: {}", worker_handle.credential_name);
                
                // Signal shutdown
                let _ = worker_handle.shutdown_tx.send(true);
                
                // Wait for worker to finish (with timeout)
                tokio::select! {
                    _ = worker_handle.handle => {
                        info!("Worker stopped gracefully: {}", worker_handle.credential_name);
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {
                        warn!("Worker shutdown timed out, aborting: {}", worker_handle.credential_name);
                        // Note: The blocking task will be cleaned up when the runtime shuts down
                    }
                }
                
                Ok(())
            }
            None => Err(AppError::WorkerNotRunning(format!(
                "No worker running for credential {}",
                credential_id
            ))),
        }
    }

    /// Restart a worker
    pub async fn restart_worker(&self, credential: &Credential) -> AppResult<()> {
        // Stop if running (ignore error if not running)
        let _ = self.stop_worker(&credential.id).await;
        
        // Start fresh
        self.start_worker(credential).await
    }

    /// Check if a worker is running
    pub async fn is_running(&self, credential_id: &str) -> bool {
        let workers = self.workers.read().await;
        if let Some(handle) = workers.get(credential_id) {
            !handle.handle.is_finished()
        } else {
            false
        }
    }

    /// Get status of all workers
    #[allow(dead_code)]
    pub async fn get_status(&self) -> HashMap<String, bool> {
        let workers = self.workers.read().await;
        workers
            .iter()
            .map(|(id, handle)| (id.clone(), !handle.handle.is_finished()))
            .collect()
    }

    /// Get count of active workers
    pub async fn active_count(&self) -> usize {
        let workers = self.workers.read().await;
        workers.values().filter(|h| !h.handle.is_finished()).count()
    }

    /// Shutdown all workers gracefully
    pub async fn shutdown_all(&self) {
        info!("Shutting down all FCM workers...");
        
        // Signal global shutdown
        let _ = self.global_shutdown_tx.send(true);

        // Collect all handles
        let handles: Vec<WorkerHandle> = {
            let mut workers = self.workers.write().await;
            workers.drain().map(|(_, h)| h).collect()
        };

        // Signal each worker and wait with short timeout
        for handle in handles {
            let _ = handle.shutdown_tx.send(true);
            
            tokio::select! {
                _ = handle.handle => {
                    info!("Worker {} stopped gracefully", handle.credential_name);
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {
                    // Blocking tasks can't be aborted, just move on
                    warn!("Worker {} shutdown timed out (blocking task)", handle.credential_name);
                }
            }
        }

        info!("All FCM workers stopped");
    }
}
