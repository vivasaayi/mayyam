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


use super::client_factory::AwsClientFactory;
use crate::config::{AwsConfig, Config};
use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::repositories::aws_resource::AwsResourceRepository;
use crate::repositories::cloud_resource::CloudResourceRepository;
use async_trait::async_trait;
use aws_config;
use std::str::FromStr;
use aws_config::sts::AssumeRoleProvider;
use aws_config::BehaviorVersion;
use aws_credential_types::provider::SharedCredentialsProvider;
use aws_credential_types::Credentials as StaticCredentials;
use aws_sdk_cloudwatch::Client as CloudWatchClient;
use aws_sdk_cloudwatchlogs::Client as CloudWatchLogsClient;
use aws_sdk_costexplorer::Client as CostExplorerClient;
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
use aws_sdk_sns::Client as SnsClient;
use aws_sdk_sqs::Client as SqsClient;
use aws_sdk_sts::Client as StsClient;
use aws_types;
use std::fs;
use std::sync::Arc;
use tracing::{debug, trace};

// Base AWS service
#[derive(Debug)]
pub struct AwsService {
    pub(crate) aws_resource_repo: Arc<AwsResourceRepository>,
    pub(crate) cloud_resource_repo: Arc<CloudResourceRepository>,
    config: Config,
}

impl AwsService {
    pub fn new(
        aws_resource_repo: Arc<AwsResourceRepository>,
        cloud_resource_repo: Arc<CloudResourceRepository>,
        config: Config,
    ) -> Self {
        Self {
            aws_resource_repo,
            cloud_resource_repo,
            config,
        }
    }

    // Get AWS configuration based on profile/region
    async fn get_aws_config_impl(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<AwsConfig, AppError> {
        let aws_configs = &self.config.cloud.aws;

        match &aws_account_dto.profile {
            Some(profile_name) => {
                let config = aws_configs
                    .iter()
                    .find(|c| c.profile.as_ref().map_or(false, |p| p == profile_name))
                    .cloned();

                if config.is_none() {
                    debug!("AWS profile not found in configuration: {:?}", profile_name);
                    if let Ok(profiles) = Self::list_available_profiles() {
                        if profiles.contains(&profile_name.to_string()) {
                            debug!("Profile exists in credentials file but not in app config");
                        } else {
                            debug!("Profile does not exist in credentials file");
                        }
                    }
                }

                config
            }
            None => aws_configs.first().cloned(),
        }
        .ok_or_else(|| {
            AppError::Config(format!(
                "AWS configuration not found for profile: {:?}",
                aws_account_dto.profile
            ))
        })
    }

    // Helper method to list available AWS profiles
    fn list_available_profiles() -> Result<Vec<String>, std::io::Error> {
        let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
        let credentials_path = format!("{}/.aws/credentials", home);

        if let Ok(content) = fs::read_to_string(credentials_path) {
            let mut profiles = Vec::new();
            for line in content.lines() {
                if line.starts_with('[') && line.ends_with(']') {
                    let profile = line
                        .trim_start_matches('[')
                        .trim_end_matches(']')
                        .to_string();
                    if profile != "default" {
                        profiles.push(profile);
                    } else {
                        profiles.insert(0, profile); // Put default first
                    }
                }
            }
            Ok(profiles)
        } else {
            Ok(vec!["default".to_string()])
        }
    }

    pub async fn get_aws_sdk_config(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<aws_config::SdkConfig, AppError> {
        let region = aws_types::region::Region::new(aws_account_dto.default_region.clone());
        let mut base_builder =
            aws_config::defaults(BehaviorVersion::latest()).region(region.clone());

        // Support overriding the AWS endpoint via environment variable for localstack/testing
        if let Ok(endpoint_url) = std::env::var("AWS_ENDPOINT") {
            if !endpoint_url.is_empty() {
                base_builder = base_builder.endpoint_url(endpoint_url.clone());
                trace!("Using custom AWS_ENDPOINT: {}", endpoint_url);
            }
        }

        // Helper: build config with a specific credentials provider
        async fn load_with_provider<
            P: aws_credential_types::provider::ProvideCredentials + 'static,
        >(
            builder: aws_config::ConfigLoader,
            provider: P,
        ) -> aws_config::SdkConfig {
            builder.credentials_provider(provider).load().await
        }

        // Normalize auth type
        let auth_type = aws_account_dto.auth_type.to_lowercase();

        // Fallback heuristics for legacy data (auth_type may be 'auto')
        let resolved_auth = if auth_type == "auto" {
            if let Some(profile) = &aws_account_dto.profile {
                debug!("Auth[auto]: using profile {:?}", profile);
                "profile".to_string()
            } else if aws_account_dto.use_role && aws_account_dto.role_arn.is_some() {
                debug!(
                    "Auth[auto]: using assume_role with role {:?}",
                    aws_account_dto.role_arn
                );
                "assume_role".to_string()
            } else if aws_account_dto.has_access_key
                || (aws_account_dto.access_key_id.is_some()
                    && aws_account_dto.secret_access_key.is_some())
            {
                debug!("Auth[auto]: using access_keys");
                "access_keys".to_string()
            } else {
                debug!("Auth[auto]: using default/instance role");
                "instance_role".to_string()
            }
        } else {
            auth_type
        };

        // Apply profile name to builder when appropriate
        let profile_to_use = match resolved_auth.as_str() {
            "profile" => aws_account_dto.profile.clone(),
            "sso" => aws_account_dto
                .sso_profile
                .clone()
                .or_else(|| aws_account_dto.profile.clone()),
            "assume_role" => aws_account_dto
                .source_profile
                .clone()
                .or_else(|| aws_account_dto.profile.clone()),
            _ => None,
        };
        if let Some(p) = profile_to_use.as_deref() {
            base_builder = base_builder.profile_name(p);
        }

        let config = match resolved_auth.as_str() {
            "access_keys" => {
                if let (Some(ak), Some(sk)) = (
                    &aws_account_dto.access_key_id,
                    &aws_account_dto.secret_access_key,
                ) {
                    debug!(
                        "Using static access keys for AWS auth (account {})",
                        aws_account_dto.account_id
                    );
                    let static_creds = StaticCredentials::new(
                        ak.clone(),
                        sk.clone(),
                        None,
                        None,
                        "static-credentials",
                    );
                    let provider = SharedCredentialsProvider::new(static_creds);
                    load_with_provider(base_builder, provider).await
                } else {
                    debug!("Missing access keys, falling back to default chain");
                    base_builder.load().await
                }
            }
            "profile" => {
                let pf = profile_to_use.unwrap_or_else(|| "default".to_string());
                debug!("Using AWS profile '{}'", pf);
                base_builder.load().await
            }
            "assume_role" => {
                let role_arn = aws_account_dto.role_arn.clone().ok_or_else(|| {
                    AppError::Validation("Missing role_arn for assume_role".into())
                })?;
                debug!(
                    "Assuming role {} in region {}",
                    role_arn, aws_account_dto.default_region
                );
                // Load a base config (from env / profile) for STS to call AssumeRole
                let mut base_for_sts =
                    aws_config::defaults(BehaviorVersion::latest()).region(region.clone());
                if let Some(p) = profile_to_use.as_deref() {
                    base_for_sts = base_for_sts.profile_name(p);
                }
                let base_conf = base_for_sts.load().await;

                // Build AssumeRole provider
                let mut builder = AssumeRoleProvider::builder(role_arn);
                if let Some(sn) = aws_account_dto.session_name.as_ref() {
                    builder = builder.session_name(sn);
                }
                // Note: external_id may not be supported directly by builder in all SDK versions
                // If available, uncomment next line:
                // if let Some(ext) = aws_account_dto.external_id.as_ref() { builder = builder.external_id(ext); }
                builder = builder.region(region.clone());
                let assume_provider = builder.configure(&base_conf).build().await;
                load_with_provider(base_builder, assume_provider).await
            }
            "web_identity" => {
                // Leverage default chain for web identity by setting env vars if provided
                if let Some(token_file) = &aws_account_dto.web_identity_token_file {
                    std::env::set_var("AWS_WEB_IDENTITY_TOKEN_FILE", token_file);
                }
                if let Some(role_arn) = &aws_account_dto.role_arn {
                    std::env::set_var("AWS_ROLE_ARN", role_arn);
                }
                if let Some(sn) = &aws_account_dto.session_name {
                    std::env::set_var("AWS_ROLE_SESSION_NAME", sn);
                }
                debug!("Using web identity (OIDC) flow via default chain");
                base_builder.load().await
            }
            "sso" => {
                let pf = profile_to_use.unwrap_or_else(|| "default".to_string());
                debug!("Using AWS SSO via profile '{}'", pf);
                base_builder.load().await
            }
            // Default / instance / container / IRSA roles use the default chain
            _ => {
                debug!("Using default credential chain (may resolve to instance/task/IRSA role)");
                base_builder.load().await
            }
        };

        Ok(config)
    }

    // Get AWS account ID using STS
    pub async fn get_account_id(
        &self,
        aws_account_dto: &AwsAccountDto,
        _region: &str,
    ) -> Result<String, AppError> {
        let client = self.create_sts_client(aws_account_dto).await?;
        let identity = client.get_caller_identity().send().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to get caller identity: {}", e))
        })?;

        identity
            .account()
            .ok_or_else(|| {
                AppError::ExternalService("Account ID not found in caller identity".to_string())
            })
            .map(|s| s.to_string())
    }

    // List all AWS regions available to this account using EC2 DescribeRegions
    pub async fn list_all_regions(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<Vec<String>, AppError> {
        // Use provided region to bootstrap the client; AWS will return regions globally
        let ec2 = self.create_ec2_client(aws_account_dto).await?;
        let resp = ec2
            .describe_regions()
            .all_regions(true)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list AWS regions: {}", e)))?;

        let regions = resp
            .regions()
            .iter()
            .filter_map(|r| r.region_name())
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        Ok(regions)
    }
}

#[async_trait]
impl AwsClientFactory for AwsService {
    async fn create_cloudwatch_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<CloudWatchClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(CloudWatchClient::new(&config))
    }

    async fn create_cloudwatch_logs_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<CloudWatchLogsClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(CloudWatchLogsClient::new(&config))
    }

    async fn create_cost_explorer_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<CostExplorerClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(CostExplorerClient::new(&config))
    }

    async fn create_ec2_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<Ec2Client, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        trace!("EC2 Client Config: {:?}", config);
        Ok(Ec2Client::new(&config))
    }

    async fn create_s3_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<S3Client, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(S3Client::new(&config))
    }

    async fn create_rds_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<RdsClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(RdsClient::new(&config))
    }

    async fn create_dynamodb_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<DynamoDbClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(DynamoDbClient::new(&config))
    }

    async fn create_kinesis_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<KinesisClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(KinesisClient::new(&config))
    }

    async fn create_sqs_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<SqsClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(SqsClient::new(&config))
    }

    async fn create_sns_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<SnsClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(SnsClient::new(&config))
    }

    async fn create_lambda_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<LambdaClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(LambdaClient::new(&config))
    }

    async fn create_elasticache_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<ElasticacheClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(ElasticacheClient::new(&config))
    }

    async fn create_opensearch_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<OpenSearchClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(OpenSearchClient::new(&config))
    }

    async fn create_sts_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<StsClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(StsClient::new(&config))
    }

    async fn create_elbv2_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<Elbv2Client, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(Elbv2Client::new(&config))
    }

    async fn create_elb_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<ElbClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(ElbClient::new(&config))
    }

    async fn create_cloudfront_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<CloudFrontClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(CloudFrontClient::new(&config))
    }

    async fn create_api_gateway_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<ApiGatewayClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(ApiGatewayClient::new(&config))
    }

    async fn create_efs_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<EfsClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(EfsClient::new(&config))
    }
}
