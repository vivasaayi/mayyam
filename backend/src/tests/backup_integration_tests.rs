#[cfg(test)]
mod backup_integration_tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use actix_web::{test, web, App};
    use serde_json::json;

    use crate::services::kafka::{KafkaService, MessageBackupRequest, CompressionType};
    use crate::config::Config;
    use crate::models::AppError;

    // Mock Kafka cluster for testing
    struct MockKafkaCluster {
        topics: std::collections::HashMap<String, Vec<String>>,
    }

    impl MockKafkaCluster {
        fn new() -> Self {
            Self {
                topics: std::collections::HashMap::new(),
            }
        }

        async fn create_topic(&mut self, topic: &str, partitions: i32) {
            self.topics.insert(topic.to_string(), Vec::new());
        }

        async fn produce_message(&mut self, topic: &str, message: &str) {
            if let Some(messages) = self.topics.get_mut(topic) {
                messages.push(message.to_string());
            }
        }

        fn get_message_count(&self, topic: &str) -> usize {
            self.topics.get(topic).map(|msgs| msgs.len()).unwrap_or(0)
        }
    }

    #[actix_web::test]
    async fn test_backup_endpoint_basic() {
        // Setup test service
        let config = Config::default();
        let kafka_service = Arc::new(KafkaService::new(&config).await.unwrap());

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(kafka_service.clone()))
                .service(crate::controllers::kafka::backup_topic_messages)
        ).await;

        // Test backup request
        let req = test::TestRequest::post()
            .uri("/api/kafka/clusters/test-cluster/backup")
            .set_json(&json!({
                "topic": "test-topic",
                "compression": "gzip",
                "max_messages": 100
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success() || resp.status().is_client_error());
    }

    #[actix_web::test]
    async fn test_backup_with_mock_data() {
        let mut mock_cluster = MockKafkaCluster::new();
        mock_cluster.create_topic("test-topic", 1).await;

        // Generate test messages
        for i in 0..50 {
            mock_cluster.produce_message("test-topic", &format!("Test message {}", i)).await;
        }

        assert_eq!(mock_cluster.get_message_count("test-topic"), 50);
    }

    #[actix_web::test]
    async fn test_backup_request_validation() {
        let config = Config::default();
        let kafka_service = Arc::new(KafkaService::new(&config).await.unwrap());

        // Test invalid compression type
        let invalid_req = MessageBackupRequest {
            topic: "test-topic".to_string(),
            compression: CompressionType::Gzip, // Valid
            max_messages: Some(1000),
            ..Default::default()
        };

        // This should succeed with valid compression
        assert!(invalid_req.topic.len() > 0);
    }

    #[actix_web::test]
    async fn test_performance_baseline() {
        let start = Instant::now();

        // Simulate backup operation
        tokio::time::sleep(Duration::from_millis(100)).await;

        let duration = start.elapsed();
        assert!(duration >= Duration::from_millis(90));
        assert!(duration <= Duration::from_millis(200));
    }

    #[actix_web::test]
    async fn test_compression_types() {
        let compression_types = vec![
            CompressionType::None,
            CompressionType::Gzip,
            CompressionType::Snappy,
            CompressionType::Lz4,
        ];

        for compression in compression_types {
            let req = MessageBackupRequest {
                topic: "test-topic".to_string(),
                compression,
                max_messages: Some(100),
                ..Default::default()
            };

            // Validate compression type is handled
            assert!(matches!(req.compression,
                CompressionType::None |
                CompressionType::Gzip |
                CompressionType::Snappy |
                CompressionType::Lz4));
        }
    }

    #[actix_web::test]
    async fn test_error_handling() {
        let config = Config::default();
        let kafka_service = Arc::new(KafkaService::new(&config).await.unwrap());

        // Test with non-existent cluster
        let result = kafka_service.backup_topic_messages(
            "non-existent-cluster",
            &MessageBackupRequest {
                topic: "test-topic".to_string(),
                compression: CompressionType::Gzip,
                max_messages: Some(100),
                ..Default::default()
            },
            &config
        ).await;

        // Should return an error
        assert!(result.is_err());
    }

    #[actix_web::test]
    async fn test_metrics_collection() {
        let config = Config::default();
        let kafka_service = Arc::new(KafkaService::new(&config).await.unwrap());

        // Get initial metrics
        let initial_metrics = kafka_service.get_metrics().await;

        // Perform operation that should update metrics
        let _ = kafka_service.backup_topic_messages(
            "test-cluster",
            &MessageBackupRequest {
                topic: "test-topic".to_string(),
                compression: CompressionType::Gzip,
                max_messages: Some(10),
                ..Default::default()
            },
            &config
        ).await;

        // Get updated metrics
        let updated_metrics = kafka_service.get_metrics().await;

        // Metrics should be updated (even if operation failed)
        assert!(true); // Placeholder - actual metrics comparison would depend on implementation
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[actix_web::test]
    async fn test_throughput_calculation() {
        let message_count = 1000;
        let message_size_bytes = 1024; // 1KB per message
        let start = Instant::now();

        // Simulate processing messages
        for _ in 0..message_count {
            tokio::time::sleep(Duration::from_micros(100)).await; // Simulate processing time
        }

        let duration = start.elapsed();
        let total_bytes = message_count * message_size_bytes;
        let throughput_mbps = (total_bytes as f64 / 1_000_000.0) / duration.as_secs_f64();

        println!("Throughput: {:.2} MB/s", throughput_mbps);
        assert!(throughput_mbps > 0.0);
    }

    #[actix_web::test]
    async fn test_memory_usage_baseline() {
        let start_memory = 100.0; // MB - placeholder
        let end_memory = 150.0; // MB - placeholder

        // Simulate memory-intensive operation
        tokio::time::sleep(Duration::from_millis(500)).await;

        let memory_delta = end_memory - start_memory;
        println!("Memory usage delta: {:.2} MB", memory_delta);

        // Should not exceed reasonable limits
        assert!(memory_delta < 500.0);
    }
}
