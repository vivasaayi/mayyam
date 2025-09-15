use std::path::PathBuf;
use std::time::Duration;

use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::{Header, OwnedHeaders, Message, Headers};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::topic_partition_list::Offset;
use tokio::fs;
use tracing::warn;

use crate::errors::AppError;

use super::storage::{BackupData, BackupMetadata, BackupMessage, CompressionType, FileSystemStorage, BackupStorage};
use super::types::{KafkaService, MessageBackupRequest, MessageBackupResponse, MessageRestoreRequest, MessageRestoreResponse};

impl KafkaService {
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

        let backup_id = format!("backup_{}_{}", request.topic, chrono::Utc::now().timestamp());
        let mut total_messages = 0u64;
        let start_time = chrono::Utc::now();
        let start_time_str = start_time.to_rfc3339();

        let storage_path = PathBuf::from("./backups");
        let storage = FileSystemStorage::new(storage_path);
        let compression = CompressionType::Gzip;

        let metadata = consumer
            .fetch_metadata(Some(&request.topic), Duration::from_secs(30))
            .map_err(|e| AppError::Kafka(format!("Failed to fetch topic metadata: {}", e)))?;

        let topic_metadata = metadata
            .topics()
            .iter()
            .find(|t| t.name() == request.topic)
            .ok_or_else(|| AppError::NotFound(format!("Topic {} not found", request.topic)))?;

        let partitions_to_backup = request.partitions.clone().unwrap_or_else(|| {
            (0..topic_metadata.partitions().len() as i32).collect::<Vec<_>>()
        });

        consumer
            .subscribe(&[&request.topic])
            .map_err(|e| AppError::Kafka(format!("Failed to subscribe to topic: {}", e)))?;

        for partition in &partitions_to_backup {
            let mut partition_messages = Vec::new();
            let timeout = Duration::from_secs(10);
            let seek_offset = match request.start_offset {
                Some(offset) => Offset::Offset(offset),
                None => Offset::Beginning,
            };
            if let Err(e) = consumer.seek(&request.topic, *partition, seek_offset, timeout) {
                warn!("Failed to seek partition {} to offset: {}", partition, e);
                continue;
            }

            let max_messages = request.max_messages.unwrap_or(u64::MAX);
            while let Ok(message) = tokio::time::timeout(Duration::from_secs(5), consumer.recv()).await {
                match message {
                    Ok(msg) => {
                        if msg.partition() != *partition {
                            continue;
                        }
                        if let Some(end_offset) = request.end_offset {
                            if msg.offset() >= end_offset {
                                break;
                            }
                        }
                        if total_messages >= max_messages {
                            break;
                        }
                        let key = msg.key().map(|k| String::from_utf8_lossy(k).to_string());
                        let value = String::from_utf8_lossy(msg.payload().unwrap_or(&[])).to_string();
                        let headers = if request.include_headers.unwrap_or(true) {
                            msg.headers().map(|hdrs| {
                                (0..hdrs.count())
                                    .map(|i| hdrs.get(i))
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

            if !partition_messages.is_empty() {
                let backup_data = BackupData {
                    backup_id: backup_id.clone(),
                    topic: request.topic.clone(),
                    partition: *partition,
                    messages: partition_messages,
                    checksum: 0,
                    created_at: start_time_str.clone(),
                };
                storage
                    .store_backup(&backup_data, &compression)
                    .await
                    .map_err(|e| AppError::Internal(format!("Failed to store backup for partition {}: {}", partition, e)))?;
            }
        }

        let end_time = chrono::Utc::now();
        let end_time_str = end_time.to_rfc3339();

        Ok(MessageBackupResponse {
            topic: request.topic.clone(),
            partitions_backed_up: partitions_to_backup,
            total_messages,
            start_time: start_time_str,
            end_time: end_time_str,
            backup_id,
        })
    }

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

        let storage_path = PathBuf::from("./backups");
        let storage = FileSystemStorage::new(storage_path);
        let metadata_path = storage.get_metadata_path(&request.backup_id);
        if !metadata_path.exists() {
            return Err(AppError::NotFound(format!("Backup {} not found", request.backup_id)));
        }

        let metadata_json = fs::read(&metadata_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read backup metadata: {}", e)))?;
        let metadata: BackupMetadata = serde_json::from_slice(&metadata_json)
            .map_err(|e| AppError::Internal(format!("Failed to deserialize backup metadata: {}", e)))?;

        let partitions_to_restore = request
            .partitions
            .clone()
            .unwrap_or_else(|| metadata.partitions.clone());

        for &partition in &partitions_to_restore {
            if !metadata.partitions.contains(&partition) {
                warn!(
                    "Partition {} not found in backup {}, skipping",
                    partition, request.backup_id
                );
                continue;
            }
            let backup_data = storage
                .load_backup(&request.backup_id, partition)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to load backup data for partition {}: {}", partition, e)))?;

            for message in &backup_data.messages {
                let mut record = FutureRecord::to(&request.target_topic).payload(&message.value);
                if request.preserve_keys.unwrap_or(true) {
                    if let Some(ref key) = message.key {
                        record = record.key(key.as_bytes());
                    }
                }
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
                match producer.send(record, Duration::from_secs(10)).await {
                    Ok(_) => {
                        messages_restored += 1;
                    }
                    Err((e, _)) => {
                        warn!("Failed to send message during restore: {}", e);
                    }
                }
            }
        }

        let end_time = chrono::Utc::now();
        let end_time_str = end_time.to_rfc3339();

        Ok(MessageRestoreResponse {
            target_topic: request.target_topic.clone(),
            messages_restored,
            partitions_restored: partitions_to_restore,
            start_time: start_time_str,
            end_time: end_time_str,
        })
    }
}
