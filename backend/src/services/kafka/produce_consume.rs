use std::time::{Duration, Instant};

use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::{Header, OwnedHeaders, Message, Headers};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::topic_partition_list::{Offset, TopicPartitionList};
use tracing::{error, warn};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::cluster::KafkaClusterConfig;
use crate::repositories::cluster::ClusterRepository;

use super::types::{
    ConsumeOptions, ConsumerGroupOffset, KafkaMessage, KafkaMetrics, KafkaService,
};

impl KafkaService {
    pub async fn get_cluster(
        &self,
        id: &str,
        config: &crate::config::Config,
    ) -> Result<KafkaClusterConfig, AppError> {
        if let Ok(cluster_uuid) = Uuid::parse_str(id) {
            if let Some(cluster) = self.cluster_repository.find_by_id(cluster_uuid).await? {
                let kafka_config: KafkaClusterConfig = serde_json::from_value(cluster.config)
                    .map_err(|e| AppError::Validation(format!("Invalid cluster configuration: {}", e)))?;
                return Ok(kafka_config);
            }
        }

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
            .ok_or_else(|| AppError::NotFound(format!("Kafka cluster '{}' not found (by id or name)", id)))
    }

    pub async fn health_check(
        &self,
        cluster_id: &str,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("client.id", "mayyam-health-check");

        let producer: FutureProducer = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to connect to Kafka cluster: {}", e)))?;

        let timeout = Duration::from_secs(10);
        let metadata = producer
            .client()
            .fetch_metadata(None, timeout)
            .map_err(|e| AppError::ExternalService(format!("Failed to fetch cluster metadata: {:?}", e)))?;

        let brokers = metadata
            .brokers()
            .iter()
            .map(|broker| serde_json::json!({
                "id": broker.id(),
                "host": broker.host(),
                "port": broker.port()
            }))
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "status": "healthy",
            "cluster_id": cluster_id,
            "brokers": brokers,
            "topics_count": metadata.topics().len(),
            "timestamp": chrono::Utc::now().timestamp_millis()
        }))
    }

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

        let producer: FutureProducer = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka producer: {}", e)))?;

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

        let mut record = FutureRecord::to(topic_name).payload(&message.value);

        if let Some(ref key) = message.key {
            record = record.key(key.as_bytes());
        }
        if let Some(h) = headers {
            record = record.headers(h);
        }

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
        client_config.set("enable.auto.commit", "false");

        if options.from_beginning.unwrap_or(false) {
            client_config.set("auto.offset.reset", "earliest");
        } else {
            client_config.set("auto.offset.reset", "latest");
        }

        let consumer: StreamConsumer = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka consumer: {}", e)))?;

        consumer
            .subscribe(&[topic_name])
            .map_err(|e| AppError::ExternalService(format!("Failed to subscribe to topic: {}", e)))?;

        let max_messages = options.max_messages.unwrap_or(10);
        let timeout_ms = options.timeout_ms.unwrap_or(5000);
        let mut messages = Vec::new();

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
                    let headers = message
                        .headers()
                        .map(|hdrs| {
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

                    if let Err(e) = consumer.commit_message(&message, CommitMode::Async) {
                        error!("Failed to commit message offset: {:?}", e);
                    }
                }
                Err(e) => {
                    error!("Error while consuming message: {:?}", e);
                    break;
                }
            }
            if start_time.elapsed() >= timeout_duration {
                break;
            }
        }
        Ok(messages)
    }

    pub async fn list_consumer_groups(
        &self,
        cluster_id: &str,
        config: &crate::config::Config,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("client.id", "mayyam-group-list");

        let producer: FutureProducer = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka client: {}", e)))?;

        let timeout = Duration::from_secs(30);
        let group_list = producer
            .client()
            .fetch_group_list(None, timeout)
            .map_err(|e| AppError::ExternalService(format!("Failed to fetch consumer groups: {}", e)))?;

        let groups = group_list
            .groups()
            .iter()
            .map(|g| serde_json::json!({
                "group_id": g.name(),
                "state": format!("{:?}", g.state()),
                "members": g.members().len()
            }))
            .collect::<Vec<_>>();
        Ok(groups)
    }

    pub async fn get_consumer_group(
        &self,
        cluster_id: &str,
        group_id: &str,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("client.id", "mayyam-group-detail");

        let producer: FutureProducer = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka client: {}", e)))?;

        let timeout = Duration::from_secs(30);
        let group_list = producer
            .client()
            .fetch_group_list(Some(group_id), timeout)
            .map_err(|e| AppError::ExternalService(format!("Failed to fetch consumer group {}: {}", group_id, e)))?;

        let group = group_list
            .groups()
            .iter()
            .find(|g| g.name() == group_id)
            .ok_or_else(|| AppError::NotFound(format!("Consumer group {} not found", group_id)))?;

        let members = group
            .members()
            .iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id().to_string(),
                    "client_id": m.client_id().to_string(),
                    "client_host": m.client_host().to_string(),
                    "assignments": []
                })
            })
            .collect::<Vec<_>>();

        let offsets: Vec<serde_json::Value> = Vec::new();

        Ok(serde_json::json!({
            "group_id": group_id,
            "state": format!("{:?}", group.state()),
            "members": members,
            "offsets": offsets,
        }))
    }

    pub async fn reset_offsets(
        &self,
        cluster_id: &str,
        group_id: &str,
        offset_req: &super::types::OffsetReset,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("group.id", group_id);
        client_config.set("enable.auto.commit", "false");

        let consumer: StreamConsumer = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create consumer for group '{}': {}", group_id, e)))?;

        let timeout = Duration::from_secs(30);

        let md = consumer
            .client()
            .fetch_metadata(None, timeout)
            .map_err(|e| AppError::ExternalService(format!("Failed to fetch metadata: {}", e)))?;

        let topics: Vec<String> = md.topics().iter().map(|t| t.name().to_string()).collect();
        if topics.is_empty() {
            return Err(AppError::Validation(format!(
                "No topics found in cluster to reset offsets for group '{}'",
                group_id
            )));
        }

        let mut tpl = TopicPartitionList::new();
        for t in &topics {
            if let Some(tmd) = md.topics().iter().find(|mt| mt.name() == t) {
                for p in tmd.partitions() {
                    tpl.add_partition(t, p.id());
                }
            }
        }

        let mut new_tpl = TopicPartitionList::new();
        for elem in tpl.elements() {
            let topic = elem.topic();
            let partition = elem.partition();
            let (low, high) = consumer
                .client()
                .fetch_watermarks(topic, partition, timeout)
                .unwrap_or((0, 0));

            let mut target_offset_opt: Option<i64> = None;
            if let Some(po) = offset_req.partitions.iter().find(|po| po.partition == partition) {
                if let Some(off) = po.offset {
                    target_offset_opt = Some(off);
                }
            }

            let target = if let Some(off) = target_offset_opt {
                off
            } else if offset_req.to_earliest.unwrap_or(false) {
                low
            } else if offset_req.to_latest.unwrap_or(false) {
                high
            } else if let Some(off) = offset_req.to_offset {
                off
            } else {
                high
            };
            new_tpl
                .add_partition_offset(topic, partition, Offset::Offset(target))
                .ok();
        }

        consumer
            .commit(&new_tpl, CommitMode::Sync)
            .map_err(|e| AppError::ExternalService(format!("Failed to commit offsets: {}", e)))?;

        Ok(serde_json::json!({
            "message": format!("Consumer group {} offsets reset successfully", group_id),
            "partitions": new_tpl.count(),
        }))
    }

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

        let producer: FutureProducer = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka producer: {}", e)))?;

        let mut send_futures = Vec::new();
        let mut total_size = 0;
        for message in messages {
            total_size += message.value.len();
            let topic_name = topic_name.to_string();
            let value_data = message.value.into_bytes();
            let key_data = message.key.map(|k| k.into_bytes());
            let headers_data = message.headers;
            let producer_clone = producer.clone();
            let send_future = async move {
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

        let futures = send_futures;
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
                    let _duration = start_time.elapsed().as_millis() as f64;
                    return Ok(result);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        let delay = Duration::from_millis(100 * (2_u64.pow(attempt)));
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        Err(last_error.unwrap_or_else(|| AppError::Internal("Unknown error".to_string())))
    }

    pub async fn get_consumer_group_offsets(
        &self,
        consumer: &StreamConsumer,
        topics: &[String],
    ) -> Result<Vec<ConsumerGroupOffset>, AppError> {
        let timeout = Duration::from_secs(30);
        let topic_refs: Vec<&str> = topics.iter().map(|s| s.as_str()).collect();
        if !topic_refs.is_empty() {
            consumer
                .subscribe(&topic_refs)
                .map_err(|e| AppError::ExternalService(format!("Failed to subscribe for offsets query: {}", e)))?;
        }

        let committed = consumer
            .committed(timeout)
            .map_err(|e| AppError::ExternalService(format!("Failed to fetch committed offsets: {}", e)))?;

        let mut results = Vec::new();
        for elem in committed.elements() {
            let topic = elem.topic().to_string();
            let partition = elem.partition();
            let committed_offset = match elem.offset() {
                Offset::Invalid => -1,
                Offset::Offset(v) => v,
                Offset::Beginning => 0,
                Offset::End => consumer
                    .client()
                    .fetch_watermarks(&topic, partition, timeout)
                    .map(|(_, high)| high)
                    .unwrap_or(0),
                _ => -1,
            };
            let (_low, high) = consumer
                .client()
                .fetch_watermarks(&topic, partition, timeout)
                .unwrap_or((0, 0));
            let lag = if committed_offset >= 0 {
                high.saturating_sub(committed_offset)
            } else {
                high
            };
            results.push(ConsumerGroupOffset {
                topic,
                partition,
                offset: committed_offset,
                lag,
            });
        }
        Ok(results)
    }
}
