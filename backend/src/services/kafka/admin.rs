use std::time::Duration;

use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::admin::{AlterConfig, ResourceSpecifier};
use rdkafka::admin::NewPartitions;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, Producer};

use crate::errors::AppError;
use crate::models::cluster::KafkaClusterConfig;

use super::types::{KafkaService, KafkaTopic};

impl KafkaService {
    pub(crate) fn build_client_config(&self, cluster: &KafkaClusterConfig) -> ClientConfig {
        let mut client_config = ClientConfig::new();
        client_config.set("bootstrap.servers", &cluster.bootstrap_servers.join(","));

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

        client_config.set("request.timeout.ms", "30000");
        client_config.set("message.timeout.ms", "300000");
        client_config.set("socket.timeout.ms", "60000");
        client_config
    }

    pub async fn list_topics(
        &self,
        cluster_id: &str,
        config: &crate::config::Config,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("client.id", "mayyam-admin");

        let admin: AdminClient<_> = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka admin client: {}", e)))?;

        let timeout = Duration::from_secs(30);
        let metadata = admin
            .inner()
            .fetch_metadata(None, timeout)
            .map_err(|e| AppError::ExternalService(format!("Failed to fetch topic metadata: {}", e)))?;

        let topics = metadata
            .topics()
            .iter()
            .filter(|topic| !topic.name().starts_with("__"))
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

    pub async fn create_topic(
        &self,
        cluster_id: &str,
        topic: &KafkaTopic,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);

        let admin: AdminClient<_> = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka admin client: {}", e)))?;

        let new_topic = NewTopic::new(
            &topic.name,
            topic.partitions,
            TopicReplication::Fixed(topic.replication_factor as i32),
        );

        let new_topic = if let Some(configs) = &topic.configs {
            let mut nt = new_topic;
            for (key, value) in configs {
                nt = nt.set(key, value);
            }
            nt
        } else {
            new_topic
        };

        let opts = AdminOptions::new().operation_timeout(Some(Duration::from_secs(30)));
        let results = admin
            .create_topics(&[new_topic], &opts)
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to create topic: {}", e)))?;

        match results.first() {
            Some(Ok(name)) => Ok(serde_json::json!({
                "name": name,
                "partitions": topic.partitions,
                "replication_factor": topic.replication_factor,
                "message": "Topic creation initiated successfully"
            })),
            Some(Err((name, code))) => Err(AppError::ExternalService(format!(
                "Failed to create topic '{}': {:?}", name, code
            ))),
            None => Err(AppError::ExternalService("Empty result from create_topics".to_string())),
        }
    }

    pub async fn get_topic_details(
        &self,
        cluster_id: &str,
        topic_name: &str,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);

        let timeout = Duration::from_secs(30);
        let producer: FutureProducer = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka producer: {}", e)))?;

        let md = producer
            .client()
            .fetch_metadata(Some(topic_name), timeout)
            .map_err(|e| AppError::ExternalService(format!("Failed to fetch metadata for topic {}: {}", topic_name, e)))?;

        let topic_md = md
            .topics()
            .iter()
            .find(|t| t.name() == topic_name)
            .ok_or_else(|| AppError::NotFound(format!("Topic {} not found", topic_name)))?;

        let mut partitions = Vec::new();
        for p in topic_md.partitions() {
            let (low, high) = producer
                .client()
                .fetch_watermarks(topic_name, p.id(), timeout)
                .unwrap_or((0, 0));
            let replicas: Vec<i32> = p.replicas().to_vec();
            let isr: Vec<i32> = p.isr().to_vec();
            partitions.push(serde_json::json!({
                "id": p.id(),
                "leader": p.leader(),
                "replicas": replicas,
                "isr": isr,
                "offsets": {"earliest": low, "latest": high}
            }));
        }

        Ok(serde_json::json!({
            "name": topic_name,
            "partitions": partitions,
        }))
    }

    pub async fn delete_topic(
        &self,
        cluster_id: &str,
        topic_name: &str,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);
        let admin: AdminClient<_> = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka admin client: {}", e)))?;

        let opts = AdminOptions::new().operation_timeout(Some(Duration::from_secs(30)));
        let results = admin
            .delete_topics(&[topic_name], &opts)
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to delete topic: {}", e)))?;

        match results.first() {
            Some(Ok(name)) => Ok(serde_json::json!({
                "message": format!("Topic {} deleted successfully", name)
            })),
            Some(Err((name, code))) => Err(AppError::ExternalService(format!(
                "Failed to delete topic '{}': {:?}", name, code
            ))),
            None => Err(AppError::ExternalService("Empty result from delete_topics".to_string())),
        }
    }

    // Placeholder: update a topic's config (validate_only is accepted but not used yet)
    pub async fn update_topic_config(
        &self,
        cluster_id: &str,
        topic_name: &str,
        configs: Option<Vec<(String, String)>>,
        _validate_only: bool,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);

        let admin: AdminClient<_> = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka admin client: {}", e)))?;

        let pairs = configs.unwrap_or_default();
        if pairs.is_empty() {
            return Err(AppError::BadRequest("No configs provided to update".to_string()));
        }

        // Build AlterConfig for the topic
        let mut ac = AlterConfig::new(ResourceSpecifier::Topic(topic_name));
        for (k, v) in pairs.iter() {
            ac = ac.set(k, v);
        }

        let opts = AdminOptions::new()
            .request_timeout(Some(Duration::from_secs(30)))
            .validate_only(_validate_only);

        let results = admin
            .alter_configs([&ac], &opts)
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to alter topic config: {}", e)))?;

        match results.first() {
            Some(Ok(spec)) => Ok(serde_json::json!({
                "resource": format!("{:?}", spec),
                "topic": topic_name,
                "validate_only": _validate_only,
                "message": if _validate_only { "Validation successful" } else { "Config update initiated" }
            })),
            Some(Err((spec, code))) => Err(AppError::ExternalService(format!(
                "Failed to alter config for '{:?}': {:?}", spec, code
            ))),
            None => Err(AppError::ExternalService("Empty result from alter_configs".to_string())),
        }
    }

    // Update cluster config stored in repository (supports cluster ID UUID or name)
    pub async fn update_cluster_config(
        &self,
        cluster_id: &str,
        update: &serde_json::Value,
        _config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        use uuid::Uuid;
        use sea_orm::JsonValue;

        // Resolve cluster by UUID or by name
        let cluster_model = if let Ok(uuid) = Uuid::parse_str(cluster_id) {
            self.cluster_repository
                .find_by_id(uuid)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Cluster not found with ID: {}", cluster_id)))?
        } else {
            self.cluster_repository
                .find_by_name(cluster_id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Cluster not found with name: {}", cluster_id)))?
        };

        // Merge provided fields into existing config JSON
        let mut cfg_val: serde_json::Value = cluster_model.config.clone().into();
        if !cfg_val.is_object() {
            cfg_val = serde_json::json!({});
        }
        let cfg_obj = cfg_val.as_object_mut().unwrap();

        // Only update known Kafka fields if present in update JSON
        if let Some(v) = update.get("bootstrap_servers") {
            cfg_obj.insert("bootstrap_servers".to_string(), v.clone());
        }
        if let Some(v) = update.get("sasl_username") {
            cfg_obj.insert("sasl_username".to_string(), v.clone());
        }
        if let Some(v) = update.get("sasl_password") {
            cfg_obj.insert("sasl_password".to_string(), v.clone());
        }
        if let Some(v) = update.get("sasl_mechanism") {
            cfg_obj.insert("sasl_mechanism".to_string(), v.clone());
        }
        if let Some(v) = update.get("security_protocol") {
            cfg_obj.insert("security_protocol".to_string(), v.clone());
        }

        // Determine new name (optional)
        let new_name = update
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&cluster_model.name)
            .to_string();

        // Persist using repository
        let updated = self
            .cluster_repository
            .update(cluster_model.id, &new_name, JsonValue::from(cfg_val))
            .await?;

        Ok(serde_json::json!({
            "id": updated.id,
            "name": updated.name,
            "cluster_type": updated.cluster_type,
            "config": updated.config,
            "message": "Cluster config updated"
        }))
    }

    // Add partitions to an existing topic
    pub async fn add_topic_partitions(
        &self,
        cluster_id: &str,
        topic_name: &str,
        new_total_count: i32,
        _validate_only: bool,
        config: &crate::config::Config,
    ) -> Result<serde_json::Value, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let client_config = self.build_client_config(&cluster);
        let admin: AdminClient<_> = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka admin client: {}", e)))?;

        let opts = AdminOptions::new().operation_timeout(Some(Duration::from_secs(30)));
        if new_total_count < 0 {
            return Err(AppError::BadRequest("new_total_count must be non-negative".to_string()));
        }
        let new_total_count_usize = new_total_count as usize;
        let part = NewPartitions::new(topic_name, new_total_count_usize);
        let results = admin
            .create_partitions(&[part], &opts)
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to add partitions: {}", e)))?;

        match results.first() {
            Some(Ok(name)) => Ok(serde_json::json!({
                "message": format!("Partitions update for {} initiated", name),
                "new_total_partitions": new_total_count,
            })),
            Some(Err((name, code))) => Err(AppError::ExternalService(format!(
                "Failed to add partitions for '{}': {:?}", name, code
            ))),
            None => Err(AppError::ExternalService("Empty result from create_partitions".to_string())),
        }
    }

    // Return broker status info
    pub async fn get_broker_status(
        &self,
        cluster_id: &str,
        config: &crate::config::Config,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let cluster = self.get_cluster(cluster_id, config).await?;
        let mut client_config = self.build_client_config(&cluster);
        client_config.set("client.id", "mayyam-broker-status");

        let producer: FutureProducer = client_config
            .create()
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kafka producer: {}", e)))?;

        let timeout = Duration::from_secs(10);
        let metadata = producer
            .client()
            .fetch_metadata(None, timeout)
            .map_err(|e| AppError::ExternalService(format!("Failed to fetch cluster metadata: {:?}", e)))?;

        let brokers = metadata
            .brokers()
            .iter()
            .map(|b| serde_json::json!({
                "id": b.id(),
                "host": b.host(),
                "port": b.port()
            }))
            .collect::<Vec<_>>();
        Ok(brokers)
    }
}
