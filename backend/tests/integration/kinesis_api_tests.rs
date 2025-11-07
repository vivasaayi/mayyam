#![cfg(feature = "integration-tests")]

use chrono::Utc;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

use mayyam::services::aws::aws_types::kinesis::{
    KinesisCreateStreamRequest,
    KinesisDeleteStreamRequest,
    KinesisDescribeStreamRequest,
    KinesisPutRecordRequest,
};

fn base_url() -> String {
    if let Ok(url) = std::env::var("TEST_API_BASE_URL") {
        return url;
    }
    let port = std::env::var("BACKEND_PORT").unwrap_or_else(|_| "8010".to_string());
    format!("http://127.0.0.1:{port}")
}

const DEFAULT_PROFILE: &str = "default";
const DEFAULT_REGION: &str = "us-east-1";
const ADMIN_USERNAME: &str = "admin";
const ADMIN_PASSWORD: &str = "admin123";

async fn get_auth_token() -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::new();
    let login_url = format!("{}/api/auth/login", base_url());

    let login_request = json!({
        "username": ADMIN_USERNAME,
        "password": ADMIN_PASSWORD,
    });

    let response = client.post(&login_url).json(&login_request).send().await?;

    if !response.status().is_success() {
        return Err(format!("Login failed with status: {}", response.status()).into());
    }

    let response_json: serde_json::Value = response.json().await?;
    let token = response_json["token"]
        .as_str()
        .ok_or("No token in response")?
        .to_string();

    Ok(token)
}

async fn wait_for_server() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let health_url = format!("{}/health", base_url());

    for attempt in 1..=30 {
        match client.get(&health_url).send().await {
            Ok(response) if response.status().is_success() => {
                return Ok(());
            }
            Ok(response) => {
                tracing::warn!(attempt, status = ?response.status(), "backend not ready");
            }
            Err(e) => {
                tracing::warn!(attempt, error = %e, "backend connection failed");
            }
        }
        sleep(Duration::from_secs(1)).await;
    }

    Err("Backend server is not responding after 30 attempts".into())
}

fn tests_enabled() -> bool {
    std::env::var("RUN_KINESIS_TESTS")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}

async fn authorized_client() -> Result<(Client, String), Box<dyn std::error::Error>> {
    wait_for_server().await?;
    let token = get_auth_token().await?;
    Ok((Client::new(), token))
}

fn api_url(path: &str) -> String {
    format!(
        "{}/api/aws-data/profiles/{}/regions/{}/kinesis{}",
        base_url(),
        DEFAULT_PROFILE,
        DEFAULT_REGION,
        path
    )
}

fn unique_stream_name(prefix: &str) -> String {
    format!("{}-{}", prefix, Utc::now().timestamp_millis())
}

async fn create_stream_resource(
    client: &Client,
    token: &str,
    stream_name: &str,
) -> reqwest::Result<reqwest::Response> {
    let request = KinesisCreateStreamRequest {
        stream_name: stream_name.to_string(),
        shard_count: Some(1),
    };

    client
        .post(&api_url("/streams"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&request)
        .send()
        .await
}

async fn delete_stream_resource(
    client: &Client,
    token: &str,
    stream_name: &str,
) -> reqwest::Result<reqwest::Response> {
    let request = KinesisDeleteStreamRequest {
        stream_name: stream_name.to_string(),
    };

    client
        .delete(&api_url("/streams"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&request)
        .send()
        .await
}

#[tokio::test]
async fn kinesis_put_record_api() {
    if !tests_enabled() {
        eprintln!("Skipping Kinesis tests (set RUN_KINESIS_TESTS=1 to enable)");
        return;
    }

    let (client, token) = authorized_client().await.expect("auth token");

    let stream_name = unique_stream_name("test-stream-put");

    let create = create_stream_resource(&client, &token, &stream_name)
        .await
        .expect("stream creation request failed");
    assert!(
        create.status().is_success(),
        "stream creation failed: {}",
        create.status()
    );

    sleep(Duration::from_secs(5)).await;

    let url = api_url("");
    let request = KinesisPutRecordRequest {
        stream_name: stream_name.clone(),
        data: base64::encode("test data"),
        partition_key: "test-key".to_string(),
        sequence_number: None,
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .json(&request)
        .send()
        .await
        .expect("HTTP request must succeed");

    assert!(
        response.status().is_success(),
        "expected success, got {}",
        response.status()
    );

    let cleanup = delete_stream_resource(&client, &token, &stream_name)
        .await
        .expect("cleanup request failed");
    assert!(cleanup.status().is_success(), "cleanup failed: {}", cleanup.status());
}

#[tokio::test]
async fn kinesis_create_stream_api() {
    if !tests_enabled() {
        eprintln!("Skipping Kinesis tests (set RUN_KINESIS_TESTS=1 to enable)");
        return;
    }

    let (client, token) = authorized_client().await.expect("auth token");

    let stream_name = unique_stream_name("test-integration-stream");

    let url = api_url("/streams");
    let request = KinesisCreateStreamRequest {
        stream_name: stream_name.clone(),
        shard_count: Some(1),
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .json(&request)
        .send()
        .await
        .expect("HTTP request must succeed");

    assert!(
        response.status().is_success(),
        "expected success, got {}",
        response.status()
    );

    let cleanup = delete_stream_resource(&client, &token, &stream_name)
        .await
        .expect("cleanup request failed");
    assert!(cleanup.status().is_success(), "cleanup failed: {}", cleanup.status());
}

#[tokio::test]
async fn kinesis_describe_stream_api() {
    if !tests_enabled() {
        eprintln!("Skipping Kinesis tests (set RUN_KINESIS_TESTS=1 to enable)");
        return;
    }

    let (client, token) = authorized_client().await.expect("auth token");

    let stream_name = unique_stream_name("test-describe-stream");

    let create = create_stream_resource(&client, &token, &stream_name)
        .await
        .expect("stream creation request failed");
    assert!(
        create.status().is_success(),
        "stream creation failed: {}",
        create.status()
    );

    sleep(Duration::from_secs(5)).await;

    let url = api_url("/streams");
    let request = KinesisDescribeStreamRequest {
        stream_name: stream_name.clone(),
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .json(&request)
        .send()
        .await
        .expect("HTTP request must succeed");

    assert!(
        response.status().is_success(),
        "expected success, got {}",
        response.status()
    );

    let cleanup = delete_stream_resource(&client, &token, &stream_name)
        .await
        .expect("cleanup request failed");
    assert!(cleanup.status().is_success(), "cleanup failed: {}", cleanup.status());
}

#[tokio::test]
async fn kinesis_delete_stream_api() {
    if !tests_enabled() {
        eprintln!("Skipping Kinesis tests (set RUN_KINESIS_TESTS=1 to enable)");
        return;
    }

    let (client, token) = authorized_client().await.expect("auth token");

    let stream_name = unique_stream_name("test-delete-stream");

    let create = create_stream_resource(&client, &token, &stream_name)
        .await
        .expect("stream creation request failed");
    assert!(
        create.status().is_success(),
        "stream creation failed: {}",
        create.status()
    );

    sleep(Duration::from_secs(5)).await;

    let request = KinesisDeleteStreamRequest {
        stream_name: stream_name.clone(),
    };

    let response = client
        .delete(&api_url("/streams"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&request)
        .send()
        .await
        .expect("HTTP request must succeed");

    assert!(
        response.status().is_success(),
        "expected success, got {}",
        response.status()
    );
}
