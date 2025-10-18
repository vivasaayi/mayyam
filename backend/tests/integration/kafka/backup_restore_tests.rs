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
        .post(&harness.build_url(&format!(
            "/api/kafka/clusters/{}/topics",
            cluster_id()
        )))
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
            cluster_id(), topic_name
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

async fn produce_messages(harness: &TestHarness, topic_name: &str, messages: Vec<Value>) {
    for message in messages {
        let produce_response = harness
            .client()
            .post(&harness.build_url(&format!(
                "/api/kafka/clusters/{}/topics/{}/produce",
                cluster_id(), topic_name
            )))
            .header("Authorization", format!("Bearer {}", harness.auth_token()))
            .json(&message)
            .send()
            .await
            .expect("failed to produce message");

        assert!(produce_response.status().is_success());
    }
}

#[tokio::test]
async fn test_backup_topic_messages() {
    if !kafka_tests_enabled() {
        eprintln!("Skipping Kafka backup test: set ENABLE_KAFKA_TESTS=1 to run");
        return;
    }

    let harness = TestHarness::new().await;
    let topic_name = format!("backup-test-topic-{}", Uuid::new_v4().simple());

    // Create topic
    create_topic(&harness, &topic_name).await;

    // Produce some test messages
    let messages = vec![
        json!({
            "key": "key1",
            "value": "{\"id\":1,\"data\":\"test message 1\"}",
            "headers": {"source": "test"}
        }),
        json!({
            "key": "key2",
            "value": "{\"id\":2,\"data\":\"test message 2\"}",
            "headers": {"source": "test"}
        }),
        json!({
            "key": "key3",
            "value": "{\"id\":3,\"data\":\"test message 3\"}",
            "headers": {"source": "test"}
        }),
    ];

    produce_messages(&harness, &topic_name, messages).await;

    // Backup the topic
    let backup_request = json!({
        "topic": topic_name,
        "partitions": null,
        "start_offset": null,
        "end_offset": null,
        "max_messages": null,
        "include_headers": true,
        "include_timestamps": true
    });

    let backup_response = harness
        .client()
        .post(&harness.build_url(&format!(
            "/api/kafka/clusters/{}/backup",
            cluster_id()
        )))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&backup_request)
        .send()
        .await
        .expect("failed to backup topic");

    assert!(backup_response.status().is_success());

    let backup_result: Value = backup_response
        .json()
        .await
        .expect("invalid backup response");

    // Verify backup response structure
    assert_eq!(backup_result["topic"], topic_name);
    assert!(backup_result["partitions_backed_up"].is_array());
    assert!(backup_result["total_messages"].is_number());
    assert!(backup_result["backup_id"].is_string());
    assert!(backup_result["start_time"].is_string());
    assert!(backup_result["end_time"].is_string());

    let total_messages = backup_result["total_messages"].as_u64().unwrap();
    assert_eq!(total_messages, 3, "expected 3 messages to be backed up");

    // Clean up
    delete_topic(&harness, &topic_name).await;
}

#[tokio::test]
async fn test_backup_and_restore_topic_messages() {
    if !kafka_tests_enabled() {
        eprintln!("Skipping Kafka backup/restore test: set ENABLE_KAFKA_TESTS=1 to run");
        return;
    }

    let harness = TestHarness::new().await;
    let source_topic = format!("backup-source-topic-{}", Uuid::new_v4().simple());
    let target_topic = format!("restore-target-topic-{}", Uuid::new_v4().simple());

    // Create source topic
    create_topic(&harness, &source_topic).await;
    create_topic(&harness, &target_topic).await;

    // Produce test messages to source topic
    let messages = vec![
        json!({
            "key": "msg1",
            "value": "{\"id\":1,\"content\":\"backup test message 1\"}",
            "headers": {"test": "backup-restore"}
        }),
        json!({
            "key": "msg2",
            "value": "{\"id\":2,\"content\":\"backup test message 2\"}",
            "headers": {"test": "backup-restore"}
        }),
    ];

    produce_messages(&harness, &source_topic, messages).await;

    // Backup the source topic
    let backup_request = json!({
        "topic": source_topic,
        "partitions": null,
        "start_offset": null,
        "end_offset": null,
        "max_messages": null,
        "include_headers": true,
        "include_timestamps": true
    });

    let backup_response = harness
        .client()
        .post(&harness.build_url(&format!(
            "/api/kafka/clusters/{}/backup",
            cluster_id()
        )))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&backup_request)
        .send()
        .await
        .expect("failed to backup topic");

    assert!(backup_response.status().is_success());

    let backup_result: Value = backup_response
        .json()
        .await
        .expect("invalid backup response");

    let backup_id = backup_result["backup_id"].as_str().unwrap();

    // Restore to target topic
    let restore_request = json!({
        "target_topic": target_topic,
        "backup_id": backup_id,
        "partitions": null,
        "preserve_timestamps": true,
        "preserve_keys": true,
        "preserve_headers": true
    });

    let restore_response = harness
        .client()
        .post(&harness.build_url(&format!(
            "/api/kafka/clusters/{}/restore",
            cluster_id()
        )))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&restore_request)
        .send()
        .await
        .expect("failed to restore topic");

    assert!(restore_response.status().is_success());

    let restore_result: Value = restore_response
        .json()
        .await
        .expect("invalid restore response");

    // Verify restore response
    assert_eq!(restore_result["target_topic"], target_topic);
    assert!(restore_result["messages_restored"].is_number());
    assert!(restore_result["partitions_restored"].is_array());

    let messages_restored = restore_result["messages_restored"].as_u64().unwrap();
    assert_eq!(messages_restored, 2, "expected 2 messages to be restored");

    // Verify messages were restored by consuming from target topic
    let consume_payload = json!({
        "group_id": format!("restore-test-group-{}", Uuid::new_v4().simple()),
        "max_messages": 10,
        "timeout_ms": 5000,
        "from_beginning": true
    });

    let consume_response = harness
        .client()
        .post(&harness.build_url(&format!(
            "/api/kafka/clusters/{}/topics/{}/consume",
            cluster_id(), target_topic
        )))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&consume_payload)
        .send()
        .await
        .expect("failed to consume restored messages");

    assert!(consume_response.status().is_success());

    let consumed_messages: Vec<Value> = consume_response
        .json()
        .await
        .expect("invalid consume response");

    assert_eq!(consumed_messages.len(), 2, "expected 2 messages in restored topic");

    // Verify message content
    let keys: Vec<&str> = consumed_messages.iter()
        .map(|msg| msg["key"].as_str().unwrap())
        .collect();

    assert!(keys.contains(&"msg1"));
    assert!(keys.contains(&"msg2"));

    // Clean up
    delete_topic(&harness, &source_topic).await;
    delete_topic(&harness, &target_topic).await;
}

#[tokio::test]
async fn test_backup_with_filters() {
    if !kafka_tests_enabled() {
        eprintln!("Skipping Kafka backup filters test: set ENABLE_KAFKA_TESTS=1 to run");
        return;
    }

    let harness = TestHarness::new().await;
    let topic_name = format!("backup-filters-topic-{}", Uuid::new_v4().simple());

    // Create topic
    create_topic(&harness, &topic_name).await;

    // Produce multiple messages
    let messages = (1..=10).map(|i| {
        json!({
            "key": format!("key{}", i),
            "value": format!("{{\"id\":{},\"data\":\"message {}\"}}", i, i),
            "headers": {"batch": "test"}
        })
    }).collect::<Vec<_>>();

    produce_messages(&harness, &topic_name, messages).await;

    // Backup with max_messages limit
    let backup_request = json!({
        "topic": topic_name,
        "partitions": null,
        "start_offset": null,
        "end_offset": null,
        "max_messages": 5,
        "include_headers": true,
        "include_timestamps": true
    });

    let backup_response = harness
        .client()
        .post(&harness.build_url(&format!(
            "/api/kafka/clusters/{}/backup",
            cluster_id()
        )))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&backup_request)
        .send()
        .await
        .expect("failed to backup topic with filters");

    assert!(backup_response.status().is_success());

    let backup_result: Value = backup_response
        .json()
        .await
        .expect("invalid backup response");

    let total_messages = backup_result["total_messages"].as_u64().unwrap();
    assert_eq!(total_messages, 5, "expected only 5 messages due to max_messages limit");

    // Clean up
    delete_topic(&harness, &topic_name).await;
}
