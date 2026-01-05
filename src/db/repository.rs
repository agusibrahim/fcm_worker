use crate::models::{Credential, MessageLog};
use anyhow::Result;
use sqlx::{Row, SqlitePool};

#[derive(Clone)]
pub struct Repository {
    pool: SqlitePool,
}

impl Repository {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;

        // Run migrations
        sqlx::query(include_str!("../../migrations/001_init.sql"))
            .execute(&pool)
            .await?;

        Ok(Self { pool })
    }

    // ========== Credential Operations ==========

    pub async fn create_credential(&self, cred: &Credential) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO credentials (
                id, name, api_key, app_id, project_id,
                fcm_token, gcm_token, android_id, security_token,
                private_key_base64, auth_secret_base64,
                webhook_url, webhook_headers, is_active, is_suspended, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&cred.id)
        .bind(&cred.name)
        .bind(&cred.api_key)
        .bind(&cred.app_id)
        .bind(&cred.project_id)
        .bind(&cred.fcm_token)
        .bind(&cred.gcm_token)
        .bind(cred.android_id)
        .bind(cred.security_token)
        .bind(&cred.private_key_base64)
        .bind(&cred.auth_secret_base64)
        .bind(&cred.webhook_url)
        .bind(&cred.webhook_headers)
        .bind(cred.is_active)
        .bind(cred.is_suspended)
        .bind(cred.created_at)
        .bind(cred.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_credential(&self, id: &str) -> Result<Option<Credential>> {
        let cred = sqlx::query_as::<_, Credential>("SELECT * FROM credentials WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(cred)
    }

    pub async fn list_credentials(&self, active_only: bool) -> Result<Vec<Credential>> {
        let query = if active_only {
            "SELECT * FROM credentials WHERE is_active = 1 ORDER BY created_at DESC"
        } else {
            "SELECT * FROM credentials ORDER BY created_at DESC"
        };

        let creds = sqlx::query_as::<_, Credential>(query)
            .fetch_all(&self.pool)
            .await?;

        Ok(creds)
    }

    /// List credentials that should auto-start (active and not suspended)
    pub async fn list_runnable_credentials(&self) -> Result<Vec<Credential>> {
        let creds = sqlx::query_as::<_, Credential>(
            "SELECT * FROM credentials WHERE is_active = 1 AND is_suspended = 0 ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(creds)
    }

    pub async fn update_credential(
        &self,
        id: &str,
        name: Option<&str>,
        webhook_url: Option<&str>,
        webhook_headers: Option<&str>,
        is_active: Option<bool>,
        api_key: Option<&str>,
        app_id: Option<&str>,
        project_id: Option<&str>,
    ) -> Result<bool> {
        let mut query = String::from("UPDATE credentials SET updated_at = CURRENT_TIMESTAMP");
        let mut params: Vec<String> = Vec::new();

        if let Some(n) = name {
            query.push_str(", name = ?");
            params.push(n.to_string());
        }
        if let Some(w) = webhook_url {
            query.push_str(", webhook_url = ?");
            params.push(w.to_string());
        }
        if let Some(h) = webhook_headers {
            query.push_str(", webhook_headers = ?");
            params.push(h.to_string());
        }
        if let Some(a) = is_active {
            query.push_str(", is_active = ?");
            params.push(a.to_string());
        }
        if let Some(k) = api_key {
            query.push_str(", api_key = ?");
            params.push(k.to_string());
        }
        if let Some(a) = app_id {
            query.push_str(", app_id = ?");
            params.push(a.to_string());
        }
        if let Some(p) = project_id {
            query.push_str(", project_id = ?");
            params.push(p.to_string());
        }

        query.push_str(" WHERE id = ?");
        params.push(id.to_string());

        let mut q = sqlx::query(&query);
        for param in &params[..params.len() - 1] {
            q = q.bind(param);
        }
        q = q.bind(id);

        let result = q.execute(&self.pool).await?;
        Ok(result.rows_affected() > 0)
    }

    /// Suspend a credential (prevent auto-start)
    pub async fn suspend_credential(&self, id: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE credentials SET is_suspended = 1, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Unsuspend a credential (allow auto-start)
    pub async fn unsuspend_credential(&self, id: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE credentials SET is_suspended = 0, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn update_credential_registration(
        &self,
        id: &str,
        fcm_token: &str,
        gcm_token: &str,
        android_id: i64,
        security_token: i64,
        private_key: &str,
        auth_secret: &str,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE credentials
            SET fcm_token = ?, gcm_token = ?, android_id = ?, security_token = ?,
                private_key_base64 = ?, auth_secret_base64 = ?, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(fcm_token)
        .bind(gcm_token)
        .bind(android_id)
        .bind(security_token)
        .bind(private_key)
        .bind(auth_secret)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_credential(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM credentials WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    // ========== Message Log Operations ==========

    pub async fn create_message_log(&self, log: &MessageLog) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO message_logs (
                id, credential_id, fcm_message_id, payload, webhook_status, webhook_response, received_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&log.id)
        .bind(&log.credential_id)
        .bind(&log.fcm_message_id)
        .bind(&log.payload)
        .bind(log.webhook_status)
        .bind(&log.webhook_response)
        .bind(log.received_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Check if fcmMessageId already exists for this credential
    pub async fn is_fcm_message_duplicate(&self, credential_id: &str, fcm_message_id: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM message_logs WHERE credential_id = ? AND fcm_message_id = ?"
        )
        .bind(credential_id)
        .bind(fcm_message_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count > 0)
    }

    /// Delete oldest messages to keep only max_count per credential
    pub async fn cleanup_old_messages(&self, credential_id: &str, max_count: i64) -> Result<u64> {
        // Delete messages older than the Nth newest
        let result = sqlx::query(
            r#"
            DELETE FROM message_logs 
            WHERE credential_id = ? 
            AND id NOT IN (
                SELECT id FROM message_logs 
                WHERE credential_id = ? 
                ORDER BY received_at DESC 
                LIMIT ?
            )
            "#,
        )
        .bind(credential_id)
        .bind(credential_id)
        .bind(max_count)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Clear all messages for a credential
    pub async fn clear_credential_messages(&self, credential_id: &str) -> Result<u64> {
        let result = sqlx::query("DELETE FROM message_logs WHERE credential_id = ?")
            .bind(credential_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    pub async fn update_message_webhook_status(
        &self,
        id: &str,
        status: i32,
        response: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE message_logs SET webhook_status = ?, webhook_response = ? WHERE id = ?",
        )
        .bind(status)
        .bind(response)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_message_logs(
        &self,
        credential_id: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<MessageLog>> {
        let logs = if let Some(cid) = credential_id {
            sqlx::query_as::<_, MessageLog>(
                "SELECT * FROM message_logs WHERE credential_id = ? ORDER BY received_at DESC LIMIT ? OFFSET ?"
            )
            .bind(cid)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, MessageLog>(
                "SELECT * FROM message_logs ORDER BY received_at DESC LIMIT ? OFFSET ?",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(logs)
    }

    pub async fn count_message_logs(&self, credential_id: Option<&str>) -> Result<i64> {
        let count = if let Some(cid) = credential_id {
            sqlx::query("SELECT COUNT(*) as count FROM message_logs WHERE credential_id = ?")
                .bind(cid)
                .fetch_one(&self.pool)
                .await?
                .get::<i64, _>("count")
        } else {
            sqlx::query("SELECT COUNT(*) as count FROM message_logs")
                .fetch_one(&self.pool)
                .await?
                .get::<i64, _>("count")
        };

        Ok(count)
    }

    #[allow(dead_code)]
    pub async fn delete_old_message_logs(&self, days: i64) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM message_logs WHERE received_at < datetime('now', ? || ' days')",
        )
        .bind(format!("-{}", days))
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ========== Topic Operations ==========

    pub async fn set_credential_topics(&self, credential_id: &str, topics: &[String]) -> Result<()> {
        // Delete existing topics
        sqlx::query("DELETE FROM credential_topics WHERE credential_id = ?")
            .bind(credential_id)
            .execute(&self.pool)
            .await?;

        // Insert new topics
        for topic in topics {
            sqlx::query(
                "INSERT INTO credential_topics (credential_id, topic) VALUES (?, ?)"
            )
            .bind(credential_id)
            .bind(topic)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub async fn get_credential_topics(&self, credential_id: &str) -> Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT topic FROM credential_topics WHERE credential_id = ?"
        )
        .bind(credential_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(t,)| t).collect())
    }

    #[allow(dead_code)]
    pub async fn add_credential_topic(&self, credential_id: &str, topic: &str) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO credential_topics (credential_id, topic) VALUES (?, ?)"
        )
        .bind(credential_id)
        .bind(topic)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn remove_credential_topic(&self, credential_id: &str, topic: &str) -> Result<()> {
        sqlx::query(
            "DELETE FROM credential_topics WHERE credential_id = ? AND topic = ?"
        )
        .bind(credential_id)
        .bind(topic)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
