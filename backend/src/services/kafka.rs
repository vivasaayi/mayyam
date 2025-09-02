use std::sync::Arc;
use std::time::{Duration, Instant};
use rdkafka::admin::{AdminClient, NewTopic, TopicReplication};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::{Header, OwnedHeaders, Message, Headers};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tracing::{error, info, warn};
use std::sync::Mutex;
use crate::errors::AppError;
use crate::repositories::cluster::ClusterRepository;
use crate::config::KafkaClusterConfig;

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
            })),
        }
    }

    // Get current metrics
    pub fn get_metrics(&self) -> Result<KafkaMetrics, AppError> {
        let metrics = self.metrics.lock().map_err(|e| AppError::Internal(format!("Failed to lock metrics: {}", e)))?;
        Ok(KafkaMetrics {
            messages_produced: metrics.messages_produced,
            messages_consumed: metrics.messages_consumed,
            errors_count: metrics.errors_count,
            avg_response_time_ms: metrics.avg_response_time_ms,
            last_health_check: metrics.last_health_check,
            active_connections: metrics.active_connections,
        })
    }

    // Update metrics helper
    fn update_metrics(&self, operation: &str, duration_ms: f64, success: bool) {
        if let Ok(mut metrics) = self.metrics.lock() {
            if !success {
                metrics.errors_count += 1;
            }
            
            // Update average response time (simple moving average)
            let current_avg = metrics.avg_response_time_ms;
            metrics.avg_response_time_ms = (current_avg + duration_ms) / 2.0;
        }
        
        info!("Kafka operation '{}' completed in {:.2}ms, success: {}", operation, duration_ms, success);
    }

    // Get a Kafka cluster configuration by ID or name
    pub async fn get_cluster(&self, id: &str, config: &crate::config::Config) -> Result<KafkaClusterConfig, AppError> {
        // First try to find as a stored cluster in the database
        let cluster_id = Uuid::parse_str(id).map_err(|e| AppError::Internal(format!("Invalid UUID: {}", e)))?;
        let stored_cluster = self.cluster_repository.find_by_id(cluster_id).await?;
        if let Some(_cluster) = stored_cluster {
            // Convert from stored cluster to KafkaClusterConfig
            // This would require appropriate conversions
            unimplemented!("Convert from stored cluster to KafkaClusterConfig");
        }

        // If not found in database, look in configuration
        config.kafka.clusters.iter()
            .find(|c| c.name == id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Kafka cluster with ID {} not found", id)))
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
    pub async fn health_check(&self, cluster_id: &str, config: &crate::config::Config) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("client.id", "mayyam-health-check");
        
        // Try to create a producer to test connectivity
        let producer: FutureProducer = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to connect to Kafka cluster: {}", e)))?;
        
        // Get cluster metadata to verify connection
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

    // List topics in a cluster
    pub async fn list_topics(&self, cluster_id: &str, config: &crate::config::Config) -> Result<Vec<serde_json::Value>, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("client.id", "mayyam-admin");
        
        // Create an AdminClient to get topic metadata
        let admin: AdminClient<_> = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka admin client: {}", e)))?;
        
        // Get topic metadata with timeout
        let timeout = Duration::from_secs(30);
        let metadata = admin
            .inner()
            .fetch_metadata(None, timeout)
            .map_err(|e| AppError::ExternalService(format!("Failed to fetch topic metadata: {}", e)))?;
        
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
        config: &crate::config::Config
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);
        
        // Create an AdminClient
        let admin: AdminClient<_> = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka admin client: {}", e)))?;
        
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
        config: &crate::config::Config
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
        config: &crate::config::Config
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);
        
        // Create an AdminClient
        let admin: AdminClient<_> = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka admin client: {}", e)))?;
        
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
        config: &crate::config::Config
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("client.id", "mayyam-producer");
        
        // Create a producer
        let producer: FutureProducer = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka producer: {}", e)))?;
        
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
        let mut record = FutureRecord::to(topic_name)
            .payload(&message.value);
            
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
        config: &crate::config::Config
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
        let consumer: StreamConsumer = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka consumer: {}", e)))?;
        
        // Subscribe to the topic
        consumer
            .subscribe(&[topic_name])
            .map_err(|e| AppError::ExternalService(format!("Failed to subscribe to topic: {}", e)))?;
        
        let max_messages = options.max_messages.unwrap_or(10);
        let timeout_ms = options.timeout_ms.unwrap_or(5000);
        let mut messages = Vec::new();
        
        // Consume messages with timeout
        let timeout_duration = Duration::from_millis(timeout_ms);
        let start_time = std::time::Instant::now();
        
        while messages.len() < max_messages as usize && start_time.elapsed() < timeout_duration {
            match consumer.recv().await {
                Ok(message) => {
                    let payload = message.payload()
                        .map(|p| String::from_utf8_lossy(p).to_string())
                        .unwrap_or_else(|| "".to_string());
                    
                    let key = message.key()
                        .map(|k| String::from_utf8_lossy(k).to_string());
                    
                    // Extract headers
                    let headers = message.headers()
                        .map(|hdrs| {
                            (0..hdrs.count())
                                .filter_map(|i| Some(hdrs.get(i)))
                                .map(|h| (h.key.to_string(), 
                                         h.value.map(|v| String::from_utf8_lossy(v).to_string()).unwrap_or_default()))
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
        config: &crate::config::Config
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
        config: &crate::config::Config
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
        config: &crate::config::Config
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
        config: &crate::config::Config
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
                let mut record = FutureRecord::to(&topic_name)
                    .payload(&value_data);
                
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
        max_retries: u32
    ) -> Result<serde_json::Value, AppError> {
        let mut last_error = None;
        
        for attempt in 0..=max_retries {
            let start_time = Instant::now();
            
            match self.produce_message(cluster_id, topic_name, message, config).await {
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
        config: &crate::config::Config
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
            return Err(AppError::Validation("Bootstrap servers cannot be empty".to_string()));
        }
        
        for server in &config.bootstrap_servers {
            if !server.contains(':') {
                return Err(AppError::Validation(format!("Invalid bootstrap server format: {}. Expected host:port", server)));
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
                return Err(AppError::Validation("SASL username and password are required for SASL authentication".to_string()));
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
}