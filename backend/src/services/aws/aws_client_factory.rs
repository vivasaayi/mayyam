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
use crate::models::aws_auth::AccountAuthInfo;

// Client factory trait for AWS service clients
#[async_trait]
pub trait AwsClientFactory {
    async fn get_aws_config(&self, profile: Option<&str>, region: &str) -> Result<AwsConfig, AppError>;
    async fn get_aws_config_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<AwsConfig, AppError>;

    // Core AWS service client creation methods
    async fn create_cloudwatch_client(&self, profile: Option<&str>, region: &str) -> Result<CloudWatchClient, AppError>;
    async fn create_cloudwatch_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<CloudWatchClient, AppError>;
    
    // CloudWatch Logs client creation methods
    async fn create_cloudwatch_logs_client(&self, profile: Option<&str>, region: &str) -> Result<CloudWatchLogsClient, AppError>;
    async fn create_cloudwatch_logs_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<CloudWatchLogsClient, AppError>;

    async fn create_cost_explorer_client(&self, profile: Option<&str>, region: &str) -> Result<CostExplorerClient, AppError>;
    async fn create_cost_explorer_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<CostExplorerClient, AppError>;

    async fn create_ec2_client(&self, profile: Option<&str>, region: &str) -> Result<Ec2Client, AppError>;
    async fn create_ec2_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Ec2Client, AppError>;

    async fn create_s3_client(&self, profile: Option<&str>, region: &str) -> Result<S3Client, AppError>;
    async fn create_s3_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<S3Client, AppError>;

    async fn create_rds_client(&self, profile: Option<&str>, region: &str) -> Result<RdsClient, AppError>;
    async fn create_rds_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<RdsClient, AppError>;

    async fn create_dynamodb_client(&self, profile: Option<&str>, region: &str) -> Result<DynamoDbClient, AppError>;
    async fn create_dynamodb_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<DynamoDbClient, AppError>;

    async fn create_kinesis_client(&self, profile: Option<&str>, region: &str) -> Result<KinesisClient, AppError>;
    async fn create_kinesis_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<KinesisClient, AppError>;

    async fn create_sqs_client(&self, profile: Option<&str>, region: &str) -> Result<SqsClient, AppError>;
    async fn create_sqs_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<SqsClient, AppError>;

    async fn create_sns_client(&self, profile: Option<&str>, region: &str) -> Result<SnsClient, AppError>;
    async fn create_sns_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<SnsClient, AppError>;

    async fn create_lambda_client(&self, profile: Option<&str>, region: &str) -> Result<LambdaClient, AppError>;
    async fn create_lambda_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<LambdaClient, AppError>;

    async fn create_elasticache_client(&self, profile: Option<&str>, region: &str) -> Result<ElasticacheClient, AppError>;
    async fn create_elasticache_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<ElasticacheClient, AppError>;

    async fn create_opensearch_client(&self, profile: Option<&str>, region: &str) -> Result<OpenSearchClient, AppError>;
    async fn create_opensearch_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<OpenSearchClient, AppError>;

    async fn create_sts_client(&self, profile: Option<&str>, region: &str) -> Result<StsClient, AppError>;
    async fn create_sts_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<StsClient, AppError>;

    // Generic client creation method for any AWS service client that can be created from AwsConfig
    async fn create_client_with_auth<C>(&self, profile: Option<&str>, region: &str) -> Result<C, AppError> 
    where 
        C: From<AwsConfig>;
}
