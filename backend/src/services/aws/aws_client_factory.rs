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


use async_trait::async_trait;
use aws_sdk_apigateway::Client as ApiGatewayClient;
use aws_sdk_cloudfront::Client as CloudFrontClient;
use aws_sdk_elasticloadbalancing::Client as ElbClient;
use aws_sdk_elasticloadbalancingv2::Client as Elbv2Client;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_ec2::Client as Ec2Client;
use aws_sdk_efs::Client as EfsClient;
use aws_sdk_elasticache::Client as ElasticacheClient;
use aws_sdk_kinesis::Client as KinesisClient;
use aws_sdk_lambda::Client as LambdaClient;
use aws_sdk_opensearch::Client as OpenSearchClient;
use aws_sdk_rds::Client as RdsClient;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_cloudwatch::Client as CloudWatchClient;
use aws_sdk_cloudwatchlogs::Client as CloudWatchLogsClient;
use aws_sdk_costexplorer::Client as CostExplorerClient;
use aws_sdk_sns::Client as SnsClient;
use aws_sdk_sqs::Client as SqsClient;
use aws_sdk_sts::Client as StsClient;
use aws_sdk_iam::Client as IamClient;
use aws_sdk_kms::Client as KmsClient;
use aws_sdk_acm::Client as AcmClient;
use aws_sdk_cloudtrail::Client as CloudTrailClient;
use aws_sdk_config::Client as ConfigServiceClient;
use aws_sdk_ecs::Client as EcsClient;
use aws_sdk_eks::Client as EksClient;
use aws_sdk_sfn::Client as SfnClient;
use aws_sdk_eventbridge::Client as EventBridgeClient;
use aws_sdk_redshift::Client as RedshiftClient;
use aws_sdk_emr::Client as EmrClient;
use aws_sdk_athena::Client as AthenaClient;
use aws_sdk_glue::Client as GlueClient;
use aws_sdk_sesv2::Client as SesV2Client;
use aws_sdk_wafv2::Client as WafV2Client;
use aws_sdk_backup::Client as BackupClient;
use aws_sdk_ssm::Client as SsmClient;
use aws_sdk_apprunner::Client as AppRunnerClient;
use aws_sdk_globalaccelerator::Client as GlobalAcceleratorClient;
use aws_sdk_batch::Client as BatchClient;
use aws_sdk_glacier::Client as GlacierClient;
use aws_sdk_storagegateway::Client as StorageGatewayClient;
use aws_sdk_connect::Client as ConnectClient;
use aws_sdk_appsync::Client as AppSyncClient;
use aws_sdk_kinesisanalyticsv2::Client as KinesisAnalyticsClient;

use crate::models::aws_account::AwsAccountDto;
use crate::{errors::AppError};

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
    async fn create_efs_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<EfsClient, AppError>;
    async fn create_iam_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<IamClient, AppError>;
    async fn create_kms_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<KmsClient, AppError>;
    async fn create_acm_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<AcmClient, AppError>;
    async fn create_cloudtrail_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<CloudTrailClient, AppError>;
    async fn create_config_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<ConfigServiceClient, AppError>;
    async fn create_ecs_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<EcsClient, AppError>;
    async fn create_eks_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<EksClient, AppError>;
    async fn create_sfn_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<SfnClient, AppError>;
    async fn create_eventbridge_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<EventBridgeClient, AppError>;
    async fn create_redshift_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<RedshiftClient, AppError>;
    async fn create_emr_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<EmrClient, AppError>;
    async fn create_athena_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<AthenaClient, AppError>;
    async fn create_glue_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<GlueClient, AppError>;
    async fn create_ses_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<SesV2Client, AppError>;
    async fn create_waf_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<WafV2Client, AppError>;
    async fn create_backup_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<BackupClient, AppError>;
    async fn create_ssm_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<SsmClient, AppError>;
    async fn create_apprunner_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<AppRunnerClient, AppError>;
    async fn create_globalaccelerator_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<GlobalAcceleratorClient, AppError>;
    async fn create_batch_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<BatchClient, AppError>;
    async fn create_glacier_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<GlacierClient, AppError>;
    async fn create_storagegateway_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<StorageGatewayClient, AppError>;
    async fn create_connect_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<ConnectClient, AppError>;
    async fn create_appsync_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<AppSyncClient, AppError>;
    async fn create_kinesisanalytics_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<KinesisAnalyticsClient, AppError>;
}
