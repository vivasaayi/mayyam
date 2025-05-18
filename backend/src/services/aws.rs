use std::sync::Arc;
use uuid::Uuid;
use tracing::{info, error, debug};
use serde::{Deserialize, Serialize};
use serde_json::json;
use chrono::{Utc, DateTime};
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
use aws_sdk_costexplorer::Client as CostExplorerClient;
use aws_sdk_cloudwatch::Client as CloudWatchClient;
use std::path::Path;
use std::fs::{self, File, create_dir_all};
use std::io::Write;

use crate::config::{Config, AwsConfig};
use crate::repositories::aws_resource::AwsResourceRepository;
use crate::models::aws_resource::{
    AwsResourceDto, AwsResourceQuery, AwsResourcePage, AwsResourceType, Model as AwsResourceModel,
};
// Using the AccountAuthInfo model
use crate::models::aws_auth::AccountAuthInfo;
use crate::errors::AppError;

// Separate control plane and data plane operations
pub struct AwsService {
    aws_resource_repo: Arc<AwsResourceRepository>,
    config: Config,
}

// Control plane operations for AWS resources
pub struct AwsControlPlane {
    aws_service: Arc<AwsService>,
}

// Data plane operations for AWS resources
pub struct AwsDataPlane {
    aws_service: Arc<AwsService>,
}

// Common Request/Response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSyncRequest {
    pub account_id: String,
    pub profile: Option<String>,
    pub region: String,
    pub resource_types: Option<Vec<String>>,
    // Authentication fields
    pub use_role: bool,
    pub role_arn: Option<String>,
    pub external_id: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSyncResponse {
    pub summary: Vec<ResourceTypeSyncSummary>,
    pub total_resources: usize,
    pub sync_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTypeSyncSummary {
    pub resource_type: String,
    pub count: usize,
    pub status: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3PutObjectRequest {
    pub bucket: String,
    pub key: String,
    pub content_type: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3GetObjectRequest {
    pub bucket: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqsSendMessageRequest {
    pub queue_url: String,
    pub message_body: String,
    pub delay_seconds: Option<i32>,
    pub message_attributes: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqsReceiveMessageRequest {
    pub queue_url: String,
    pub max_messages: Option<i32>,
    pub wait_time_seconds: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisPutRecordRequest {
    pub stream_name: String,
    pub data: String,
    pub partition_key: String,
    pub sequence_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBPutItemRequest {
    pub table_name: String,
    pub item: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBGetItemRequest {
    pub table_name: String,
    pub key: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamoDBQueryRequest {
    pub table_name: String,
    pub key_condition_expression: String,
    pub expression_attribute_values: serde_json::Value,
    pub expression_attribute_names: Option<serde_json::Value>,
}

// CloudWatch metrics request/response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchMetricsRequest {
    pub resource_id: String,
    pub resource_type: String,
    pub metrics: Vec<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub period: i32,
    pub export_path: Option<String>,
    pub upload_to_s3: Option<bool>,
    pub s3_bucket: Option<String>,
    pub post_to_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchMetricsResult {
    pub resource_id: String,
    pub resource_type: String,
    pub metrics: Vec<CloudWatchMetricData>,
    pub export_path: Option<String>,
    pub s3_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchMetricData {
    pub metric_name: String,
    pub namespace: String,
    pub unit: String,
    pub datapoints: Vec<CloudWatchDatapoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchDatapoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchLogsRequest {
    pub log_group_name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub filter_pattern: Option<String>,
    pub export_path: Option<String>,
    pub upload_to_s3: Option<bool>,
    pub s3_bucket: Option<String>,
    pub post_to_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchLogsResult {
    pub log_group_name: String,
    pub events: Vec<CloudWatchLogEvent>,
    pub export_path: Option<String>,
    pub s3_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchLogEvent {
    pub timestamp: DateTime<Utc>,
    pub message: String,
    pub event_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleMetricsRequest {
    pub resource_id: String,
    pub resource_type: String,
    pub metrics: Vec<String>,
    pub period: i32,
    pub interval_seconds: u64,
    pub export_path: Option<String>,
    pub upload_to_s3: Option<bool>,
    pub s3_bucket: Option<String>,
    pub post_to_url: Option<String>,
}

impl AwsService {
    pub fn new(aws_resource_repo: Arc<AwsResourceRepository>, config: Config) -> Self {
        Self { aws_resource_repo, config }
    }

    // Get AWS configuration based on profile/region
    pub async fn get_aws_config(&self, profile: Option<&str>, region: &str) -> Result<AwsConfig, AppError> {
        let aws_configs = &self.config.cloud.aws;
        
        // Find the AWS config based on profile or default if not specified
        let aws_config = match profile {
            Some(profile_name) => {
                let config = aws_configs.iter()
                    .find(|c| c.profile.as_ref().map_or(false, |p| p == profile_name))
                    .cloned();
                
                if config.is_none() {
                    debug!("AWS profile not found in configuration: {:?}", profile_name);
                    // Check if the profile exists in the credentials file but isn't in our config
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
        })?;
        
        Ok(aws_config)
    }
    
    // Helper method to list available AWS profiles (for better error messages)
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
        // Get the AWS config from our application's configuration
        let aws_config = match self.get_aws_config(profile, region).await {
            Ok(config) => config,
            Err(e) => {
                debug!("Could not find AWS configuration for profile: {:?}. Using account credentials if available.", profile);
                // Instead of failing immediately, create a default config that we can add credentials to
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
        
        // Start building the AWS SDK config with the region
        let config_builder = aws_config::from_env()
            .region(aws_types::region::Region::new(region.to_string()));
        
        // Try different authentication methods in order of preference:
        // 1. If access keys are provided directly, use them
        // 2. If IAM role is specified, use role assumption
        // 3. If profile is specified, try to use it
        // 4. Fall back to default credential provider chain
        
        let config = if let (Some(access_key), Some(secret_key)) = (&aws_config.access_key_id, &aws_config.secret_access_key) {
            debug!("Using access key authentication");
            // Use API key authentication
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
            // Use assumed role
            // In a real implementation, would use STS to assume role with AWS SDK
            // For now, we'll just use default credential provider which should include role assumption
            config_builder.load().await
        } else if let Some(profile_name) = &aws_config.profile {
            debug!("Attempting to use AWS profile: {}", profile_name);
            // Try to use the named profile
            let provider = aws_config::profile::ProfileFileCredentialsProvider::builder()
                .profile_name(profile_name)
                .build();
            
            config_builder.credentials_provider(provider).load().await
        } else {
            debug!("No explicit authentication method configured, using default credential provider chain");
            // Use default credential provider chain (environment, instance profile, etc.)
            config_builder.load().await
        };
        
        Ok(config)
    }

    // Create EC2 client - backward compatible
    pub async fn create_ec2_client(&self, profile: Option<&str>, region: &str) -> Result<Ec2Client, AppError> {
        self.create_ec2_client_with_auth(profile, region, None).await
    }
    
    // Create EC2 client with authentication
    pub async fn create_ec2_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Ec2Client, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(Ec2Client::new(&config))
    }
    
    // Create S3 client - backward compatible
    pub async fn create_s3_client(&self, profile: Option<&str>, region: &str) -> Result<S3Client, AppError> {
        self.create_s3_client_with_auth(profile, region, None).await
    }
    
    // Create S3 client with authentication
    pub async fn create_s3_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<S3Client, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(S3Client::new(&config))
    }
    
    // Create RDS client - backward compatible
    pub async fn create_rds_client(&self, profile: Option<&str>, region: &str) -> Result<RdsClient, AppError> {
        self.create_rds_client_with_auth(profile, region, None).await
    }
    
    // Create RDS client with authentication
    pub async fn create_rds_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<RdsClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(RdsClient::new(&config))
    }
    
    // Create DynamoDB client - backward compatible
    pub async fn create_dynamodb_client(&self, profile: Option<&str>, region: &str) -> Result<DynamoDbClient, AppError> {
        self.create_dynamodb_client_with_auth(profile, region, None).await
    }
    
    // Create DynamoDB client with authentication
    pub async fn create_dynamodb_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<DynamoDbClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(DynamoDbClient::new(&config))
    }
    
    // Create Kinesis client - backward compatible
    pub async fn create_kinesis_client(&self, profile: Option<&str>, region: &str) -> Result<KinesisClient, AppError> {
        self.create_kinesis_client_with_auth(profile, region, None).await
    }
    
    // Create Kinesis client with authentication
    pub async fn create_kinesis_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<KinesisClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(KinesisClient::new(&config))
    }
    
    // Create SQS client - backward compatible
    pub async fn create_sqs_client(&self, profile: Option<&str>, region: &str) -> Result<SqsClient, AppError> {
        self.create_sqs_client_with_auth(profile, region, None).await
    }
    
    // Create SQS client with authentication
    pub async fn create_sqs_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<SqsClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(SqsClient::new(&config))
    }
    
    // Create SNS client - backward compatible
    pub async fn create_sns_client(&self, profile: Option<&str>, region: &str) -> Result<SnsClient, AppError> {
        self.create_sns_client_with_auth(profile, region, None).await
    }
    
    // Create SNS client with authentication
    pub async fn create_sns_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<SnsClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(SnsClient::new(&config))
    }
    
    // Create Lambda client - backward compatible
    pub async fn create_lambda_client(&self, profile: Option<&str>, region: &str) -> Result<LambdaClient, AppError> {
        self.create_lambda_client_with_auth(profile, region, None).await
    }
    
    // Create Lambda client with authentication
    pub async fn create_lambda_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<LambdaClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(LambdaClient::new(&config))
    }
    
    // Create ElastiCache client - backward compatible
    pub async fn create_elasticache_client(&self, profile: Option<&str>, region: &str) -> Result<ElasticacheClient, AppError> {
        self.create_elasticache_client_with_auth(profile, region, None).await
    }
    
    // Create ElastiCache client with authentication
    pub async fn create_elasticache_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<ElasticacheClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(ElasticacheClient::new(&config))
    }
    
    // Create OpenSearch client - backward compatible
    pub async fn create_opensearch_client(&self, profile: Option<&str>, region: &str) -> Result<OpenSearchClient, AppError> {
        self.create_opensearch_client_with_auth(profile, region, None).await
    }
    
    // Create OpenSearch client with authentication
    pub async fn create_opensearch_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<OpenSearchClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(OpenSearchClient::new(&config))
    }
    
    // Create Cost Explorer client - backward compatible
    pub async fn create_cost_explorer_client(&self, profile: Option<&str>, region: &str) -> Result<CostExplorerClient, AppError> {
        self.create_cost_explorer_client_with_auth(profile, region, None).await
    }
    
    // Create Cost Explorer client with authentication
    pub async fn create_cost_explorer_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<CostExplorerClient, AppError> {
        let config = self.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(CostExplorerClient::new(&config))
    }
}

impl AwsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    // Sync EC2 instances for an account and region - backward compatible version
    pub async fn sync_ec2_instances(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_ec2_instances_with_auth(account_id, profile, region, None).await
    }

    // Sync EC2 instances with authentication
    pub async fn sync_ec2_instances_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_ec2_client_with_auth(profile, region, account_auth).await?;
        self.sync_ec2_instances_with_client(account_id, profile, region, client).await
    }

    // Sync EC2 instances with provided client
    pub async fn sync_ec2_instances_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, client: Ec2Client) -> Result<Vec<AwsResourceModel>, AppError> {
        let repo = &self.aws_service.aws_resource_repo;
        
        // In a real implementation, this would call describe_instances and process the results
        // For now, we'll just create some sample data
        
        let mut instances = Vec::new();
        
        // Sample instance 1
        let instance1_data = json!({
            "instance_id": "i-0123456789abcdef0",
            "instance_type": "t2.micro",
            "state": "running",
            "availability_zone": format!("{}a", region),
            "public_ip": "203.0.113.1",
            "private_ip": "10.0.0.1",
            "launch_time": "2023-05-01T12:00:00Z",
            "vpc_id": "vpc-0123abcd",
            "subnet_id": "subnet-0123abcd"
        });
        
        let instance1 = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: AwsResourceType::EC2Instance.to_string(),
            resource_id: "i-0123456789abcdef0".to_string(),
            arn: format!("arn:aws:ec2:{}:{}:instance/i-0123456789abcdef0", region, account_id),
            name: Some("Sample EC2 Instance 1".to_string()),
            tags: json!({"Name": "Sample EC2 Instance 1", "Environment": "Development"}),
            resource_data: instance1_data,
        };
        
        // Sample instance 2
        let instance2_data = json!({
            "instance_id": "i-0123456789abcdef1",
            "instance_type": "t2.small",
            "state": "stopped",
            "availability_zone": format!("{}b", region),
            "public_ip": null,
            "private_ip": "10.0.0.2",
            "launch_time": "2023-05-01T12:00:00Z",
            "vpc_id": "vpc-0123abcd",
            "subnet_id": "subnet-0123efgh"
        });
        
        let instance2 = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: AwsResourceType::EC2Instance.to_string(),
            resource_id: "i-0123456789abcdef1".to_string(),
            arn: format!("arn:aws:ec2:{}:{}:instance/i-0123456789abcdef1", region, account_id),
            name: Some("Sample EC2 Instance 2".to_string()),
            tags: json!({"Name": "Sample EC2 Instance 2", "Environment": "Production"}),
            resource_data: instance2_data,
        };
        
        // Save instances to database
        let saved_instance1 = match repo.find_by_arn(&instance1.arn).await? {
            Some(existing) => repo.update(existing.id, &instance1).await?,
            None => repo.create(&instance1).await?,
        };
        instances.push(saved_instance1);
        
        let saved_instance2 = match repo.find_by_arn(&instance2.arn).await? {
            Some(existing) => repo.update(existing.id, &instance2).await?,
            None => repo.create(&instance2).await?,
        };
        instances.push(saved_instance2);
        
        Ok(instances)
    }
    
    // Sync S3 buckets for an account - backward compatible version
    pub async fn sync_s3_buckets(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_s3_buckets_with_auth(account_id, profile, region, None).await
    }
    
    // Sync S3 buckets with authentication
    pub async fn sync_s3_buckets_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_s3_client_with_auth(profile, region, account_auth).await?;
        self.sync_s3_buckets_with_client(account_id, profile, region, client).await
    }
    
    // Sync S3 buckets with provided client
    pub async fn sync_s3_buckets_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, client: S3Client) -> Result<Vec<AwsResourceModel>, AppError> {
        let repo = &self.aws_service.aws_resource_repo;
        
        // In a real implementation, this would call list_buckets and process the results
        
        let mut buckets = Vec::new();
        
        // Sample bucket 1
        let bucket1_data = json!({
            "creation_date": "2023-01-15T10:00:00Z",
            "region": region
        });
        
        let bucket1 = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: AwsResourceType::S3Bucket.to_string(),
            resource_id: "example-bucket-1".to_string(),
            arn: format!("arn:aws:s3:::example-bucket-1"),
            name: Some("example-bucket-1".to_string()),
            tags: json!({"Purpose": "Logs", "Environment": "Development"}),
            resource_data: bucket1_data,
        };
        
        // Sample bucket 2
        let bucket2_data = json!({
            "creation_date": "2023-02-20T14:30:00Z",
            "region": region
        });
        
        let bucket2 = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: AwsResourceType::S3Bucket.to_string(),
            resource_id: "example-bucket-2".to_string(),
            arn: format!("arn:aws:s3:::example-bucket-2"),
            name: Some("example-bucket-2".to_string()),
            tags: json!({"Purpose": "Data", "Environment": "Production"}),
            resource_data: bucket2_data,
        };
        
        // Save buckets to database
        let saved_bucket1 = match repo.find_by_arn(&bucket1.arn).await? {
            Some(existing) => repo.update(existing.id, &bucket1).await?,
            None => repo.create(&bucket1).await?,
        };
        buckets.push(saved_bucket1);
        
        let saved_bucket2 = match repo.find_by_arn(&bucket2.arn).await? {
            Some(existing) => repo.update(existing.id, &bucket2).await?,
            None => repo.create(&bucket2).await?,
        };
        buckets.push(saved_bucket2);
        
        Ok(buckets)
    }

    // Sync RDS instances - backward compatible version
    pub async fn sync_rds_instances(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_rds_instances_with_auth(account_id, profile, region, None).await
    }
    
    // Sync RDS instances with authentication
    pub async fn sync_rds_instances_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_rds_client_with_auth(profile, region, account_auth).await?;
        self.sync_rds_instances_with_client(account_id, profile, region, client).await
    }
    
    // Sync RDS instances with provided client
    pub async fn sync_rds_instances_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, client: RdsClient) -> Result<Vec<AwsResourceModel>, AppError> {
        let repo = &self.aws_service.aws_resource_repo;
        
        // In a real implementation, this would call describe_db_instances
        
        let mut instances = Vec::new();
        
        // Sample RDS instance
        let rds_data = json!({
            "identifier": "sample-postgres",
            "engine": "postgres",
            "engine_version": "13.4",
            "instance_class": "db.t3.micro",
            "storage": 20,
            "status": "available",
            "endpoint": {
                "address": format!("sample-postgres.abcdef.{}.rds.amazonaws.com", region),
                "port": 5432
            },
            "availability_zone": format!("{}a", region),
            "multi_az": false,
            "vpc_id": "vpc-0123abcd"
        });
        
        let rds = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: AwsResourceType::RdsInstance.to_string(),
            resource_id: "sample-postgres".to_string(),
            arn: format!("arn:aws:rds:{}:{}:db:sample-postgres", region, account_id),
            name: Some("Sample Postgres DB".to_string()),
            tags: json!({"Name": "Sample Postgres DB", "Environment": "Development"}),
            resource_data: rds_data,
        };
        
        // Save to database
        let saved_rds = match repo.find_by_arn(&rds.arn).await? {
            Some(existing) => repo.update(existing.id, &rds).await?,
            None => repo.create(&rds).await?,
        };
        instances.push(saved_rds);
        
        Ok(instances)
    }

    // Sync DynamoDB tables - backward compatible version
    pub async fn sync_dynamodb_tables(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_dynamodb_tables_with_auth(account_id, profile, region, None).await
    }
    
    // Sync DynamoDB tables with authentication
    pub async fn sync_dynamodb_tables_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_dynamodb_client_with_auth(profile, region, account_auth).await?;
        self.sync_dynamodb_tables_with_client(account_id, profile, region, client).await
    }
    
    // Sync DynamoDB tables with provided client
    pub async fn sync_dynamodb_tables_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, client: DynamoDbClient) -> Result<Vec<AwsResourceModel>, AppError> {
        let repo = &self.aws_service.aws_resource_repo;
        
        // In a real implementation, this would call list_tables and describe_table
        
        let mut tables = Vec::new();
        
        // Sample DynamoDB table
        let dynamo_data = json!({
            "table_name": "sample-users",
            "status": "ACTIVE",
            "creation_date": "2023-03-15T08:45:00Z",
            "provisioned_throughput": {
                "read_capacity_units": 5,
                "write_capacity_units": 5
            },
            "key_schema": [
                {
                    "attribute_name": "user_id",
                    "key_type": "HASH"
                }
            ],
            "attribute_definitions": [
                {
                    "attribute_name": "user_id",
                    "attribute_type": "S"
                }
            ],
            "item_count": 42,
            "table_size_bytes": 12345
        });
        
        let dynamo = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: AwsResourceType::DynamoDbTable.to_string(),
            resource_id: "sample-users".to_string(),
            arn: format!("arn:aws:dynamodb:{}:{}:table/sample-users", region, account_id),
            name: Some("Sample Users Table".to_string()),
            tags: json!({"Name": "Users Table", "Environment": "Development"}),
            resource_data: dynamo_data,
        };
        
        // Save to database
        let saved_dynamo = match repo.find_by_arn(&dynamo.arn).await? {
            Some(existing) => repo.update(existing.id, &dynamo).await?,
            None => repo.create(&dynamo).await?,
        };
        tables.push(saved_dynamo);
        
        Ok(tables)
    }

    // Sync all resources for an account and region
    pub async fn sync_resources(&self, request: &ResourceSyncRequest) -> Result<ResourceSyncResponse, AppError> {
        let account_id = &request.account_id;
        let profile = request.profile.as_deref();
        let region = &request.region;
        
        // Create account auth info from request for authentication fallback
        let account_auth = AccountAuthInfo::from(request);
        
        // Get resource types to sync
        let resource_types = match &request.resource_types {
            Some(types) => types.clone(),
            None => vec![
                AwsResourceType::EC2Instance.to_string(),
                AwsResourceType::S3Bucket.to_string(),
                AwsResourceType::RdsInstance.to_string(),
                AwsResourceType::DynamoDbTable.to_string(),
                AwsResourceType::ElasticacheCluster.to_string(),
            ],
        };
        
        let mut summary = Vec::new();
        let mut total_resources = 0;
        
        for resource_type in resource_types {
            let result = match resource_type.as_str() {
                "EC2Instance" => {
                    let instances = self.sync_ec2_instances_with_auth(account_id, profile, region, Some(&account_auth)).await?;
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: AwsResourceType::EC2Instance.to_string(),
                        count: instances.len(),
                        status: "success".to_string(),
                        details: None,
                    });
                    total_resources += instances.len();
                    Ok(()) as Result<(), AppError>
                },
                "S3Bucket" => {
                    let buckets = self.sync_s3_buckets_with_auth(account_id, profile, region, Some(&account_auth)).await?;
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: AwsResourceType::S3Bucket.to_string(),
                        count: buckets.len(),
                        status: "success".to_string(),
                        details: None,
                    });
                    total_resources += buckets.len();
                    Ok(()) as Result<(), AppError>
                },
                "RdsInstance" => {
                    let instances = self.sync_rds_instances_with_auth(account_id, profile, region, Some(&account_auth)).await?;
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: AwsResourceType::RdsInstance.to_string(),
                        count: instances.len(),
                        status: "success".to_string(),
                        details: None,
                    });
                    total_resources += instances.len();
                    Ok(()) as Result<(), AppError>
                },
                "DynamoDbTable" => {
                    let tables = self.sync_dynamodb_tables_with_auth(account_id, profile, region, Some(&account_auth)).await?;
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: AwsResourceType::DynamoDbTable.to_string(),
                        count: tables.len(),
                        status: "success".to_string(),
                        details: None,
                    });
                    total_resources += tables.len();
                    Ok(()) as Result<(), AppError>
                },
                "ElasticacheCluster" => {
                    // Create a CloudWatchService and use it to sync ElastiCache clusters
                    let cloudwatch_service = CloudWatchService::new(self.aws_service.clone());
                    let clusters = cloudwatch_service.sync_elasticache_clusters_with_auth(account_id, profile, region, Some(&account_auth)).await?;
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: AwsResourceType::ElasticacheCluster.to_string(),
                        count: clusters.len(),
                        status: "success".to_string(),
                        details: None,
                    });
                    total_resources += clusters.len();
                    Ok(()) as Result<(), AppError>
                },
                _ => {
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: resource_type.clone(),
                        count: 0,
                        status: "skipped".to_string(),
                        details: Some("Resource type not supported".to_string()),
                    });
                    Ok(()) as Result<(), AppError>
                },
            };
            
            if let Err(e) = result {
                summary.push(ResourceTypeSyncSummary {
                    resource_type: resource_type.clone(),
                    count: 0,
                    status: "error".to_string(),
                    details: Some(e.to_string()),
                });
            }
        }
        
        Ok(ResourceSyncResponse {
            summary,
            total_resources,
            sync_time: Utc::now().to_rfc3339(),
        })
    }
}

impl AwsDataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }
    
    // S3 data plane operations
    pub async fn s3_get_object(&self, profile: Option<&str>, region: &str, request: &S3GetObjectRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_s3_client(profile, region).await?;
        
        // In a real implementation, this would call get_object
        // For now, provide mock data
        
        info!("Getting object {} from bucket {}", request.key, request.bucket);
        
        let response = json!({
            "body": "This is sample content for the S3 object",
            "content_type": "text/plain",
            "content_length": 38,
            "last_modified": Utc::now().to_rfc3339(),
            "etag": "\"abcdef1234567890\"",
            "metadata": {
                "custom-key": "custom-value"
            }
        });
        
        Ok(response)
    }
    
    pub async fn s3_put_object(&self, profile: Option<&str>, region: &str, request: &S3PutObjectRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_s3_client(profile, region).await?;
        
        // In a real implementation, this would call put_object
        
        info!("Putting object {} in bucket {}", request.key, request.bucket);
        
        let response = json!({
            "etag": "\"abcdef1234567890\"",
            "version_id": null,
            "content_length": request.body.len(),
            "content_type": request.content_type.clone().unwrap_or_else(|| "application/octet-stream".to_string())
        });
        
        Ok(response)
    }
    
    // DynamoDB data plane operations
    pub async fn dynamodb_get_item(&self, profile: Option<&str>, region: &str, request: &DynamoDBGetItemRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // In a real implementation, this would call get_item
        
        info!("Getting item from table {}", request.table_name);
        
        // Sample response
        let response = json!({
            "item": {
                "user_id": {"S": "12345"},
                "username": {"S": "johndoe"},
                "email": {"S": "john@example.com"},
                "created_at": {"S": "2023-01-15T10:30:00Z"},
                "active": {"BOOL": true}
            }
        });
        
        Ok(response)
    }
    
    pub async fn dynamodb_put_item(&self, profile: Option<&str>, region: &str, request: &DynamoDBPutItemRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // In a real implementation, this would call put_item
        
        info!("Putting item into table {}", request.table_name);
        
        // Sample response
        let response = json!({
            "consumed_capacity": {
                "capacity_units": 1.0,
                "table_name": request.table_name
            }
        });
        
        Ok(response)
    }
    
    pub async fn dynamodb_query(&self, profile: Option<&str>, region: &str, request: &DynamoDBQueryRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
        
        // In a real implementation, this would call query
        
        info!("Querying table {} with expression {}", request.table_name, request.key_condition_expression);
        
        // Sample response
        let response = json!({
            "items": [
                {
                    "user_id": {"S": "12345"},
                    "username": {"S": "johndoe"},
                    "email": {"S": "john@example.com"}
                },
                {
                    "user_id": {"S": "67890"},
                    "username": {"S": "janedoe"},
                    "email": {"S": "jane@example.com"}
                }
            ],
            "count": 2,
            "scanned_count": 2,
            "consumed_capacity": {
                "capacity_units": 0.5,
                "table_name": request.table_name
            }
        });
        
        Ok(response)
    }
    
    // SQS data plane operations
    pub async fn sqs_send_message(&self, profile: Option<&str>, region: &str, request: &SqsSendMessageRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_sqs_client(profile, region).await?;
        
        // In a real implementation, this would call send_message
        
        info!("Sending message to queue {}", request.queue_url);
        
        // Sample response
        let response = json!({
            "message_id": "12345678-1234-1234-1234-123456789012",
            "md5_of_message_body": "abcdef1234567890abcdef1234567890",
            "sequence_number": null,
        });
        
        Ok(response)
    }
    
    pub async fn sqs_receive_messages(&self, profile: Option<&str>, region: &str, request: &SqsReceiveMessageRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_sqs_client(profile, region).await?;
        
        // In a real implementation, this would call receive_message
        
        info!("Receiving messages from queue {}", request.queue_url);
        
        // Sample response
        let response = json!({
            "messages": [
                {
                    "message_id": "12345678-1234-1234-1234-123456789012",
                    "receipt_handle": "AQEBBX8nesZEXmkhsmZeyIE8iQAMig7qw...",
                    "body": "Hello from SQS!",
                    "md5_of_body": "abcdef1234567890abcdef1234567890",
                    "attributes": {
                        "SentTimestamp": "1678912345678",
                        "ApproximateReceiveCount": "1"
                    }
                }
            ]
        });
        
        Ok(response)
    }
    
    // Kinesis data plane operations
    pub async fn kinesis_put_record(&self, profile: Option<&str>, region: &str, request: &KinesisPutRecordRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;
        
        // In a real implementation, this would call put_record
        
        info!("Putting record to stream {}", request.stream_name);
        
        // Sample response
        let response = json!({
            "shard_id": "shardId-000000000000",
            "sequence_number": "49598630142999655949899115247677708214880825439595202562",
            "encryption_type": null
        });
        
        Ok(response)
    }
}

// AWS Cost functions (can be part of either control plane or separate)
pub struct AwsCostService {
    aws_service: Arc<AwsService>,
}

impl AwsCostService {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }
    
    // Get cost data for a specific period
    pub async fn get_cost_and_usage(&self, account_id: &str, profile: Option<&str>, region: &str, start_date: &str, end_date: &str) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_cost_explorer_client(profile, region).await?;
        
        // In a real implementation, this would call get_cost_and_usage
        
        info!("Getting cost and usage for account {} from {} to {}", account_id, start_date, end_date);
        
        // Sample response
        let response = json!({
            "group_by": "SERVICE",
            "time_period": {
                "start": start_date,
                "end": end_date
            },
            "total": {
                "unblended_cost": {
                    "amount": "543.21",
                    "unit": "USD"
                }
            },
            "groups": [
                {
                    "keys": ["Amazon Elastic Compute Cloud - Compute"],
                    "metrics": {
                        "unblended_cost": {
                            "amount": "285.43",
                            "unit": "USD"
                        }
                    }
                },
                {
                    "keys": ["Amazon Simple Storage Service"],
                    "metrics": {
                        "unblended_cost": {
                            "amount": "92.78",
                            "unit": "USD"
                        }
                    }
                },
                {
                    "keys": ["Amazon Relational Database Service"],
                    "metrics": {
                        "unblended_cost": {
                            "amount": "165.00",
                            "unit": "USD"
                        }
                    }
                }
            ]
        });
        
        Ok(response)
    }
}

// CloudWatch Metrics service
pub struct CloudWatchService {
    aws_service: Arc<AwsService>,
}

impl CloudWatchService {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }
    
    // Create CloudWatch client - backward compatible version
    pub async fn create_cloudwatch_client(&self, profile: Option<&str>, region: &str) -> Result<CloudWatchClient, AppError> {
        self.create_cloudwatch_client_with_auth(profile, region, None).await
    }

    // Create CloudWatch client with authentication
    pub async fn create_cloudwatch_client_with_auth(&self, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<CloudWatchClient, AppError> {
        let config = self.aws_service.load_aws_sdk_config_with_auth(profile, region, account_auth).await?;
        Ok(CloudWatchClient::new(&config))
    }
    
    // Sync ElastiCache clusters - backward compatible version
    pub async fn sync_elasticache_clusters(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_elasticache_clusters_with_auth(account_id, profile, region, None).await
    }
    
    // Sync ElastiCache clusters with authentication
    pub async fn sync_elasticache_clusters_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_elasticache_client_with_auth(profile, region, account_auth).await?;
        self.sync_elasticache_clusters_with_client(account_id, profile, region, client).await
    }
    
    // Sync ElastiCache clusters with provided client
    pub async fn sync_elasticache_clusters_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, client: ElasticacheClient) -> Result<Vec<AwsResourceModel>, AppError> {
        let repo = &self.aws_service.aws_resource_repo;
        
        // In a real implementation, this would call describe_cache_clusters
        // For now, we'll just create sample data
        
        let mut clusters = Vec::new();
        
        // Sample Redis cluster 1
        let redis1_data = json!({
            "cluster_id": "redis-cluster-1",
            "node_type": "cache.t3.micro",
            "engine": "redis",
            "engine_version": "6.x",
            "status": "available",
            "cache_nodes": [
                {
                    "cache_node_id": "0001",
                    "endpoint": {
                        "address": format!("redis-cluster-1.abcdef.{}.cache.amazonaws.com", region),
                        "port": 6379
                    },
                    "status": "available",
                    "parameter_group_status": "in-sync"
                }
            ],
            "preferred_availability_zone": format!("{}a", region),
            "created_at": "2023-03-10T08:30:00Z"
        });
        
        let redis1 = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: AwsResourceType::ElasticacheCluster.to_string(),
            resource_id: "redis-cluster-1".to_string(),
            arn: format!("arn:aws:elasticache:{}:{}:cluster:redis-cluster-1", region, account_id),
            name: Some("Redis Cluster 1".to_string()),
            tags: json!({"Name": "Redis Cluster 1", "Environment": "Development"}),
            resource_data: redis1_data,
        };
        
        // Sample Redis cluster 2
        let redis2_data = json!({
            "cluster_id": "redis-cluster-2",
            "node_type": "cache.t3.small",
            "engine": "redis",
            "engine_version": "6.x",
            "status": "available",
            "cache_nodes": [
                {
                    "cache_node_id": "0001",
                    "endpoint": {
                        "address": format!("redis-cluster-2.abcdef.{}.cache.amazonaws.com", region),
                        "port": 6379
                    },
                    "status": "available",
                    "parameter_group_status": "in-sync"
                }
            ],
            "preferred_availability_zone": format!("{}b", region),
            "created_at": "2023-04-05T14:45:00Z"
        });
        
        let redis2 = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: AwsResourceType::ElasticacheCluster.to_string(),
            resource_id: "redis-cluster-2".to_string(),
            arn: format!("arn:aws:elasticache:{}:{}:cluster:redis-cluster-2", region, account_id),
            name: Some("Redis Cluster 2".to_string()),
            tags: json!({"Name": "Redis Cluster 2", "Environment": "Production"}),
            resource_data: redis2_data,
        };
        
        // Save clusters to database
        let saved_redis1 = match repo.find_by_arn(&redis1.arn).await? {
            Some(existing) => repo.update(existing.id, &redis1).await?,
            None => repo.create(&redis1).await?,
        };
        clusters.push(saved_redis1);
        
        let saved_redis2 = match repo.find_by_arn(&redis2.arn).await? {
            Some(existing) => repo.update(existing.id, &redis2).await?,
            None => repo.create(&redis2).await?,
        };
        clusters.push(saved_redis2);
        
        Ok(clusters)
    }
    
    // Fetch CloudWatch metrics for a resource - backward compatible version
    pub async fn get_metrics(&self, profile: Option<&str>, region: &str, request: &CloudWatchMetricsRequest) -> Result<CloudWatchMetricsResult, AppError> {
        self.get_metrics_with_auth(profile, region, request, None).await
    }
    
    // Fetch CloudWatch metrics with authentication
    pub async fn get_metrics_with_auth(&self, profile: Option<&str>, region: &str, request: &CloudWatchMetricsRequest, account_auth: Option<&AccountAuthInfo>) -> Result<CloudWatchMetricsResult, AppError> {
        let client = self.create_cloudwatch_client_with_auth(profile, region, account_auth).await?;
        
        info!("Fetching metrics for {} ({})", request.resource_id, request.resource_type);
        
        // In a real implementation, this would call get_metric_data
        // For now, we'll create sample data
        
        let now = Utc::now();
        let period = request.period;
        let datapoints_count = ((request.end_time - request.start_time).num_seconds() / period as i64) as usize;
        
        // Define different metrics based on the resource type
        let mut metrics = Vec::new();
        
        match request.resource_type.as_str() {
            "EC2Instance" => {
                // CPU utilization for EC2
                if request.metrics.contains(&"CPUUtilization".to_string()) {
                    let mut datapoints = Vec::with_capacity(datapoints_count);
                    for i in 0..datapoints_count {
                        let timestamp = request.start_time + chrono::Duration::seconds(i as i64 * period as i64);
                        let value = 20.0 + (i as f64 * 0.5) % 60.0; // Sample data with some variation
                        datapoints.push(CloudWatchDatapoint { timestamp, value });
                    }
                    
                    metrics.push(CloudWatchMetricData {
                        metric_name: "CPUUtilization".to_string(),
                        namespace: "AWS/EC2".to_string(),
                        unit: "Percent".to_string(),
                        datapoints,
                    });
                }
                
                // NetworkIn for EC2
                if request.metrics.contains(&"NetworkIn".to_string()) {
                    let mut datapoints = Vec::with_capacity(datapoints_count);
                    for i in 0..datapoints_count {
                        let timestamp = request.start_time + chrono::Duration::seconds(i as i64 * period as i64);
                        let value = 1000.0 + (i as f64 * 50.0) % 5000.0; // Sample data with some variation
                        datapoints.push(CloudWatchDatapoint { timestamp, value });
                    }
                    
                    metrics.push(CloudWatchMetricData {
                        metric_name: "NetworkIn".to_string(),
                        namespace: "AWS/EC2".to_string(),
                        unit: "Bytes".to_string(),
                        datapoints,
                    });
                }
                
                // NetworkOut for EC2
                if request.metrics.contains(&"NetworkOut".to_string()) {
                    let mut datapoints = Vec::with_capacity(datapoints_count);
                    for i in 0..datapoints_count {
                        let timestamp = request.start_time + chrono::Duration::seconds(i as i64 * period as i64);
                        let value = 800.0 + (i as f64 * 40.0) % 4000.0; // Sample data with some variation
                        datapoints.push(CloudWatchDatapoint { timestamp, value });
                    }
                    
                    metrics.push(CloudWatchMetricData {
                        metric_name: "NetworkOut".to_string(),
                        namespace: "AWS/EC2".to_string(),
                        unit: "Bytes".to_string(),
                        datapoints,
                    });
                }
                
                // DiskReadOps for EC2
                if request.metrics.contains(&"DiskReadOps".to_string()) {
                    let mut datapoints = Vec::with_capacity(datapoints_count);
                    for i in 0..datapoints_count {
                        let timestamp = request.start_time + chrono::Duration::seconds(i as i64 * period as i64);
                        let value = 10.0 + (i as f64 * 2.0) % 100.0; // Sample data with some variation
                        datapoints.push(CloudWatchDatapoint { timestamp, value });
                    }
                    
                    metrics.push(CloudWatchMetricData {
                        metric_name: "DiskReadOps".to_string(),
                        namespace: "AWS/EC2".to_string(),
                        unit: "Count".to_string(),
                        datapoints,
                    });
                }
                
                // DiskWriteOps for EC2
                if request.metrics.contains(&"DiskWriteOps".to_string()) {
                    let mut datapoints = Vec::with_capacity(datapoints_count);
                    for i in 0..datapoints_count {
                        let timestamp = request.start_time + chrono::Duration::seconds(i as i64 * period as i64);
                        let value = 15.0 + (i as f64 * 3.0) % 150.0; // Sample data with some variation
                        datapoints.push(CloudWatchDatapoint { timestamp, value });
                    }
                    
                    metrics.push(CloudWatchMetricData {
                        metric_name: "DiskWriteOps".to_string(),
                        namespace: "AWS/EC2".to_string(),
                        unit: "Count".to_string(),
                        datapoints,
                    });
                }
            },
            "ElasticacheCluster" => {
                // CPUUtilization for Redis
                if request.metrics.contains(&"CPUUtilization".to_string()) {
                    let mut datapoints = Vec::with_capacity(datapoints_count);
                    for i in 0..datapoints_count {
                        let timestamp = request.start_time + chrono::Duration::seconds(i as i64 * period as i64);
                        let value = 15.0 + (i as f64 * 0.4) % 40.0; // Sample data with some variation
                        datapoints.push(CloudWatchDatapoint { timestamp, value });
                    }
                    
                    metrics.push(CloudWatchMetricData {
                        metric_name: "CPUUtilization".to_string(),
                        namespace: "AWS/ElastiCache".to_string(),
                        unit: "Percent".to_string(),
                        datapoints,
                    });
                }
                
                // NetworkBytesIn for Redis
                if request.metrics.contains(&"NetworkBytesIn".to_string()) {
                    let mut datapoints = Vec::with_capacity(datapoints_count);
                    for i in 0..datapoints_count {
                        let timestamp = request.start_time + chrono::Duration::seconds(i as i64 * period as i64);
                        let value = 500.0 + (i as f64 * 25.0) % 2500.0; // Sample data with some variation
                        datapoints.push(CloudWatchDatapoint { timestamp, value });
                    }
                    
                    metrics.push(CloudWatchMetricData {
                        metric_name: "NetworkBytesIn".to_string(),
                        namespace: "AWS/ElastiCache".to_string(),
                        unit: "Bytes".to_string(),
                        datapoints,
                    });
                }
                
                // NetworkBytesOut for Redis
                if request.metrics.contains(&"NetworkBytesOut".to_string()) {
                    let mut datapoints = Vec::with_capacity(datapoints_count);
                    for i in 0..datapoints_count {
                        let timestamp = request.start_time + chrono::Duration::seconds(i as i64 * period as i64);
                        let value = 400.0 + (i as f64 * 20.0) % 2000.0; // Sample data with some variation
                        datapoints.push(CloudWatchDatapoint { timestamp, value });
                    }
                    
                    metrics.push(CloudWatchMetricData {
                        metric_name: "NetworkBytesOut".to_string(),
                        namespace: "AWS/ElastiCache".to_string(),
                        unit: "Bytes".to_string(),
                        datapoints,
                    });
                }
                
                // CacheHits for Redis
                if request.metrics.contains(&"CacheHits".to_string()) {
                    let mut datapoints = Vec::with_capacity(datapoints_count);
                    for i in 0..datapoints_count {
                        let timestamp = request.start_time + chrono::Duration::seconds(i as i64 * period as i64);
                        let value = 1000.0 + (i as f64 * 100.0) % 10000.0; // Sample data with some variation
                        datapoints.push(CloudWatchDatapoint { timestamp, value });
                    }
                    
                    metrics.push(CloudWatchMetricData {
                        metric_name: "CacheHits".to_string(),
                        namespace: "AWS/ElastiCache".to_string(),
                        unit: "Count".to_string(),
                        datapoints,
                    });
                }
                
                // CacheMisses for Redis
                if request.metrics.contains(&"CacheMisses".to_string()) {
                    let mut datapoints = Vec::with_capacity(datapoints_count);
                    for i in 0..datapoints_count {
                        let timestamp = request.start_time + chrono::Duration::seconds(i as i64 * period as i64);
                        let value = 10.0 + (i as f64 * 1.0) % 100.0; // Sample data with some variation
                        datapoints.push(CloudWatchDatapoint { timestamp, value });
                    }
                    
                    metrics.push(CloudWatchMetricData {
                        metric_name: "CacheMisses".to_string(),
                        namespace: "AWS/ElastiCache".to_string(),
                        unit: "Count".to_string(),
                        datapoints,
                    });
                }
            },
            _ => {
                return Err(AppError::Validation(format!("Unsupported resource type: {}", request.resource_type)));
            }
        }
        
        // Export metrics if requested
        let mut export_path = None;
        let mut s3_url = None;
        
        // In a real implementation, this would export to file, upload to S3, etc.
        // For now, we'll just log the intent
        
        if let Some(path) = &request.export_path {
            info!("Would export metrics to: {}", path);
            export_path = Some(format!("{}/metrics_{}.json", path, Utc::now().format("%Y%m%d_%H%M%S")));
        }
        
        // Upload to S3 if requested
        if let (Some(true), Some(bucket)) = (request.upload_to_s3, &request.s3_bucket) {
            if let Some(path) = &export_path {
                info!("Would upload metrics to S3: {}/{}", bucket, path);
                s3_url = Some(format!("s3://{}/{}", bucket, path));
            }
        }
        
        // Post to URL if requested
        if let Some(url) = &request.post_to_url {
            info!("Would post metrics to URL: {}", url);
        }
        
        let result = CloudWatchMetricsResult {
            resource_id: request.resource_id.clone(),
            resource_type: request.resource_type.clone(),
            metrics,
            export_path,
            s3_url,
        };
        
        Ok(result)
    }
    
    // Get CloudWatch logs - backward compatible version
    pub async fn get_logs(&self, profile: Option<&str>, region: &str, request: &CloudWatchLogsRequest) -> Result<CloudWatchLogsResult, AppError> {
        self.get_logs_with_auth(profile, region, request, None).await
    }
    
    // Get CloudWatch logs with authentication
    pub async fn get_logs_with_auth(&self, profile: Option<&str>, region: &str, request: &CloudWatchLogsRequest, account_auth: Option<&AccountAuthInfo>) -> Result<CloudWatchLogsResult, AppError> {
        // No longer using CloudWatchLogsClient - simulating with mock data instead
        info!("Fetching logs for log group: {}", request.log_group_name);
        
        // In a real implementation, this would call filter_log_events
        // For now, we'll create sample data
        
        let mut events = Vec::new();
        
        // Generate sample log events
        let num_events = 20;
        for i in 0..num_events {
            let timestamp = request.start_time + chrono::Duration::seconds(i as i64 * 60); // One event per minute
            
            let message = match i % 5 {
                0 => format!("[INFO] Successfully processed request"),
                1 => format!("[DEBUG] Connection established"),
                2 => format!("[INFO] User login successful"),
                3 => format!("[WARN] High memory usage detected"),
                _ => format!("[ERROR] Failed to connect to database, retrying..."),
            };
            
            events.push(CloudWatchLogEvent {
                timestamp,
                message,
                event_id: format!("event-{}", i),
            });
        }
        
        // Export logs if requested
        let mut export_path = None;
        let mut s3_url = None;
        
        // In a real implementation, this would export to file, upload to S3, etc.
        // For now, we'll just log the intent
        
        if let Some(path) = &request.export_path {
            info!("Would export logs to: {}", path);
            export_path = Some(format!("{}/logs_{}.json", path, Utc::now().format("%Y%m%d_%H%M%S")));
        }
        
        // Upload to S3 if requested
        if let (Some(true), Some(bucket)) = (request.upload_to_s3, &request.s3_bucket) {
            if let Some(path) = &export_path {
                info!("Would upload logs to S3: {}/{}", bucket, path);
                s3_url = Some(format!("s3://{}/{}", bucket, path));
            }
        }
        
        // Post to URL if requested
        if let Some(url) = &request.post_to_url {
            info!("Would post logs to URL: {}", url);
        }
        
        let result = CloudWatchLogsResult {
            log_group_name: request.log_group_name.clone(),
            events,
            export_path,
            s3_url,
        };
        
        Ok(result)
    }
    
    // Schedule regular metric collection
    pub async fn schedule_metrics_collection(&self, request: &CloudWatchMetricsRequest, interval_seconds: u64) -> Result<String, AppError> {
        // In a real implementation, this would set up a scheduled task
        // For now, just log the scheduling request
        
        info!("Scheduled metrics collection for {} ({}), interval: {} seconds", 
            request.resource_id, request.resource_type, interval_seconds);
        
        // Return a unique job ID
        let job_id = Uuid::new_v4().to_string();
        
        Ok(job_id)
    }
}