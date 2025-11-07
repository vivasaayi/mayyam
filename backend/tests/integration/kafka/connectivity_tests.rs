#![cfg(feature = "integration-tests")]

use crate::integration::helpers::TestHarness;
use serde_json::{json, Value};
use uuid::Uuid;

fn kafka_tests_enabled() -> bool {
    std::env::var("ENABLE_KAFKA_TESTS").ok().as_deref() == Some("1")
}

fn cluster_id() -> String {
    std::env::var("KAFKA_CLUSTER_ID").unwrap_or_else(|_| "test-cluster".to_string())
}

async fn create_topic(harness: &TestHarness, topic_name: &str) {
    let response = harness
        .client()
        .post(&harness.build_url(&format!("/api/kafka/clusters/{}/topics", cluster_id())))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&json!({
            "name": topic_name,
            "partitions": 1,
            "replication_factor": 1,
            "configs": null
        }))
        .send()
        .await
        .expect("failed to create topic");

    assert!(
        response.status().is_success(),
        "topic creation failed: {}",
        response.status()
    );
}

async fn delete_topic(harness: &TestHarness, topic_name: &str) {
    let response = harness
        .client()
        .delete(&harness.build_url(&format!(
            "/api/kafka/clusters/{}/topics/{}",
            cluster_id(),
            topic_name
        )))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to delete topic");

    assert!(
        response.status().is_success(),
        "topic deletion failed: {}",
        response.status()
    );
}

#[tokio::test]
async fn test_kafka_connectivity() {
    if !kafka_tests_enabled() {
        eprintln!("Skipping Kafka test: set ENABLE_KAFKA_TESTS=1 to run");
        return;
    }

    let harness = TestHarness::new().await;

    let response = harness
        .client()
        .get(&harness.build_url(&format!("/api/kafka/clusters/{}/health", cluster_id())))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to check kafka health");

    assert!(response.status().is_success());
    let body: Value = response.json().await.expect("invalid health response");
    assert!(body["status"].is_string());
}

#[tokio::test]
async fn test_topic_creation_and_deletion() {
    if !kafka_tests_enabled() {
        eprintln!("Skipping Kafka test: set ENABLE_KAFKA_TESTS=1 to run");
        return;
    }

    let harness = TestHarness::new().await;
    let topic_name = format!("integration-topic-{}", Uuid::new_v4().simple());

    create_topic(&harness, &topic_name).await;

    let topics_response = harness
        .client()
        .get(&harness.build_url(&format!("/api/kafka/clusters/{}/topics", cluster_id())))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to list topics");

    assert!(topics_response.status().is_success());

    let topics: Vec<Value> = topics_response
        .json()
        .await
        .expect("invalid topics response");

    assert!(
        topics.iter().any(|topic| topic["name"] == topic_name),
        "created topic missing from list"
    );

    delete_topic(&harness, &topic_name).await;
}

#[tokio::test]
async fn test_message_produce_and_consume() {
    if !kafka_tests_enabled() {
        eprintln!("Skipping Kafka test: set ENABLE_KAFKA_TESTS=1 to run");
        return;
    }

    let harness = TestHarness::new().await;
    let topic_name = format!("integration-topic-{}", Uuid::new_v4().simple());

    create_topic(&harness, &topic_name).await;

    let produce_payload = json!({
        "key": "test-key",
        "value": "{\"test\":\"data\",\"timestamp\":1234567890}",
        "headers": null
    });

    let produce_response = harness
        .client()
        .post(&harness.build_url(&format!(
            "/api/kafka/clusters/{}/topics/{}/produce",
            cluster_id(),
            topic_name
        )))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&produce_payload)
        .send()
        .await
        .expect("failed to produce message");

    assert!(produce_response.status().is_success());

    let consume_payload = json!({
        "group_id": format!("integration-group-{}", Uuid::new_v4().simple()),
        "max_messages": 1,
        "timeout_ms": 5000,
        "from_beginning": true
    });

    let consume_response = harness
        .client()
        .post(&harness.build_url(&format!(
            "/api/kafka/clusters/{}/topics/{}/consume",
            cluster_id(),
            topic_name
        )))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&consume_payload)
        .send()
        .await
        .expect("failed to consume messages");

    assert!(consume_response.status().is_success());

    let messages: Vec<Value> = consume_response
        .json()
        .await
        .expect("invalid consume response");

    assert_eq!(messages.len(), 1, "expected a single consumed message");
    assert_eq!(messages[0]["key"], json!("test-key"));

    delete_topic(&harness, &topic_name).await;
}
