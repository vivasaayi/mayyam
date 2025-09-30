use super::client_factory::AwsClientFactory;
use crate::api::routes::aws_account;
use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::services::aws::aws_data_plane::kinesis_data_plane::KinesisDataPlane;
use crate::services::aws::aws_types::cloud_watch::{
    CloudWatchMetricsRequest, CloudWatchMetricsResult,
};
use crate::services::aws::aws_types::kinesis::{
    KinesisGetRecordsRequest, KinesisGetRecordsResponse, KinesisGetShardIteratorRequest,
    KinesisGetShardIteratorResponse, KinesisPutRecordsRequest, KinesisPutRecordsResponse,
};
use crate::services::aws::AwsService;
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

// Base data plane for AWS resources
pub struct AwsDataPlane {
    aws_service: Arc<AwsService>,
}

impl AwsDataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    // CloudWatch metrics operation - this is a common operation that works across services
    pub async fn get_cloudwatch_metrics(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &CloudWatchMetricsRequest,
    ) -> Result<CloudWatchMetricsResult, AppError> {
        let client = self
            .aws_service
            .create_cloudwatch_client(aws_account_dto)
            .await?;

        let namespace = match request.resource_type.as_str() {
            "EC2Instance" => "AWS/EC2",
            "RdsInstance" => "AWS/RDS",
            "DynamoDbTable" => "AWS/DynamoDB",
            "KinesisStream" => "AWS/Kinesis",
            "SqsQueue" => "AWS/SQS",
            "ElasticacheCluster" => "AWS/ElastiCache",
            "SnsTopic" => "AWS/SNS",
            "LambdaFunction" => "AWS/Lambda",
            "OpenSearchDomain" => "AWS/ES",
            _ => {
                return Err(AppError::BadRequest(format!(
                    "Unsupported resource type: {}",
                    request.resource_type
                )))
            }
        };

        // Note: Actual metric collection will be delegated to individual service modules
        // This provides the base implementation and common functionality

        Ok(CloudWatchMetricsResult {
            resource_id: request.resource_id.clone(),
            resource_type: request.resource_type.clone(),
            metrics: vec![],
        })
    }

    // Kinesis Data Plane Operations
    pub async fn kinesis_put_records(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisPutRecordsRequest,
    ) -> Result<KinesisPutRecordsResponse, AppError> {
        let kinesis_data_plane = KinesisDataPlane::new(self.aws_service.clone());
        kinesis_data_plane
            .put_records(aws_account_dto, request)
            .await
    }

    pub async fn kinesis_get_records(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisGetRecordsRequest,
    ) -> Result<KinesisGetRecordsResponse, AppError> {
        let kinesis_data_plane = KinesisDataPlane::new(self.aws_service.clone());
        kinesis_data_plane
            .get_records(aws_account_dto, request)
            .await
    }

    pub async fn kinesis_get_shard_iterator(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisGetShardIteratorRequest,
    ) -> Result<KinesisGetShardIteratorResponse, AppError> {
        let kinesis_data_plane = KinesisDataPlane::new(self.aws_service.clone());
        kinesis_data_plane
            .get_shard_iterator(aws_account_dto, request)
            .await
    }
}
