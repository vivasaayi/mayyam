// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::cluster;
use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::services::analytics::kafka_analytics::acl_inventory::{
    acl_inventory_item_from_config, evaluate_kafka_acl_inventory,
    RESOURCE_TYPE as KAFKA_ACL_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::admin_api_inventory::{
    admin_api_inventory_item_from_config, evaluate_kafka_admin_api_inventory,
    RESOURCE_TYPE as KAFKA_ADMIN_API_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::backup_inventory::{
    backup_inventory_item_from_config, evaluate_backup_inventory,
    RESOURCE_TYPE as KAFKA_BACKUP_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::broker_disk_inventory::{
    broker_disk_inventory_item_from_config, evaluate_broker_disk_inventory,
    RESOURCE_TYPE as KAFKA_BROKER_DISK_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::broker_inventory::{
    broker_inventory_items_from_config, evaluate_kafka_broker_inventory,
    RESOURCE_TYPE as KAFKA_BROKER_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::cluster_inventory::{
    cluster_inventory_item_from_config, evaluate_kafka_cluster_inventory,
    RESOURCE_TYPE as KAFKA_CLUSTER_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::compaction_policy_inventory::{
    compaction_policy_inventory_item_from_config, evaluate_kafka_compaction_policy_inventory,
    RESOURCE_TYPE as KAFKA_COMPACTION_POLICY_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::connect_inventory::{
    connect_inventory_item_from_config, evaluate_kafka_connect_inventory,
    RESOURCE_TYPE as KAFKA_CONNECT_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::connector_inventory::{
    connector_inventory_item_from_config, evaluate_kafka_connector_inventory,
    RESOURCE_TYPE as KAFKA_CONNECTOR_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::consumer_group_inventory::{
    consumer_group_inventory_item_from_config, evaluate_kafka_consumer_group_inventory,
    RESOURCE_TYPE as KAFKA_CONSUMER_GROUP_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::consumer_inventory::{
    consumer_inventory_item_from_config, evaluate_kafka_consumer_inventory,
    RESOURCE_TYPE as KAFKA_CONSUMER_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::controller_quorum_inventory::{
    controller_quorum_item_from_config, evaluate_kafka_controller_quorum_inventory,
    RESOURCE_TYPE as KAFKA_CONTROLLER_QUORUM_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::dead_letter_topic_inventory::{
    dead_letter_topic_inventory_item_from_config, evaluate_dead_letter_topic_inventory,
    RESOURCE_TYPE as KAFKA_DEAD_LETTER_TOPIC_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::isr_health_inventory::{
    evaluate_kafka_isr_health_inventory, isr_health_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_ISR_HEALTH_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::kraft_inventory::{
    evaluate_kraft_inventory, kraft_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_KRAFT_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::lag_inventory::{
    evaluate_kafka_lag_inventory, lag_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_LAG_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::managed_service_inventory::{
    evaluate_managed_service_inventory, managed_service_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_MANAGED_SERVICE_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::message_replay_inventory::{
    evaluate_message_replay_inventory, message_replay_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_MESSAGE_REPLAY_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::mirror_maker_inventory::{
    evaluate_mirror_maker_inventory, mirror_maker_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_MIRROR_MAKER_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::network_throughput_inventory::{
    evaluate_network_throughput_inventory, network_throughput_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_NETWORK_THROUGHPUT_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::offset_inventory::{
    evaluate_kafka_offset_inventory, offset_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_OFFSET_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::partition_inventory::{
    evaluate_kafka_partition_inventory, partition_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_PARTITION_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::poison_message_inventory::{
    evaluate_poison_message_inventory, poison_message_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_POISON_MESSAGE_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::producer_inventory::{
    evaluate_kafka_producer_inventory, producer_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_PRODUCER_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::quota_inventory::{
    evaluate_kafka_quota_inventory, quota_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_QUOTA_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::replica_inventory::{
    evaluate_kafka_replica_inventory, replica_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_REPLICA_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::restore_inventory::{
    evaluate_restore_inventory, restore_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_RESTORE_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::retention_policy_inventory::{
    evaluate_kafka_retention_policy_inventory, retention_policy_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_RETENTION_POLICY_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::sasl_inventory::{
    evaluate_kafka_sasl_inventory, sasl_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_SASL_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::schema_registry_inventory::{
    evaluate_kafka_schema_registry_inventory, schema_registry_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_SCHEMA_REGISTRY_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::streams_inventory::{
    evaluate_kafka_streams_inventory, streams_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_STREAMS_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::tiered_storage_inventory::{
    evaluate_tiered_storage_inventory, tiered_storage_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_TIERED_STORAGE_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::tls_inventory::{
    evaluate_kafka_tls_inventory, tls_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_TLS_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::topic_inventory::{
    evaluate_kafka_topic_inventory, topic_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_TOPIC_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::topic_migration_inventory::{
    evaluate_topic_migration_inventory, topic_migration_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_TOPIC_MIGRATION_RESOURCE_TYPE,
};
use crate::services::analytics::kafka_analytics::zookeeper_migration_inventory::{
    evaluate_zookeeper_migration_inventory, zookeeper_migration_inventory_item_from_config,
    RESOURCE_TYPE as KAFKA_ZOOKEEPER_MIGRATION_RESOURCE_TYPE,
};
use crate::services::aws::inventory::types::{Pillar, DEFAULT_STALE_AFTER_HOURS};
use crate::services::kafka::{
    ClusterUpdateRequest, ConsumeOptions, KafkaMessage, KafkaService, KafkaTopic,
    MessageBackupRequest, MessageMigrationRequest, MessageRestoreRequest, OffsetReset,
    PartitionAdditionRequest, PartitionOffset, QueueDrainRequest, TopicConfigUpdateRequest,
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

#[derive(Debug, Deserialize)]
pub struct KafkaInventoryQuery {
    pub pillar: Option<String>,
}

fn parse_kafka_inventory_pillars(
    pillar: &Option<String>,
    workflow: &str,
) -> Result<Vec<Pillar>, AppError> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        AppError::BadRequest(format!(
                            "Unsupported pillar '{}' for {}; supported pillars are cost, resilience, and security",
                            part, workflow
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(AppError::BadRequest(format!(
                    "At least one pillar must be provided for {}",
                    workflow
                )));
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
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
    let kafka_messages: Vec<KafkaMessage> = batch_req
        .messages
        .iter()
        .map(|msg| KafkaMessage {
            key: msg.key.clone(),
            value: msg.value.clone(),
            headers: msg.headers.clone(),
        })
        .collect();

    // Use the KafkaService to produce batch messages
    let response = kafka_service
        .produce_batch(&cluster_id, "batch-topic", kafka_messages, &config)
        .await?;

    Ok(HttpResponse::Ok().json(response))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RetryProduceRequest {
    pub topic: String,
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
    let response = kafka_service
        .produce_with_retry(
            &cluster_id,
            &retry_req.topic,
            &kafka_message,
            &config,
            max_retries,
        )
        .await?;

    Ok(HttpResponse::Ok().json(response))
}

pub async fn list_clusters(
    kafka_service: web::Data<Arc<KafkaService>>,
    _config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    // Get clusters from the service
    let clusters = kafka_service.list_clusters().await?;

    let response = serde_json::json!({
        "clusters": clusters,
        "total": clusters.len()
    });

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_kafka_cluster_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka cluster inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| cluster_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_cluster_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_CLUSTER_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_broker_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka broker inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .flat_map(|cluster| broker_inventory_items_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_broker_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_BROKER_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_controller_quorum_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars =
        parse_kafka_inventory_pillars(&query.pillar, "Kafka controller quorum inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| controller_quorum_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_controller_quorum_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_CONTROLLER_QUORUM_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_topic_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka topic inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| topic_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_topic_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_TOPIC_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_partition_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka partition inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| partition_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_partition_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_PARTITION_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_replica_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka replica inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| replica_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_replica_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_REPLICA_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_isr_health_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka ISR health inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| isr_health_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_isr_health_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_ISR_HEALTH_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_consumer_group_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka consumer group inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| consumer_group_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_consumer_group_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_CONSUMER_GROUP_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_offset_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka offset inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| offset_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_offset_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_OFFSET_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_lag_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka lag inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| lag_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_lag_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_LAG_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_producer_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka producer inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| producer_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_producer_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_PRODUCER_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_consumer_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka consumer inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| consumer_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_consumer_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_CONSUMER_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_admin_api_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka Admin API inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| admin_api_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_admin_api_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_ADMIN_API_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_acl_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka ACL inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| acl_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_acl_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_ACL_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_sasl_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka SASL inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| sasl_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_sasl_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_SASL_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_tls_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka TLS inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| tls_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_tls_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_TLS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_quota_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka quota inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| quota_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_quota_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_QUOTA_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_retention_policy_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka retention policy inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| retention_policy_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_retention_policy_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_RETENTION_POLICY_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_compaction_policy_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars =
        parse_kafka_inventory_pillars(&query.pillar, "Kafka compaction policy inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| compaction_policy_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_compaction_policy_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_COMPACTION_POLICY_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_schema_registry_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka Schema Registry inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| schema_registry_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_schema_registry_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_SCHEMA_REGISTRY_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_connect_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka Connect inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| connect_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_connect_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_CONNECT_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_connector_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka connector inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| connector_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_connector_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_CONNECTOR_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_streams_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka Streams inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| streams_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kafka_streams_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_STREAMS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_mirror_maker_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "MirrorMaker 2 inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| mirror_maker_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_mirror_maker_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_MIRROR_MAKER_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_tiered_storage_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka tiered storage inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| tiered_storage_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_tiered_storage_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_TIERED_STORAGE_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_kraft_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka KRaft inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| kraft_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kraft_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_KRAFT_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_zookeeper_migration_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars =
        parse_kafka_inventory_pillars(&query.pillar, "Kafka ZooKeeper migration inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| zookeeper_migration_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_zookeeper_migration_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_ZOOKEEPER_MIGRATION_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_backup_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka backup inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| backup_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_backup_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_BACKUP_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_restore_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka restore inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| restore_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_restore_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_RESTORE_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_topic_migration_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka topic migration inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| topic_migration_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_topic_migration_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_TOPIC_MIGRATION_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_message_replay_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka message replay inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| message_replay_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_message_replay_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_MESSAGE_REPLAY_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_dead_letter_topic_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars =
        parse_kafka_inventory_pillars(&query.pillar, "Kafka dead-letter topic inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| dead_letter_topic_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_dead_letter_topic_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_DEAD_LETTER_TOPIC_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_poison_message_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka poison message inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| poison_message_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_poison_message_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_POISON_MESSAGE_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_broker_disk_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Kafka broker disk inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| broker_disk_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_broker_disk_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_BROKER_DISK_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_network_throughput_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars =
        parse_kafka_inventory_pillars(&query.pillar, "Kafka network throughput inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| network_throughput_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_network_throughput_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_NETWORK_THROUGHPUT_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_kafka_managed_service_inventory_pillar_reports(
    query: web::Query<KafkaInventoryQuery>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    let pillars = parse_kafka_inventory_pillars(&query.pillar, "Managed Kafka services inventory")?;
    let now = Utc::now();
    let items = config
        .kafka
        .clusters
        .iter()
        .map(|cluster| managed_service_inventory_item_from_config(cluster, now))
        .collect::<Vec<_>>();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_managed_service_inventory(&items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = items.iter().map(|item| item.collected_at).min();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "resource_type": KAFKA_MANAGED_SERVICE_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn create_cluster(
    cluster: web::Json<KafkaClusterRequest>,
    kafka_service: web::Data<Arc<KafkaService>>,
    _config: web::Data<crate::config::Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    // Convert the request to the service model
    let create_request = cluster::CreateKafkaClusterRequest {
        name: cluster.name.clone(),
        bootstrap_servers: cluster.bootstrap_servers.clone(),
        sasl_username: cluster.sasl_username.clone(),
        sasl_password: cluster.sasl_password.clone(),
        sasl_mechanism: cluster.sasl_mechanism.clone(),
        security_protocol: cluster
            .security_protocol
            .clone()
            .unwrap_or_else(|| "PLAINTEXT".to_string()),
    };

    // Create the cluster using the service
    let response = kafka_service
        .create_cluster(&create_request, &claims.sub)
        .await?;

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
        Err(AppError::NotFound(format!(
            "Kafka cluster with ID {} not found",
            cluster_id
        )))
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
    let response = kafka_service
        .create_topic(&cluster_id, &kafka_topic, &config)
        .await?;

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
    let topic_details = kafka_service
        .get_topic_details(&cluster_id, &topic_name, &config)
        .await?;

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
    let response = kafka_service
        .delete_topic(&cluster_id, &topic_name, &config)
        .await?;

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
    let response = kafka_service
        .produce_message(&cluster_id, &topic_name, &kafka_message, &config)
        .await?;

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
    let messages = kafka_service
        .consume_messages(&cluster_id, &topic_name, &consume_options, &config)
        .await?;

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
    let consumer_groups = kafka_service
        .list_consumer_groups(&cluster_id, &config)
        .await?;

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
    let group_details = kafka_service
        .get_consumer_group(&cluster_id, &group_id, &config)
        .await?;

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
    let response = kafka_service
        .reset_offsets(&cluster_id, &group_id, &offset_reset, &config)
        .await?;

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
    let response = kafka_service
        .update_topic_config(
            &cluster_id,
            &topic_name,
            config_req.configs.clone(),
            validate_only,
            &config,
        )
        .await?;

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
    let response = kafka_service
        .update_cluster_config(&cluster_id, &*update_req, &config)
        .await?;

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
    let response = kafka_service
        .add_topic_partitions(
            &cluster_id,
            &topic_name,
            partition_req.count,
            validate_only,
            &config,
        )
        .await?;

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
    let brokers = kafka_service
        .get_broker_status(&cluster_id, &config)
        .await?;

    let response = serde_json::json!({
        "cluster_id": cluster_id,
        "brokers": brokers,
        "total_brokers": brokers.len()
    });

    Ok(HttpResponse::Ok().json(response))
}

// ===== BACKUP AND RESTORE CONTROLLERS =====

// Backup messages from a topic
pub async fn backup_topic_messages(
    path: web::Path<String>,
    backup_req: web::Json<MessageBackupRequest>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();

    let response = kafka_service
        .backup_topic_messages(&cluster_id, &*backup_req, &config)
        .await?;

    Ok(HttpResponse::Ok().json(response))
}

// Restore messages to a topic
pub async fn restore_topic_messages(
    path: web::Path<String>,
    restore_req: web::Json<MessageRestoreRequest>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();

    let response = kafka_service
        .restore_topic_messages(&cluster_id, &*restore_req, &config)
        .await?;

    Ok(HttpResponse::Ok().json(response))
}

// Migrate messages between topics (can be cross-cluster)
pub async fn migrate_topic_messages(
    migration_req: web::Json<MessageMigrationRequest>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let response = kafka_service
        .migrate_topic_messages(&*migration_req, &config)
        .await?;

    Ok(HttpResponse::Ok().json(response))
}

// Wait for consumer group to drain all messages
pub async fn wait_for_queue_drain(
    path: web::Path<String>,
    drain_req: web::Json<QueueDrainRequest>,
    kafka_service: web::Data<Arc<KafkaService>>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();

    let response = kafka_service
        .wait_for_queue_drain(&cluster_id, &*drain_req, &config)
        .await?;

    Ok(HttpResponse::Ok().json(response))
}
