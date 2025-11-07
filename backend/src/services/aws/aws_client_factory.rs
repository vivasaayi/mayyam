use async_trait::async_trait;
use aws_sdk_apigateway::Client as ApiGatewayClient;
use aws_sdk_cloudfront::Client as CloudFrontClient;
use aws_sdk_elb::Client as ElbClient;
use aws_sdk_elbv2::Client as Elbv2Client;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_ec2::Client as Ec2Client;
use aws_sdk_elasticache::Client as ElasticacheClient;
use aws_sdk_kinesis::Client as KinesisClient;
use aws_sdk_lambda::Client as LambdaClient;
use aws_sdk_opensearch::Client as OpenSearchClient;
use aws_sdk_rds::Client as RdsClient;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_sns::Client as SnsClient;
use aws_sdk_sqs::Client as SqsClient;
use aws_sdk_sts::Client as StsClient;

use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_auth::AccountAuthInfo;
use crate::{config::AwsConfig, errors::AppError};

// Client factory trait for AWS service clients
#[async_trait]
pub trait AwsClientFactory {
    async fn create_cloudwatch_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<CloudWatchClient, AppError>;
    async fn create_cloudwatch_logs_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<CloudWatchLogsClient, AppError>;
    async fn create_cost_explorer_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<CostExplorerClient, AppError>;
    async fn create_ec2_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<Ec2Client, AppError>;
    async fn create_s3_client(&self, aws_account_dto: &AwsAccountDto)
        -> Result<S3Client, AppError>;
    async fn create_rds_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<RdsClient, AppError>;
    async fn create_dynamodb_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<DynamoDbClient, AppError>;
    async fn create_kinesis_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<KinesisClient, AppError>;
    async fn create_sqs_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<SqsClient, AppError>;
    async fn create_sns_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<SnsClient, AppError>;
    async fn create_lambda_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<LambdaClient, AppError>;
    async fn create_elasticache_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<ElasticacheClient, AppError>;
    async fn create_opensearch_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<OpenSearchClient, AppError>;
    async fn create_sts_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<StsClient, AppError>;
    async fn create_elbv2_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<Elbv2Client, AppError>;
    async fn create_elb_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<ElbClient, AppError>;
    async fn create_cloudfront_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<CloudFrontClient, AppError>;
    async fn create_api_gateway_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<ApiGatewayClient, AppError>;
}
