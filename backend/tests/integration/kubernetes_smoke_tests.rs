use reqwest::Client;
use std::sync::OnceLock;
use crate::integration::helpers::server::try_ensure_server;
use crate::integration::helpers::auth::get_auth_token;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

fn client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build client")
    })
}

fn base_url() -> String {
    std::env::var("TEST_API_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
}

#[tokio::test]
async fn test_kubernetes_list_namespaces_empty_when_no_clusters() {
    // Guard: only run if K8S tests are explicitly enabled OR clusters exist in config
    if std::env::var("ENABLE_K8S_TESTS").ok().as_deref() != Some("1") {
        // Still allow test to run but expect a 404/400 if no clusters are configured
    }

    if let Some(_url) = try_ensure_server().await {
        let token = get_auth_token().await;
        let url = format!("{}/api/kubernetes/clusters/default/namespaces", base_url());

        let resp = client()
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .expect("request failed");

        // With no clusters in config.test.yml, this should be 404 or 400 depending on controller behavior
        assert!(resp.status().is_client_error() || resp.status().as_u16() == 404);
    } else {
        eprintln!("Skipping namespaces test: backend not healthy (likely DB down). Set up test DB or run with Docker.");
    }
}

#[tokio::test]
async fn test_kubernetes_health_route_exists() {
    if let Some(_url) = try_ensure_server().await {
        let url = format!("{}/health", base_url());

        let resp = client()
            .get(&url)
            .send()
            .await
            .expect("health check failed");

        assert!(resp.status().is_success());
    } else {
        eprintln!("Skipping health test: backend not healthy (likely DB down). Set up test DB or run with Docker.");
    }
}
