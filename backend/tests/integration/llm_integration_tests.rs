#![cfg(feature = "integration-tests")]

use crate::integration::helpers::TestHarness;
use serde_json::{json, Value};
use uuid::Uuid;

async fn create_test_provider(harness: &TestHarness) -> Value {
    let provider_name = format!("integration-provider-{}", Uuid::new_v4());
    let payload = json!({
        "name": provider_name,
        "provider_type": "DeepSeek",
        "model_name": "deepseek-chat",
        "api_endpoint": "https://api.deepseek.com/v1",
        "api_key": null,
        "model_config": {"temperature": 0.2},
        "prompt_format": "OpenAI",
        "description": "integration test provider",
        "enabled": true,
        "is_default": false
    });

    let response = harness
        .client()
        .post(&harness.build_url("/api/v1/llm-providers"))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&payload)
        .send()
        .await
        .expect("failed to create llm provider");

    assert_eq!(response.status().as_u16(), 201);

    response
        .json::<Value>()
        .await
        .expect("failed to parse provider creation response")
}

async fn delete_test_provider(harness: &TestHarness, provider_id: &str) {
    let response = harness
        .client()
        .delete(&harness.build_url(&format!("/api/v1/llm-providers/{}", provider_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to delete provider");

    assert_eq!(response.status().as_u16(), 204);
}

#[tokio::test]
async fn test_llm_providers_endpoint_lists_created_provider() {
    let harness = TestHarness::new().await;

    let created_provider = create_test_provider(&harness).await;
    let created_id = created_provider["id"].as_str().expect("missing provider id");

    let list_response = harness
        .client()
        .get(&harness.build_url("/api/llm/providers"))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to list providers");

    assert_eq!(list_response.status().as_u16(), 200);

    let body: Value = list_response
        .json()
        .await
        .expect("failed to parse provider list");

    let providers = body["providers"].as_array().expect("providers should be an array");
    assert!(
        providers.iter().any(|provider| provider["id"] == created_id),
        "created provider missing from list"
    );

    delete_test_provider(&harness, created_id).await;
}

#[tokio::test]
async fn test_llm_chat_rejects_empty_messages() {
    let harness = TestHarness::new().await;

    let payload = json!({
        "messages": [],
        "temperature": 0.7
    });

    let response = harness
        .client()
        .post(&harness.build_url("/api/ai/chat"))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&payload)
        .send()
        .await
        .expect("chat request failed");

    assert_eq!(response.status().as_u16(), 400);
}
