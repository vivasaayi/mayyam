use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::errors::AppError;
use crate::models::cluster::KafkaClusterConfig;
use crate::repositories::cluster::ClusterRepository;

#[derive(Debug, Serialize, Deserialize)]
pub struct KafkaCluster {
    pub id: String,
    pub name: String,
    pub bootstrap_servers: Vec<String>,
    pub sasl_username: Option<String>,
    pub sasl_password: Option<String>,
    pub sasl_mechanism: Option<String>,
    pub security_protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaTopic {
    pub name: String,
    pub partitions: i32,
    pub replication_factor: i16,
    pub configs: Option<Vec<(String, String)>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaMessage {
    pub key: Option<String>,
    pub value: String,
    pub headers: Option<Vec<(String, String)>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumeOptions {
    pub group_id: String,
    pub max_messages: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub from_beginning: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerGroup {
    pub group_id: String,
    pub is_simple: bool,
    pub state: String,
    pub members: Vec<ConsumerGroupMember>,
    pub offsets: Vec<ConsumerGroupOffset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerGroupMember {
    pub id: String,
    pub client_id: String,
    pub client_host: String,
    pub assignments: Vec<ConsumerGroupAssignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerGroupAssignment {
    pub topic: String,
    pub partition: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerGroupOffset {
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
    pub lag: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffsetReset {
    pub partitions: Vec<PartitionOffset>,
    pub to_earliest: Option<bool>,
    pub to_latest: Option<bool>,
    pub to_offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionOffset {
    pub partition: i32,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageBackupRequest {
    pub topic: String,
    pub partitions: Option<Vec<i32>>, // None means all partitions
    pub start_offset: Option<i64>,    // None means earliest
    pub end_offset: Option<i64>,      // None means latest
    pub max_messages: Option<u64>,    // Limit number of messages
    pub include_headers: Option<bool>, // Default true
    pub include_timestamps: Option<bool>, // Default true
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageBackupResponse {
    pub topic: String,
    pub partitions_backed_up: Vec<i32>,
    pub total_messages: u64,
    pub start_time: String,
    pub end_time: String,
    pub backup_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageRestoreRequest {
    pub target_topic: String,
    pub backup_id: String,
    pub partitions: Option<Vec<i32>>, // Which partitions to restore
    pub preserve_timestamps: Option<bool>, // Default true
    pub preserve_keys: Option<bool>,       // Default true
    pub preserve_headers: Option<bool>,    // Default true
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageRestoreResponse {
    pub target_topic: String,
    pub messages_restored: u64,
    pub partitions_restored: Vec<i32>,
    pub start_time: String,
    pub end_time: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageMigrationRequest {
    pub source_topic: String,
    pub target_topic: String,
    pub source_cluster_id: String,
    pub target_cluster_id: String,
    pub partitions: Option<Vec<i32>>,     // None means all partitions
    pub start_offset: Option<i64>,        // None means earliest
    pub end_offset: Option<i64>,          // None means latest
    pub preserve_partitioning: Option<bool>, // Keep same partition assignment
    pub transform_messages: Option<MessageTransformation>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageTransformation {
    pub key_prefix: Option<String>,
    pub header_additions: Option<Vec<(String, String)>>,
    pub value_transformation: Option<String>, // Could be a script or template
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageMigrationResponse {
    pub source_topic: String,
    pub target_topic: String,
    pub messages_migrated: u64,
    pub partitions_migrated: Vec<i32>,
    pub start_time: String,
    pub end_time: String,
    pub status: MigrationStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MigrationStatus {
    Completed,
    InProgress,
    Failed(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueDrainRequest {
    pub topics: Vec<String>,
    pub consumer_group: String,
    pub timeout_seconds: Option<u64>,     // How long to wait
    pub check_interval_ms: Option<u64>,   // How often to check lag
    pub max_lag_threshold: Option<i64>,   // Acceptable lag before considering drained
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueDrainResponse {
    pub topics: Vec<String>,
    pub consumer_group: String,
    pub total_lag_start: i64,
    pub total_lag_end: i64,
    pub drain_duration_seconds: u64,
    pub is_drained: bool,
    pub partitions_status: Vec<PartitionDrainStatus>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PartitionDrainStatus {
    pub topic: String,
    pub partition: i32,
    pub lag_start: i64,
    pub lag_end: i64,
    pub is_drained: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicPartitionInfo {
    pub id: i32,
    pub leader: i32,
    pub replicas: Vec<i32>,
    pub isr: Vec<i32>,
    pub offsets: PartitionOffsets,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionOffsets {
    pub earliest: i64,
    pub latest: i64,
}

// Requests for admin operations used by controllers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicConfigUpdateRequest {
    pub configs: Option<Vec<(String, String)>>,
    pub validate_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionAdditionRequest {
    pub count: i32,
    pub validate_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterUpdateRequest {
    pub name: Option<String>,
    pub bootstrap_servers: Option<Vec<String>>,
    pub sasl_username: Option<String>,
    pub sasl_password: Option<String>,
    pub sasl_mechanism: Option<String>,
    pub security_protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaMetrics {
    pub messages_produced: u64,
    pub messages_consumed: u64,
    pub errors_count: u64,
    pub avg_response_time_ms: f64,
    pub last_health_check: i64,
    pub active_connections: u32,
    // Backup and restore metrics
    pub backups_created: u64,
    pub backups_restored: u64,
    pub messages_backed_up: u64,
    pub messages_restored: u64,
    pub total_backup_size_bytes: u64,
    pub total_restore_size_bytes: u64,
    pub backup_errors: u64,
    pub restore_errors: u64,
    pub avg_backup_duration_ms: f64,
    pub avg_restore_duration_ms: f64,
    pub active_backups: u32,
    pub active_restores: u32,
    // Migration metrics
    pub migrations_completed: u64,
    pub messages_migrated: u64,
    pub migration_errors: u64,
    pub avg_migration_duration_ms: f64,
    // Queue drain metrics
    pub drain_operations: u64,
    pub drain_success_rate: f64,
    pub avg_drain_duration_ms: f64,
}

#[derive(Debug)]
pub struct KafkaService {
    pub(crate) cluster_repository: Arc<ClusterRepository>,
    pub(crate) metrics: Arc<Mutex<KafkaMetrics>>,
}

impl KafkaService {
    pub fn new(cluster_repository: Arc<ClusterRepository>) -> Self {
        Self {
            cluster_repository,
            metrics: Arc::new(Mutex::new(KafkaMetrics {
                messages_produced: 0,
                messages_consumed: 0,
                errors_count: 0,
                avg_response_time_ms: 0.0,
                last_health_check: 0,
                active_connections: 0,
                // Backup and restore metrics
                backups_created: 0,
                backups_restored: 0,
                messages_backed_up: 0,
                messages_restored: 0,
                total_backup_size_bytes: 0,
                total_restore_size_bytes: 0,
                backup_errors: 0,
                restore_errors: 0,
                avg_backup_duration_ms: 0.0,
                avg_restore_duration_ms: 0.0,
                active_backups: 0,
                active_restores: 0,
                // Migration metrics
                migrations_completed: 0,
                messages_migrated: 0,
                migration_errors: 0,
                avg_migration_duration_ms: 0.0,
                // Queue drain metrics
                drain_operations: 0,
                drain_success_rate: 0.0,
                avg_drain_duration_ms: 0.0,
            })),
        }
    }

    // Metrics accessors
    pub fn get_metrics(&self) -> Result<KafkaMetrics, AppError> {
        let metrics = self
            .metrics
            .lock()
            .map_err(|e| AppError::Internal(format!("Failed to lock metrics: {}", e)))?;
        Ok(metrics.clone())
    }
}
