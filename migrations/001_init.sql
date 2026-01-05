-- Credentials table
CREATE TABLE IF NOT EXISTS credentials (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    api_key TEXT NOT NULL,
    app_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    fcm_token TEXT,
    gcm_token TEXT,
    android_id INTEGER,
    security_token INTEGER,
    private_key_base64 TEXT,
    auth_secret_base64 TEXT,
    webhook_url TEXT NOT NULL,
    webhook_headers TEXT, -- JSON string for custom headers
    is_active BOOLEAN NOT NULL DEFAULT 1,
    is_suspended BOOLEAN NOT NULL DEFAULT 0, -- Suspended workers won't auto-start
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Message logs table
CREATE TABLE IF NOT EXISTS message_logs (
    id TEXT PRIMARY KEY,
    credential_id TEXT NOT NULL,
    fcm_message_id TEXT, -- FCM message ID for deduplication
    payload TEXT NOT NULL,
    webhook_status INTEGER, -- HTTP status code
    webhook_response TEXT,
    received_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (credential_id) REFERENCES credentials(id) ON DELETE CASCADE
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_credentials_active ON credentials(is_active);
CREATE INDEX IF NOT EXISTS idx_message_logs_credential ON message_logs(credential_id);
CREATE INDEX IF NOT EXISTS idx_message_logs_received ON message_logs(received_at);
CREATE INDEX IF NOT EXISTS idx_message_logs_fcm_id ON message_logs(credential_id, fcm_message_id);

-- Credential topics table
CREATE TABLE IF NOT EXISTS credential_topics (
    credential_id TEXT NOT NULL,
    topic TEXT NOT NULL,
    PRIMARY KEY (credential_id, topic),
    FOREIGN KEY (credential_id) REFERENCES credentials(id) ON DELETE CASCADE
);

