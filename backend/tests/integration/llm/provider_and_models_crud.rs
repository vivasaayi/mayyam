use crate::integration::helpers::TestHarness;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn provider_and_models_crud_roundtrip() {
    let harness = TestHarness::new().await;
    let client = harness.client();

    // Create provider
    let create_payload = json!({
        "name": "Test DeepSeek",
        "provider_type": "DeepSeek",
        "model_name": "deepseek-chat",
        "api_endpoint": "https://api.deepseek.com/v1",
        "model_config": {"temperature": 0.2},
        "prompt_format": "OpenAI",
        "enabled": true,
        "is_default": false
    });

    let resp = client
        .post(&harness.build_url("/api/v1/llm-providers"))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&create_payload)
        .send()
        .await
        .expect("create provider failed");
    assert!(
        resp.status().is_success(),
        "create failed: {}",
        resp.status()
    );
    let created: serde_json::Value = resp.json().await.expect("invalid create response");
    let provider_id = Uuid::parse_str(created["id"].as_str().unwrap()).expect("uuid");

    // Update provider
    let update_payload = json!({
        "name": "Test DeepSeek Updated",
        "model_name": "deepseek-reasoner",
        "enabled": false
    });
    let resp = client
        .put(&harness.build_url(&format!("/api/v1/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&update_payload)
        .send()
        .await
        .expect("update provider failed");
    assert!(resp.status().is_success());

    // Get provider and assert fields persisted
    let resp = client
        .get(&harness.build_url(&format!("/api/v1/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("get provider failed");
    assert!(resp.status().is_success());
    let fetched: serde_json::Value = resp.json().await.expect("invalid get response");
    assert_eq!(fetched["name"], "Test DeepSeek Updated");
    assert_eq!(fetched["model_name"], "deepseek-reasoner");
    assert_eq!(fetched["enabled"], false);
    // api_endpoint should mirror base_url
    assert_eq!(fetched["api_endpoint"], "https://api.deepseek.com/v1");

    // Create two models under provider
    let m1 = client
        .post(&harness.build_url(&format!("/api/v1/llm-providers/{}/models", provider_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&json!({"model_name":"deepseek-chat", "model_config": {"temperature": 0.5}, "enabled": true}))
        .send().await.expect("create model1 failed");
    assert!(m1.status().is_success());

    let m2 = client
        .post(&harness.build_url(&format!("/api/v1/llm-providers/{}/models", provider_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&json!({"model_name":"deepseek-reasoner", "model_config": {"temperature": 0.1}, "enabled": false}))
        .send().await.expect("create model2 failed");
    assert!(m2.status().is_success());

    // List models
    let list = client
        .get(&harness.build_url(&format!("/api/v1/llm-providers/{}/models", provider_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("list models failed");
    assert!(list.status().is_success());
    let list_body: serde_json::Value = list.json().await.expect("invalid list body");
    assert_eq!(list_body["total"].as_u64().unwrap_or(0), 2);

    // Toggle a model
    let model_id = list_body["models"][0]["id"].as_str().unwrap().to_string();
    let toggle = client
        .post(&harness.build_url(&format!(
            "/api/v1/llm-providers/{}/models/{}/toggle?enabled=false",
            provider_id, model_id
        )))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("toggle model failed");
    assert!(toggle.status().is_success());

    // Delete provider (cascade should remove models)
    let del = client
        .delete(&harness.build_url(&format!("/api/v1/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("delete provider failed");
    assert!(del.status().is_success() || del.status().as_u16() == 204);
}
