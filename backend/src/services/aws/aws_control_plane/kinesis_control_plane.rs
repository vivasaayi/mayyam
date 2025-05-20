use std::sync::Arc;
use aws_sdk_kinesis::Client as KinesisClient;
use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

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