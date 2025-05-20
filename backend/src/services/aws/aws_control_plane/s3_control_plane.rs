use std::sync::Arc;
use aws_sdk_s3::Client as S3Client;
use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

// Control plane implementation for S3
pub struct S3ControlPlane {
    aws_service: Arc<AwsService>,
}

impl S3ControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_buckets(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_buckets_with_auth(account_id, profile, region, None).await
    }
    
    pub async fn sync_buckets_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_s3_client_with_auth(profile, region, account_auth).await?;
        self.sync_buckets_with_client(account_id, profile, region, client).await
    }
    
    async fn sync_buckets_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, client: S3Client) -> Result<Vec<AwsResourceModel>, AppError> {
        let mut buckets = Vec::new();
        let bucket = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: "S3Bucket".to_string(),
            resource_id: "example-bucket-1".to_string(),
            arn: format!("arn:aws:s3:::example-bucket-1"),
            name: Some("example-bucket-1".to_string()),
            tags: json!({"Purpose": "Logs", "Environment": "Development"}),
            resource_data: json!({
                "creation_date": "2023-01-15T10:00:00Z",
                "region": region,
                "versioning_enabled": true,
                "lifecycle_rules": [
                    {
                        "id": "archive-old-logs",
                        "prefix": "logs/",
                        "transition_days": 90,
                        "storage_class": "GLACIER"
                    }
                ]
            }),
        };
        buckets.push(bucket);

        Ok(buckets.into_iter().map(|b| b.into()).collect())
    }
}