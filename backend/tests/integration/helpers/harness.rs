use reqwest::Client;
use std::sync::OnceLock;
use crate::integration::helpers::server::ensure_server;
use crate::integration::helpers::auth::get_auth_token;

/// Global HTTP client shared across all integration tests
static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

/// Test configuration structure
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub base_url: String,
    pub enable_aws_tests: bool,
    pub enable_kafka_tests: bool,
    pub enable_k8s_tests: bool,
    pub test_delay_ms: u64,
}

impl TestConfig {
    /// Create test configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            base_url: std::env::var("TEST_API_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string()),
            enable_aws_tests: std::env::var("ENABLE_AWS_TESTS").ok().as_deref() == Some("1"),
            enable_kafka_tests: std::env::var("ENABLE_KAFKA_TESTS").ok().as_deref() == Some("1"),
            enable_k8s_tests: std::env::var("ENABLE_K8S_TESTS").ok().as_deref() == Some("1"),
            test_delay_ms: std::env::var("TEST_DELAY_MS")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .unwrap_or(100),
        }
    }
}

/// Test harness that provides shared resources for all integration tests
#[derive(Debug)]
pub struct TestHarness {
    pub client: &'static Client,
    pub config: TestConfig,
    pub auth_token: String,
}

impl TestHarness {
    /// Create a new test harness with all necessary setup
    pub async fn new() -> Self {
        // Ensure server is running
        ensure_server().await;

        let config = TestConfig::from_env();
        let client = get_shared_client();
        let auth_token = get_auth_token().await;

        Self {
            client,
            config,
            auth_token,
        }
    }

    /// Get the shared HTTP client
    pub fn client(&self) -> &Client {
        self.client
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    /// Get the auth token
    pub fn auth_token(&self) -> &str {
        &self.auth_token
    }

    /// Check if AWS tests are enabled
    pub fn aws_tests_enabled(&self) -> bool {
        self.config.enable_aws_tests
    }

    /// Check if Kafka tests are enabled
    pub fn kafka_tests_enabled(&self) -> bool {
        self.config.enable_kafka_tests
    }

    /// Check if K8s tests are enabled
    pub fn k8s_tests_enabled(&self) -> bool {
        self.config.enable_k8s_tests
    }

    /// Add test delay to prevent overwhelming the server
    pub async fn test_delay(&self) {
        tokio::time::sleep(std::time::Duration::from_millis(self.config.test_delay_ms)).await;
    }

    /// Build a full URL for an endpoint
    pub fn build_url(&self, path: &str) -> String {
        format!("{}{}", self.base_url(), path)
    }
}

/// Get the shared HTTP client (used by TestHarness)
fn get_shared_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .http1_only()
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(std::time::Duration::from_secs(30))
            .tcp_nodelay(true)
            .build()
            .expect("Failed to create shared HTTP client")
    })
}

/// Get AWS credentials from environment variables
/// Panics if required environment variables are not set
pub fn get_aws_credentials() -> (String, String, String, String) {
    let access_key = std::env::var("AWS_ACCESS_KEY_ID")
        .expect("AWS_ACCESS_KEY_ID environment variable must be set for integration tests. Set it to run tests against real AWS.");
    let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY")
        .expect("AWS_SECRET_ACCESS_KEY environment variable must be set for integration tests. Set it to run tests against real AWS.");
    let region = std::env::var("AWS_DEFAULT_REGION")
        .unwrap_or_else(|_| "us-east-1".to_string());
    let account_id = std::env::var("AWS_ACCOUNT_ID")
        .expect("AWS_ACCOUNT_ID environment variable must be set for integration tests. Set it to run tests against real AWS.");

    (access_key, secret_key, region, account_id)
}

/// Get test account ID (different from real account to avoid conflicts)
pub fn get_test_account_id() -> String {
    std::env::var("TEST_AWS_ACCOUNT_ID")
        .unwrap_or_else(|_| "123456789012".to_string())
}
