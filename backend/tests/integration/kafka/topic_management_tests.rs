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
async fn test_topic_creation_with_partition_count() {
    if !kafka_tests_enabled() {
        eprintln!("Skipping Kafka test: set ENABLE_KAFKA_TESTS=1 to run");
        return;
    }

    let harness = TestHarness::new().await;
    let topic_name = format!("integration-topic-{}", Uuid::new_v4().simple());

    // Create topic with 2 partitions
    let response = harness
        .client()
        .post(&harness.build_url(&format!("/api/kafka/clusters/{}/topics", cluster_id())))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&json!({
            "name": topic_name,
            "partitions": 2,
            "replication_factor": 1,
            "configs": null
        }))
        .send()
        .await
        .expect("failed to create topic");

    assert!(response.status().is_success());

    // Verify it exists in the topic list
    let list_response = harness
        .client()
        .get(&harness.build_url(&format!("/api/kafka/clusters/{}/topics", cluster_id())))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to list topics");

    assert!(list_response.status().is_success());
    let topics: Vec<Value> = list_response.json().await.expect("invalid topics response");
    
    // We can verify partitions by fetching the specific topic details
    let details_response = harness
        .client()
        .get(&harness.build_url(&format!("/api/kafka/clusters/{}/topics/{}", cluster_id(), topic_name)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to get topic details");

    assert!(details_response.status().is_success());
    let topic_details: Value = details_response.json().await.expect("invalid topic details response");
    assert_eq!(topic_details["partitions"].as_array().unwrap().len(), 2);

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
async fn test_update_topic_config() {
    if !kafka_tests_enabled() {
        eprintln!("Skipping Kafka test: set ENABLE_KAFKA_TESTS=1 to run");
        return;
    }

    let harness = TestHarness::new().await;
    let topic_name = format!("integration-topic-{}", Uuid::new_v4().simple());

    // Create a topic first
    let create_res = harness
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
    assert!(create_res.status().is_success());

    // Update the config: e.g., retention.ms
    let update_res = harness
        .client()
        .put(&harness.build_url(&format!("/api/kafka/clusters/{}/topics/{}/config", cluster_id(), topic_name)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&json!({
            "configs": [
                ["retention.ms", "86400000"]
            ],
            "validate_only": false
        }))
        .send()
        .await
        .expect("failed to update config");
    
    assert!(update_res.status().is_success());

    // Delete
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
async fn test_add_partitions() {
    if !kafka_tests_enabled() {
        eprintln!("Skipping Kafka test: set ENABLE_KAFKA_TESTS=1 to run");
        return;
    }

    let harness = TestHarness::new().await;
    let topic_name = format!("integration-topic-{}", Uuid::new_v4().simple());

    // Create a topic with 1 partition
    let create_res = harness
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
    assert!(create_res.status().is_success());

    // Add partitions (total count becomes 2)
    let add_res = harness
        .client()
        .post(&harness.build_url(&format!("/api/kafka/clusters/{}/topics/{}/partitions", cluster_id(), topic_name)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&json!({
            "count": 2,
            "validate_only": false
        }))
        .send()
        .await
        .expect("failed to add partitions");
    
    assert!(add_res.status().is_success());

    // Verify
    let details_response = harness
        .client()
        .get(&harness.build_url(&format!("/api/kafka/clusters/{}/topics/{}", cluster_id(), topic_name)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to get topic details");

    assert!(details_response.status().is_success());
    let topic_details: Value = details_response.json().await.expect("invalid topic details response");
    assert_eq!(topic_details["partitions"].as_array().unwrap().len(), 2);

    // Delete
    let del_res = harness
        .client()
        .delete(&harness.build_url(&format!("/api/kafka/clusters/{}/topics/{}", cluster_id(), topic_name)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to delete topic");
    assert!(del_res.status().is_success());
}
