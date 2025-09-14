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
use aws_sdk_sts::Client as StsClient;
use crate::config::{Config, AwsConfig};
use crate::repositories::aws_resource::AwsResourceRepository;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use super::client_factory::AwsClientFactory;
use async_trait::async_trait;
use tracing::debug;
use std::fs;
use crate::models::aws_account::AwsAccountDto;

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
    async fn get_aws_config_impl(&self, profile: &AwsAccountDto) -> Result<AwsConfig, AppError> {
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

    pub async fn get_aws_sdk_config(&self, aws_account_dto: &AwsAccountDto) -> Result<aws_config::SdkConfig, AppError> {
        // if(!aws_account_dto.profile) {
        //     // Throw error
        // }

        // if aws_account_dto.profile.is_none() {
        //     Err(AppError::InvalidInput("Missing AWS profile".to_string()))
        // }
        
        let aws_config = AwsConfig {
            name: "fallback".to_string(),
            profile: aws_account_dto.profile.map(String::from),
            region: aws_account_dto.default_region,
            access_key_id: aws_account_dto.access_key_id.clone().map(String::from),
            secret_access_key: aws_account_dto.access_key_id.clone().map(String::from),
            role_arn: aws_account_dto.role_arn.map(String::from),
        };
        
        let config_builder = aws_config::from_env()
            .region(aws_types::region::Region::new(aws_account_dto.default_region));
        
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

    // Get AWS account ID using STS
    pub async fn get_account_id(&self, aws_account_dto: &AwsAccountDto, region: &str) -> Result<String, AppError> {
        let client = self.create_sts_client(aws_account_dto).await?;
        let identity = client.get_caller_identity().send().await
            .map_err(|e| AppError::ExternalService(format!("Failed to get caller identity: {}", e)))?;
        
        identity.account()
            .ok_or_else(|| AppError::ExternalService("Account ID not found in caller identity".to_string()))
            .map(|s| s.to_string())
    }
}

#[async_trait]
impl AwsClientFactory for AwsService {
    async fn create_cloudwatch_client(&self, aws_account_dto: &AwsAccountDto) -> Result<CloudWatchClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(CloudWatchClient::new(&config))
    }

    async fn create_cloudwatch_logs_client(&self, aws_account_dto: &AwsAccountDto) -> Result<CloudWatchLogsClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(CloudWatchLogsClient::new(&config))
    }


    async fn create_cost_explorer_client(&self, aws_account_dto: &AwsAccountDto) -> Result<CostExplorerClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(CostExplorerClient::new(&config))
    }

    async fn create_ec2_client(&self, aws_account_dto: &AwsAccountDto) -> Result<Ec2Client, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(Ec2Client::new(&config))
    }

    async fn create_s3_client(&self, aws_account_dto: &AwsAccountDto) -> Result<S3Client, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(S3Client::new(&config))
    }


    async fn create_rds_client(&self, aws_account_dto: &AwsAccountDto) -> Result<RdsClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(RdsClient::new(&config))
    }

    async fn create_dynamodb_client(&self, aws_account_dto: &AwsAccountDto) -> Result<DynamoDbClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(DynamoDbClient::new(&config))
    }

    async fn create_kinesis_client(&self, aws_account_dto: &AwsAccountDto) -> Result<KinesisClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(KinesisClient::new(&config))
    }

    async fn create_sqs_client(&self, aws_account_dto: &AwsAccountDto) -> Result<SqsClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(SqsClient::new(&config))
    }

    async fn create_sns_client(&self, aws_account_dto: &AwsAccountDto) -> Result<SnsClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(SnsClient::new(&config))
    }

    async fn create_lambda_client(&self, aws_account_dto: &AwsAccountDto) -> Result<LambdaClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(LambdaClient::new(&config))
    }

    async fn create_elasticache_client(&self, aws_account_dto: &AwsAccountDto) -> Result<ElasticacheClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(ElasticacheClient::new(&config))
    }

    async fn create_opensearch_client(&self, aws_account_dto: &AwsAccountDto) -> Result<OpenSearchClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(OpenSearchClient::new(&config))
    }

    async fn create_sts_client(&self, aws_account_dto: &AwsAccountDto) -> Result<StsClient, AppError> {
        let config = self.get_aws_sdk_config(aws_account_dto).await?;
        Ok(StsClient::new(&config))
    }
}
