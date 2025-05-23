use std::sync::Arc;
use aws_sdk_elasticache::Client as ElasticacheClient;
use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

pub struct ElasticacheControlPlane {
    aws_service: Arc<AwsService>,
}

impl ElasticacheControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_clusters(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_clusters_with_auth(account_id, profile, region, None).await
    }

    pub async fn sync_clusters_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_elasticache_client_with_auth(profile, region, account_auth).await?;
        self.sync_clusters_with_client(account_id, profile, region, client).await
    }

    async fn sync_clusters_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, client: ElasticacheClient) -> Result<Vec<AwsResourceModel>, AppError> {
        // Get ElastiCache clusters from AWS
        let response = client.describe_cache_clusters()
            .show_cache_node_info(true)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to describe ElastiCache clusters: {}", e)))?;
            
        let mut clusters = Vec::new();
        
        if let Some(cache_clusters) = response.cache_clusters() {
            for cache_cluster in cache_clusters {
                let cluster_id = cache_cluster.cache_cluster_id().unwrap_or_default();
                
                // Get ARN for the cluster
                let arn = format!("arn:aws:elasticache:{}:{}:cluster:{}", region, account_id, cluster_id);
                
                // Get tags for the cluster
                let tags_response = client.list_tags_for_resource()
                    .resource_name(&arn)
                    .send()
                    .await
                    .map_err(|e| AppError::ExternalService(format!("Failed to get tags for ElastiCache cluster {}: {}", cluster_id, e)))?;
                    
                let mut tags_map = serde_json::Map::new();
                let mut name = None;
                
                if let Some(tag_list) = tags_response.tag_list() {
                    for tag in tag_list {
                        if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                            if key == "Name" {
                                name = Some(value.to_string());
                            }
                            tags_map.insert(key.to_string(), json!(value));
                        }
                    }
                }
                
                // If no name tag was found, use the cluster ID as name
                if name.is_none() {
                    name = Some(cluster_id.to_string());
                }
                
                // Build resource data
                let mut resource_data = serde_json::Map::new();
                
                if let Some(engine) = cache_cluster.engine() {
                    resource_data.insert("engine".to_string(), json!(engine));
                }
                
                if let Some(version) = cache_cluster.engine_version() {
                    resource_data.insert("engine_version".to_string(), json!(version));
                }
                
                if let Some(node_type) = cache_cluster.cache_node_type() {
                    resource_data.insert("cache_node_type".to_string(), json!(node_type));
                }
                
                if let Some(node_count) = cache_cluster.num_cache_nodes() {
                    resource_data.insert("num_cache_nodes".to_string(), json!(node_count));
                }
                
                // Handle port from configuration endpoint or first node
                let mut port_added = false;
                if let Some(config_endpoint) = cache_cluster.configuration_endpoint() {
                    resource_data.insert("port".to_string(), json!(config_endpoint.port()));
                    port_added = true;
                } 
                
                if !port_added {
                    if let Some(nodes) = cache_cluster.cache_nodes() {
                        if let Some(first_node) = nodes.first() {
                            if let Some(endpoint) = first_node.endpoint() {
                                resource_data.insert("port".to_string(), json!(endpoint.port()));
                            }
                        }
                    }
                }
                
                if let Some(status) = cache_cluster.cache_cluster_status() {
                    resource_data.insert("status".to_string(), json!(status));
                }
                
                if let Some(window) = cache_cluster.preferred_maintenance_window() {
                    resource_data.insert("preferred_maintenance_window".to_string(), json!(window));
                }
                
                if let Some(retention) = cache_cluster.snapshot_retention_limit() {
                    resource_data.insert("snapshot_retention_limit".to_string(), json!(retention));
                }
                
                if let Some(window) = cache_cluster.snapshot_window() {
                    resource_data.insert("snapshot_window".to_string(), json!(window));
                }
                
                if let Some(encryption) = cache_cluster.at_rest_encryption_enabled() {
                    resource_data.insert("at_rest_encryption_enabled".to_string(), json!(encryption));
                }
                
                if let Some(encryption) = cache_cluster.transit_encryption_enabled() {
                    resource_data.insert("transit_encryption_enabled".to_string(), json!(encryption));
                }
                
                if let Some(auth) = cache_cluster.auth_token_enabled() {
                    resource_data.insert("auth_token_enabled".to_string(), json!(auth));
                }
                
                resource_data.insert("auto_minor_version_upgrade".to_string(), json!(cache_cluster.auto_minor_version_upgrade()));
                
                // Create resource DTO
                let cluster = AwsResourceDto {
                    id: None,
                    account_id: account_id.to_string(),
                    profile: profile.map(|p| p.to_string()),
                    region: region.to_string(),
                    resource_type: "ElasticacheCluster".to_string(),
                    resource_id: cluster_id.to_string(),
                    arn,
                    name,
                    tags: serde_json::Value::Object(tags_map),
                    resource_data: serde_json::Value::Object(resource_data),
                };
                
                clusters.push(cluster);
            }
        }

        Ok(clusters.into_iter().map(|c| c.into()).collect())
    }
}