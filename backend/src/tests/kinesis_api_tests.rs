#[cfg(all(test, feature = "integration-tests"))]
mod kinesis_api_tests {
    use reqwest::Client;
    use serde_json::json;
    use std::time::Duration;
    use tokio::time::sleep;

    use crate::services::aws::aws_types::kinesis::{
        KinesisCreateStreamRequest, KinesisDeleteStreamRequest, KinesisDescribeStreamRequest,
        KinesisPutRecordRequest,
    };

    fn base_url() -> String {
        if let Ok(url) = std::env::var("TEST_API_BASE_URL") {
            return url;
        }
        let port = std::env::var("BACKEND_PORT").unwrap_or_else(|_| "8010".to_string());
        format!("http://127.0.0.1:{}", port)
    }
    const DEFAULT_PROFILE: &str = "default";
    const DEFAULT_REGION: &str = "us-east-1";
    const ADMIN_USERNAME: &str = "admin";
    const ADMIN_PASSWORD: &str = "admin123";

    /// Helper function to get authentication token
    async fn get_auth_token() -> Result<String, Box<dyn std::error::Error>> {
        let client = Client::new();
        let login_url = format!("{}/api/auth/login", base_url());

        let login_request = json!({
            "username": ADMIN_USERNAME,
            "password": ADMIN_PASSWORD
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

        println!("✅ Successfully obtained auth token");
        Ok(token)
    }

    /// Helper function to wait for the backend server to be ready
    async fn wait_for_server() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::new();
        let health_url = format!("{}/health", base_url());

        for attempt in 1..=30 {
            match client.get(&health_url).send().await {
                Ok(response) if response.status().is_success() => {
                    println!("✅ Backend server is ready after {} attempts", attempt);
                    return Ok(());
                }
                Ok(response) => {
                    println!(
                        "⚠️  Attempt {}: Server responded with status {}",
                        attempt,
                        response.status()
                    );
                }
                Err(e) => {
                    println!("⚠️  Attempt {}: Connection failed: {}", attempt, e);
                }
            }
            sleep(Duration::from_secs(1)).await;
        }

        Err("Backend server is not responding after 30 attempts".into())
    }

    fn enabled() -> bool {
        std::env::var("RUN_KINESIS_TESTS")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false)
    }

    /// Test the Kinesis data plane put_record endpoint via HTTP
    #[tokio::test]
    async fn test_kinesis_put_record_api() {
        if !enabled() {
            println!("Skipping Kinesis tests (set RUN_KINESIS_TESTS=1 to enable)");
            return;
        }
        println!("🧪 Testing Kinesis put_record API endpoint...");

        // Wait for server to be ready
        wait_for_server()
            .await
            .expect("Backend server must be ready");

        // Get authentication token
        let token = get_auth_token()
            .await
            .expect("Must be able to authenticate");

        let client = Client::new();
        let url = format!(
            "{}/api/aws-data/profiles/{}/regions/{}/kinesis",
            base_url(),
            DEFAULT_PROFILE,
            DEFAULT_REGION
        );

        // Test data
        let test_request = KinesisPutRecordRequest {
            stream_name: "test-stream".to_string(),
            data: base64::encode("test data").to_string(),
            partition_key: "test-key".to_string(),
            sequence_number: None,
        };

        // Make HTTP request with authentication
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&test_request)
            .send()
            .await
            .expect("HTTP request must succeed");

        println!("📡 HTTP Response status: {}", response.status());

        // Assert success only - no softballing!
        assert!(
            response.status().is_success(),
            "Kinesis put_record must succeed, got status: {}",
            response.status()
        );

        let response_body = response
            .text()
            .await
            .expect("Must be able to read response body");
        println!(
            "✅ Kinesis put_record successful! Response: {}",
            response_body
        );
    }

    /// Test the Kinesis control plane create_stream endpoint via HTTP
    #[tokio::test]
    async fn test_kinesis_create_stream_api() {
        if !enabled() {
            println!("Skipping Kinesis tests (set RUN_KINESIS_TESTS=1 to enable)");
            return;
        }
        println!("🧪 Testing Kinesis create_stream API endpoint...");

        // Wait for server to be ready
        wait_for_server()
            .await
            .expect("Backend server must be ready");

        // Get authentication token
        let token = get_auth_token()
            .await
            .expect("Must be able to authenticate");

        let client = Client::new();
        let url = format!(
            "{}/api/aws-data/profiles/{}/regions/{}/kinesis/streams",
            base_url(),
            DEFAULT_PROFILE,
            DEFAULT_REGION
        );

        // Test data
        let test_request = KinesisCreateStreamRequest {
            stream_name: "test-integration-stream".to_string(),
            shard_count: Some(1),
        };

        // Make HTTP request with authentication
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&test_request)
            .send()
            .await
            .expect("HTTP request must succeed");

        println!("📡 HTTP Response status: {}", response.status());

        // Assert success only - stream MUST be created successfully!
        assert!(
            response.status().is_success(),
            "Kinesis create_stream must succeed, got status: {}",
            response.status()
        );

        let response_body = response
            .text()
            .await
            .expect("Must be able to read response body");
        println!(
            "✅ Kinesis stream created successfully! Response: {}",
            response_body
        );
    }

    /// Test the Kinesis control plane describe_stream endpoint via HTTP
    #[tokio::test]
    async fn test_kinesis_describe_stream_api() {
        if !enabled() {
            println!("Skipping Kinesis tests (set RUN_KINESIS_TESTS=1 to enable)");
            return;
        }
        println!("🧪 Testing Kinesis describe_stream API endpoint...");

        // Wait for server to be ready
        wait_for_server()
            .await
            .expect("Backend server must be ready");

        // Get authentication token
        let token = get_auth_token()
            .await
            .expect("Must be able to authenticate");

        let client = Client::new();
        let url = format!(
            "{}/api/aws-data/profiles/{}/regions/{}/kinesis/streams",
            base_url(),
            DEFAULT_PROFILE,
            DEFAULT_REGION
        );

        // Test data
        let test_request = KinesisDescribeStreamRequest {
            stream_name: "test-integration-stream".to_string(),
        };

        // Make HTTP request with authentication
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&test_request)
            .send()
            .await
            .expect("HTTP request must succeed");

        println!("📡 HTTP Response status: {}", response.status());

        // Assert success only - stream description MUST work!
        assert!(
            response.status().is_success(),
            "Kinesis describe_stream must succeed, got status: {}",
            response.status()
        );

        let response_body = response
            .text()
            .await
            .expect("Must be able to read response body");
        println!(
            "✅ Kinesis stream described successfully! Response: {}",
            response_body
        );
    }

    /// Test the Kinesis control plane delete_stream endpoint via HTTP
    #[tokio::test]
    async fn test_kinesis_delete_stream_api() {
        if !enabled() {
            println!("Skipping Kinesis tests (set RUN_KINESIS_TESTS=1 to enable)");
            return;
        }
        println!("🧪 Testing Kinesis delete_stream API endpoint...");

        // Wait for server to be ready
        wait_for_server()
            .await
            .expect("Backend server must be ready");

        // Get authentication token
        let token = get_auth_token()
            .await
            .expect("Must be able to authenticate");

        // Wait 2 minutes before deleting the stream to ensure it's fully created
        println!("⏰ Waiting 2 minutes before deleting stream to ensure it's fully created...");
        sleep(Duration::from_secs(120)).await;

        let client = Client::new();
        let url = format!(
            "{}/api/aws-data/profiles/{}/regions/{}/kinesis/streams",
            base_url(),
            DEFAULT_PROFILE,
            DEFAULT_REGION
        );

        // Test data
        let test_request = KinesisDeleteStreamRequest {
            stream_name: "test-integration-stream".to_string(),
        };

        // Make HTTP request with authentication
        let response = client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&test_request)
            .send()
            .await
            .expect("HTTP request must succeed");

        println!("📡 HTTP Response status: {}", response.status());

        // Assert success only - stream deletion MUST work!
        assert!(
            response.status().is_success(),
            "Kinesis delete_stream must succeed, got status: {}",
            response.status()
        );

        let response_body = response
            .text()
            .await
            .expect("Must be able to read response body");
        println!(
            "✅ Kinesis stream deleted successfully! Response: {}",
            response_body
        );
    }
}
