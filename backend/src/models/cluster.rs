use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Deserialize, Serialize)]
#[sea_orm(table_name = "clusters")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub name: String,
    pub cluster_type: String, // kafka, kubernetes, aws, azure, etc.
    #[sea_orm(column_type = "Json")]
    pub config: Json,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_connected_at: Option<DateTime<Utc>>,
    pub status: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Kafka cluster models
#[derive(Debug, Serialize, Deserialize)]
pub struct KafkaClusterConfig {
    pub bootstrap_servers: Vec<String>,
    pub sasl_username: Option<String>,
    pub sasl_password: Option<String>,
    pub sasl_mechanism: Option<String>,
    pub security_protocol: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateKafkaClusterRequest {
    pub name: String,
    pub bootstrap_servers: Vec<String>,
    pub sasl_username: Option<String>,
    pub sasl_password: Option<String>,
    pub sasl_mechanism: Option<String>,
    pub security_protocol: String,
}

#[derive(Debug, Serialize)]
pub struct KafkaTopicInfo {
    pub name: String,
    pub partition_count: i32,
    pub replication_factor: i32,
    pub configs: std::collections::HashMap<String, String>,
}

// Kubernetes cluster models
#[derive(Debug, Serialize, Deserialize)]
pub struct KubernetesClusterConfig {
    pub kube_config_path: Option<String>,
    pub kube_context: Option<String>,
    pub api_server_url: Option<String>,
    pub certificate_authority_data: Option<String>,
    pub client_certificate_data: Option<String>,
    pub client_key_data: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateKubernetesClusterRequest {
    pub name: String,
    pub kube_config_path: Option<String>,
    pub kube_context: Option<String>,
    pub api_server_url: Option<String>,
    pub certificate_authority_data: Option<String>,
    pub client_certificate_data: Option<String>,
    pub client_key_data: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)] // Added Serialize for potential use in responses
pub struct UpdateKubernetesClusterRequest {
    pub name: String,
    pub kube_config_path: Option<String>,
    pub kube_context: Option<String>,
    pub api_server_url: Option<String>,
    pub certificate_authority_data: Option<String>,
    pub client_certificate_data: Option<String>,
    pub client_key_data: Option<String>,
    pub token: Option<String>,
}

// Cloud provider models
#[derive(Debug, Serialize, Deserialize)]
pub struct AwsCloudConfig {
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub region: String,
    pub role_arn: Option<String>,
    pub profile: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AzureCloudConfig {
    pub tenant_id: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub subscription_id: String,
    pub use_managed_identity: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateCloudConnectionRequest {
    pub name: String,
    pub cloud_type: String, // aws or azure
    pub config: serde_json::Value,
}

// Cluster status response
#[derive(Debug, Serialize)]
pub struct ClusterStatusResponse {
    pub id: String,
    pub name: String,
    pub cluster_type: String,
    pub status: String,
    pub details: Option<serde_json::Value>,
    pub last_connected_at: Option<DateTime<Utc>>,
}
