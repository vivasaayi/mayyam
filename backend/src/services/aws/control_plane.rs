use std::sync::Arc;
use chrono::Utc;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceType, Model as AwsResourceModel, AwsResourceDto};
use crate::services::aws::{
    AwsService, 
    ResourceSyncRequest, 
    ResourceSyncResponse,
    ResourceTypeSyncSummary,
};

// Import control planes from their respective modules
use super::ec2::Ec2ControlPlane;
use super::s3::S3ControlPlane;
use crate::services::aws::aws_control_plane::rds_control_plane::RdsControlPlane;
use super::dynamodb::DynamoDbControlPlane;
use super::kinesis::KinesisControlPlane;
use crate::services::aws::aws_control_plane::sqs_control_plane::SqsControlPlane;
use super::elasticache::ElasticacheControlPlane;

// Base control plane for AWS resources
pub struct AwsControlPlane {
    aws_service: Arc<AwsService>,
}

impl AwsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    async fn sync_ec2_resources(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let ec2 = Ec2ControlPlane::new(self.aws_service.clone());
        ec2.sync_instances_with_auth(account_id, profile, region, account_auth).await
    }

    async fn sync_s3_buckets(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let s3 = S3ControlPlane::new(self.aws_service.clone());
        s3.sync_buckets_with_auth(account_id, profile, region, account_auth).await
    }

    async fn sync_rds_resources(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let rds = RdsControlPlane::new(self.aws_service.clone());
        rds.sync_instances_with_auth(account_id, profile, region, account_auth).await
    }

    async fn sync_dynamodb_resources(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let dynamodb = DynamoDbControlPlane::new(self.aws_service.clone());
        dynamodb.sync_tables_with_auth(account_id, profile, region, account_auth).await
    }

    async fn sync_kinesis_resources(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        kinesis.sync_streams_with_auth(account_id, profile, region, account_auth).await
    }

    async fn sync_sqs_resources(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let sqs = SqsControlPlane::new(self.aws_service.clone());
        sqs.sync_queues_with_auth(account_id, profile, region, account_auth).await
    }

    async fn sync_elasticache_resources(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let elasticache = ElasticacheControlPlane::new(self.aws_service.clone());
        elasticache.sync_clusters_with_auth(account_id, profile, region, account_auth).await
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
