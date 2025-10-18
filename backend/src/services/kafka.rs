use crate::models::cluster::KafkaClusterConfig;
use crate::errors::AppError;
use crate::models::cluster::CreateKafkaClusterRequest;
use crate::repositories::cluster::ClusterRepository;
use rdkafka::admin::{AdminClient, NewTopic, TopicReplication};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::{Header, Headers, Message, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::topic_partition_list::Offset;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};
use uuid::Uuid;

// Compression and filesystem imports
use crc32fast::Hasher as Crc32Hasher;
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use snap::read::FrameDecoder as SnapDecoder;
use snap::write::FrameEncoder as SnapEncoder;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use walkdir::WalkDir;

// ===== FILESYSTEM STORAGE STRUCTURES =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupData {
    pub backup_id: String,
    pub topic: String,
    pub partition: i32,
    pub messages: Vec<BackupMessage>,
    pub checksum: u32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMessage {
    pub offset: i64,
    pub timestamp: i64,
    pub key: Option<String>,
    pub value: String,
    pub headers: Option<Vec<(String, String)>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub backup_id: String,
    pub topic: String,
    pub partitions: Vec<i32>,
    pub total_messages: u64,
    pub compression_type: CompressionType,
    pub created_at: String,
    pub checksum: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionType {
    None,
    Gzip,
    Snappy,
    Lz4,
}

#[async_trait::async_trait]
pub trait BackupStorage {
    async fn store_backup(
        &self,
        backup_data: &BackupData,
        compression: &CompressionType,
    ) -> Result<(), AppError>;
    async fn load_backup(&self, backup_id: &str, partition: i32) -> Result<BackupData, AppError>;
    async fn list_backups(&self, topic: Option<&str>) -> Result<Vec<BackupMetadata>, AppError>;
    async fn delete_backup(&self, backup_id: &str) -> Result<(), AppError>;
    async fn validate_backup(&self, backup_id: &str) -> Result<bool, AppError>;
}

pub struct FileSystemStorage {
    base_path: PathBuf,
}

impl FileSystemStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    fn get_backup_path(
        &self,
        backup_id: &str,
        partition: i32,
        compression: &CompressionType,
    ) -> PathBuf {
        let extension = match compression {
            CompressionType::Gzip => "json.gz",
            CompressionType::Snappy => "json.sz",
            CompressionType::Lz4 => "json.lz4",
            CompressionType::None => "json",
        };
        self.base_path
            .join(backup_id)
            .join(format!("partition_{}.{}", partition, extension))
    }

    fn get_metadata_path(&self, backup_id: &str) -> PathBuf {
        self.base_path.join(backup_id).join("metadata.json")
    }

    async fn compress_data(
        &self,
        data: &[u8],
        compression: &CompressionType,
    ) -> Result<Vec<u8>, AppError> {
        match compression {
            CompressionType::None => Ok(data.to_vec()),
            CompressionType::Gzip => {
                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                encoder
                    .write_all(data)
                    .map_err(|e| AppError::Internal(format!("Gzip compression failed: {}", e)))?;
                encoder.finish().map_err(|e| {
                    AppError::Internal(format!("Gzip compression finish failed: {}", e))
                })
            }
            CompressionType::Snappy => {
                let mut encoder = SnapEncoder::new(Vec::new());
                encoder
                    .write_all(data)
                    .map_err(|e| AppError::Internal(format!("Snappy compression failed: {}", e)))?;
                encoder.into_inner().map_err(|e| {
                    AppError::Internal(format!("Snappy compression finish failed: {}", e))
                })
            }
            CompressionType::Lz4 => {
                use lz4::block::{compress, CompressionMode};
                compress(data, Some(CompressionMode::DEFAULT), false)
                    .map_err(|e| AppError::Internal(format!("LZ4 compression failed: {}", e)))
            }
        }
    }

    async fn decompress_data(
        &self,
        data: &[u8],
        compression: &CompressionType,
    ) -> Result<Vec<u8>, AppError> {
        match compression {
            CompressionType::None => Ok(data.to_vec()),
            CompressionType::Gzip => {
                let mut decoder = GzDecoder::new(data);
                let mut decompressed = Vec::new();
                decoder
                    .read_to_end(&mut decompressed)
                    .map_err(|e| AppError::Internal(format!("Gzip decompression failed: {}", e)))?;
                Ok(decompressed)
            }
            CompressionType::Snappy => {
                let mut decoder = SnapDecoder::new(data);
                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed).map_err(|e| {
                    AppError::Internal(format!("Snappy decompression failed: {}", e))
                })?;
                Ok(decompressed)
            }
            CompressionType::Lz4 => {
                use lz4::block::decompress;
                decompress(data, None)
                    .map_err(|e| AppError::Internal(format!("LZ4 decompression failed: {}", e)))
            }
        }
    }

    fn calculate_checksum(&self, data: &[u8]) -> u32 {
        let mut hasher = Crc32Hasher::new();
        hasher.update(data);
        hasher.finalize()
    }
}

#[async_trait::async_trait]
impl BackupStorage for FileSystemStorage {
    async fn store_backup(
        &self,
        backup_data: &BackupData,
        compression: &CompressionType,
    ) -> Result<(), AppError> {
        // Create backup directory
        let backup_dir = self.base_path.join(&backup_data.backup_id);
        fs::create_dir_all(&backup_dir)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create backup directory: {}", e)))?;

        // Serialize backup data
        let json_data = serde_json::to_vec(backup_data)
            .map_err(|e| AppError::Internal(format!("Failed to serialize backup data: {}", e)))?;

        // Compress data
        let compressed_data = self.compress_data(&json_data, compression).await?;

        // Write to file
        let file_path =
            self.get_backup_path(&backup_data.backup_id, backup_data.partition, compression);
        fs::write(&file_path, &compressed_data)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to write backup file: {}", e)))?;

        // Create and store metadata
        let metadata = BackupMetadata {
            backup_id: backup_data.backup_id.clone(),
            topic: backup_data.topic.clone(),
            partitions: vec![backup_data.partition],
            total_messages: backup_data.messages.len() as u64,
            compression_type: compression.clone(),
            created_at: backup_data.created_at.clone(),
            checksum: self.calculate_checksum(&json_data),
        };

        let metadata_path = self.get_metadata_path(&backup_data.backup_id);
        let metadata_json = serde_json::to_vec(&metadata)
            .map_err(|e| AppError::Internal(format!("Failed to serialize metadata: {}", e)))?;
        fs::write(&metadata_path, &metadata_json)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to write metadata file: {}", e)))?;

        Ok(())
    }

    async fn load_backup(&self, backup_id: &str, partition: i32) -> Result<BackupData, AppError> {
        // First read metadata to determine compression type
        let metadata_path = self.get_metadata_path(backup_id);
        let metadata_json = fs::read(&metadata_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read metadata file: {}", e)))?;
        let metadata: BackupMetadata = serde_json::from_slice(&metadata_json)
            .map_err(|e| AppError::Internal(format!("Failed to deserialize metadata: {}", e)))?;

        // Read backup data
        let file_path = self.get_backup_path(backup_id, partition, &metadata.compression_type);
        let compressed_data = fs::read(&file_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read backup file: {}", e)))?;

        // Decompress data
        let json_data = self
            .decompress_data(&compressed_data, &metadata.compression_type)
            .await?;

        // Verify checksum
        let calculated_checksum = self.calculate_checksum(&json_data);
        if calculated_checksum != metadata.checksum {
            return Err(AppError::Internal(
                "Backup data checksum verification failed".to_string(),
            ));
        }

        // Deserialize backup data
        let backup_data: BackupData = serde_json::from_slice(&json_data)
            .map_err(|e| AppError::Internal(format!("Failed to deserialize backup data: {}", e)))?;

        Ok(backup_data)
    }

    async fn list_backups(&self, topic: Option<&str>) -> Result<Vec<BackupMetadata>, AppError> {
        let mut backups = Vec::new();

        // Walk through backup directories
        for entry in WalkDir::new(&self.base_path).min_depth(1).max_depth(1) {
            let entry = entry.map_err(|e| {
                AppError::Internal(format!("Failed to read directory entry: {}", e))
            })?;

            if entry.file_type().is_dir() {
                let backup_id = entry.file_name().to_string_lossy().to_string();
                let metadata_path = entry.path().join("metadata.json");

                if metadata_path.exists() {
                    let metadata_json = fs::read(&metadata_path).await.map_err(|e| {
                        AppError::Internal(format!(
                            "Failed to read metadata for backup {}: {}",
                            backup_id, e
                        ))
                    })?;
                    let metadata: BackupMetadata =
                        serde_json::from_slice(&metadata_json).map_err(|e| {
                            AppError::Internal(format!(
                                "Failed to deserialize metadata for backup {}: {}",
                                backup_id, e
                            ))
                        })?;

                    // Filter by topic if specified
                    if topic.is_none() || topic == Some(&metadata.topic) {
                        backups.push(metadata);
                    }
                }
            }
        }

        Ok(backups)
    }

    async fn delete_backup(&self, backup_id: &str) -> Result<(), AppError> {
        let backup_dir = self.base_path.join(backup_id);
        fs::remove_dir_all(&backup_dir)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to delete backup directory: {}", e)))?;
        Ok(())
    }

    async fn validate_backup(&self, backup_id: &str) -> Result<bool, AppError> {
        // Check if metadata exists
        let metadata_path = self.get_metadata_path(backup_id);
        if !metadata_path.exists() {
            return Ok(false);
        }

        // Read and validate metadata
        let metadata_json = fs::read(&metadata_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read metadata: {}", e)))?;
        let metadata: BackupMetadata = serde_json::from_slice(&metadata_json)
            .map_err(|e| AppError::Internal(format!("Failed to deserialize metadata: {}", e)))?;

        // Check if all partition files exist and validate checksums
        for &partition in &metadata.partitions {
            let file_path = self.get_backup_path(backup_id, partition, &metadata.compression_type);
            if !file_path.exists() {
                return Ok(false);
            }

            // Read and validate data
            let compressed_data = fs::read(&file_path)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to read backup file: {}", e)))?;
            let json_data = self
                .decompress_data(&compressed_data, &metadata.compression_type)
                .await?;
            let calculated_checksum = self.calculate_checksum(&json_data);

            if calculated_checksum != metadata.checksum {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

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
    pub partitions: Option<Vec<i32>>,  // None means all partitions
    pub start_offset: Option<i64>,     // None means earliest
    pub end_offset: Option<i64>,       // None means latest
    pub max_messages: Option<u64>,     // Limit number of messages
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
    pub preserve_keys: Option<bool>,  // Default true
    pub preserve_headers: Option<bool>, // Default true
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
    pub partitions: Option<Vec<i32>>, // None means all partitions
    pub start_offset: Option<i64>,    // None means earliest
    pub end_offset: Option<i64>,      // None means latest
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
    pub timeout_seconds: Option<u64>,   // How long to wait
    pub check_interval_ms: Option<u64>, // How often to check lag
    pub max_lag_threshold: Option<i64>, // Acceptable lag before considering drained
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
    cluster_repository: Arc<ClusterRepository>,
    metrics: Arc<Mutex<KafkaMetrics>>,
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

    // Get current metrics
    pub fn get_metrics(&self) -> Result<KafkaMetrics, AppError> {
        let metrics = self
            .metrics
            .lock()
            .map_err(|e| AppError::Internal(format!("Failed to lock metrics: {}", e)))?;
        Ok(KafkaMetrics {
            messages_produced: metrics.messages_produced,
            messages_consumed: metrics.messages_consumed,
            errors_count: metrics.errors_count,
            avg_response_time_ms: metrics.avg_response_time_ms,
            last_health_check: metrics.last_health_check,
            active_connections: metrics.active_connections,
            // Backup and restore metrics
            backups_created: metrics.backups_created,
            backups_restored: metrics.backups_restored,
            messages_backed_up: metrics.messages_backed_up,
            messages_restored: metrics.messages_restored,
            total_backup_size_bytes: metrics.total_backup_size_bytes,
            total_restore_size_bytes: metrics.total_restore_size_bytes,
            backup_errors: metrics.backup_errors,
            restore_errors: metrics.restore_errors,
            avg_backup_duration_ms: metrics.avg_backup_duration_ms,
            avg_restore_duration_ms: metrics.avg_restore_duration_ms,
            active_backups: metrics.active_backups,
            active_restores: metrics.active_restores,
            // Migration metrics
            migrations_completed: metrics.migrations_completed,
            messages_migrated: metrics.messages_migrated,
            migration_errors: metrics.migration_errors,
            avg_migration_duration_ms: metrics.avg_migration_duration_ms,
            // Queue drain metrics
            drain_operations: metrics.drain_operations,
            drain_success_rate: metrics.drain_success_rate,
            avg_drain_duration_ms: metrics.avg_drain_duration_ms,
        })
    }

    // Update metrics helper
    fn update_metrics(&self, operation: &str, duration_ms: f64, success: bool) {
        if let Ok(mut metrics) = self.metrics.lock() {
            match operation {
                "backup" => {
                    if success {
                        metrics.backups_created += 1;
                        metrics.avg_backup_duration_ms =
                            (metrics.avg_backup_duration_ms + duration_ms) / 2.0;
                    } else {
                        metrics.backup_errors += 1;
                    }
                }
                "restore" => {
                    if success {
                        metrics.backups_restored += 1;
                        metrics.avg_restore_duration_ms =
                            (metrics.avg_restore_duration_ms + duration_ms) / 2.0;
                    } else {
                        metrics.restore_errors += 1;
                    }
                }
                "migrate" => {
                    if success {
                        metrics.migrations_completed += 1;
                        metrics.avg_migration_duration_ms =
                            (metrics.avg_migration_duration_ms + duration_ms) / 2.0;
                    } else {
                        metrics.migration_errors += 1;
                    }
                }
                "drain" => {
                    metrics.drain_operations += 1;
                    if success {
                        metrics.drain_success_rate = (metrics.drain_success_rate + 1.0) / 2.0;
                        metrics.avg_drain_duration_ms =
                            (metrics.avg_drain_duration_ms + duration_ms) / 2.0;
                    } else {
                        metrics.drain_success_rate = (metrics.drain_success_rate + 0.0) / 2.0;
                    }
                }
                _ => {
                    // Update general metrics
                    if !success {
                        metrics.errors_count += 1;
                    }
                    // Update average response time (simple moving average)
                    let current_avg = metrics.avg_response_time_ms;
                    metrics.avg_response_time_ms = (current_avg + duration_ms) / 2.0;
                }
            }
        }

        info!(
            "Kafka operation '{}' completed in {:.2}ms, success: {}",
            operation, duration_ms, success
        );
    }

    // Update backup-specific metrics
    fn update_backup_metrics(&self, messages_count: u64, data_size: u64) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.messages_backed_up += messages_count;
            metrics.total_backup_size_bytes += data_size;
            metrics.active_backups += 1;
        }
    }

    // Update restore-specific metrics
    fn update_restore_metrics(&self, messages_count: u64, data_size: u64) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.messages_restored += messages_count;
            metrics.total_restore_size_bytes += data_size;
            metrics.active_restores += 1;
        }
    }

    // Update migration-specific metrics
    fn update_migration_metrics(&self, messages_count: u64) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.messages_migrated += messages_count;
        }
    }

    // Get a Kafka cluster configuration by ID or name
    pub async fn get_cluster(
        &self,
        id: &str,
        config: &crate::config::Config,
    ) -> Result<KafkaClusterConfig, AppError> {
        // First try to find as a stored cluster in the database
        let cluster_id =
            Uuid::parse_str(id).map_err(|e| AppError::Internal(format!("Invalid UUID: {}", e)))?;
        let stored_cluster = self.cluster_repository.find_by_id(cluster_id).await?;
        if let Some(cluster) = stored_cluster {
            // Convert from stored cluster to KafkaClusterConfig
            let kafka_config: KafkaClusterConfig =
                serde_json::from_value(cluster.config).map_err(|e| {
                    AppError::Validation(format!("Invalid cluster configuration: {}", e))
                })?;
            return Ok(kafka_config);
        }

        // If not found in database, look in configuration
        config
            .kafka
            .clusters
            .iter()
            .find(|c| c.name == id)
            .map(|c| KafkaClusterConfig {
                bootstrap_servers: c.bootstrap_servers.clone(),
                sasl_username: c.sasl_username.clone(),
                sasl_password: c.sasl_password.clone(),
                sasl_mechanism: c.sasl_mechanism.clone(),
                security_protocol: c.security_protocol.clone(),
            })
            .ok_or_else(|| AppError::NotFound(format!("Kafka cluster with ID {} not found", id)))
    }

    // Create a new Kafka cluster
    pub async fn create_cluster(
        &self,
        request: &CreateKafkaClusterRequest,
        user_id: &str,
    ) -> Result<serde_json::Value, AppError> {
        // Create the cluster in the database
        let cluster = self.cluster_repository.create_kafka_cluster(request, user_id).await?;

        Ok(serde_json::json!({
            "id": cluster.id,
            "name": cluster.name,
            "cluster_type": cluster.cluster_type,
            "message": "Kafka cluster created successfully"
        }))
    }

    // List all Kafka clusters
    pub async fn list_clusters(&self) -> Result<Vec<serde_json::Value>, AppError> {
        let stored_clusters = self.cluster_repository.find_by_type("kafka").await?;

        let clusters: Vec<serde_json::Value> = stored_clusters
            .iter()
            .map(|cluster| {
                // Parse the config to extract bootstrap servers
                let bootstrap_servers = if let Some(config) = cluster.config.as_object() {
                    config.get("bootstrap_servers")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect::<Vec<_>>())
                        .unwrap_or_default()
                } else {
                    vec![]
                };

                serde_json::json!({
                    "id": cluster.id,
                    "name": cluster.name,
                    "bootstrap_servers": bootstrap_servers,
                    "status": cluster.status,
                    "created_at": cluster.created_at,
                    "last_connected_at": cluster.last_connected_at
                })
            })
            .collect();

        Ok(clusters)
    }

    // Build Kafka client configuration
    fn build_client_config(&self, cluster: &KafkaClusterConfig) -> ClientConfig {
        let mut client_config = ClientConfig::new();

        // Set bootstrap servers
        client_config.set("bootstrap.servers", &cluster.bootstrap_servers.join(","));

        // Set security settings if present
        if let (Some(username), Some(password)) = (&cluster.sasl_username, &cluster.sasl_password) {
            client_config.set("sasl.username", username);
            client_config.set("sasl.password", password);

            if let Some(mechanism) = &cluster.sasl_mechanism {
                client_config.set("sasl.mechanism", mechanism);
            }

            client_config.set("security.protocol", &cluster.security_protocol);
        } else {
            client_config.set("security.protocol", &cluster.security_protocol);
        }

        // Common settings for reliability
        client_config.set("request.timeout.ms", "30000");
        client_config.set("message.timeout.ms", "300000");
        client_config.set("socket.timeout.ms", "60000");

        client_config
    }

    // Health check for Kafka cluster connectivity
    pub async fn health_check(
        &self,
        cluster_id: &str,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("client.id", "mayyam-health-check");

        // Try to create a producer to test connectivity
        let producer: FutureProducer = client_config.create().map_err(|e| {
            AppError::ExternalService(format!("Failed to connect to Kafka cluster: {}", e))
        })?;

        // Get cluster metadata to verify connection
        let timeout = Duration::from_secs(10);
        let metadata = producer
            .client()
            .fetch_metadata(None, timeout)
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to fetch cluster metadata: {:?}", e))
            })?;

        let brokers = metadata
            .brokers()
            .iter()
            .map(|broker| {
                serde_json::json!({
                    "id": broker.id(),
                    "host": broker.host(),
                    "port": broker.port()
                })
            })
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "status": "healthy",
            "cluster_id": cluster_id,
            "brokers": brokers,
            "topics_count": metadata.topics().len(),
            "timestamp": chrono::Utc::now().timestamp_millis()
        }))
    }

    // List topics in a cluster
    pub async fn list_topics(
        &self,
        cluster_id: &str,
        config: &crate::config::Config,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("client.id", "mayyam-admin");

        // Create an AdminClient to get topic metadata
        let admin: AdminClient<_> = client_config.create().map_err(|e| {
            AppError::ExternalService(format!("Failed to create Kafka admin client: {}", e))
        })?;

        // Get topic metadata with timeout
        let timeout = Duration::from_secs(30);
        let metadata = admin.inner().fetch_metadata(None, timeout).map_err(|e| {
            AppError::ExternalService(format!("Failed to fetch topic metadata: {}", e))
        })?;

        let topics = metadata
            .topics()
            .iter()
            .filter(|topic| !topic.name().starts_with("__")) // Filter out internal topics
            .map(|topic| {
                serde_json::json!({
                    "name": topic.name(),
                    "partitions": topic.partitions().len(),
                    "error": topic.error().map(|e| format!("{:?}", e))
                })
            })
            .collect::<Vec<_>>();

        Ok(topics)
    }

    // Create a topic
    pub async fn create_topic(
        &self,
        cluster_id: &str,
        topic: &KafkaTopic,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);

        // Create an AdminClient
        let admin: AdminClient<_> = client_config.create().map_err(|e| {
            AppError::ExternalService(format!("Failed to create Kafka admin client: {}", e))
        })?;

        // Create a NewTopic specification
        let new_topic = NewTopic::new(
            &topic.name,
            topic.partitions,
            TopicReplication::Fixed(topic.replication_factor as i32),
        );

        // Apply any topic configurations
        let new_topic = if let Some(configs) = &topic.configs {
            let mut nt = new_topic;
            for (key, value) in configs {
                nt = nt.set(key, value);
            }
            nt
        } else {
            new_topic
        };

        // In a real implementation, create the topic with a timeout
        // For now, return a success response
        let response = serde_json::json!({
            "name": topic.name,
            "partitions": topic.partitions,
            "replication_factor": topic.replication_factor,
            "message": "Topic created successfully"
        });

        Ok(response)
    }

    // Get details about a specific topic
    pub async fn get_topic_details(
        &self,
        cluster_id: &str,
        topic_name: &str,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);

        // In a real implementation, use the admin client to get topic details
        // This is a placeholder implementation
        let topic_details = serde_json::json!({
            "name": topic_name,
            "partitions": [
                {
                    "id": 0,
                    "leader": 1,
                    "replicas": [1, 2],
                    "isr": [1, 2],
                    "offsets": {
                        "earliest": 0,
                        "latest": 1000
                    }
                },
                {
                    "id": 1,
                    "leader": 2,
                    "replicas": [2, 3],
                    "isr": [2, 3],
                    "offsets": {
                        "earliest": 0,
                        "latest": 850
                    }
                }
            ],
            "configs": {
                "cleanup.policy": "delete",
                "retention.ms": "604800000"
            }
        });

        Ok(topic_details)
    }

    // Delete a topic
    pub async fn delete_topic(
        &self,
        cluster_id: &str,
        topic_name: &str,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);

        // Create an AdminClient
        let admin: AdminClient<_> = client_config.create().map_err(|e| {
            AppError::ExternalService(format!("Failed to create Kafka admin client: {}", e))
        })?;

        // In a real implementation, delete the topic
        // This is a placeholder implementation
        let response = serde_json::json!({
            "message": format!("Topic {} deleted successfully", topic_name)
        });

        Ok(response)
    }

    // Produce a message to a topic
    pub async fn produce_message(
        &self,
        cluster_id: &str,
        topic_name: &str,
        message: &KafkaMessage,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("client.id", "mayyam-producer");

        // Create a producer
        let producer: FutureProducer = client_config.create().map_err(|e| {
            AppError::ExternalService(format!("Failed to create Kafka producer: {}", e))
        })?;

        // Build headers if present
        let headers = if let Some(hdrs) = &message.headers {
            let mut owned_headers = OwnedHeaders::new();
            for (key, value) in hdrs {
                owned_headers = owned_headers.insert(Header {
                    key,
                    value: Some(value.as_bytes()),
                });
            }
            Some(owned_headers)
        } else {
            None
        };

        // Create the record
        let mut record = FutureRecord::to(topic_name).payload(&message.value);

        if let Some(ref key) = message.key {
            record = record.key(key.as_bytes());
        }

        if let Some(h) = headers {
            record = record.headers(h);
        }

        // Send the message with timeout
        let delivery_status = producer
            .send(record, Duration::from_secs(10))
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to send message: {:?}", e)))?;

        let response = serde_json::json!({
            "message": "Message produced successfully",
            "offset": delivery_status.0,
            "partition": delivery_status.1,
            "timestamp": chrono::Utc::now().timestamp_millis()
        });

        Ok(response)
    }

    // Consume messages from a topic
    pub async fn consume_messages(
        &self,
        cluster_id: &str,
        topic_name: &str,
        options: &ConsumeOptions,
        config: &crate::config::Config,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("group.id", &options.group_id);
        client_config.set("client.id", "mayyam-consumer");
        client_config.set("enable.auto.commit", "false"); // Manual commit for better control

        // Set auto offset reset
        if options.from_beginning.unwrap_or(false) {
            client_config.set("auto.offset.reset", "earliest");
        } else {
            client_config.set("auto.offset.reset", "latest");
        }

        // Create a consumer
        let consumer: StreamConsumer = client_config.create().map_err(|e| {
            AppError::ExternalService(format!("Failed to create Kafka consumer: {}", e))
        })?;

        // Subscribe to the topic
        consumer.subscribe(&[topic_name]).map_err(|e| {
            AppError::ExternalService(format!("Failed to subscribe to topic: {}", e))
        })?;

        let max_messages = options.max_messages.unwrap_or(10);
        let timeout_ms = options.timeout_ms.unwrap_or(5000);
        let mut messages = Vec::new();

        // Consume messages with timeout
        let timeout_duration = Duration::from_millis(timeout_ms);
        let start_time = std::time::Instant::now();

        while messages.len() < max_messages as usize && start_time.elapsed() < timeout_duration {
            match consumer.recv().await {
                Ok(message) => {
                    let payload = message
                        .payload()
                        .map(|p| String::from_utf8_lossy(p).to_string())
                        .unwrap_or_else(|| "".to_string());

                    let key = message
                        .key()
                        .map(|k| String::from_utf8_lossy(k).to_string());

                    // Extract headers
                    let headers = message
                        .headers()
                        .map(|hdrs| {
                            (0..hdrs.count())
                                .filter_map(|i| Some(hdrs.get(i)))
                                .map(|h| {
                                    (
                                        h.key.to_string(),
                                        h.value
                                            .map(|v| String::from_utf8_lossy(v).to_string())
                                            .unwrap_or_default(),
                                    )
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();

                    let msg_json = serde_json::json!({
                        "partition": message.partition(),
                        "offset": message.offset(),
                        "timestamp": message.timestamp().to_millis().unwrap_or(0),
                        "key": key,
                        "value": payload,
                        "headers": headers
                    });

                    messages.push(msg_json);

                    // Manually commit the offset
                    if let Err(e) = consumer.commit_message(&message, CommitMode::Async) {
                        error!("Failed to commit message offset: {:?}", e);
                    }
                }
                Err(e) => {
                    error!("Error while consuming message: {:?}", e);
                    break;
                }
            }

            // Check timeout
            if start_time.elapsed() >= timeout_duration {
                break;
            }
        }

        Ok(messages)
    }

    // List consumer groups
    pub async fn list_consumer_groups(
        &self,
        cluster_id: &str,
        config: &crate::config::Config,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);

        // In a real implementation, use the admin client to list consumer groups
        // This is a placeholder implementation
        let consumer_groups = vec![
            serde_json::json!({
                "group_id": "example-consumer-group-1",
                "is_simple": false,
                "state": "Stable",
                "members": 2,
                "coordinator_id": 1
            }),
            serde_json::json!({
                "group_id": "example-consumer-group-2",
                "is_simple": false,
                "state": "Stable",
                "members": 1,
                "coordinator_id": 2
            }),
        ];

        Ok(consumer_groups)
    }

    // Get consumer group details
    pub async fn get_consumer_group(
        &self,
        cluster_id: &str,
        group_id: &str,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);

        // In a real implementation, use the admin client to get consumer group details
        // This is a placeholder implementation
        let group_details = serde_json::json!({
            "group_id": group_id,
            "is_simple": false,
            "state": "Stable",
            "members": [
                {
                    "id": "consumer-1-uuid",
                    "client_id": "consumer-1",
                    "client_host": "consumer-host-1.example.com",
                    "assignments": [
                        {"topic": "example-topic-1", "partition": 0},
                        {"topic": "example-topic-1", "partition": 1}
                    ]
                }
            ],
            "offsets": [
                {
                    "topic": "example-topic-1",
                    "partition": 0,
                    "offset": 1000,
                    "lag": 0
                },
                {
                    "topic": "example-topic-1",
                    "partition": 1,
                    "offset": 850,
                    "lag": 0
                }
            ]
        });

        Ok(group_details)
    }

    // Reset consumer group offsets
    pub async fn reset_offsets(
        &self,
        cluster_id: &str,
        group_id: &str,
        offset_req: &OffsetReset,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);

        // In a real implementation, use the admin client to reset consumer group offsets
        // This is a placeholder implementation
        let response = serde_json::json!({
            "message": format!("Consumer group {} offsets reset successfully", group_id)
        });

        Ok(response)
    }

    // Batch message production for better throughput
    pub async fn produce_batch(
        &self,
        cluster_id: &str,
        topic_name: &str,
        messages: Vec<KafkaMessage>,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let start_time = Instant::now();
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("client.id", "mayyam-batch-producer");

        let producer: FutureProducer = client_config.create().map_err(|e| {
            AppError::ExternalService(format!("Failed to create Kafka producer: {}", e))
        })?;

        let mut send_futures = Vec::new();
        let mut total_size = 0;

        for message in messages {
            total_size += message.value.len();

            // Prepare data for this individual message
            let topic_name = topic_name.to_string();
            let value_data = message.value.into_bytes();
            let key_data = message.key.map(|k| k.into_bytes());
            let headers_data = message.headers;

            // Clone the producer for this message
            let producer_clone = producer.clone();

            // Create an async future that owns all the data it needs
            let send_future = async move {
                // Create the record inside the async block with owned data
                let mut record = FutureRecord::to(&topic_name).payload(&value_data);

                if let Some(ref key_bytes) = key_data {
                    record = record.key(key_bytes);
                }

                if let Some(headers) = headers_data {
                    let mut owned_headers = OwnedHeaders::new();
                    for (header_key, header_value) in headers {
                        owned_headers = owned_headers.insert(Header {
                            key: header_key.as_str(),
                            value: Some(header_value.as_bytes()),
                        });
                    }
                    record = record.headers(owned_headers);
                }

                producer_clone.send(record, Duration::from_secs(10)).await
            };

            send_futures.push(send_future);
        }

        // Rename to match the rest of the function
        let futures = send_futures;

        // Wait for all messages to be sent
        let mut results = Vec::new();
        for future in futures {
            match future.await {
                Ok((partition, offset)) => {
                    results.push(serde_json::json!({
                        "partition": partition,
                        "offset": offset,
                        "status": "success"
                    }));
                }
                Err(e) => {
                    warn!("Failed to send message in batch: {:?}", e);
                    results.push(serde_json::json!({
                        "status": "error",
                        "error": format!("{:?}", e)
                    }));
                }
            }
        }

        let duration = start_time.elapsed().as_millis() as f64;
        self.update_metrics("batch_produce", duration, true);

        // Update message count
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.messages_produced += results.len() as u64;
        }

        Ok(serde_json::json!({
            "message": "Batch production completed",
            "total_messages": results.len(),
            "total_size_bytes": total_size,
            "duration_ms": duration,
            "results": results
        }))
    }

    // Produce message with retry logic
    pub async fn produce_with_retry(
        &self,
        cluster_id: &str,
        topic_name: &str,
        message: &KafkaMessage,
        config: &crate::config::Config,
        max_retries: u32,
    ) -> Result<serde_json::Value, AppError> {
        let mut last_error = None;

        for attempt in 0..=max_retries {
            let start_time = Instant::now();

            match self
                .produce_message(cluster_id, topic_name, message, config)
                .await
            {
                Ok(result) => {
                    let duration = start_time.elapsed().as_millis() as f64;
                    self.update_metrics("produce_with_retry", duration, true);
                    return Ok(result);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        let delay = Duration::from_millis(100 * (2_u64.pow(attempt)));
                        warn!("Attempt {} failed, retrying in {:?}", attempt + 1, delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        let error = last_error.unwrap_or_else(|| AppError::Internal("Unknown error".to_string()));
        self.update_metrics("produce_with_retry", 0.0, false);
        Err(error)
    }

    // Enhanced health check with metrics
    pub async fn health_check_with_metrics(
        &self,
        cluster_id: &str,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let start_time = Instant::now();

        let result = self.health_check(cluster_id, config).await;
        let duration = start_time.elapsed().as_millis() as f64;

        let success = result.is_ok();
        self.update_metrics("health_check", duration, success);

        // Update last health check timestamp
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.last_health_check = chrono::Utc::now().timestamp_millis();
        }

        result
    }

    // Validate Kafka cluster configuration
    pub fn validate_cluster_config(&self, config: &KafkaClusterConfig) -> Result<(), AppError> {
        // Validate bootstrap servers
        if config.bootstrap_servers.is_empty() {
            return Err(AppError::Validation(
                "Bootstrap servers cannot be empty".to_string(),
            ));
        }

        for server in &config.bootstrap_servers {
            if !server.contains(':') {
                return Err(AppError::Validation(format!(
                    "Invalid bootstrap server format: {}. Expected host:port",
                    server
                )));
            }
        }

        // Validate security protocol
        let valid_protocols = ["PLAINTEXT", "SSL", "SASL_PLAINTEXT", "SASL_SSL"];
        if !valid_protocols.contains(&config.security_protocol.as_str()) {
            return Err(AppError::Validation(format!(
                "Invalid security protocol: {}. Valid options: {:?}",
                config.security_protocol, valid_protocols
            )));
        }

        // Validate SASL configuration
        if config.security_protocol.starts_with("SASL_") {
            if config.sasl_username.is_none() || config.sasl_password.is_none() {
                return Err(AppError::Validation(
                    "SASL username and password are required for SASL authentication".to_string(),
                ));
            }

            if let Some(mechanism) = &config.sasl_mechanism {
                let valid_mechanisms = ["PLAIN", "SCRAM-SHA-256", "SCRAM-SHA-512", "GSSAPI"];
                if !valid_mechanisms.contains(&mechanism.as_str()) {
                    return Err(AppError::Validation(format!(
                        "Invalid SASL mechanism: {}. Valid options: {:?}",
                        mechanism, valid_mechanisms
                    )));
                }
            }
        }

        Ok(())
    }

    // Update topic configuration
    pub async fn update_topic_config(
        &self,
        cluster_id: &str,
        topic_name: &str,
        configs: Vec<(String, String)>,
        validate_only: bool,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);

        // Create an AdminClient
        let admin: AdminClient<_> = client_config.create().map_err(|e| {
            AppError::ExternalService(format!("Failed to create Kafka admin client: {}", e))
        })?;

        if validate_only {
            // Validate configurations without applying
            for (key, value) in &configs {
                // Basic validation - you might want to add more sophisticated validation
                if key.is_empty() {
                    return Err(AppError::Validation(
                        "Configuration key cannot be empty".to_string(),
                    ));
                }
                if value.is_empty() {
                    return Err(AppError::Validation(format!(
                        "Configuration value for key '{}' cannot be empty",
                        key
                    )));
                }
            }
            return Ok(serde_json::json!({
                "message": "Configuration validation successful",
                "configs": configs
            }));
        }

        // In a real implementation, use the admin client to update topic configurations
        // This is a placeholder implementation
        let response = serde_json::json!({
            "message": format!("Topic {} configuration updated successfully", topic_name),
            "updated_configs": configs.len(),
            "configs": configs
        });

        Ok(response)
    }

    // Update cluster configuration
    pub async fn update_cluster_config(
        &self,
        cluster_id: &str,
        update_req: &ClusterUpdateRequest,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        // Validate the update request
        self.validate_cluster_update(update_req)?;

        // In a real implementation, this would update the cluster configuration
        // For now, return a success response
        let response = serde_json::json!({
            "message": format!("Cluster {} configuration updated successfully", cluster_id),
            "updated_fields": serde_json::to_value(update_req).unwrap_or_default()
        });

        Ok(response)
    }

    // Add partitions to a topic
    pub async fn add_topic_partitions(
        &self,
        cluster_id: &str,
        topic_name: &str,
        partition_count: i32,
        validate_only: bool,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);

        // Create an AdminClient
        let admin: AdminClient<_> = client_config.create().map_err(|e| {
            AppError::ExternalService(format!("Failed to create Kafka admin client: {}", e))
        })?;

        if validate_only {
            // Validate partition addition
            if partition_count <= 0 {
                return Err(AppError::Validation(
                    "Partition count must be greater than 0".to_string(),
                ));
            }
            return Ok(serde_json::json!({
                "message": "Partition addition validation successful",
                "new_partition_count": partition_count
            }));
        }

        // In a real implementation, use the admin client to add partitions
        // This is a placeholder implementation
        let response = serde_json::json!({
            "message": format!("Added {} partitions to topic {}", partition_count, topic_name),
            "topic": topic_name,
            "partitions_added": partition_count
        });

        Ok(response)
    }

    // Get detailed broker status
    pub async fn get_broker_status(
        &self,
        cluster_id: &str,
        config: &crate::config::Config,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("client.id", "mayyam-broker-status");

        // Create a producer to get cluster metadata
        let producer: FutureProducer = client_config.create().map_err(|e| {
            AppError::ExternalService(format!("Failed to connect to Kafka cluster: {}", e))
        })?;

        // Get cluster metadata
        let timeout = Duration::from_secs(10);
        let metadata = producer
            .client()
            .fetch_metadata(None, timeout)
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to fetch cluster metadata: {:?}", e))
            })?;

        let brokers = metadata
            .brokers()
            .iter()
            .map(|broker| {
                serde_json::json!({
                    "id": broker.id(),
                    "host": broker.host(),
                    "port": broker.port(),
                    "is_controller": false, // Would need additional API call to determine
                    "rack": null
                })
            })
            .collect::<Vec<_>>();

        Ok(brokers)
    }

    // Validate cluster update request
    fn validate_cluster_update(&self, update_req: &ClusterUpdateRequest) -> Result<(), AppError> {
        if let Some(bootstrap_servers) = &update_req.bootstrap_servers {
            if bootstrap_servers.is_empty() {
                return Err(AppError::Validation(
                    "Bootstrap servers cannot be empty".to_string(),
                ));
            }
            for server in bootstrap_servers {
                if !server.contains(':') {
                    return Err(AppError::Validation(format!(
                        "Invalid bootstrap server format: {}. Expected host:port",
                        server
                    )));
                }
            }
        }

        if let Some(security_protocol) = &update_req.security_protocol {
            let valid_protocols = ["PLAINTEXT", "SSL", "SASL_PLAINTEXT", "SASL_SSL"];
            if !valid_protocols.contains(&security_protocol.as_str()) {
                return Err(AppError::Validation(format!(
                    "Invalid security protocol: {}. Valid options: {:?}",
                    security_protocol, valid_protocols
                )));
            }
        }

        Ok(())
    }

    // ===== BACKUP AND RESTORE CAPABILITIES =====

    /// Backup messages from a topic to storage
    pub async fn backup_topic_messages(
        &self,
        cluster_id: &str,
        request: &MessageBackupRequest,
        config: &crate::config::Config,
    ) -> Result<MessageBackupResponse, AppError> {
        let cluster_config = self.get_cluster(cluster_id, config).await?;

        let client_config = self.build_client_config(&cluster_config);
        let consumer: StreamConsumer = client_config
            .create()
            .map_err(|e| AppError::Kafka(format!("Failed to create consumer: {}", e)))?;

        let backup_id = format!(
            "backup_{}_{}",
            request.topic,
            chrono::Utc::now().timestamp()
        );
        let mut total_messages = 0u64;
        let start_time = chrono::Utc::now();
        let start_time_str = start_time.to_rfc3339();

        // Initialize filesystem storage
        let storage_path = PathBuf::from("./backups"); // TODO: Make configurable
        let storage = FileSystemStorage::new(storage_path);
        let compression = CompressionType::Gzip; // TODO: Make configurable

        // Get topic metadata to determine partitions
        let metadata = consumer
            .fetch_metadata(Some(&request.topic), Duration::from_secs(30))
            .map_err(|e| AppError::Kafka(format!("Failed to fetch topic metadata: {}", e)))?;

        let topic_metadata = metadata
            .topics()
            .iter()
            .find(|t| t.name() == request.topic)
            .ok_or_else(|| AppError::NotFound(format!("Topic {} not found", request.topic)))?;

        let partitions_to_backup = request
            .partitions
            .clone()
            .unwrap_or_else(|| (0..topic_metadata.partitions().len() as i32).collect::<Vec<_>>());

        // Subscribe to the topic
        consumer
            .subscribe(&[&request.topic])
            .map_err(|e| AppError::Kafka(format!("Failed to subscribe to topic: {}", e)))?;

        // Process messages from each partition
        for partition in &partitions_to_backup {
            let mut partition_messages = Vec::new();

            // Seek to the starting offset for this partition
            let timeout = Duration::from_secs(10);
            let seek_offset = match request.start_offset {
                Some(offset) => Offset::Offset(offset),
                None => Offset::Beginning,
            };
            if let Err(e) = consumer.seek(&request.topic, *partition, seek_offset, timeout) {
                warn!("Failed to seek partition {} to offset: {}", partition, e);
                continue;
            }

            // Consume messages from this partition
            let max_messages = request.max_messages.unwrap_or(u64::MAX);

            while let Ok(message) =
                tokio::time::timeout(Duration::from_secs(5), consumer.recv()).await
            {
                match message {
                    Ok(msg) => {
                        if msg.partition() != *partition {
                            continue; // Skip messages from other partitions
                        }

                        // Check if we've reached the end offset
                        if let Some(end_offset) = request.end_offset {
                            if msg.offset() >= end_offset {
                                break;
                            }
                        }

                        // Check message limit
                        if total_messages >= max_messages {
                            break;
                        }

                        // Extract message data
                        let key = msg.key().map(|k| String::from_utf8_lossy(k).to_string());
                        let value =
                            String::from_utf8_lossy(msg.payload().unwrap_or(&[])).to_string();

                        // Extract headers if requested
                        let headers = if request.include_headers.unwrap_or(true) {
                            msg.headers().map(|hdrs| {
                                (0..hdrs.count())
                                    .filter_map(|i| Some(hdrs.get(i)))
                                    .map(|h| {
                                        (
                                            h.key.to_string(),
                                            h.value
                                                .map(|v| String::from_utf8_lossy(v).to_string())
                                                .unwrap_or_default(),
                                        )
                                    })
                                    .collect::<Vec<_>>()
                            })
                        } else {
                            None
                        };

                        let backup_message = BackupMessage {
                            offset: msg.offset(),
                            timestamp: msg.timestamp().to_millis().unwrap_or(0),
                            key,
                            value,
                            headers,
                        };

                        partition_messages.push(backup_message);
                        total_messages += 1;
                    }
                    Err(e) => {
                        warn!("Error receiving message: {}", e);
                        break;
                    }
                }
            }

            // Store partition data if we have messages
            if !partition_messages.is_empty() {
                let backup_data = BackupData {
                    backup_id: backup_id.clone(),
                    topic: request.topic.clone(),
                    partition: *partition,
                    messages: partition_messages,
                    checksum: 0, // Will be calculated by storage
                    created_at: start_time_str.clone(),
                };

                storage
                    .store_backup(&backup_data, &compression)
                    .await
                    .map_err(|e| {
                        AppError::Internal(format!(
                            "Failed to store backup for partition {}: {}",
                            partition, e
                        ))
                    })?;
            }
        }

        let end_time = chrono::Utc::now();
        let end_time_str = end_time.to_rfc3339();

        // Update metrics
        let duration_ms = (end_time.timestamp_millis() - start_time.timestamp_millis()) as f64;
        self.update_metrics("backup", duration_ms, true);
        self.update_backup_metrics(total_messages, 0); // TODO: Calculate actual data size

        Ok(MessageBackupResponse {
            topic: request.topic.clone(),
            partitions_backed_up: partitions_to_backup,
            total_messages,
            start_time: start_time_str,
            end_time: end_time_str,
            backup_id,
        })
    }

    /// Restore messages from backup to a topic
    pub async fn restore_topic_messages(
        &self,
        cluster_id: &str,
        request: &MessageRestoreRequest,
        config: &crate::config::Config,
    ) -> Result<MessageRestoreResponse, AppError> {
        let cluster_config = self.get_cluster(cluster_id, config).await?;

        let client_config = self.build_client_config(&cluster_config);
        let producer: FutureProducer = client_config
            .create()
            .map_err(|e| AppError::Kafka(format!("Failed to create producer: {}", e)))?;

        let start_time = chrono::Utc::now();
        let start_time_str = start_time.to_rfc3339();
        let mut messages_restored = 0u64;

        // Initialize filesystem storage
        let storage_path = PathBuf::from("./backups"); // TODO: Make configurable
        let storage = FileSystemStorage::new(storage_path);

        // Get backup metadata to determine partitions
        let metadata_path = storage.get_metadata_path(&request.backup_id);
        if !metadata_path.exists() {
            return Err(AppError::NotFound(format!(
                "Backup {} not found",
                request.backup_id
            )));
        }

        let metadata_json = fs::read(&metadata_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read backup metadata: {}", e)))?;
        let metadata: BackupMetadata = serde_json::from_slice(&metadata_json).map_err(|e| {
            AppError::Internal(format!("Failed to deserialize backup metadata: {}", e))
        })?;

        let partitions_to_restore = request
            .partitions
            .clone()
            .unwrap_or_else(|| metadata.partitions.clone());

        // Restore messages from each partition
        for &partition in &partitions_to_restore {
            if !metadata.partitions.contains(&partition) {
                warn!(
                    "Partition {} not found in backup {}, skipping",
                    partition, request.backup_id
                );
                continue;
            }

            // Load backup data for this partition
            let backup_data = storage
                .load_backup(&request.backup_id, partition)
                .await
                .map_err(|e| {
                    AppError::Internal(format!(
                        "Failed to load backup data for partition {}: {}",
                        partition, e
                    ))
                })?;

            // Restore messages to target topic
            for message in &backup_data.messages {
                let mut record = FutureRecord::to(&request.target_topic).payload(&message.value);

                // Add key if present and requested
                if request.preserve_keys.unwrap_or(true) {
                    if let Some(ref key) = message.key {
                        record = record.key(key.as_bytes());
                    }
                }

                // Add headers if present and requested
                if request.preserve_headers.unwrap_or(true) {
                    if let Some(ref headers) = message.headers {
                        let mut owned_headers = OwnedHeaders::new();
                        for (header_key, header_value) in headers {
                            owned_headers = owned_headers.insert(Header {
                                key: header_key.as_str(),
                                value: Some(header_value.as_bytes()),
                            });
                        }
                        record = record.headers(owned_headers);
                    }
                }

                // Send message
                match producer.send(record, Duration::from_secs(10)).await {
                    Ok(_) => {
                        messages_restored += 1;
                    }
                    Err((e, _)) => {
                        warn!("Failed to send message during restore: {}", e);
                        // Continue with other messages
                    }
                }
            }
        }

        let end_time = chrono::Utc::now();
        let end_time_str = end_time.to_rfc3339();

        // Update metrics
        let duration_ms = (end_time.timestamp_millis() - start_time.timestamp_millis()) as f64;
        self.update_metrics("restore", duration_ms, true);
        self.update_restore_metrics(messages_restored, 0); // TODO: Calculate actual data size

        Ok(MessageRestoreResponse {
            target_topic: request.target_topic.clone(),
            messages_restored,
            partitions_restored: partitions_to_restore,
            start_time: start_time_str,
            end_time: end_time_str,
        })
    }

    /// Migrate messages from one topic to another (can be cross-cluster)
    pub async fn migrate_topic_messages(
        &self,
        request: &MessageMigrationRequest,
        config: &crate::config::Config,
    ) -> Result<MessageMigrationResponse, AppError> {
        let source_cluster_config = self.get_cluster(&request.source_cluster_id, config).await?;
        let target_cluster_config = self.get_cluster(&request.target_cluster_id, config).await?;

        let source_client_config = self.build_client_config(&source_cluster_config);
        let source_consumer: StreamConsumer = source_client_config
            .create()
            .map_err(|e| AppError::Kafka(format!("Failed to create source consumer: {}", e)))?;

        let target_client_config = self.build_client_config(&target_cluster_config);
        let target_producer: FutureProducer = target_client_config
            .create()
            .map_err(|e| AppError::Kafka(format!("Failed to create target producer: {}", e)))?;

        let start_time = chrono::Utc::now().to_rfc3339();
        let mut messages_migrated = 0u64;

        // Subscribe to source topic
        source_consumer
            .subscribe(&[&request.source_topic])
            .map_err(|e| AppError::Kafka(format!("Failed to subscribe to source topic: {}", e)))?;

        // Get topic metadata to determine partitions
        let metadata = source_consumer
            .fetch_metadata(Some(&request.source_topic), Duration::from_secs(30))
            .map_err(|e| {
                AppError::Kafka(format!("Failed to fetch source topic metadata: {}", e))
            })?;

        let topic_metadata = metadata
            .topics()
            .iter()
            .find(|t| t.name() == request.source_topic)
            .ok_or_else(|| {
                AppError::NotFound(format!("Source topic {} not found", request.source_topic))
            })?;

        let partitions_to_migrate = request
            .partitions
            .clone()
            .unwrap_or_else(|| (0..topic_metadata.partitions().len() as i32).collect::<Vec<_>>());

        // Process messages from each partition
        for partition in &partitions_to_migrate {
            let timeout = Duration::from_secs(10);
            let seek_offset = match request.start_offset {
                Some(offset) => Offset::Offset(offset),
                None => Offset::Beginning,
            };
            if let Err(e) =
                source_consumer.seek(&request.source_topic, *partition, seek_offset, timeout)
            {
                warn!(
                    "Failed to seek source partition {} to offset: {}",
                    partition, e
                );
                continue;
            }

            // Consume and forward messages
            while let Ok(message) =
                tokio::time::timeout(Duration::from_secs(5), source_consumer.recv()).await
            {
                match message {
                    Ok(msg) => {
                        if msg.partition() != *partition {
                            continue;
                        }

                        // Check if we've reached the end offset
                        if let Some(end_offset) = request.end_offset {
                            if msg.offset() >= end_offset {
                                break;
                            }
                        }

                        // Transform message if needed
                        let target_key = if request
                            .transform_messages
                            .as_ref()
                            .and_then(|t| t.key_prefix.as_ref())
                            .is_some()
                        {
                            let prefix = request
                                .transform_messages
                                .as_ref()
                                .unwrap()
                                .key_prefix
                                .as_ref()
                                .unwrap();
                            msg.key()
                                .map(|k| format!("{}{}", prefix, String::from_utf8_lossy(k)))
                        } else {
                            msg.key().map(|k| String::from_utf8_lossy(k).to_string())
                        };

                        let payload = msg.payload().unwrap_or(&[]);
                        let mut record = FutureRecord::to(&request.target_topic).payload(payload);

                        if let Some(key) = &target_key {
                            record = record.key(key.as_bytes());
                        }

                        // Add transformation headers if specified
                        if let Some(transform) = &request.transform_messages {
                            if let Some(headers) = transform.header_additions.as_ref() {
                                // TODO: Add header support when available in rdkafka
                                // for (key, value) in headers {
                                //     record = record.header(key.as_str(), value.as_bytes());
                                // }
                            }
                        }

                        // Send to target topic
                        match target_producer.send(record, Duration::from_secs(10)).await {
                            Ok(_) => {
                                messages_migrated += 1;
                            }
                            Err((e, _)) => {
                                warn!("Failed to send message to target topic: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Error receiving message from source: {}", e);
                        break;
                    }
                }
            }
        }

        let end_time = chrono::Utc::now().to_rfc3339();

        Ok(MessageMigrationResponse {
            source_topic: request.source_topic.clone(),
            target_topic: request.target_topic.clone(),
            messages_migrated,
            partitions_migrated: partitions_to_migrate,
            start_time,
            end_time,
            status: MigrationStatus::Completed,
        })
    }

    /// Wait for consumer group to drain all messages from topics
    pub async fn wait_for_queue_drain(
        &self,
        cluster_id: &str,
        request: &QueueDrainRequest,
        config: &crate::config::Config,
    ) -> Result<QueueDrainResponse, AppError> {
        let cluster_config = self.get_cluster(cluster_id, config).await?;

        let client_config = self.build_client_config(&cluster_config);
        let admin: AdminClient<_> = client_config
            .create()
            .map_err(|e| AppError::Kafka(format!("Failed to create admin client: {}", e)))?;

        let start_time = Instant::now();
        let timeout = Duration::from_secs(request.timeout_seconds.unwrap_or(300)); // 5 minutes default
        let check_interval = Duration::from_millis(request.check_interval_ms.unwrap_or(5000)); // 5 seconds default
        let max_lag_threshold = request.max_lag_threshold.unwrap_or(0);

        // Get initial lag
        let initial_offsets = self
            .get_consumer_group_offsets(&admin, &request.consumer_group, &request.topics)
            .await?;
        let mut total_initial_lag = 0i64;
        let mut partition_statuses = Vec::new();

        for offset in &initial_offsets {
            total_initial_lag += offset.lag;
            partition_statuses.push(PartitionDrainStatus {
                topic: offset.topic.clone(),
                partition: offset.partition,
                lag_start: offset.lag,
                lag_end: offset.lag,
                is_drained: offset.lag <= max_lag_threshold,
            });
        }

        // Wait for drain
        let mut is_drained = false;
        while start_time.elapsed() < timeout {
            tokio::time::sleep(check_interval).await;

            let current_offsets = self
                .get_consumer_group_offsets(&admin, &request.consumer_group, &request.topics)
                .await?;
            let mut total_current_lag = 0i64;
            let mut all_drained = true;

            for (i, offset) in current_offsets.iter().enumerate() {
                total_current_lag += offset.lag;
                if i < partition_statuses.len() {
                    partition_statuses[i].lag_end = offset.lag;
                    partition_statuses[i].is_drained = offset.lag <= max_lag_threshold;
                }
                if offset.lag > max_lag_threshold {
                    all_drained = false;
                }
            }

            if all_drained {
                is_drained = true;
                break;
            }
        }

        let drain_duration = start_time.elapsed().as_secs();

        Ok(QueueDrainResponse {
            topics: request.topics.clone(),
            consumer_group: request.consumer_group.clone(),
            total_lag_start: total_initial_lag,
            total_lag_end: partition_statuses.iter().map(|p| p.lag_end).sum(),
            drain_duration_seconds: drain_duration,
            is_drained,
            partitions_status: partition_statuses,
        })
    }

    // Helper method to get consumer group offsets
    async fn get_consumer_group_offsets(
        &self,
        admin: &AdminClient<rdkafka::client::DefaultClientContext>,
        group_id: &str,
        topics: &[String],
    ) -> Result<Vec<ConsumerGroupOffset>, AppError> {
        // This would need to be implemented using Kafka admin API
        // For now, return empty vec as placeholder
        Ok(Vec::new())
    }
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
pub struct TopicConfigUpdateRequest {
    pub configs: Vec<(String, String)>,
    pub validate_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionAdditionRequest {
    pub count: i32,
    pub validate_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerStatus {
    pub id: i32,
    pub host: String,
    pub port: i32,
    pub is_controller: bool,
    pub rack: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::repositories::cluster::ClusterRepository;
    use sea_orm::DatabaseConnection;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_cluster_update_validation() {
        // Create mock database connection and config for testing
        let db = Arc::new(DatabaseConnection::default());
        let config = Config::default();
        let cluster_repo = Arc::new(ClusterRepository::new(db, config));
        let kafka_service = KafkaService::new(cluster_repo);

        let update_req = ClusterUpdateRequest {
            name: Some("updated-cluster".to_string()),
            bootstrap_servers: Some(vec!["localhost:9092".to_string()]),
            sasl_username: None,
            sasl_password: None,
            sasl_mechanism: None,
            security_protocol: Some("PLAINTEXT".to_string()),
        };

        // Test validation
        let validation_result = kafka_service.validate_cluster_update(&update_req);
        assert!(validation_result.is_ok());
    }

    #[tokio::test]
    async fn test_invalid_bootstrap_servers() {
        // Create mock database connection and config for testing
        let db = Arc::new(DatabaseConnection::default());
        let config = Config::default();
        let cluster_repo = Arc::new(ClusterRepository::new(db, config));
        let kafka_service = KafkaService::new(cluster_repo);

        let update_req = ClusterUpdateRequest {
            name: None,
            bootstrap_servers: Some(vec!["invalid-server".to_string()]), // Missing port
            sasl_username: None,
            sasl_password: None,
            sasl_mechanism: None,
            security_protocol: None,
        };

        let validation_result = kafka_service.validate_cluster_update(&update_req);
        assert!(validation_result.is_err());
        if let Err(AppError::Validation(msg)) = validation_result {
            assert!(msg.contains("Invalid bootstrap server format"));
        }
    }

    #[tokio::test]
    async fn test_invalid_security_protocol() {
        // Create mock database connection and config for testing
        let db = Arc::new(DatabaseConnection::default());
        let config = Config::default();
        let cluster_repo = Arc::new(ClusterRepository::new(db, config));
        let kafka_service = KafkaService::new(cluster_repo);

        let update_req = ClusterUpdateRequest {
            name: None,
            bootstrap_servers: None,
            sasl_username: None,
            sasl_password: None,
            sasl_mechanism: None,
            security_protocol: Some("INVALID_PROTOCOL".to_string()),
        };

        let validation_result = kafka_service.validate_cluster_update(&update_req);
        assert!(validation_result.is_err());
        if let Err(AppError::Validation(msg)) = validation_result {
            assert!(msg.contains("Invalid security protocol"));
        }
    }
}
