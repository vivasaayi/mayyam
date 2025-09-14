use aws_sdk_cloudwatch::Client as CloudWatchClient;
use aws_sdk_cloudwatchlogs::Client as CloudWatchLogsClient;
use aws_sdk_costexplorer::Client as CostExplorerClient;
use aws_sdk_ec2::Client as Ec2Client;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_rds::Client as RdsClient;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_kinesis::Client as KinesisClient;
use aws_sdk_sqs::Client as SqsClient;
use aws_sdk_sns::Client as SnsClient;
use aws_sdk_lambda::Client as LambdaClient;
use aws_sdk_elasticache::Client as ElasticacheClient;
use aws_sdk_opensearch::Client as OpenSearchClient;
use aws_sdk_sts::Client as StsClient;
use async_trait::async_trait;

use crate::{errors::AppError, config::AwsConfig};
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_auth::AccountAuthInfo;

// Client factory trait for AWS service clients
#[async_trait]
pub trait AwsClientFactory {
    async fn create_cloudwatch_client(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<CloudWatchClient, AppError>;    
    async fn create_cloudwatch_logs_client(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<CloudWatchLogsClient, AppError>;
    async fn create_cost_explorer_client(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<CostExplorerClient, AppError>;
    async fn create_ec2_client(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<Ec2Client, AppError>;
    async fn create_s3_client(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<S3Client, AppError>;
    async fn create_rds_client(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<RdsClient, AppError>;
    async fn create_dynamodb_client(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<DynamoDbClient, AppError>;
    async fn create_kinesis_client(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<KinesisClient, AppError>;
    async fn create_sqs_client(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<SqsClient, AppError>;
    async fn create_sns_client(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<SnsClient, AppError>;
    async fn create_lambda_client(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<LambdaClient, AppError>;
    async fn create_elasticache_client(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<ElasticacheClient, AppError>;
    async fn create_opensearch_client(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<OpenSearchClient, AppError>;
    async fn create_sts_client(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<StsClient, AppError>;
}
