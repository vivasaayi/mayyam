use std::sync::Arc;
use chrono::Utc;
use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, AwsResourceType, Model as AwsResourceModel};
use crate::services::aws::AwsService;

// Import control planes from their respective modules
use crate::services::aws::aws_control_plane::ec2_control_plane::Ec2ControlPlane;
use crate::services::aws::aws_control_plane::s3_control_plane::S3ControlPlane;
use crate::services::aws::aws_control_plane::rds_control_plane::RdsControlPlane;
use crate::services::aws::aws_control_plane::dynamodb_control_plane::DynamoDbControlPlane;
use crate::services::aws::aws_control_plane::kinesis_control_plane::KinesisControlPlane;
use crate::services::aws::aws_control_plane::sqs_control_plane::SqsControlPlane;
use crate::services::aws::aws_types::resource_sync::{ResourceSyncRequest, ResourceSyncResponse, ResourceTypeSyncSummary};
use crate::services::aws::aws_control_plane::elasticache_control_plane::ElasticacheControlPlane;

// Base control plane for AWS resources
pub struct AwsControlPlane {
    aws_service: Arc<AwsService>,
}

impl AwsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    fn to_aws_account_dto(&self, account_id: Option<&str>, profile: Option<&str>, region: Option<&str>) -> Result<AwsAccountDto, AppError> {
        Ok(AwsAccountDto {
            id: uuid::Uuid::nil(),
            account_id: account_id.unwrap_or_default().to_string(),
            account_name: "".to_string(),
            profile: profile.map(|p| p.to_string()),
            default_region: region.unwrap_or("us-east-1").to_string(),
            use_role: false,
            role_arn: Some("".to_string()),
            external_id: Some("".to_string()),
            has_access_key: false,
            access_key_id: Some("".to_string()),
            secret_access_key: Some("".to_string()),
            last_synced_at: Some(Utc::now()),
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        })
    }

    async fn sync_ec2_resources(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let ec2 = Ec2ControlPlane::new(self.aws_service.clone());
        ec2.sync_instances(account_id, profile, region).await
    }

    async fn sync_s3_buckets(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let s3 = S3ControlPlane::new(self.aws_service.clone());
        s3.sync_buckets(account_id, profile, region, account_auth).await
    }

    async fn sync_rds_resources(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let rds = RdsControlPlane::new(self.aws_service.clone());
        rds.sync_instances(account_id, profile, region, account_auth).await
    }

    async fn sync_dynamodb_resources(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let dynamodb = DynamoDbControlPlane::new(self.aws_service.clone());
        dynamodb.sync_tables(account_id, profile, region, account_auth).await
    }

    async fn sync_kinesis_resources(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        kinesis.sync_streams(account_id, profile, region, account_auth).await
    }

    async fn sync_sqs_resources(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let sqs = SqsControlPlane::new(self.aws_service.clone());
        sqs.sync_queues(account_id, profile, region, account_auth).await
    }

    async fn sync_elasticache_resources(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let elasticache = ElasticacheControlPlane::new(self.aws_service.clone());
        elasticache.sync_clusters(account_id, profile, region, account_auth).await
    }

    // Kinesis control plane operations
    pub async fn kinesis_create_stream(&self, profile: Option<&str>, region: &str, request: &crate::services::aws::aws_types::kinesis::KinesisCreateStreamRequest) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let response = kinesis.create_stream(self.to_aws_account_dto(profile, region), request).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_delete_stream(&self, profile: Option<&str>, region: Option<&str>, request: &crate::services::aws::aws_types::kinesis::KinesisDeleteStreamRequest) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let account_dto = self.to_aws_account_dto(None, profile, region).expect("Failed to create AWS account DTO");
        let response = kinesis.delete_stream(&account_dto, request).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_describe_stream(&self, profile: Option<&str>, region: Option<&str>, request: &crate::services::aws::aws_types::kinesis::KinesisDescribeStreamRequest) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let account_dto = self.to_aws_account_dto(None, profile, region).expect("Failed to create AWS account DTO");
        let response = kinesis.describe_stream(&account_dto, request).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_list_streams(&self, profile: Option<&str>, region: Option<&str>, request: &crate::services::aws::aws_types::kinesis::KinesisListStreamsRequest) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let account_dto = self.to_aws_account_dto(None, profile, region).expect("Failed to create AWS account DTO");
        let response = kinesis.list_streams(&account_dto, request).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_describe_limits(&self, profile: Option<&str>, region: Option<&str>) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let account_dto = self.to_aws_account_dto(None, profile, region).expect("Failed to create AWS account DTO");
        let response = kinesis.describe_limits(&account_dto).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_describe_stream_summary(&self, profile: Option<&str>, region: Option<&str>, stream_name: &str) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let account_dto = self.to_aws_account_dto(None, profile, region).expect("Failed to create AWS account DTO");
        let response = kinesis.describe_stream_summary(&account_dto, stream_name).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_update_shard_count(&self, profile: Option<&str>, region: Option<&str>, request: &crate::services::aws::aws_types::kinesis::KinesisUpdateShardCountRequest) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let account_dto = self.to_aws_account_dto(None, profile, region).expect("Failed to create AWS account DTO");
        let response = kinesis.update_shard_count(&account_dto, request).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_increase_retention_period(&self, profile: Option<&str>, region: Option<&str>, request: &crate::services::aws::aws_types::kinesis::KinesisRetentionPeriodRequest) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let account_dto = self.to_aws_account_dto(None, profile, region).expect("Failed to create AWS account DTO");
        let response = kinesis.increase_stream_retention_period(&account_dto, request).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_decrease_retention_period(&self, profile: Option<&str>, region: Option<&str>, request: &crate::services::aws::aws_types::kinesis::KinesisRetentionPeriodRequest) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let account_dto = self.to_aws_account_dto(None, profile, region).expect("Failed to create AWS account DTO");
        let response = kinesis.decrease_stream_retention_period(&account_dto, request).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_enable_enhanced_monitoring(&self, profile: Option<&str>, region: Option<&str>, request: &crate::services::aws::aws_types::kinesis::KinesisEnhancedMonitoringRequest) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let account_dto = self.to_aws_account_dto(None, profile, region).expect("Failed to create AWS account DTO");
        let response = kinesis.enable_enhanced_monitoring(&account_dto, request).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_disable_enhanced_monitoring(&self, profile: Option<&str>, region: Option<&str>, request: &crate::services::aws::aws_types::kinesis::KinesisEnhancedMonitoringRequest) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let account_dto = self.to_aws_account_dto(None, profile, region).expect("Failed to create AWS account DTO");
        let response = kinesis.disable_enhanced_monitoring(&account_dto, request).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_list_shards(&self, profile: Option<&str>, region: Option<&str>, request: &crate::services::aws::aws_types::kinesis::KinesisListShardsRequest) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let account_dto = self.to_aws_account_dto(None, profile, region).expect("Failed to create AWS account DTO");
        let response = kinesis.list_shards(&account_dto, request).await?;
        Ok(serde_json::to_value(response)?)
    }

    // Sync all resources for an account and region
    pub async fn sync_resources(&self, request: &ResourceSyncRequest) -> Result<ResourceSyncResponse, AppError> {
        let account_id = &request.account_id;
        let profile = request.profile.as_deref();
        let region = &request.region;
        let account_auth = AccountAuthInfo::from(request);

        let resource_types = match &request.resource_types {
            Some(types) => types.clone(),
            None => vec![
                AwsResourceType::EC2Instance.to_string(),
                AwsResourceType::S3Bucket.to_string(),
                AwsResourceType::RdsInstance.to_string(),
                AwsResourceType::DynamoDbTable.to_string(),
                AwsResourceType::KinesisStream.to_string(),
                AwsResourceType::SqsQueue.to_string(),
                AwsResourceType::ElasticacheCluster.to_string(),
            ],
        };
        
        let mut summary = Vec::new();
        let mut total_resources = 0;

        for resource_type in resource_types {
            // Note: The actual resource syncing will be handled by individual service modules
            // This provides the orchestration layer
            let result = match resource_type.as_str() {
                "EC2Instance" => self.sync_ec2_resources(account_id, profile, region, Some(&account_auth)).await,
                "S3Bucket" => self.sync_s3_buckets(account_id, profile, region, Some(&account_auth)).await,
                "RdsInstance" => self.sync_rds_resources(account_id, profile, region, Some(&account_auth)).await,
                "DynamoDbTable" => self.sync_dynamodb_resources(account_id, profile, region, Some(&account_auth)).await,
                "KinesisStream" => self.sync_kinesis_resources(account_id, profile, region, Some(&account_auth)).await,
                "SqsQueue" => self.sync_sqs_resources(account_id, profile, region, Some(&account_auth)).await,
                "ElasticacheCluster" => self.sync_elasticache_resources(account_id, profile, region, Some(&account_auth)).await,
                _ => Ok(vec![]),
            };

            match result {
                Ok(resources) => {
                    // Save resources to the database
                    for resource in &resources {
                        // Convert to DTO for persistence
                        let resource_dto = AwsResourceDto {
                            id: Some(resource.id),
                            account_id: resource.account_id.clone(),
                            profile: resource.profile.clone(),
                            region: resource.region.clone(),
                            resource_type: resource.resource_type.clone(),
                            resource_id: resource.resource_id.clone(),
                            arn: resource.arn.clone(),
                            name: resource.name.clone(),
                            tags: resource.tags.clone(),
                            resource_data: resource.resource_data.clone(),
                        };
                        
                        // Try to find an existing resource with this ARN
                        match self.aws_service.aws_resource_repo.find_by_arn(&resource.arn).await {
                            Ok(Some(existing)) => {
                                // Update existing resource
                                let _ = self.aws_service.aws_resource_repo.update(existing.id, &resource_dto).await;
                            },
                            _ => {
                                let _ = self.aws_service.aws_resource_repo.create(&resource_dto).await;
                            }
                        }
                    }
                    
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: resource_type.clone(),
                        count: resources.len(),
                        status: "success".to_string(),
                        details: None,
                    });
                    total_resources += resources.len();
                },
                Err(e) => {
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: resource_type.clone(),
                        count: 0,
                        status: "error".to_string(),
                        details: Some(e.to_string()),
                    });
                }
            }
        }

        Ok(ResourceSyncResponse {
            summary,
            total_resources,
            sync_time: Utc::now().to_rfc3339(),
        })
    }
}
