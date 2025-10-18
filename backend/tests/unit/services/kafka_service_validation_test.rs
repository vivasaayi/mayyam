use std::sync::Arc;

use mayyam::models::cluster::KafkaClusterConfig;
use mayyam::errors::AppError;
use mayyam::repositories::cluster::ClusterRepository;
use mayyam::services::kafka::KafkaService;
use sea_orm::Database;

async fn build_service() -> KafkaService {
    let connection = Database::connect("sqlite::memory:?cache=shared")
        .await
        .expect("sqlite memory db");
    let repo = ClusterRepository::new(Arc::new(connection), Config::default());
    KafkaService::new(Arc::new(repo))
}

fn valid_cluster_config() -> KafkaClusterConfig {
    KafkaClusterConfig {
        bootstrap_servers: vec!["localhost:9092".to_string()],
        sasl_username: None,
        sasl_password: None,
        sasl_mechanism: None,
        security_protocol: "PLAINTEXT".to_string(),
    }
}

#[tokio::test]
async fn validate_cluster_config_accepts_valid_inputs() {
    let service = build_service().await;
    let config = valid_cluster_config();

    let result = service.validate_cluster_config(&config);

    assert!(result.is_ok());
}

#[tokio::test]
async fn bootstrap_servers_require_port() {
    let service = build_service().await;
    let mut config = valid_cluster_config();
    config.bootstrap_servers = vec!["localhost".to_string()];

    let err = service
        .validate_cluster_config(&config)
        .expect_err("validation should fail");

    match err {
        AppError::Validation(message) => {
            assert!(message.contains("host:port"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn sasl_protocol_requires_credentials() {
    let service = build_service().await;
    let mut config = valid_cluster_config();
    config.security_protocol = "SASL_SSL".to_string();

    let err = service
        .validate_cluster_config(&config)
        .expect_err("validation should fail");

    assert!(matches!(err, AppError::Validation(_)));

    // Populate credentials but leave mechanism invalid to exercise mechanism validation.
    config.sasl_username = Some("user".to_string());
    config.sasl_password = Some("pass".to_string());
    config.sasl_mechanism = Some("INVALID".to_string());

    let err = service
        .validate_cluster_config(&config)
        .expect_err("invalid mechanism should be rejected");
    match err {
        AppError::Validation(message) => {
            assert!(message.contains("Invalid SASL mechanism"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
