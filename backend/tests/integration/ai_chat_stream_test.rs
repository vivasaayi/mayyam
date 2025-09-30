#![cfg(feature = "integration-tests")]

use crate::integration::helpers::TestHarness;
use serde_json::json;

#[tokio::test]
async fn chat_stream_empty_messages_returns_400() {
    let harness = TestHarness::new().await;

    let payload = json!({
        "messages": [],
        "model": null,
        "temperature": 1.0
    });

    let response = harness
        .client()
        .post(&harness.build_url("/api/ai/chat/stream"))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&payload)
        .send()
        .await
        .expect("chat stream request failed");

    assert_eq!(response.status().as_u16(), 400);
}
