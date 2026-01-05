#!/usr/bin/env rust-script
//! Test script to send FCM notifications via Firebase Admin SDK
//! 
//! Dependencies (for rust-script):
//! ```cargo
//! [dependencies]
//! reqwest = { version = "0.11", features = ["json", "blocking"] }
//! serde = { version = "1", features = ["derive"] }
//! serde_json = "1"
//! base64 = "0.21"
//! rsa = "0.9"
//! sha2 = "0.10"
//! ```
//!
//! Usage: 
//!   cargo run --bin test_notification
//!   OR
//!   TOPIC=your_topic cargo run --bin test_notification

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

// Firebase service account configuration
// REPLACE with your actual service account credentials
const SERVICE_ACCOUNT: &str = r#"{
    "type": "service_account",
    "project_id": "testingmachine-agus",
    "private_key_id": "c226c6bd30ca5d5dea2f338f52911d7d6c0e019e",
    "private_key": "-----BEGIN PRIVATE KEY-----\nMIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDyEnNyBZo9QhX3\nM2A1I3X+B8xzsI4cJPPaxgCA+ROcjXml1vei5w6Mq2tm0AQF2ejkyIHmugsIxmS6\nvekv29kiIIGy1S1irx5Gdx+qVl1rvr0Uwwu8SwGiN1dG9amJyYSgcqpDh6gcKClP\nuJM4uQ+lBSk63tndVka5yZC6+1s/H/mTTBxndJmmOj+tG8K77Iqm1QtEe+tsUU/V\nr1IhkVxB2X70/bbYb6F+6kP0BZ7sq98f58cOeNTPuNTgHBFE5De7IW54S6bvK/tT\nLy/ohp7MgLSBCOSyTC5PHZfBQO8rrtG8ONJDxAuFCnQo9AKrRbyGAk3+OEvFRDRC\nRkA6ujOtAgMBAAECggEAB0lkZbyC79CfMiV/AiJ64QZxklLweCrplz6KEdfNjhMZ\nJBOUOThc0QGw9jORR27PiqF5fY1Am0dDjXZ9gDYNM3CIju5JUzTjw6m+z4URg1aG\nwQr8/bJBTpV9YFKxQ4dIIX9KHkXFWvGfirmK3vR9ItHEs7O1GatI7jtByssZrEz2\nXGsXn2Y6JRWGcrBNOqCjVmwwT+Gg+SCw/vHsnCoffLfE57Q+mJwc7g0Vb65gJW7N\nHj2NJUL7Dk3nkw7B+SCDwHnnhTUqSXHisWvC4U+f9ZNrQ0YByCNJ61oPL5ZzU4uk\nb9AKSH+PH29SICrYezSymcWhBN/SYlhcvSaMmQW/AQKBgQD8vhSR64u2gjPvOnc5\nx7hAPJLzmHvLmsocoqOzFQjwL0n+OBkl17kIPOnlBpxGBGrHHHik+XuoGtgJ8DgJ\nRrVR1AckKQrhwfGBdaJR1uQ7Q03ylaGRPxsp+OsDMVc2zgp/nGf5DplOjFpluBS2\nL2K0cvXBPXwEGbTzmb3KIPMvAQKBgQD1MSno9M7lPdOSHXuBqeWXQymA30yLTEzB\ngT20YmBvE+wb4QvY4XoW0dCuw+j/IfFo2oCy9bsdKYt0GDzrGh4LPqS4iHTnvt7o\nIOfkpS4U3tQc1F4CCL1zdv/NvQWp9kkg6mMFEHaKGmb9Nzn8xGdri6vNaM5IwGNI\nYLvh1NNwrQKBgQCEWsZK5B72JJkt3mAxUfWbLh2Gk4PAy/6roEA5t/pGTX1iM953\ngtDTD5Ms7JlJ0WZZfv9u5XdsQSKBkdrGgNDTWUWkGhoov3fJY+DtGqvKnSRktRc3\nCOUgxcnMkjop6Rx2V2Hbe1mtWLK+MfgmsAnwlYM8/wXC3/Ny3kAVacvwAQKBgFJz\nQ9qYZ/JihgC+dUe28AObjBtP+5dkAvOXGD7OTgtMMbt2Q9uchsehqoD6VCFnMpzF\nzT1gsJkv3Tse421TjQLO/+klydocLyzz08bpXMOb4swHBc29TqfDPmXayErNDO5K\nox2S2am7EyLXLWK4UOazQwSB18xTFY/DJ6rbPHbJAoGBAN8gGpeWnItuPBfkk1TK\nqX+Iw+agHtto1aVqDiPJVPGKszBt6/kYuPaCCiaayZonR/rpMmiby5M2gbKCroTF\n9ETH7MLc14vY+dwTUc5K+YG6EsMkdSYENd6CEQixpwVCCmORfdLOfcsdKrbtOO7T\n6fS1w2CLZ39wOV1m9wxLYbb4\n-----END PRIVATE KEY-----\n",
    "client_email": "firebase-adminsdk-7fcl6@testingmachine-agus.iam.gserviceaccount.com",
    "client_id": "116131406041097345849",
    "auth_uri": "https://accounts.google.com/o/oauth2/auth",
    "token_uri": "https://oauth2.googleapis.com/token"
}"#;

#[derive(Debug, Deserialize)]
struct ServiceAccount {
    project_id: String,
    private_key: String,
    client_email: String,
    token_uri: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    expires_in: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== FCM Notification Test ===\n");
    
    // Parse service account
    let sa: ServiceAccount = serde_json::from_str(SERVICE_ACCOUNT)?;
    println!("Project ID: {}", sa.project_id);
    println!("Client Email: {}", sa.client_email);
    
    // Get topic from env or use default
    let topic = std::env::var("TOPIC").unwrap_or_else(|_| "test-topic".to_string());
    println!("Target Topic: {}\n", topic);
    
    // Get access token
    println!("1. Getting access token...");
    let access_token = get_access_token(&sa)?;
    println!("   ✓ Access token obtained\n");
    
    // Send notification
    println!("2. Sending notification to topic '{}'...", topic);
    let result = send_notification(&sa.project_id, &access_token, &topic)?;
    println!("   Response: {}\n", result);
    
    println!("=== Done ===");
    Ok(())
}

fn get_access_token(sa: &ServiceAccount) -> Result<String, Box<dyn std::error::Error>> {
    use rsa::{RsaPrivateKey, pkcs8::DecodePrivateKey};
    use rsa::pkcs1v15::SigningKey;
    use rsa::signature::{Signer, SignatureEncoding};
    use sha2::Sha256;
    
    // Parse private key
    let private_key = RsaPrivateKey::from_pkcs8_pem(&sa.private_key)?;
    let signing_key = SigningKey::<Sha256>::new(private_key);
    
    // Create JWT header
    let header = json!({
        "typ": "JWT",
        "alg": "RS256"
    });
    
    // Create JWT payload
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let payload = json!({
        "iss": sa.client_email,
        "scope": "https://www.googleapis.com/auth/firebase.messaging",
        "aud": sa.token_uri,
        "exp": now + 3600,
        "iat": now
    });
    
    // Encode header and payload
    let encoded_header = URL_SAFE_NO_PAD.encode(serde_json::to_string(&header)?);
    let encoded_payload = URL_SAFE_NO_PAD.encode(serde_json::to_string(&payload)?);
    
    // Create signature
    let message = format!("{}.{}", encoded_header, encoded_payload);
    let signature = signing_key.sign(message.as_bytes());
    let encoded_signature = URL_SAFE_NO_PAD.encode(signature.to_bytes());
    
    // Create JWT
    let jwt = format!("{}.{}.{}", encoded_header, encoded_payload, encoded_signature);
    
    // Request access token
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;
    
    let response: TokenResponse = client
        .post(&sa.token_uri)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", &jwt),
        ])
        .send()?
        .json()?;
    
    Ok(response.access_token)
}

fn send_notification(
    project_id: &str,
    access_token: &str,
    topic: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!(
        "https://fcm.googleapis.com/v1/projects/{}/messages:send",
        project_id
    );
    
    let notification = json!({
        "message": {
            "token": topic,
            "notification": {
                "title": "Test Notification from Rust",
                "body": "This is a test notification sent via FCM v1 API"
            },
            "data": {
                "type": "test hehe",
                "timestamp": SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    .to_string(),
                "source": "rust_test_script"
            }
        }
    });
    
    println!("   Payload: {}", serde_json::to_string_pretty(&notification)?);
    
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;
    
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .json(&notification)
        .send()?
        .text()?;
    
    Ok(response)
}
