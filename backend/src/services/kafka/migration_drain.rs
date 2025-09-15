use std::time::{Duration, Instant};

use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::message::Message;
use rdkafka::topic_partition_list::Offset;
use tracing::warn;

use crate::errors::AppError;

use super::types::{
    KafkaService, MessageMigrationRequest, MessageMigrationResponse, MigrationStatus, QueueDrainRequest,
    QueueDrainResponse, PartitionDrainStatus,
};

impl KafkaService {
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

        source_consumer
            .subscribe(&[&request.source_topic])
            .map_err(|e| AppError::Kafka(format!("Failed to subscribe to source topic: {}", e)))?;

        let metadata = source_consumer
            .fetch_metadata(Some(&request.source_topic), Duration::from_secs(30))
            .map_err(|e| AppError::Kafka(format!("Failed to fetch source topic metadata: {}", e)))?;
        let topic_metadata = metadata
            .topics()
            .iter()
            .find(|t| t.name() == request.source_topic)
            .ok_or_else(|| AppError::NotFound(format!("Source topic {} not found", request.source_topic)))?;
        let partitions_to_migrate = request.partitions.clone().unwrap_or_else(|| {
            (0..topic_metadata.partitions().len() as i32).collect::<Vec<_>>()
        });

        for partition in &partitions_to_migrate {
            let timeout = Duration::from_secs(10);
            let seek_offset = match request.start_offset {
                Some(offset) => Offset::Offset(offset),
                None => Offset::Beginning,
            };
            if let Err(e) = source_consumer.seek(&request.source_topic, *partition, seek_offset, timeout) {
                warn!("Failed to seek source partition {} to offset: {}", partition, e);
                continue;
            }

            while let Ok(message) = tokio::time::timeout(Duration::from_secs(5), source_consumer.recv()).await {
                match message {
                    Ok(msg) => {
                        if msg.partition() != *partition { continue; }
                        if let Some(end_offset) = request.end_offset {
                            if msg.offset() >= end_offset { break; }
                        }
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
                            msg.key().map(|k| format!("{}{}", prefix, String::from_utf8_lossy(k)))
                        } else {
                            msg.key().map(|k| String::from_utf8_lossy(k).to_string())
                        };
                        let payload = msg.payload().unwrap_or(&[]);
                        let mut record = FutureRecord::to(&request.target_topic).payload(payload);
                        if let Some(key) = &target_key {
                            record = record.key(key.as_bytes());
                        }
                        if let Err((e, _)) = target_producer.send(record, Duration::from_secs(10)).await {
                            warn!("Failed to send message to target topic: {}", e);
                        } else {
                            messages_migrated += 1;
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

    pub async fn wait_for_queue_drain(
        &self,
        cluster_id: &str,
        request: &QueueDrainRequest,
        config: &crate::config::Config,
    ) -> Result<QueueDrainResponse, AppError> {
        let cluster_config = self.get_cluster(cluster_id, config).await?;
    let client_config = self.build_client_config(&cluster_config);

        let mut consumer_cfg = self.build_client_config(&cluster_config);
        consumer_cfg.set("group.id", &request.consumer_group);
        consumer_cfg.set("enable.auto.commit", "false");
        let consumer: StreamConsumer = consumer_cfg
            .create()
            .map_err(|e| AppError::Kafka(format!(
                "Failed to create consumer for group '{}': {}",
                request.consumer_group, e
            )))?;

        let start_time = Instant::now();
        let timeout = Duration::from_secs(request.timeout_seconds.unwrap_or(300));
        let check_interval = Duration::from_millis(request.check_interval_ms.unwrap_or(5000));
        let max_lag_threshold = request.max_lag_threshold.unwrap_or(0);

        let initial_offsets = self.get_consumer_group_offsets(&consumer, &request.topics).await?;
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

        let mut is_drained = false;
        while start_time.elapsed() < timeout {
            tokio::time::sleep(check_interval).await;
            let current_offsets = self.get_consumer_group_offsets(&consumer, &request.topics).await?;
            let mut all_drained = true;
            for (i, offset) in current_offsets.iter().enumerate() {
                if i < partition_statuses.len() {
                    partition_statuses[i].lag_end = offset.lag;
                    partition_statuses[i].is_drained = offset.lag <= max_lag_threshold;
                }
                if offset.lag > max_lag_threshold {
                    all_drained = false;
                }
            }
            if all_drained { is_drained = true; break; }
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
}
