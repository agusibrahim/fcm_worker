# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Run Commands

```bash
# Build
cargo build              # Development build
cargo build --release    # Release build

# Run
cargo run                # Run main server
cargo run --release      # Run in release mode
cargo run --bin test_notification  # Send test notification

# Test
cargo test               # Run all tests
cargo test <test_name>   # Run specific test

# Check
cargo check              # Fast syntax/type check
cargo clippy             # Linting
cargo fmt                # Format code
```

## Environment Variables

Create `.env` file or export:
- `DATABASE_URL` - SQLite path (default: `sqlite:fcm_receiver.db?mode=rwc`)
- `PORT` - HTTP server port (default: `3000`)
- `API_KEY` - Auth key for API (auto-generated if not set)
- `DEDUP_TTL` - Deduplication TTL in seconds
- `MAX_MESSAGES_PER_CREDENTIAL` - Max message logs per credential
- `RUST_LOG` - Logging level (default: `fcm_recv=info,tower_http=debug`)

## Architecture

This is an FCM (Firebase Cloud Messaging) receiver server that emulates Android devices to receive push notifications and forward them to webhooks.

### Core Flow
1. **Credential Registration**: User adds Firebase credentials via REST API
2. **Device Emulation**: Server registers as virtual Android device with FCM using `fcm_receiver_rs` library
3. **Message Listening**: `FcmWorker` maintains persistent connection to FCM (blocking operation in `spawn_blocking`)
4. **Webhook Delivery**: Received messages are deduplicated, logged to SQLite, and forwarded to configured webhooks

### Key Components

- **ListenerPool** (`src/workers/listener_pool.rs`): Manages lifecycle of multiple `FcmWorker` instances. Uses `watch` channels for graceful shutdown signaling.

- **FcmWorker** (`src/workers/fcm_worker.rs`): Individual FCM connection per credential. Runs in `spawn_blocking` because `fcm_receiver_rs::FcmClient` is blocking. Implements exponential backoff retry (max 10 retries).

- **Repository** (`src/db/repository.rs`): SQLite access layer using sqlx. Handles credentials, message logs, and topic subscriptions.

- **API Layer** (`src/api/`): Axum-based REST API with Swagger UI at `/swagger-ui/`. Auth via `X-API-Key` header or `Authorization: Bearer` token.

### Important Patterns

- FCM operations are **blocking** - always wrapped in `tokio::task::spawn_blocking`
- Deduplication is dual-layer: in-memory cache + database-level `fcm_message_id` check
- Workers use `watch` channels for shutdown coordination
- Credentials can be "suspended" (disabled without deletion) or "active" (auto-start on server boot)
