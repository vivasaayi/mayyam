use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use aws_sdk_elasticache::Client as ElasticacheClient;
use crate::errors::AppError;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::models::aws_auth::AccountAuthInfo;
use super::{AwsService, CloudWatchMetricsRequest, CloudWatchMetricsResult};
use super::client_factory::AwsClientFactory;

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
        self.sync_clusters_with_client(account_id, profile, region).await
    }

    async fn sync_clusters_with_client(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        let mut clusters = Vec::new();
        let cluster = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: "ElasticacheCluster".to_string(),
            resource_id: "sample-redis".to_string(),
            arn: format!("arn:aws:elasticache:{}:{}:cluster:sample-redis", region, account_id),
            name: Some("Sample Redis Cluster".to_string()),
            tags: json!({"Name": "Redis Cluster", "Environment": "Development"}),
            resource_data: json!({
                "engine": "redis",
                "engine_version": "6.x",
                "cache_node_type": "cache.t3.micro",
                "num_cache_nodes": 1,
                "port": 6379,
                "status": "available",
                "preferred_maintenance_window": "sun:05:00-sun:06:00",
                "snapshot_retention_limit": 0,
                "snapshot_window": "05:00-09:00",
                "auth_token_enabled": false,
                "transit_encryption_enabled": false,
                "at_rest_encryption_enabled": false,
                "auto_minor_version_upgrade": true
            }),
        };
        clusters.push(cluster);

        Ok(clusters.into_iter().map(|c| c.into()).collect())
    }
}

pub struct ElasticacheDataPlane {
    aws_service: Arc<AwsService>,
}

impl ElasticacheDataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn get_cluster_metrics(&self, request: &CloudWatchMetricsRequest) -> Result<CloudWatchMetricsResult, AppError> {
        let client = self.aws_service.create_cloudwatch_client(None, &request.region).await?;
        
        // ElastiCache-specific metric collection logic would go here
        // For now returning empty result
        Ok(CloudWatchMetricsResult {
            resource_id: request.resource_id.clone(),
            resource_type: request.resource_type.clone(),
            metrics: vec![],
        })
    }
}
