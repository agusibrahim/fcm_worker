# FCM Recv

A Firebase Cloud Messaging (FCM) Multi-Credential Receiver Server written in Rust. This server acts as a bridge that receives push notifications from FCM for multiple Firebase projects/credentials and forwards them to configured webhooks.

## Features

- **Multi-Credential Support** - Manage and run multiple FCM listeners simultaneously, each with its own Firebase configuration
- **Automated Registration** - Automatically registers as a virtual Android device with FCM if credentials aren't already provided
- **Webhook Integration** - Forwards received FCM messages to specified URLs via HTTP POST with custom headers support
- **Message Logging & Persistence** - Stores received messages in SQLite database with webhook delivery status tracking
- **Deduplication** - In-memory and database-level deduplication to prevent duplicate message processing
- **Topic Subscription** - Subscribe to FCM topics for each credential
- **REST API** - Full API for managing credentials, checking status, and viewing message logs
- **Swagger UI** - Built-in interactive API documentation
- **Graceful Shutdown** - Properly closes all FCM connections when the server stops

## Requirements

- Rust 1.70+ and Cargo

## Installation

### Build from Source

```bash
git clone https://github.com/agusibrahim/fcm_recv.git
cd fcm_recv
cargo build --release
```

The binary will be available at `target/release/fcm_recv`.

## Configuration

Create a `.env` file in the project root or set environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | SQLite database path | `sqlite:fcm_receiver.db?mode=rwc` |
| `PORT` | HTTP server port | `3000` |
| `API_KEY` | Master API key for authentication | Auto-generated on startup |
| `DEDUP_TTL` | Time-to-live for in-memory deduplication (seconds) | - |
| `MAX_MESSAGES_PER_CREDENTIAL` | Maximum message logs per credential | - |

## Usage

### Running the Server

```bash
# Development
cargo run

# Production
cargo run --release

# Or run the built binary
./target/release/fcm_recv
```

The server will start on `http://localhost:3000` (or the configured PORT).

### API Documentation

Access the Swagger UI at: `http://localhost:3000/swagger-ui/`

### Authentication

All `/api/` endpoints require authentication via one of:
- Header: `Authorization: Bearer <API_KEY>`
- Header: `X-API-Key: <API_KEY>`

### API Endpoints

#### Health Check
```
GET /health
```

#### Credentials Management
```
POST   /api/credentials           # Add new FCM credential
GET    /api/credentials           # List all credentials
GET    /api/credentials/{id}      # Get credential details
DELETE /api/credentials/{id}      # Remove credential
POST   /api/credentials/{id}/start  # Start listener
POST   /api/credentials/{id}/stop   # Stop listener
```

#### Messages
```
GET    /api/messages              # List received messages
POST   /api/messages/{id}/retry   # Retry webhook delivery
```

## How It Works

This project is powered by [fcm_receiver.rs](https://github.com/agusibrahim/fcm_receiver.rs), a Rust library for receiving FCM push notifications by emulating an Android device.

1. **Register Credentials** - Add Firebase project credentials via the API
2. **Start Listener** - The server registers as a virtual Android device and connects to FCM using `fcm_receiver.rs`
3. **Receive Messages** - When a push notification is sent to the registered device, the server receives it
4. **Forward to Webhook** - The message is forwarded to your configured webhook URL
5. **Persistence** - All messages are logged in the SQLite database for later reference

## Project Structure

```
fcm_recv/
├── src/
│   ├── main.rs           # Entry point
│   ├── api/              # REST API route handlers
│   │   ├── credentials.rs
│   │   ├── health.rs
│   │   └── messages.rs
│   ├── db/               # Database repository
│   ├── models/           # Data structures
│   └── workers/          # FCM listener logic
│       ├── listener_pool.rs  # Manages multiple FCM workers
│       ├── fcm_worker.rs     # Individual FCM connection
│       ├── webhook.rs        # Webhook delivery
│       └── dedup.rs          # Deduplication logic
├── migrations/           # SQL schema files
├── Cargo.toml
└── README.md
```

## Testing

Send a test notification:

```bash
cargo run --bin test_notification
```

## License

MIT License

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
