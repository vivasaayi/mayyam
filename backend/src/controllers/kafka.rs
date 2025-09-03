use actix_web::{web, HttpResponse, Responder};
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, error};
use uuid::Uuid;

use crate::services::kafka::{
    KafkaService, KafkaCluster, KafkaTopic, KafkaMessage, 
    ConsumeOptions, OffsetReset, PartitionOffset,
    ClusterUpdateRequest, TopicConfigUpdateRequest, PartitionAdditionRequest
};

#[derive(Debug, Serialize, Deserialize)]
pub struct KafkaClusterRequest {
    pub name: String,
    pub bootstrap_servers: Vec<String>,
    pub sasl_username: Option<String>,
    pub sasl_password: Option<String>,
    pub sasl_mechanism: Option<String>,
    pub security_protocol: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TopicRequest {
    pub name: String,
    pub partitions: i32,
    pub replication_factor: i16,
    pub configs: Option<Vec<(String, String)>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageRequest {
    pub key: Option<String>,
    pub value: String,
    pub headers: Option<Vec<(String, String)>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConsumeRequest {
    pub group_id: String,
    pub max_messages: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub from_beginning: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OffsetRequest {
    pub partitions: Vec<PartitionOffset>,
    pub to_earliest: Option<bool>,
    pub to_latest: Option<bool>,
    pub to_offset: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BrokerStatus {
    pub id: i32,
    pub host: String,
    pub port: i32,
    pub is_controller: bool,
    pub rack: Option<String>,
}

pub async fn health_check(
    path: web::Path<String>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    
    // Use the KafkaService to perform health check
    let health_status = kafka_service.health_check(&cluster_id, &config).await?;
    
    Ok(HttpResponse::Ok().json(health_status))
}

pub async fn get_metrics(
    kafka_service: web::Data<Arc<KafkaService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    // Get current metrics from the Kafka service
    let metrics = kafka_service.get_metrics()?;
    
    Ok(HttpResponse::Ok().json(metrics))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchProduceRequest {
    pub messages: Vec<MessageRequest>,
}

pub async fn produce_batch(
    path: web::Path<String>,
    batch_req: web::Json<BatchProduceRequest>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    
    // Convert request messages to service models
    let kafka_messages: Vec<KafkaMessage> = batch_req.messages.iter().map(|msg| {
        KafkaMessage {
            key: msg.key.clone(),
            value: msg.value.clone(),
            headers: msg.headers.clone(),
        }
    }).collect();
    
    // Use the KafkaService to produce batch messages
    let response = kafka_service.produce_batch(&cluster_id, "batch-topic", kafka_messages, &config).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RetryProduceRequest {
    pub message: MessageRequest,
    pub max_retries: Option<u32>,
}

pub async fn produce_with_retry(
    path: web::Path<String>,
    retry_req: web::Json<RetryProduceRequest>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    
    // Convert request to service model
    let kafka_message = KafkaMessage {
        key: retry_req.message.key.clone(),
        value: retry_req.message.value.clone(),
        headers: retry_req.message.headers.clone(),
    };
    
    let max_retries = retry_req.max_retries.unwrap_or(3);
    
    // Use the KafkaService to produce message with retry
    let response = kafka_service.produce_with_retry(&cluster_id, "retry-topic", &kafka_message, &config, max_retries).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

pub async fn list_clusters(
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let clusters: Vec<serde_json::Value> = config.kafka.clusters.iter().map(|cluster| {
        serde_json::json!({
            "id": cluster.name, // Using name as ID for now
            "name": cluster.name,
            "bootstrap_servers": cluster.bootstrap_servers,
            "security_protocol": cluster.security_protocol,
            "sasl_mechanism": cluster.sasl_mechanism,
        })
    }).collect();
    
    let response = serde_json::json!({
        "clusters": clusters,
        "total": clusters.len()
    });
    
    Ok(HttpResponse::Ok().json(response))
}

pub async fn create_cluster(
    cluster: web::Json<KafkaClusterRequest>,
    _config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    // In a real implementation, this would save the cluster to configuration
    // For now, we'll return a dummy response
    
    let cluster_id = Uuid::new_v4().to_string();
    
    let response = serde_json::json!({
        "id": cluster_id,
        "name": cluster.name,
        "message": "Kafka cluster connection created successfully"
    });
    
    Ok(HttpResponse::Created().json(response))
}

pub async fn get_cluster(
    path: web::Path<String>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    
    // In a real implementation, we'd look up by ID, but for now find by name
    if let Some(cluster) = config.kafka.clusters.iter().find(|c| c.name == cluster_id) {
        let response = serde_json::json!({
            "id": cluster_id,
            "name": cluster.name,
            "bootstrap_servers": cluster.bootstrap_servers,
            "security_protocol": cluster.security_protocol,
            "sasl_mechanism": cluster.sasl_mechanism,
        });
        
        Ok(HttpResponse::Ok().json(response))
    } else {
        Err(AppError::NotFound(format!("Kafka cluster with ID {} not found", cluster_id)))
    }
}

pub async fn list_topics(
    path: web::Path<String>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    
    // Use the KafkaService to list topics
    let topics = kafka_service.list_topics(&cluster_id, &config).await?;
    
    Ok(HttpResponse::Ok().json(topics))
}

pub async fn create_topic(
    path: web::Path<String>,
    topic_req: web::Json<TopicRequest>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    
    // Convert request to service model
    let kafka_topic = KafkaTopic {
        name: topic_req.name.clone(),
        partitions: topic_req.partitions,
        replication_factor: topic_req.replication_factor,
        configs: topic_req.configs.clone(),
    };
    
    // Use the KafkaService to create the topic
    let response = kafka_service.create_topic(&cluster_id, &kafka_topic, &config).await?;
    
    Ok(HttpResponse::Created().json(response))
}

pub async fn get_topic(
    path: web::Path<(String, String)>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, topic_name) = path.into_inner();
    
    // Use the KafkaService to get topic details
    let topic_details = kafka_service.get_topic_details(&cluster_id, &topic_name, &config).await?;
    
    Ok(HttpResponse::Ok().json(topic_details))
}

pub async fn delete_topic(
    path: web::Path<(String, String)>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, topic_name) = path.into_inner();
    
    // Use the KafkaService to delete the topic
    let response = kafka_service.delete_topic(&cluster_id, &topic_name, &config).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

pub async fn produce_message(
    path: web::Path<(String, String)>,
    message: web::Json<MessageRequest>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, topic_name) = path.into_inner();
    
    // Convert request to service model
    let kafka_message = KafkaMessage {
        key: message.key.clone(),
        value: message.value.clone(),
        headers: message.headers.clone(),
    };
    
    // Use the KafkaService to produce the message
    let response = kafka_service.produce_message(&cluster_id, &topic_name, &kafka_message, &config).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

pub async fn consume_messages(
    path: web::Path<(String, String)>,
    consume_req: web::Json<ConsumeRequest>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, topic_name) = path.into_inner();
    
    // Convert request to service model
    let consume_options = ConsumeOptions {
        group_id: consume_req.group_id.clone(),
        max_messages: consume_req.max_messages,
        timeout_ms: consume_req.timeout_ms,
        from_beginning: consume_req.from_beginning,
    };
    
    // Use the KafkaService to consume messages
    let messages = kafka_service.consume_messages(&cluster_id, &topic_name, &consume_options, &config).await?;
    
    Ok(HttpResponse::Ok().json(messages))
}

pub async fn list_consumer_groups(
    path: web::Path<String>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    
    // Use the KafkaService to list consumer groups
    let consumer_groups = kafka_service.list_consumer_groups(&cluster_id, &config).await?;
    
    Ok(HttpResponse::Ok().json(consumer_groups))
}

pub async fn get_consumer_group(
    path: web::Path<(String, String)>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, group_id) = path.into_inner();
    
    // Use the KafkaService to get consumer group details
    let group_details = kafka_service.get_consumer_group(&cluster_id, &group_id, &config).await?;
    
    Ok(HttpResponse::Ok().json(group_details))
}

pub async fn reset_offsets(
    path: web::Path<(String, String)>,
    offset_req: web::Json<OffsetRequest>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, group_id) = path.into_inner();
    
    // Convert request to service model
    let offset_reset = OffsetReset {
        partitions: offset_req.partitions.clone(),
        to_earliest: offset_req.to_earliest,
        to_latest: offset_req.to_latest,
        to_offset: offset_req.to_offset,
    };
    
    // Use the KafkaService to reset offsets
    let response = kafka_service.reset_offsets(&cluster_id, &group_id, &offset_reset, &config).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

// Update topic configuration
pub async fn update_topic_config(
    path: web::Path<(String, String)>,
    config_req: web::Json<TopicConfigUpdateRequest>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, topic_name) = path.into_inner();
    let validate_only = config_req.validate_only.unwrap_or(false);
    
    // Use the KafkaService to update topic configuration
    let response = kafka_service.update_topic_config(
        &cluster_id,
        &topic_name,
        config_req.configs.clone(),
        validate_only,
        &config
    ).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

// Update cluster configuration
pub async fn update_cluster(
    path: web::Path<String>,
    update_req: web::Json<ClusterUpdateRequest>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    
    // Use the KafkaService to update cluster configuration
    let response = kafka_service.update_cluster_config(&cluster_id, &*update_req, &config).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

// Add partitions to a topic
pub async fn add_topic_partitions(
    path: web::Path<(String, String)>,
    partition_req: web::Json<PartitionAdditionRequest>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, topic_name) = path.into_inner();
    let validate_only = partition_req.validate_only.unwrap_or(false);
    
    // Use the KafkaService to add partitions
    let response = kafka_service.add_topic_partitions(
        &cluster_id,
        &topic_name,
        partition_req.count,
        validate_only,
        &config
    ).await?;
    
    Ok(HttpResponse::Ok().json(response))
}

// Get detailed broker status
pub async fn get_broker_status(
    path: web::Path<String>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    
    // Use the KafkaService to get broker status
    let brokers = kafka_service.get_broker_status(&cluster_id, &config).await?;
    
    let response = serde_json::json!({
        "cluster_id": cluster_id,
        "brokers": brokers,
        "total_brokers": brokers.len()
    });
    
    Ok(HttpResponse::Ok().json(response))
}
