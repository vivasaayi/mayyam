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


use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

/// Helper struct for Kafka integration testing
pub struct KafkaTestHelper {
    bootstrap_servers: String,
    admin_client: AdminClient<DefaultClientContext>,
    producer: FutureProducer,
}

impl KafkaTestHelper {
    /// Create a new Kafka test helper
    pub async fn new(bootstrap_servers: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", bootstrap_servers);
        config.set("client.id", "kafka-test-helper");

        let admin_client: AdminClient<_> = config.create()?;
        let producer: FutureProducer = config.create()?;

        Ok(Self {
            bootstrap_servers: bootstrap_servers.to_string(),
            admin_client,
            producer,
        })
    }

    /// Create a test topic with specified partitions and replication factor
    pub async fn create_test_topic(
        &self,
        topic_name: &str,
        partitions: i32,
        replication_factor: i16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let new_topic = NewTopic::new(
            topic_name,
            partitions,
            TopicReplication::Fixed(replication_factor as i32),
        );

        let options = AdminOptions::new().request_timeout(Some(Duration::from_secs(30)));
        let results = self
            .admin_client
            .create_topics(&[new_topic], &options)
            .await?;

        for result in results {
            match result {
                Ok(_) => println!("Topic '{}' created successfully", topic_name),
                Err((topic, err)) => {
                    return Err(format!("Failed to create topic '{}': {:?}", topic, err).into())
                }
            }
        }

        Ok(())
    }

    /// Delete a test topic
    pub async fn delete_test_topic(
        &self,
        topic_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let options = AdminOptions::new().request_timeout(Some(Duration::from_secs(30)));
        let results = self
            .admin_client
            .delete_topics(&[topic_name], &options)
            .await?;

        for result in results {
            match result {
                Ok(_) => println!("Topic '{}' deleted successfully", topic_name),
                Err((topic, err)) => {
                    return Err(format!("Failed to delete topic '{}': {:?}", topic, err).into())
                }
            }
        }

        Ok(())
    }

    /// Produce a test message to a topic
    pub async fn produce_message(
        &self,
        topic: &str,
        key: Option<&str>,
        value: &str,
        headers: Option<HashMap<String, String>>,
    ) -> Result<(i32, i64), Box<dyn std::error::Error>> {
        let mut record = FutureRecord::to(topic).payload(value);

        if let Some(k) = key {
            record = record.key(k);
        }

        if let Some(hdrs) = headers {
            let mut owned_headers = rdkafka::message::OwnedHeaders::new();
            for (k, v) in hdrs {
                owned_headers = owned_headers.insert(rdkafka::message::Header {
                    key: k.as_str(),
                    value: Some(v.as_bytes()),
                });
            }
            record = record.headers(owned_headers);
        }

        let delivery_status = self
            .producer
            .send(record, Duration::from_secs(10))
            .await
            .map_err(|(e, _msg)| -> Box<dyn std::error::Error> { Box::new(e) })?;
        Ok((delivery_status.0, delivery_status.1))
    }

    /// Consume messages from a topic
    pub async fn consume_messages(
        &self,
        topic: &str,
        group_id: &str,
        max_messages: usize,
        timeout_duration: Duration,
    ) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", &self.bootstrap_servers);
        config.set("group.id", group_id);
        config.set("client.id", "kafka-test-consumer");
        config.set("enable.auto.commit", "false");
        config.set("auto.offset.reset", "earliest");

        let consumer: StreamConsumer = config.create()?;
        consumer.subscribe(&[topic])?;

        let mut messages = Vec::new();
        let start_time = std::time::Instant::now();

        while messages.len() < max_messages && start_time.elapsed() < timeout_duration {
            match timeout(Duration::from_millis(100), consumer.recv()).await {
                Ok(Ok(message)) => {
                    let payload = message
                        .payload()
                        .map(|p| String::from_utf8_lossy(p).to_string())
                        .unwrap_or_default();

                    let key = message
                        .key()
                        .map(|k| String::from_utf8_lossy(k).to_string());

                    let msg_value: Value = serde_json::from_str(&payload)?;

                    messages.push(msg_value);

                    if let Err(e) =
                        consumer.commit_message(&message, rdkafka::consumer::CommitMode::Async)
                    {
                        eprintln!("Failed to commit message: {:?}", e);
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("Error receiving message: {:?}", e);
                    break;
                }
                Err(_) => {
                    // Timeout - this is expected
                    break;
                }
            }
        }

        Ok(messages)
    }

    /// Generate a unique test topic name
    pub fn generate_test_topic_name(prefix: &str) -> String {
        format!("{}-{}", prefix, Uuid::new_v4().simple())
    }

    /// Wait for topic to be created and available
    pub async fn wait_for_topic(
        &self,
        topic_name: &str,
        timeout_secs: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let timeout_duration = Duration::from_secs(timeout_secs);
        let start_time = std::time::Instant::now();

        while start_time.elapsed() < timeout_duration {
            let metadata = self
                .admin_client
                .inner()
                .fetch_metadata(None, Duration::from_secs(5))
                .map_err(|e| format!("Failed to fetch metadata: {:?}", e))?;

            if metadata.topics().iter().any(|t| t.name() == topic_name) {
                return Ok(());
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Err(format!(
            "Topic '{}' was not created within {} seconds",
            topic_name, timeout_secs
        )
        .into())
    }

    /// Clean up all test topics with a specific prefix
    pub async fn cleanup_test_topics(
        &self,
        prefix: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let metadata = self
            .admin_client
            .inner()
            .fetch_metadata(None, Duration::from_secs(10))
            .map_err(|e| format!("Failed to fetch metadata: {:?}", e))?;

        let test_topics: Vec<String> = metadata
            .topics()
            .iter()
            .filter(|t| t.name().starts_with(prefix))
            .map(|t| t.name().to_string())
            .collect();

        if !test_topics.is_empty() {
            let options = AdminOptions::new().request_timeout(Some(Duration::from_secs(30)));
            let topic_refs: Vec<&str> = test_topics.iter().map(|s| s.as_str()).collect();
            let results = self
                .admin_client
                .delete_topics(&topic_refs, &options)
                .await?;

            for result in results {
                match result {
                    Ok(_) => println!("Cleaned up topic"),
                    Err((topic, err)) => eprintln!("Failed to delete topic '{}': {:?}", topic, err),
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_kafka_test_helper_creation() {
        // This test assumes Kafka is running on localhost:9092
        // In a real CI environment, this would be set up differently
        let result = KafkaTestHelper::new("localhost:9092").await;
        // We don't assert success here as Kafka might not be running during unit tests
        match result {
            Ok(_) => println!("Kafka test helper created successfully"),
            Err(e) => println!(
                "Kafka not available (expected in some environments): {:?}",
                e
            ),
        }
    }

    #[test]
    fn test_generate_test_topic_name() {
        let name1 = KafkaTestHelper::generate_test_topic_name("test");
        let name2 = KafkaTestHelper::generate_test_topic_name("test");

        assert!(name1.starts_with("test-"));
        assert!(name2.starts_with("test-"));
        assert_ne!(name1, name2);
    }
}
