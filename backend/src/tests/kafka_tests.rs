#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // Mock implementation for testing
    #[derive(Clone)]
    struct MockClusterRepository;

    #[async_trait::async_trait]
    impl crate::repositories::cluster::ClusterRepository for MockClusterRepository {
        async fn find_by_id(&self, _id: Uuid) -> Result<Option<crate::models::Cluster>, AppError> {
            Ok(None) // Return None to use config-based lookup
        }
    }

    #[tokio::test]
    async fn test_validate_cluster_config_valid() {
        let repo = Arc::new(MockClusterRepository);
        let service = KafkaService::new(repo);

        let config = KafkaClusterConfig {
            name: "test-cluster".to_string(),
            bootstrap_servers: vec!["localhost:9092".to_string()],
            sasl_username: None,
            sasl_password: None,
            sasl_mechanism: None,
            security_protocol: "PLAINTEXT".to_string(),
        };

        let result = service.validate_cluster_config(&config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_cluster_config_invalid_servers() {
        let repo = Arc::new(MockClusterRepository);
        let service = KafkaService::new(repo);

        let config = KafkaClusterConfig {
            name: "test-cluster".to_string(),
            bootstrap_servers: vec!["invalid-server".to_string()], // Missing port
            sasl_username: None,
            sasl_password: None,
            sasl_mechanism: None,
            security_protocol: "PLAINTEXT".to_string(),
        };

        let result = service.validate_cluster_config(&config);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_cluster_config_invalid_protocol() {
        let repo = Arc::new(MockClusterRepository);
        let service = KafkaService::new(repo);

        let config = KafkaClusterConfig {
            name: "test-cluster".to_string(),
            bootstrap_servers: vec!["localhost:9092".to_string()],
            sasl_username: None,
            sasl_password: None,
            sasl_mechanism: None,
            security_protocol: "INVALID".to_string(),
        };

        let result = service.validate_cluster_config(&config);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_metrics_initialization() {
        let repo = Arc::new(MockClusterRepository);
        let service = KafkaService::new(repo);

        let metrics = service.get_metrics().unwrap();
        assert_eq!(metrics.messages_produced, 0);
        assert_eq!(metrics.messages_consumed, 0);
        assert_eq!(metrics.errors_count, 0);
        assert_eq!(metrics.active_connections, 0);
    }
}
