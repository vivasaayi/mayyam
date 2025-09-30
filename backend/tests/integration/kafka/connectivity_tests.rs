#[cfg(feature = "integration-tests")]
mod tests {
    use std::time::Duration;
    use crate::tests::integration::helpers::kafka_test_helper::KafkaTestHelper;
    
    fn kafka_tests_enabled() -> bool {
        std::env::var("ENABLE_KAFKA_TESTS").ok().as_deref() == Some("1")
    }

    #[tokio::test]
    async fn test_kafka_connectivity() {
    if !kafka_tests_enabled() { eprintln!("Skipping Kafka test: set ENABLE_KAFKA_TESTS=1 to run"); return; }
        let helper = KafkaTestHelper::new("localhost:9092")
            .await
            .expect("Failed to create Kafka test helper");

        // Test basic connectivity by fetching metadata
        let metadata = helper.admin_client.inner()
            .fetch_metadata(None, Duration::from_secs(10))
            .expect("Failed to fetch Kafka metadata");

        assert!(metadata.brokers().len() > 0, "No brokers found in Kafka cluster");
        assert!(metadata.brokers()[0].id() >= 0, "Invalid broker ID");

        println!("✅ Kafka connectivity test passed - found {} brokers", metadata.brokers().len());
    }

    #[tokio::test]
    async fn test_topic_creation_and_deletion() {
    if !kafka_tests_enabled() { eprintln!("Skipping Kafka test: set ENABLE_KAFKA_TESTS=1 to run"); return; }
        let helper = KafkaTestHelper::new("localhost:9092")
            .await
            .expect("Failed to create Kafka test helper");

        let topic_name = KafkaTestHelper::generate_test_topic_name("test-connectivity");

        // Create topic
        helper.create_test_topic(&topic_name, 1, 1)
            .await
            .expect("Failed to create test topic");

        // Wait for topic to be available
        helper.wait_for_topic(&topic_name, 10)
            .await
            .expect("Topic was not created within timeout");

        // Verify topic exists
        let metadata = helper.admin_client.inner()
            .fetch_metadata(None, Duration::from_secs(5))
            .expect("Failed to fetch metadata after topic creation");

        let topic_exists = metadata.topics().iter().any(|t| t.name() == topic_name);
        assert!(topic_exists, "Created topic not found in metadata");

        // Delete topic
        helper.delete_test_topic(&topic_name)
            .await
            .expect("Failed to delete test topic");

        // Wait a bit for deletion to propagate
        tokio::time::sleep(Duration::from_secs(2)).await;

        println!("✅ Topic creation and deletion test passed");
    }

    #[tokio::test]
    async fn test_message_produce_and_consume() {
    if !kafka_tests_enabled() { eprintln!("Skipping Kafka test: set ENABLE_KAFKA_TESTS=1 to run"); return; }
        let helper = KafkaTestHelper::new("localhost:9092")
            .await
            .expect("Failed to create Kafka test helper");

        let topic_name = KafkaTestHelper::generate_test_topic_name("test-message");

        // Create topic
        helper.create_test_topic(&topic_name, 1, 1)
            .await
            .expect("Failed to create test topic");

        helper.wait_for_topic(&topic_name, 10)
            .await
            .expect("Topic was not created within timeout");

        // Produce a message
        let test_message = r#"{"test": "data", "timestamp": 1234567890}"#;
        let (partition, offset) = helper.produce_message(
            &topic_name,
            Some("test-key"),
            test_message,
            None,
        ).await.expect("Failed to produce message");

        assert!(partition >= 0, "Invalid partition number");
        assert!(offset >= 0, "Invalid offset");

        // Consume the message
        let group_id = KafkaTestHelper::generate_test_topic_name("test-group");
        let messages = helper.consume_messages(
            &topic_name,
            &group_id,
            1,
            Duration::from_secs(10),
        ).await.expect("Failed to consume messages");

        assert_eq!(messages.len(), 1, "Expected exactly one message");

        let consumed_message: serde_json::Value = messages[0].clone();
        assert_eq!(consumed_message["test"], "data", "Message content mismatch");
        assert_eq!(consumed_message["timestamp"], 1234567890, "Message timestamp mismatch");

        // Clean up
        helper.delete_test_topic(&topic_name)
            .await
            .expect("Failed to clean up test topic");

        println!("✅ Message produce and consume test passed");
    }
}
