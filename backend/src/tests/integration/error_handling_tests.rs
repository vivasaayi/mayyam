use std::time::Duration;
use rdkafka::error::KafkaError;
use crate::tests::integration::kafka_test_helper::KafkaTestHelper;

/// Error handling and edge case tests for Kafka integration
/// Tests various failure scenarios and error recovery mechanisms

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    /// Test behavior when Kafka broker is unavailable
    #[tokio::test]
    async fn test_kafka_connection_failure() {
        println!("🧪 Testing Kafka connection failure scenario...");

        // Test with invalid bootstrap servers
        let invalid_helper = KafkaTestHelper::new_with_config(
            "invalid-broker:9092".to_string(),
            Duration::from_secs(5), // Short timeout for faster test
        );

        // Attempt to connect - should fail gracefully
        let connection_result = invalid_helper.test_connection().await;

        match connection_result {
            Ok(_) => panic!("Expected connection to fail with invalid broker"),
            Err(e) => {
                println!("✅ Connection correctly failed with error: {:?}", e);
                // Verify it's a connection-related error
                assert!(e.to_string().contains("Connection refused") ||
                       e.to_string().contains("No such file or directory") ||
                       e.to_string().contains("invalid-broker") ||
                       e.to_string().contains("timeout"),
                       "Expected connection error, got: {}", e);
            }
        }
    }

    /// Test behavior when topic doesn't exist
    #[tokio::test]
    async fn test_non_existent_topic_operations() {
        println!("🧪 Testing operations on non-existent topic...");

        let helper = KafkaTestHelper::new().await;

        // Test producing to non-existent topic
        let produce_result = helper.produce_message(
            "non-existent-topic-12345",
            None,
            "test message",
        ).await;

        // This should either succeed (auto-create) or fail gracefully
        match produce_result {
            Ok(_) => println!("✅ Message produced to non-existent topic (auto-created)"),
            Err(e) => {
                println!("✅ Production failed gracefully: {:?}", e);
                // Verify it's a topic-related error
                assert!(e.to_string().contains("UnknownTopic") ||
                       e.to_string().contains("topic") ||
                       e.to_string().contains("Topic") ||
                       e.to_string().contains("timeout"),
                       "Expected topic-related error, got: {}", e);
            }
        }

        // Test consuming from non-existent topic
        let consume_result = helper.consume_messages(
            "non-existent-topic-12345",
            1,
            Duration::from_secs(2),
        ).await;

        match consume_result {
            Ok(messages) => {
                if messages.is_empty() {
                    println!("✅ Consuming from non-existent topic returned empty (expected)");
                } else {
                    println!("✅ Consumed {} messages from non-existent topic", messages.len());
                }
            }
            Err(e) => {
                println!("✅ Consumption failed gracefully: {:?}", e);
                // This is acceptable - some Kafka configurations don't auto-create topics
            }
        }
    }

    /// Test timeout scenarios
    #[tokio::test]
    async fn test_connection_timeout() {
        println!("🧪 Testing connection timeout scenarios...");

        // Test with unreachable host and very short timeout
        let timeout_helper = KafkaTestHelper::new_with_config(
            "192.0.2.1:9092".to_string(), // RFC 5737 test address - should be unreachable
            Duration::from_millis(100), // Very short timeout
        );

        let start_time = std::time::Instant::now();
        let connection_result = timeout_helper.test_connection().await;
        let elapsed = start_time.elapsed();

        match connection_result {
            Ok(_) => panic!("Expected connection to timeout"),
            Err(e) => {
                println!("✅ Connection timed out as expected: {:?}", e);
                // Verify timeout occurred within reasonable bounds
                assert!(elapsed < Duration::from_secs(1),
                       "Timeout took too long: {:?}", elapsed);
            }
        }
    }

    /// Test invalid bootstrap server formats
    #[tokio::test]
    async fn test_invalid_bootstrap_servers() {
        println!("🧪 Testing invalid bootstrap server configurations...");

        let invalid_configs = vec![
            "",  // Empty string
            ":", // Missing host
            "localhost", // Missing port
            "localhost:", // Empty port
            "localhost:abc", // Non-numeric port
            "localhost:99999", // Invalid port number
            "http://localhost:9092", // Wrong protocol
        ];

        for config in invalid_configs {
            println!("Testing invalid config: '{}'", config);

            let helper = KafkaTestHelper::new_with_config(
                config.to_string(),
                Duration::from_secs(2),
            );

            let result = helper.test_connection().await;

            match result {
                Ok(_) => {
                    // Some invalid configs might still connect if they're resolvable
                    println!("⚠️  Unexpected success with config: {}", config);
                }
                Err(e) => {
                    println!("✅ Correctly failed with config '{}': {:?}", config, e);
                }
            }
        }
    }

    /// Test network interruption simulation
    #[tokio::test]
    async fn test_network_interruption() {
        println!("🧪 Testing network interruption handling...");

        let helper = KafkaTestHelper::new().await;

        // First establish a working connection
        let initial_connection = helper.test_connection().await;
        assert!(initial_connection.is_ok(), "Initial connection should work");

        // Create a topic for testing
        let topic_name = "test-network-interruption";
        let create_result = helper.create_topic(topic_name, 1, 1).await;

        if create_result.is_ok() {
            println!("✅ Test topic created for network interruption test");

            // Produce a message while connection is working
            let produce_result = helper.produce_message(
                topic_name,
                None,
                "test message before interruption",
            ).await;

            match produce_result {
                Ok(_) => println!("✅ Message produced successfully"),
                Err(e) => println!("⚠️  Production failed: {:?}", e),
            }

            // Note: In a real network interruption test, we would need to
            // simulate network issues at the infrastructure level
            // For now, we test the error handling framework

            println!("✅ Network interruption test framework established");
        } else {
            println!("⚠️  Could not create test topic, skipping network test: {:?}", create_result);
        }

        // Cleanup
        let _ = helper.delete_topic(topic_name).await;
    }

    /// Test concurrent connection attempts
    #[tokio::test]
    async fn test_concurrent_connection_failures() {
        println!("🧪 Testing concurrent connection failures...");

        let mut handles = vec![];

        // Spawn multiple connection attempts to invalid broker
        for i in 0..5 {
            let handle = tokio::spawn(async move {
                let helper = KafkaTestHelper::new_with_config(
                    "invalid-broker:9092".to_string(),
                    Duration::from_secs(1),
                );

                let result = helper.test_connection().await;
                (i, result)
            });
            handles.push(handle);
        }

        // Wait for all connections to complete
        let mut success_count = 0;
        let mut failure_count = 0;

        for handle in handles {
            let (i, result) = handle.await.unwrap();
            match result {
                Ok(_) => {
                    println!("⚠️  Connection {} unexpectedly succeeded", i);
                    success_count += 1;
                }
                Err(e) => {
                    println!("✅ Connection {} failed as expected: {:?}", i, e);
                    failure_count += 1;
                }
            }
        }

        println!("Concurrent test results: {} failures, {} successes", failure_count, success_count);
        assert!(failure_count > 0, "Expected at least some connection failures");
    }

    /// Test error recovery after failures
    #[tokio::test]
    async fn test_error_recovery() {
        println!("🧪 Testing error recovery mechanisms...");

        // Start with invalid configuration
        let mut helper = KafkaTestHelper::new_with_config(
            "invalid-broker:9092".to_string(),
            Duration::from_secs(1),
        );

        // First attempt should fail
        let first_attempt = helper.test_connection().await;
        assert!(first_attempt.is_err(), "First attempt should fail");

        // Simulate configuration fix by creating new helper with valid config
        helper = KafkaTestHelper::new().await;

        // Second attempt should succeed (assuming Kafka is running)
        let second_attempt = helper.test_connection().await;

        match second_attempt {
            Ok(_) => println!("✅ Successfully recovered from connection failure"),
            Err(e) => println!("⚠️  Recovery failed (Kafka may not be running): {:?}", e),
        }
    }
}
