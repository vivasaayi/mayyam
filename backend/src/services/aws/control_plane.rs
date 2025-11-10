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
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_resource::{AwsResourceDto, AwsResourceType, Model as AwsResourceModel};
use crate::models::cloud_resource::CloudResourceDto;
use crate::services::aws::AwsService;
use aws_sdk_kinesis::types::StreamDescription;
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

// Import control planes from their respective modules
use crate::services::aws::aws_control_plane::api_gateway_control_plane::ApiGatewayControlPlane;
use crate::services::aws::aws_control_plane::cloudfront_control_plane::CloudFrontControlPlane;
use crate::services::aws::aws_control_plane::dynamodb_control_plane::DynamoDbControlPlane;
use crate::services::aws::aws_control_plane::ec2_control_plane::Ec2ControlPlane;
use crate::services::aws::aws_control_plane::ebs_control_plane::EbsControlPlane;
use crate::services::aws::aws_control_plane::efs_control_plane::EfsControlPlane;
use crate::services::aws::aws_control_plane::elasticache_control_plane::ElasticacheControlPlane;
use crate::services::aws::aws_control_plane::kinesis_control_plane::KinesisControlPlane;
use crate::services::aws::aws_control_plane::lambda_control_plane::LambdaControlPlane;
use crate::services::aws::aws_control_plane::load_balancer_control_plane::LoadBalancerControlPlane;
use crate::services::aws::aws_control_plane::rds_control_plane::RdsControlPlane;
use crate::services::aws::aws_control_plane::s3_control_plane::S3ControlPlane;
use crate::services::aws::aws_control_plane::sqs_control_plane::SqsControlPlane;
use crate::services::aws::aws_control_plane::vpc_control_plane::VpcControlPlane;
use crate::services::aws::aws_types::resource_sync::{
    ResourceSyncRequest, ResourceSyncResponse, ResourceTypeSyncSummary,
};

// Helper function to convert StreamDescription to JSON
fn stream_description_to_json(stream_desc: &StreamDescription) -> Value {
    json!({
        "stream_name": stream_desc.stream_name(),
        "stream_arn": stream_desc.stream_arn(),
        "stream_status": stream_desc.stream_status().as_str(),
        "stream_mode_details": {
            "stream_mode": stream_desc.stream_mode_details().map(|smd| smd.stream_mode().as_str())
        },
        "shards": stream_desc.shards().len(), // Just return count for now
        "has_more_shards": stream_desc.has_more_shards(),
        "retention_period_hours": stream_desc.retention_period_hours(),
        "stream_creation_timestamp": None::<String>,
        "enhanced_monitoring": stream_desc.enhanced_monitoring().len(), // Just return count for now
        "encryption_type": stream_desc.encryption_type().map(|e| e.as_str()),
        "key_id": stream_desc.key_id()
    })
}

// Base control plane for AWS resources
#[async_trait::async_trait]
pub trait AwsControlPlaneTrait: Send + Sync {
    async fn list_all_regions(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<Vec<String>, AppError>;
    async fn sync_resources(
        &self,
        request: &ResourceSyncRequest,
    ) -> Result<ResourceSyncResponse, AppError>;
}

pub struct AwsControlPlane {
    aws_service: Arc<AwsService>,
}

#[async_trait::async_trait]
impl AwsControlPlaneTrait for AwsControlPlane {
    async fn list_all_regions(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<Vec<String>, AppError> {
        self.aws_service.list_all_regions(aws_account_dto).await
    }

    async fn sync_resources(
        &self,
        request: &ResourceSyncRequest,
    ) -> Result<ResourceSyncResponse, AppError> {
        self.sync_resources(request).await
    }
}

impl AwsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    // Helper to expose region enumeration to callers without exposing inner service
    pub async fn list_all_regions(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<Vec<String>, AppError> {
        self.aws_service.list_all_regions(aws_account_dto).await
    }

    async fn sync_ec2_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing EC2 instances for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let ec2 = Ec2ControlPlane::new(self.aws_service.clone());
        ec2.sync_instances(&aws_account_dto, sync_id).await
    }

    async fn sync_s3_buckets(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing S3 buckets for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let s3 = S3ControlPlane::new(self.aws_service.clone());
        s3.sync_buckets(aws_account_dto, sync_id).await
    }

    async fn sync_rds_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing RDS instances for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let rds = RdsControlPlane::new(self.aws_service.clone());
        rds.sync_instances(&aws_account_dto, sync_id).await
    }

    async fn sync_dynamodb_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing DynamoDB tables for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let dynamodb = DynamoDbControlPlane::new(self.aws_service.clone());
        dynamodb.sync_tables(&aws_account_dto, sync_id).await
    }

    async fn sync_kinesis_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Kinesis streams for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        kinesis.sync_streams(&aws_account_dto, sync_id).await
    }

    async fn sync_sqs_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing SQS queues for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let sqs = SqsControlPlane::new(self.aws_service.clone());
        sqs.sync_queues(&aws_account_dto, sync_id).await
    }

    async fn sync_elasticache_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing ElastiCache clusters for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let elasticache = ElasticacheControlPlane::new(self.aws_service.clone());
        elasticache.sync_clusters(&aws_account_dto, sync_id).await
    }

    async fn sync_vpc_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing VPC resources for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let vpc = VpcControlPlane::new(self.aws_service.clone());

        let mut all_resources = Vec::new();

        // Sync VPCs
        match vpc.sync_vpcs(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync VPCs: {}", e),
        }

        // Sync Subnets
        match vpc.sync_subnets(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync Subnets: {}", e),
        }

        // Sync Security Groups
        match vpc.sync_security_groups(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync Security Groups: {}", e),
        }

        // Sync Internet Gateways
        match vpc.sync_internet_gateways(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync Internet Gateways: {}", e),
        }

        // Sync NAT Gateways
        match vpc.sync_nat_gateways(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync NAT Gateways: {}", e),
        }

        // Sync Route Tables
        match vpc.sync_route_tables(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync Route Tables: {}", e),
        }

        // Sync Network ACLs
        match vpc.sync_network_acls(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync Network ACLs: {}", e),
        }

        Ok(all_resources)
    }

    async fn sync_load_balancer_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Load Balancer resources for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let load_balancer = LoadBalancerControlPlane::new(self.aws_service.clone());

        let mut all_resources = Vec::new();

        // Sync ALBs
        match load_balancer.sync_albs(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync ALBs: {}", e),
        }

        // Sync NLBs
        match load_balancer.sync_nlbs(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync NLBs: {}", e),
        }

        // Sync ELBs
        match load_balancer.sync_elbs(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync ELBs: {}", e),
        }

        Ok(all_resources)
    }

    async fn sync_ebs_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing EBS resources for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let ebs = EbsControlPlane::new(self.aws_service.clone());

        let mut all_resources = Vec::new();

        // Sync EBS Volumes
        match ebs.sync_volumes(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync EBS volumes: {}", e),
        }

        // Sync EBS Snapshots
        match ebs.sync_snapshots(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync EBS snapshots: {}", e),
        }

        Ok(all_resources)
    }

    async fn sync_efs_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing EFS resources for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let efs = EfsControlPlane::new(self.aws_service.clone());

        // Sync EFS File Systems
        efs.sync_file_systems(aws_account_dto, sync_id).await
    }

    async fn sync_cloudfront_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing CloudFront resources for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let cloudfront = CloudFrontControlPlane::new(self.aws_service.clone());
        cloudfront.sync_distributions(aws_account_dto, sync_id).await
    }

    async fn sync_api_gateway_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing API Gateway resources for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let api_gateway = ApiGatewayControlPlane::new(self.aws_service.clone());

        let mut all_resources = Vec::new();

        // Sync REST APIs
        match api_gateway.sync_rest_apis(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync REST APIs: {}", e),
        }

        // Sync Stages
        match api_gateway.sync_stages(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync Stages: {}", e),
        }

        // Sync Resources
        match api_gateway.sync_resources(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync Resources: {}", e),
        }

        // Sync Methods
        match api_gateway.sync_methods(aws_account_dto, sync_id).await {
            Ok(resources) => all_resources.extend(resources),
            Err(e) => error!("Failed to sync Methods: {}", e),
        }

        Ok(all_resources)
    }

    // Kinesis control plane operations
    pub async fn kinesis_create_stream(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &crate::services::aws::aws_types::kinesis::KinesisCreateStreamRequest,
    ) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let response = kinesis.create_stream(&aws_account_dto, request).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_delete_stream(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &crate::services::aws::aws_types::kinesis::KinesisDeleteStreamRequest,
    ) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let response = kinesis.delete_stream(&aws_account_dto, request).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_describe_stream(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &crate::services::aws::aws_types::kinesis::KinesisDescribeStreamRequest,
    ) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let response = kinesis.describe_stream(&aws_account_dto, request).await?;
        Ok(stream_description_to_json(&response))
    }

    pub async fn kinesis_list_streams(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &crate::services::aws::aws_types::kinesis::KinesisListStreamsRequest,
    ) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let response = kinesis.list_streams(&aws_account_dto, request).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_describe_limits(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let response = kinesis.describe_limits(&aws_account_dto).await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_describe_stream_summary(
        &self,
        aws_account_dto: &AwsAccountDto,
        stream_name: &str,
    ) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let request = crate::services::aws::aws_types::kinesis::KinesisDescribeStreamRequest {
            stream_name: stream_name.to_string(),
        };
        let response = kinesis.describe_stream(&aws_account_dto, &request).await?;
        Ok(stream_description_to_json(&response))
    }

    pub async fn kinesis_update_shard_count(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &crate::services::aws::aws_types::kinesis::KinesisUpdateShardCountRequest,
    ) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let response = kinesis
            .update_shard_count(&aws_account_dto, request)
            .await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_increase_retention_period(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &crate::services::aws::aws_types::kinesis::KinesisRetentionPeriodRequest,
    ) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let response = kinesis
            .increase_stream_retention_period(&aws_account_dto, request)
            .await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_decrease_retention_period(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &crate::services::aws::aws_types::kinesis::KinesisRetentionPeriodRequest,
    ) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let response = kinesis
            .decrease_stream_retention_period(&aws_account_dto, request)
            .await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_enable_enhanced_monitoring(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &crate::services::aws::aws_types::kinesis::KinesisEnhancedMonitoringRequest,
    ) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let response = kinesis
            .enable_enhanced_monitoring(&aws_account_dto, request)
            .await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_disable_enhanced_monitoring(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &crate::services::aws::aws_types::kinesis::KinesisEnhancedMonitoringRequest,
    ) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let response = kinesis
            .disable_enhanced_monitoring(&aws_account_dto, request)
            .await?;
        Ok(serde_json::to_value(response)?)
    }

    pub async fn kinesis_list_shards(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &crate::services::aws::aws_types::kinesis::KinesisListShardsRequest,
    ) -> Result<serde_json::Value, AppError> {
        let kinesis = KinesisControlPlane::new(self.aws_service.clone());
        let response = kinesis.list_shards(&aws_account_dto, request).await?;
        Ok(serde_json::to_value(response)?)
    }

    // Sync all resources for an account and region
    pub async fn sync_resources(
        &self,
        request: &ResourceSyncRequest,
    ) -> Result<ResourceSyncResponse, AppError> {
        let region = &request.region;
        // Build a rich AwsAccountDto carrying account_id, auth and region from the request
        let aws_account = AwsAccountDto {
            id: Uuid::new_v4(),
            account_id: request.account_id.clone(),
            account_name: request
                .profile
                .clone()
                .unwrap_or_else(|| "aws-account".to_string()),
            profile: request.profile.clone(),
            default_region: region.clone(),
            regions: None,
            use_role: request.use_role,
            role_arn: request.role_arn.clone(),
            external_id: request.external_id.clone(),
            has_access_key: request.access_key_id.is_some(),
            access_key_id: request.access_key_id.clone(),
            secret_access_key: request.secret_access_key.clone(),
            auth_type: "auto".to_string(),
            source_profile: None,
            sso_profile: None,
            web_identity_token_file: None,
            session_name: None,
            last_synced_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let aws_account_dto = &aws_account;

        debug!(
            "Sync Request with sync_id {}: {:?}",
            request.sync_id, request
        );
        debug!(
            "Syncing resources for AWS account: {:?} with sync_id: {}",
            aws_account_dto, request.sync_id
        );

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
                AwsResourceType::Vpc.to_string(),
                AwsResourceType::Subnet.to_string(),
                AwsResourceType::SecurityGroup.to_string(),
                AwsResourceType::InternetGateway.to_string(),
                AwsResourceType::NatGateway.to_string(),
                AwsResourceType::RouteTable.to_string(),
                AwsResourceType::NetworkAcl.to_string(),
                AwsResourceType::Alb.to_string(),
                AwsResourceType::Nlb.to_string(),
                AwsResourceType::Elb.to_string(),
                AwsResourceType::CloudFrontDistribution.to_string(),
                AwsResourceType::ApiGatewayRestApi.to_string(),
                AwsResourceType::ApiGatewayStage.to_string(),
                AwsResourceType::ApiGatewayResource.to_string(),
                AwsResourceType::ApiGatewayMethod.to_string(),
                AwsResourceType::EbsVolume.to_string(),
                AwsResourceType::EbsSnapshot.to_string(),
                AwsResourceType::EfsFileSystem.to_string(),
            ],
        };

        let mut summary = Vec::new();
        let mut total_resources = 0;

        for resource_type in resource_types {
            // Note: The actual resource syncing will be handled by individual service modules
            // This provides the orchestration layer
            let result = match resource_type.as_str() {
                "EC2Instance" => {
                    self.sync_ec2_resources(aws_account_dto, request.sync_id)
                        .await
                }
                "S3Bucket" => self.sync_s3_buckets(aws_account_dto, request.sync_id).await,
                "RdsInstance" => {
                    self.sync_rds_resources(aws_account_dto, request.sync_id)
                        .await
                }
                "DynamoDbTable" => {
                    self.sync_dynamodb_resources(aws_account_dto, request.sync_id)
                        .await
                }
                "KinesisStream" => {
                    self.sync_kinesis_resources(aws_account_dto, request.sync_id)
                        .await
                }
                "SqsQueue" => {
                    self.sync_sqs_resources(aws_account_dto, request.sync_id)
                        .await
                }
                "ElasticacheCluster" => {
                    self.sync_elasticache_resources(aws_account_dto, request.sync_id)
                        .await
                }
                "Vpc" | "Subnet" | "SecurityGroup" | "InternetGateway" | "NatGateway" | "RouteTable" | "NetworkAcl" => {
                    self.sync_vpc_resources(aws_account_dto, request.sync_id)
                        .await
                }
                "LambdaFunction" => {
                    let lambda = LambdaControlPlane::new(self.aws_service.clone());
                    lambda
                        .sync_functions(aws_account_dto, request.sync_id)
                        .await
                }
                "Alb" | "Nlb" | "Elb" => {
                    self.sync_load_balancer_resources(aws_account_dto, request.sync_id)
                        .await
                }
                "EbsVolume" | "EbsSnapshot" => {
                    self.sync_ebs_resources(aws_account_dto, request.sync_id)
                        .await
                }
                "EfsFileSystem" => {
                    self.sync_efs_resources(aws_account_dto, request.sync_id)
                        .await
                }
                "CloudFrontDistribution" => {
                    self.sync_cloudfront_resources(aws_account_dto, request.sync_id)
                        .await
                }
                "ApiGatewayRestApi" | "ApiGatewayStage" | "ApiGatewayResource" | "ApiGatewayMethod" => {
                    self.sync_api_gateway_resources(aws_account_dto, request.sync_id)
                        .await
                }
                _ => Ok(vec![]),
            };

            match result {
                Ok(resources) => {
                    // Save resources to the database
                    for resource in &resources {
                        // Convert to DTO for persistence
                        let resource_dto = AwsResourceDto {
                            id: Some(resource.id),
                            sync_id: Some(request.sync_id),
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

                        // Always insert a new row per sync; schema prevents dupes per (sync_id, arn)
                        let _ = self
                            .aws_service
                            .aws_resource_repo
                            .create(&resource_dto)
                            .await;

                        // Dual-write to cloud_resources (unified table)
                        let cloud_dto = CloudResourceDto {
                            id: None,
                            sync_id: request.sync_id,
                            provider: "aws".to_string(),
                            account_id: resource.account_id.clone(),
                            region: resource.region.clone(),
                            resource_type: resource.resource_type.clone(),
                            resource_id: resource.resource_id.clone(),
                            arn_or_uri: Some(resource.arn.clone()),
                            name: resource.name.clone(),
                            tags: resource.tags.clone(),
                            resource_data: resource.resource_data.clone(),
                        };
                        let _ = self
                            .aws_service
                            .cloud_resource_repo
                            .create(&cloud_dto)
                            .await;
                    }

                    summary.push(ResourceTypeSyncSummary {
                        resource_type: resource_type.clone(),
                        count: resources.len(),
                        status: "success".to_string(),
                        details: None,
                    });
                    total_resources += resources.len();
                    info!(
                        "Successfully synced {} {} resources for sync_id: {}",
                        resources.len(),
                        resource_type,
                        request.sync_id
                    );
                }
                Err(e) => {
                    error!(
                        "Failed to sync {} resources for sync_id: {}: {}",
                        resource_type, request.sync_id, e
                    );
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: resource_type.clone(),
                        count: 0,
                        status: "error".to_string(),
                        details: Some(e.to_string()),
                    });
                }
            }
        }

        info!(
            "Completed sync for sync_id: {} - {} total resources synced",
            request.sync_id, total_resources
        );
        Ok(ResourceSyncResponse {
            summary,
            total_resources,
            sync_time: Utc::now().to_rfc3339(),
        })
    }
}
