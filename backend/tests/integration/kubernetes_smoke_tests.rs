use crate::integration::helpers::TestHarness;

#[tokio::test]
async fn test_kubernetes_list_namespaces_empty_when_no_clusters() {
    if let Some(harness) = TestHarness::try_new().await {
        let response = harness
            .client()
            .get(&harness.build_url("/api/kubernetes/clusters/default/namespaces"))
            .header("Authorization", format!("Bearer {}", harness.auth_token()))
            .send()
            .await
            .expect("request failed");

        assert_eq!(response.status().as_u16(), 404);
    } else {
        eprintln!(
            "Skipping namespaces test: backend not healthy (likely DB down). Set up test DB or run with Docker."
        );
    }
}

#[tokio::test]
async fn test_kubernetes_health_route_exists() {
    if let Some(harness) = TestHarness::try_new().await {
        let resp = harness
            .client()
            .get(&harness.build_url("/health"))
            .send()
            .await
            .expect("health check failed");

        assert!(resp.status().is_success());
    } else {
        eprintln!("Skipping health test: backend not healthy (likely DB down). Set up test DB or run with Docker.");
    }
}
