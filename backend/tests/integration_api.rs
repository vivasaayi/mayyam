use reqwest::Client;
use std::env;

#[tokio::test]
async fn test_health_endpoint() {
    let api_url = env::var("API_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let client = Client::new();

    let res = client
        .get(format!("{}/health", api_url))
        .send()
        .await
        .expect("Request failed");

    assert!(res.status().is_success(), "Health endpoint should return 200");
}
