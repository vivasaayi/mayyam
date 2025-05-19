use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use aws_sdk_s3::Client as S3Client;
use crate::errors::AppError;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::models::aws_auth::AccountAuthInfo;
use super::{AwsService, CloudWatchMetricsRequest, CloudWatchMetricsResult};
use super::client_factory::AwsClientFactory;

// S3-specific types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3BucketInfo {
    pub bucket_name: String,
    pub creation_date: String,
    pub region: String,
    pub versioning_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3GetObjectRequest {
    pub bucket: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3PutObjectRequest {
    pub bucket: String,
    pub key: String,
    pub content_type: Option<String>,
    pub body: String,
}

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

// Data plane implementation for S3
pub struct S3DataPlane {
    aws_service: Arc<AwsService>,
}

impl S3DataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn get_object(&self, profile: Option<&str>, region: &str, request: &S3GetObjectRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_s3_client(profile, region).await?;
        
        // In a real implementation, this would call get_object
        let response = json!({
            "body": "This is sample content for the S3 object",
            "content_type": "text/plain",
            "content_length": 38,
            "last_modified": chrono::Utc::now().to_rfc3339(),
            "etag": "\"abcdef1234567890\"",
            "metadata": {
                "custom-key": "custom-value"
            }
        });
        
        Ok(response)
    }

    pub async fn put_object(&self, profile: Option<&str>, region: &str, request: &S3PutObjectRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_s3_client(profile, region).await?;
        
        // In a real implementation, this would call put_object
        let response = json!({
            "etag": "\"abcdef1234567890\"",
            "version_id": null,
            "content_length": request.body.len(),
            "content_type": request.content_type.clone().unwrap_or_else(|| "application/octet-stream".to_string())
        });
        
        Ok(response)
    }

    pub async fn get_bucket_metrics(&self, request: &CloudWatchMetricsRequest) -> Result<CloudWatchMetricsResult, AppError> {
        let client = self.aws_service.create_cloudwatch_client(None, &request.region).await?;
        
        // S3-specific metric collection logic would go here
        // For now returning empty result
        Ok(CloudWatchMetricsResult {
            resource_id: request.resource_id.clone(),
            resource_type: request.resource_type.clone(),
            metrics: vec![],
        })
    }
}
