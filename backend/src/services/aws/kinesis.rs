use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use aws_sdk_kinesis::Client as KinesisClient;
use crate::errors::AppError;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::models::aws_auth::AccountAuthInfo;
use super::{AwsService, CloudWatchMetricsRequest, CloudWatchMetricsResult};
use super::client_factory::AwsClientFactory;

// Kinesis-specific types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisStreamInfo {
    pub stream_name: String,
    pub stream_status: String,
    pub retention_period_hours: i32,
    pub shard_count: i32,
    pub enhanced_monitoring: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisPutRecordRequest {
    pub stream_name: String,
    pub data: String,
    pub partition_key: String,
    pub sequence_number: Option<String>,
}

// Control plane implementation for Kinesis
pub struct KinesisControlPlane {
    aws_service: Arc<AwsService>,
}

impl KinesisControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_streams(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_streams_with_auth(account_id, profile, region, None).await
    }
    
    pub async fn sync_streams_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_kinesis_client_with_auth(profile, region, account_auth).await?;
        self.sync_streams_with_client(account_id, profile, region, client).await
    }
    
    async fn sync_streams_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, client: KinesisClient) -> Result<Vec<AwsResourceModel>, AppError> {
        let mut streams = Vec::new();
        let stream = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: "KinesisStream".to_string(),
            resource_id: "sample-data-stream".to_string(),
            arn: format!("arn:aws:kinesis:{}:{}:stream/sample-data-stream", region, account_id),
            name: Some("Sample Data Stream".to_string()),
            tags: json!({"Name": "Sample Data Stream", "Environment": "Development"}),
            resource_data: json!({
                "stream_name": "sample-data-stream",
                "stream_status": "ACTIVE",
                "retention_period_hours": 24,
                "shard_count": 2,
                "enhanced_monitoring": ["ALL"],
                "encryption_type": "KMS",
                "creation_timestamp": "2023-02-15T09:30:00Z",
                "open_shard_count": 2
            }),
        };
        streams.push(stream);

        Ok(streams.into_iter().map(|s| s.into()).collect())
    }
}

// Data plane implementation for Kinesis
pub struct KinesisDataPlane {
    aws_service: Arc<AwsService>,
}

impl KinesisDataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn put_record(&self, profile: Option<&str>, region: &str, request: &KinesisPutRecordRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;
        
        // In a real implementation, this would call put_record
        let response = json!({
            "sequence_number": "49613369067872193874107527441867152207618406126793392130",
            "shard_id": "shardId-000000000000",
            "encryption_type": "KMS"
        });
        
        Ok(response)
    }

    pub async fn get_stream_metrics(&self, request: &CloudWatchMetricsRequest) -> Result<CloudWatchMetricsResult, AppError> {
        let client = self.aws_service.create_cloudwatch_client(None, &request.region).await?;
        
        // Kinesis-specific metric collection logic would go here
        // For now returning empty result
        Ok(CloudWatchMetricsResult {
            resource_id: request.resource_id.clone(),
            resource_type: request.resource_type.clone(),
            metrics: vec![],
        })
    }

    // Additional Kinesis-specific data plane operations would go here
    // For example:
    // - get_records
    // - list_shards
    // - merge_shards
    // - split_shard
}
