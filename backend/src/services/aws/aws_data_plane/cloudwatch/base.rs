use std::sync::Arc;
use aws_sdk_cloudwatch::types::Dimension;
use crate::errors::AppError;
use crate::services::AwsService;
use crate::services::aws::client_factory::AwsClientFactory;

pub struct CloudWatchService {
    aws_service: Arc<AwsService>,
}

impl CloudWatchService {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    /// Helper method to determine the CloudWatch namespace for a resource type
    pub(crate) fn get_namespace_for_resource_type(&self, resource_type: &str) -> &str {
        match resource_type {
            "EC2Instance" => "AWS/EC2",
            "RdsInstance" => "AWS/RDS",
            "DynamoDbTable" => "AWS/DynamoDB",
            "KinesisStream" => "AWS/Kinesis",
            "SqsQueue" => "AWS/SQS",
            "ElasticacheCluster" => "AWS/ElastiCache",
            "SnsTopic" => "AWS/SNS",
            "LambdaFunction" => "AWS/Lambda",
            "S3Bucket" => "AWS/S3",
            "OpenSearchDomain" => "AWS/ES",
            _ => "AWS/CloudWatch", // Default namespace
        }
    }

    /// Helper method to create dimensions for a resource
    pub(crate) fn create_dimensions_for_resource(&self, resource_type: &str, resource_id: &str) -> Vec<Dimension> {
        match resource_type {
            "EC2Instance" => vec![
                Dimension::builder()
                    .name("InstanceId")
                    .value(resource_id)
                    .build()
            ],
            "RdsInstance" => vec![
                Dimension::builder()
                    .name("DBInstanceIdentifier")
                    .value(resource_id)
                    .build()
            ],
            "DynamoDbTable" => vec![
                Dimension::builder()
                    .name("TableName")
                    .value(resource_id)
                    .build()
            ],
            "KinesisStream" => vec![
                Dimension::builder()
                    .name("StreamName")
                    .value(resource_id)
                    .build()
            ],
            "SqsQueue" => vec![
                Dimension::builder()
                    .name("QueueName")
                    .value(resource_id)
                    .build()
            ],
            "ElasticacheCluster" => vec![
                Dimension::builder()
                    .name("CacheClusterId")
                    .value(resource_id)
                    .build()
            ],
            "S3Bucket" => vec![
                Dimension::builder()
                    .name("BucketName")
                    .value(resource_id)
                    .build()
            ],
            _ => Vec::new(), // Empty dimensions for unknown types
        }
    }

    pub(crate) async fn create_cloudwatch_client(
        &self,
        profile: Option<&str>,
        region: &str,
    ) -> Result<aws_sdk_cloudwatch::Client, AppError> {
        self.aws_service.create_cloudwatch_client(profile, region).await
    }

    pub(crate) async fn create_cloudwatch_logs_client(
        &self,
        profile: Option<&str>,
        region: &str,
    ) -> Result<aws_sdk_cloudwatchlogs::Client, AppError> {
        self.aws_service.create_cloudwatch_logs_client(profile, region).await
    }
}
