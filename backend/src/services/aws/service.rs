use std::sync::Arc;
use aws_config;
use aws_types;
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
use crate::config::{Config, AwsConfig};
use crate::repositories::aws_resource::AwsResourceRepository;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use super::client_factory::AwsClientFactory;
use async_trait::async_trait;
use tracing::debug;
use std::fs;

// Base AWS service
#[derive(Debug)]
pub struct AwsService {
    pub(crate) aws_resource_repo: Arc<AwsResourceRepository>,
    config: Config,
}

impl AwsService {
    pub fn new(aws_resource_repo: Arc<AwsResourceRepository>, config: Config) -> Self {
        Self { aws_resource_repo, config }
    }

    // Get AWS configuration based on profile/region
    async fn get_aws_config_impl(&self, profile: Option<&str>, region: &str) -> Result<AwsConfig, AppError> {
        let aws_configs = &self.config.cloud.aws;
        
        match profile {
            Some(profile_name) => {
                let config = aws_configs.iter()
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
            },
            None => aws_configs.first().cloned(),
        }.ok_or_else(|| {
            AppError::Config(format!("AWS configuration not found for profile: {:?}", profile))
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
                    let profile = line.trim_start_matches('[').trim_end_matches(']').to_string();
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

    // Load AWS SDK configuration for a given profile and region - backward compatible version
    pub async fn load_aws_sdk_config(&self, profile: Option<&str>, region: &str) -> Result<aws_config::SdkConfig, AppError> {
        self.load_aws_sdk_config_with_auth(profile, region, None).await
    }

    // Load AWS SDK configuration with authentication info
    pub async fn load_aws_sdk_config_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<aws_config::SdkConfig, AppError> {
        let aws_config = match self.get_aws_config(profile, region).await {
            Ok(config) => config,
            Err(e) => {
                debug!("Could not find AWS configuration for profile: {:?}. Using account credentials if available.", profile);
                AwsConfig {
                    name: "fallback".to_string(),
                    profile: profile.map(String::from),
                    region: region.to_string(),
                    access_key_id: account_auth.and_then(|auth| auth.access_key_id.clone()),
                    secret_access_key: account_auth.and_then(|auth| auth.secret_access_key.clone()),
                    role_arn: account_auth.and_then(|auth| auth.role_arn.clone()),
                }
            }
        };
        
        let config_builder = aws_config::from_env()
            .region(aws_types::region::Region::new(region.to_string()));
        
        let config = if let (Some(access_key), Some(secret_key)) = (&aws_config.access_key_id, &aws_config.secret_access_key) {
            debug!("Using access key authentication");
            let credentials_provider = aws_sdk_s3::config::Credentials::new(
                access_key, 
                secret_key,
                None,
                None,
                "static-credentials"
            );
            config_builder.credentials_provider(credentials_provider).load().await
        } else if let Some(role_arn) = aws_config.role_arn {
            debug!("Using IAM role authentication with role: {}", role_arn);
            config_builder.load().await
        } else if let Some(profile_name) = &aws_config.profile {
            debug!("Attempting to use AWS profile: {}", profile_name);
            let provider = aws_config::profile::ProfileFileCredentialsProvider::builder()
                .profile_name(profile_name)
                .build();
            
            config_builder.credentials_provider(provider).load().await
        } else {
            debug!("No explicit authentication method configured, using default credential provider chain");
            config_builder.load().await
        };
        
        Ok(config)
    }
}

#[async_trait]
impl AwsClientFactory for AwsService {
    async fn get_aws_config(&self, profile: Option<&str>, region: &str) -> Result<AwsConfig, AppError> {
        self.get_aws_config_impl(profile, region).await
    }

    async fn get_aws_config_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<AwsConfig, AppError> {
        Ok(match self.get_aws_config_impl(profile, region).await {
            Ok(config) => config,
            Err(_) => {
                debug!("Could not find AWS configuration for profile: {:?}. Using account credentials if available.", profile);
                AwsConfig {
                    name: "fallback".to_string(),
                    profile: profile.map(String::from),
                    region: region.to_string(),
                    access_key_id: account_auth.and_then(|auth| auth.access_key_id.clone()),
                    secret_access_key: account_auth.and_then(|auth| auth.secret_access_key.clone()),
                    role_arn: account_auth.and_then(|auth| auth.role_arn.clone()),
                }
            }
        })
    }

    async fn create_cloudwatch_client(&self, profile: Option<&str>, region: &str) -> Result<CloudWatchClient, AppError> {
        self.create_cloudwatch_client_with_auth(profile, region, None).await
    }

    async fn create_cloudwatch_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<CloudWatchClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(CloudWatchClient::new(&config))
    }
    
    async fn create_cloudwatch_logs_client(&self, profile: Option<&str>, region: &str) -> Result<CloudWatchLogsClient, AppError> {
        self.create_cloudwatch_logs_client_with_auth(profile, region, None).await
    }

    async fn create_cloudwatch_logs_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<CloudWatchLogsClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(CloudWatchLogsClient::new(&config))
    }

    async fn create_cost_explorer_client(&self, profile: Option<&str>, region: &str) -> Result<CostExplorerClient, AppError> {
        self.create_cost_explorer_client_with_auth(profile, region, None).await
    }

    async fn create_cost_explorer_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<CostExplorerClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(CostExplorerClient::new(&config))
    }

    async fn create_ec2_client(&self, profile: Option<&str>, region: &str) -> Result<Ec2Client, AppError> {
        self.create_ec2_client_with_auth(profile, region, None).await
    }

    async fn create_ec2_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Ec2Client, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(Ec2Client::new(&config))
    }

    async fn create_s3_client(&self, profile: Option<&str>, region: &str) -> Result<S3Client, AppError> {
        self.create_s3_client_with_auth(profile, region, None).await
    }

    async fn create_s3_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<S3Client, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(S3Client::new(&config))
    }

    async fn create_rds_client(&self, profile: Option<&str>, region: &str) -> Result<RdsClient, AppError> {
        self.create_rds_client_with_auth(profile, region, None).await
    }

    async fn create_rds_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<RdsClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(RdsClient::new(&config))
    }

    async fn create_dynamodb_client(&self, profile: Option<&str>, region: &str) -> Result<DynamoDbClient, AppError> {
        self.create_dynamodb_client_with_auth(profile, region, None).await
    }

    async fn create_dynamodb_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<DynamoDbClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(DynamoDbClient::new(&config))
    }

    async fn create_kinesis_client(&self, profile: Option<&str>, region: &str) -> Result<KinesisClient, AppError> {
        self.create_kinesis_client_with_auth(profile, region, None).await
    }

    async fn create_kinesis_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<KinesisClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(KinesisClient::new(&config))
    }

    async fn create_sqs_client(&self, profile: Option<&str>, region: &str) -> Result<SqsClient, AppError> {
        self.create_sqs_client_with_auth(profile, region, None).await
    }

    async fn create_sqs_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<SqsClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(SqsClient::new(&config))
    }

    async fn create_sns_client(&self, profile: Option<&str>, region: &str) -> Result<SnsClient, AppError> {
        self.create_sns_client_with_auth(profile, region, None).await
    }

    async fn create_sns_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<SnsClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(SnsClient::new(&config))
    }

    async fn create_lambda_client(&self, profile: Option<&str>, region: &str) -> Result<LambdaClient, AppError> {
        self.create_lambda_client_with_auth(profile, region, None).await
    }

    async fn create_lambda_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<LambdaClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(LambdaClient::new(&config))
    }

    async fn create_elasticache_client(&self, profile: Option<&str>, region: &str) -> Result<ElasticacheClient, AppError> {
        self.create_elasticache_client_with_auth(profile, region, None).await
    }

    async fn create_elasticache_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<ElasticacheClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(ElasticacheClient::new(&config))
    }

    async fn create_opensearch_client(&self, profile: Option<&str>, region: &str) -> Result<OpenSearchClient, AppError> {
        self.create_opensearch_client_with_auth(profile, region, None).await
    }

    async fn create_opensearch_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<OpenSearchClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(OpenSearchClient::new(&config))
    }

    async fn create_client_with_auth<C>(&self, profile: Option<&str>, region: &str) -> Result<C, AppError> 
    where 
        C: From<AwsConfig>
    {
        let aws_config = self.get_aws_config(profile, region).await?;
        Ok(C::from(aws_config))
    }
}
