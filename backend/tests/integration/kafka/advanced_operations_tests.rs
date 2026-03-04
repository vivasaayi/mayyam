// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


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

#[tokio::test]
async fn test_kafka_metrics_endpoint() {
    if !kafka_tests_enabled() {
        eprintln!("Skipping Kafka test: set ENABLE_KAFKA_TESTS=1 to run");
        return;
    }

    let harness = TestHarness::new().await;

    let response = harness
        .client()
        .get(&harness.build_url("/api/kafka/metrics"))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to check kafka metrics");

    assert!(response.status().is_success());
    let body: Value = response.json().await.expect("invalid metrics response");
    assert!(body.get("messages_produced").is_some());
    assert!(body.get("active_connections").is_some());
}

#[tokio::test]
async fn test_batch_produce() {
    if !kafka_tests_enabled() {
        eprintln!("Skipping Kafka test: set ENABLE_KAFKA_TESTS=1 to run");
        return;
    }

    let harness = TestHarness::new().await;
    let topic_name = format!("integration-topic-batch-{}", Uuid::new_v4().simple());

    // Create a topic to ensure we can produce
    let _create_res = harness
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

    // Allow time for topic metadata to propagate
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    let batch_payload = json!({
        "messages": [
            {
                "key": "key1",
                "value": "value1",
                "headers": null
            },
            {
                "key": "key2",
                "value": "value2",
                "headers": null
            }
        ]
    });

    let produce_response = harness
        .client()
        .post(&harness.build_url(&format!(
            "/api/kafka/clusters/{}/batch-produce",
            cluster_id()
        )))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&batch_payload)
        .send()
        .await
        .expect("failed to batch produce messages");

    assert!(produce_response.status().is_success());
    
    // Clean up
    let del_res = harness
        .client()
        .delete(&harness.build_url(&format!("/api/kafka/clusters/{}/topics/{}", cluster_id(), topic_name)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to delete topic");
    assert!(del_res.status().is_success());
}

#[tokio::test]
async fn test_produce_retry() {
    if !kafka_tests_enabled() {
        eprintln!("Skipping Kafka test: set ENABLE_KAFKA_TESTS=1 to run");
        return;
    }

    let harness = TestHarness::new().await;
    let topic_name = format!("integration-topic-retry-{}", Uuid::new_v4().simple());

    // Create topic
    let _create_res = harness
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

    // Allow time for topic metadata to propagate
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    let retry_payload = json!({
        "topic": topic_name.clone(),
        "message": {
            "key": "retry-key",
            "value": "retry-value",
            "headers": []
        },
        "max_retries": 3
    });

    let produce_response = harness
        .client()
        .post(&harness.build_url(&format!(
            "/api/kafka/clusters/{}/produce-retry",
            cluster_id()
        )))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&retry_payload)
        .send()
        .await
        .expect("failed to retry produce message");

    assert!(produce_response.status().is_success());

    // Clean up
    let del_res = harness
        .client()
        .delete(&harness.build_url(&format!("/api/kafka/clusters/{}/topics/{}", cluster_id(), topic_name)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to delete topic");
    assert!(del_res.status().is_success());
}

#[tokio::test]
async fn test_broker_status() {
    if !kafka_tests_enabled() {
        eprintln!("Skipping Kafka test: set ENABLE_KAFKA_TESTS=1 to run");
        return;
    }

    let harness = TestHarness::new().await;

    let response = harness
        .client()
        .get(&harness.build_url(&format!("/api/kafka/clusters/{}/brokers", cluster_id())))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to get broker status");

    assert!(response.status().is_success());
    let body: Value = response.json().await.expect("invalid broker response");
    assert!(body.get("brokers").is_some());
    assert!(body.get("total_brokers").is_some());
}
