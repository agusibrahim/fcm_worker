use crate::db::Repository;
use crate::error::AppResult;
use crate::models::MessageLog;
use reqwest::{Client, header::{HeaderMap, HeaderName, HeaderValue}};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{error, info, warn};

/// Webhook client with retry logic
#[derive(Clone)]
pub struct WebhookClient {
    client: Client,
    max_retries: u32,
    base_delay_ms: u64,
}

impl WebhookClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            max_retries: 3,
            base_delay_ms: 1000,
        }
    }

    /// Send webhook with retry logic
    pub async fn send(
        &self,
        url: &str,
        payload: &str,
        custom_headers: Option<&HashMap<String, String>>,
        log: &mut MessageLog,
        repo: &Repository,
    ) -> AppResult<()> {
        let mut last_error = String::new();
        let mut attempt = 0;

        while attempt <= self.max_retries {
            if attempt > 0 {
                let delay = self.base_delay_ms * 2u64.pow(attempt - 1);
                warn!(
                    "Webhook retry attempt {} for message {}, waiting {}ms",
                    attempt, log.id, delay
                );
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }

            match self.send_once(url, payload, custom_headers).await {
                Ok((status, response)) => {
                    log.webhook_status = Some(status as i32);
                    log.webhook_response = Some(response.clone());
                    
                    if let Err(e) = repo.update_message_webhook_status(&log.id, status as i32, &response).await {
                        error!("Failed to update webhook status: {}", e);
                    }

                    if status >= 200 && status < 300 {
                        info!(
                            "Webhook delivered successfully for message {} (status: {})",
                            log.id, status
                        );
                        return Ok(());
                    } else {
                        last_error = format!("HTTP {}: {}", status, response);
                        warn!("Webhook returned non-2xx status: {}", last_error);
                    }
                }
                Err(e) => {
                    last_error = e.to_string();
                    error!("Webhook request failed: {}", last_error);
                }
            }

            attempt += 1;
        }

        // All retries exhausted
        let final_error = format!("All {} retries failed. Last error: {}", self.max_retries, last_error);
        log.webhook_status = Some(0);
        log.webhook_response = Some(final_error.clone());

        if let Err(e) = repo.update_message_webhook_status(&log.id, 0, &final_error).await {
            error!("Failed to update webhook status after failure: {}", e);
        }

        warn!("Webhook delivery failed for message {}: {}", log.id, final_error);
        Ok(())
    }

    async fn send_once(
        &self,
        url: &str,
        payload: &str,
        custom_headers: Option<&HashMap<String, String>>,
    ) -> Result<(u16, String), reqwest::Error> {
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));

        if let Some(custom) = custom_headers {
            for (key, value) in custom {
                if let (Ok(name), Ok(val)) = (
                    HeaderName::try_from(key.as_str()),
                    HeaderValue::try_from(value.as_str()),
                ) {
                    headers.insert(name, val);
                }
            }
        }

        let response = self
            .client
            .post(url)
            .headers(headers)
            .body(payload.to_string())
            .send()
            .await?;

        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();

        Ok((status, body))
    }

    /// Retry a failed webhook delivery
    pub async fn retry_message(
        &self,
        log: &mut MessageLog,
        url: &str,
        custom_headers: Option<&HashMap<String, String>>,
        repo: &Repository,
    ) -> AppResult<()> {
        info!("Retrying webhook for message {}", log.id);
        let payload = log.payload.clone();
        self.send(url, &payload, custom_headers, log, repo).await
    }
}

impl Default for WebhookClient {
    fn default() -> Self {
        Self::new()
    }
}
