use std::sync::Arc;
use uuid::Uuid;
use tracing::{info, error, debug};
use serde::{Deserialize, Serialize};
use serde_json::json;
use chrono::{Utc, DateTime};
use aws_config::BehaviorVersion;
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
            Some(profile_name) => aws_configs.iter()
                .find(|c| c.profile.as_ref().map_or(false, |p| p == profile_name))
                .cloned(),
            None => aws_configs.first().cloned(),
        }.ok_or_else(|| {
            AppError::Configuration(format!("AWS configuration not found for profile: {:?}", profile))
        })?;
        
        Ok(aws_config)
    }

    // Load AWS SDK configuration for a given profile and region
    pub async fn load_aws_sdk_config(&self, profile: Option<&str>, region: &str) -> Result<aws_config::SdkConfig, AppError> {
        let aws_config = self.get_aws_config(profile, region).await?;
        
        let config_builder = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_types::region::Region::new(region.to_string()));
        
        // Apply credentials based on configuration
        let config = if let (Some(access_key), Some(secret_key)) = (&aws_config.access_key_id, &aws_config.secret_access_key) {
            // Use API key authentication
            let creds = aws_types::credential_provider::SharedCredentialsProvider::new(
                aws_types::credentials::Credentials::new(
                    access_key, 
                    secret_key, 
                    None, 
                    None, 
                    "mayyam",
                )
            );
            config_builder.credentials_provider(creds).load().await
        } else if let Some(profile_name) = aws_config.profile {
            // Use named profile
            let provider = aws_config::profile::ProfileFileCredentialsProvider::builder()
                .profile_name(profile_name)
                .build();
            
            config_builder.credentials_provider(provider).load().await
        } else if let Some(role_arn) = aws_config.role_arn {
            // Use assumed role
            // In real implementation, would use STS to assume role
            config_builder.load().await
        } else {
            // Use default credential provider chain
            config_builder.load().await
        };
        
        Ok(config)
    }

    // Create clients for various AWS services
    pub async fn create_ec2_client(&self, profile: Option<&str>, region: &str) -> Result<Ec2Client, AppError> {
        let config = self.load_aws_sdk_config(profile, region).await?;
        Ok(Ec2Client::new(&config))
    }
    
    pub async fn create_s3_client(&self, profile: Option<&str>, region: &str) -> Result<S3Client, AppError> {
        let config = self.load_aws_sdk_config(profile, region).await?;
        Ok(S3Client::new(&config))
    }
    
    pub async fn create_rds_client(&self, profile: Option<&str>, region: &str) -> Result<RdsClient, AppError> {
        let config = self.load_aws_sdk_config(profile, region).await?;
        Ok(RdsClient::new(&config))
    }
    
    pub async fn create_dynamodb_client(&self, profile: Option<&str>, region: &str) -> Result<DynamoDbClient, AppError> {
        let config = self.load_aws_sdk_config(profile, region).await?;
        Ok(DynamoDbClient::new(&config))
    }
    
    pub async fn create_kinesis_client(&self, profile: Option<&str>, region: &str) -> Result<KinesisClient, AppError> {
        let config = self.load_aws_sdk_config(profile, region).await?;
        Ok(KinesisClient::new(&config))
    }
    
    pub async fn create_sqs_client(&self, profile: Option<&str>, region: &str) -> Result<SqsClient, AppError> {
        let config = self.load_aws_sdk_config(profile, region).await?;
        Ok(SqsClient::new(&config))
    }
    
    pub async fn create_sns_client(&self, profile: Option<&str>, region: &str) -> Result<SnsClient, AppError> {
        let config = self.load_aws_sdk_config(profile, region).await?;
        Ok(SnsClient::new(&config))
    }
    
    pub async fn create_lambda_client(&self, profile: Option<&str>, region: &str) -> Result<LambdaClient, AppError> {
        let config = self.load_aws_sdk_config(profile, region).await?;
        Ok(LambdaClient::new(&config))
    }
    
    pub async fn create_elasticache_client(&self, profile: Option<&str>, region: &str) -> Result<ElasticacheClient, AppError> {
        let config = self.load_aws_sdk_config(profile, region).await?;
        Ok(ElasticacheClient::new(&config))
    }
    
    pub async fn create_opensearch_client(&self, profile: Option<&str>, region: &str) -> Result<OpenSearchClient, AppError> {
        let config = self.load_aws_sdk_config(profile, region).await?;
        Ok(OpenSearchClient::new(&config))
    }
    
    pub async fn create_cost_explorer_client(&self, profile: Option<&str>, region: &str) -> Result<CostExplorerClient, AppError> {
        let config = self.load_aws_sdk_config(profile, region).await?;
        Ok(CostExplorerClient::new(&config))
    }
}

impl AwsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    // Sync EC2 instances for an account and region
    pub async fn sync_ec2_instances(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_ec2_client(profile, region).await?;
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
    
    // Sync S3 buckets for an account
    pub async fn sync_s3_buckets(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_s3_client(profile, region).await?;
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

    // Sync RDS instances
    pub async fn sync_rds_instances(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_rds_client(profile, region).await?;
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

    // Sync DynamoDB tables
    pub async fn sync_dynamodb_tables(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_dynamodb_client(profile, region).await?;
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
                    let instances = self.sync_ec2_instances(account_id, profile, region).await?;
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: AwsResourceType::EC2Instance.to_string(),
                        count: instances.len(),
                        status: "success".to_string(),
                        details: None,
                    });
                    total_resources += instances.len();
                    Ok(())
                },
                "S3Bucket" => {
                    let buckets = self.sync_s3_buckets(account_id, profile, region).await?;
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: AwsResourceType::S3Bucket.to_string(),
                        count: buckets.len(),
                        status: "success".to_string(),
                        details: None,
                    });
                    total_resources += buckets.len();
                    Ok(())
                },
                "RdsInstance" => {
                    let instances = self.sync_rds_instances(account_id, profile, region).await?;
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: AwsResourceType::RdsInstance.to_string(),
                        count: instances.len(),
                        status: "success".to_string(),
                        details: None,
                    });
                    total_resources += instances.len();
                    Ok(())
                },
                "DynamoDbTable" => {
                    let tables = self.sync_dynamodb_tables(account_id, profile, region).await?;
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: AwsResourceType::DynamoDbTable.to_string(),
                        count: tables.len(),
                        status: "success".to_string(),
                        details: None,
                    });
                    total_resources += tables.len();
                    Ok(())
                },
                "ElasticacheCluster" => {
                    // Create a CloudWatchService and use it to sync ElastiCache clusters
                    let cloudwatch_service = CloudWatchService::new(self.aws_service.clone());
                    let clusters = cloudwatch_service.sync_elasticache_clusters(account_id, profile, region).await?;
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: AwsResourceType::ElasticacheCluster.to_string(),
                        count: clusters.len(),
                        status: "success".to_string(),
                        details: None,
                    });
                    total_resources += clusters.len();
                    Ok(())
                },
                _ => {
                    summary.push(ResourceTypeSyncSummary {
                        resource_type: resource_type.clone(),
                        count: 0,
                        status: "skipped".to_string(),
                        details: Some("Resource type not supported".to_string()),
                    });
                    Ok(())
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
    
    // Create CloudWatch client
    pub async fn create_cloudwatch_client(&self, profile: Option<&str>, region: &str) -> Result<CloudWatchClient, AppError> {
        let config = self.aws_service.load_aws_sdk_config(profile, region).await?;
        Ok(CloudWatchClient::new(&config))
    }
    
    // Fetch CloudWatch metrics for a resource
    pub async fn get_metrics(&self, profile: Option<&str>, region: &str, request: &CloudWatchMetricsRequest) -> Result<CloudWatchMetricsResult, AppError> {
        let client = self.create_cloudwatch_client(profile, region).await?;
        
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
        
        if let Some(path) = &request.export_path {
            export_path = Some(self.export_metrics_to_file(&request.resource_id, &request.resource_type, &metrics, path).await?);
        }
        
        // Upload to S3 if requested
        if let (Some(true), Some(bucket)) = (request.upload_to_s3, &request.s3_bucket) {
            if let Some(path) = &export_path {
                s3_url = Some(self.upload_metrics_to_s3(profile, region, bucket, &request.resource_id, &request.resource_type, path).await?);
            }
        }
        
        // Post to URL if requested
        if let Some(url) = &request.post_to_url {
            self.post_metrics_to_url(url, &metrics).await?;
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
    
    // Export metrics to a file
    async fn export_metrics_to_file(&self, resource_id: &str, resource_type: &str, metrics: &[CloudWatchMetricData], base_path: &str) -> Result<String, AppError> {
        let now = Utc::now();
        let year = now.format("%Y").to_string();
        let month = now.format("%m").to_string();
        let day = now.format("%d").to_string();
        
        let export_dir = Path::new(base_path)
            .join(resource_type)
            .join(resource_id)
            .join(&year)
            .join(&month)
            .join(&day);
        
        // Create directory structure
        create_dir_all(&export_dir)
            .map_err(|e| AppError::IO(format!("Failed to create export directory: {}", e)))?;
        
        let file_name = format!("metrics_{}.json", now.format("%H%M%S"));
        let file_path = export_dir.join(&file_name);
        
        // Create and write to file
        let mut file = File::create(&file_path)
            .map_err(|e| AppError::IO(format!("Failed to create metrics file: {}", e)))?;
        
        let json = serde_json::to_string_pretty(metrics)
            .map_err(|e| AppError::Serialization(format!("Failed to serialize metrics: {}", e)))?;
        
        file.write_all(json.as_bytes())
            .map_err(|e| AppError::IO(format!("Failed to write metrics to file: {}", e)))?;
        
        info!("Exported metrics to file: {:?}", file_path);
        
        Ok(file_path.to_string_lossy().to_string())
    }
    
    // Upload metrics to S3
    async fn upload_metrics_to_s3(&self, profile: Option<&str>, region: &str, bucket: &str, resource_id: &str, resource_type: &str, file_path: &str) -> Result<String, AppError> {
        let client = self.aws_service.create_s3_client(profile, region).await?;
        
        // In a real implementation, this would call put_object with the file content
        
        // Extract just the file name from the path
        let file_name = Path::new(file_path)
            .file_name()
            .ok_or_else(|| AppError::Validation("Invalid file path".to_string()))?
            .to_string_lossy();
        
        // Create S3 key with the same directory structure as local
        let now = Utc::now();
        let year = now.format("%Y").to_string();
        let month = now.format("%m").to_string();
        let day = now.format("%d").to_string();
        
        let s3_key = format!("{}/{}/{}/{}/{}/{}", resource_type, resource_id, year, month, day, file_name);
        
        info!("Uploading metrics to S3: {}/{}", bucket, s3_key);
        
        // Return the S3 URL
        let s3_url = format!("s3://{}/{}", bucket, s3_key);
        
        Ok(s3_url)
    }
    
    // Post metrics to an API endpoint
    async fn post_metrics_to_url(&self, url: &str, metrics: &[CloudWatchMetricData]) -> Result<(), AppError> {
        info!("Posting metrics to URL: {}", url);
        
        // In a real implementation, this would make an HTTP POST request
        // For now, just log the action
        
        Ok(())
    }
    
    // Fetch CloudWatch logs for a resource
    pub async fn get_logs(&self, profile: Option<&str>, region: &str, request: &CloudWatchLogsRequest) -> Result<CloudWatchLogsResult, AppError> {
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
        
        if let Some(path) = &request.export_path {
            export_path = Some(self.export_logs_to_file(&request.log_group_name, &events, path).await?);
        }
        
        // Upload to S3 if requested
        if let (Some(true), Some(bucket)) = (request.upload_to_s3, &request.s3_bucket) {
            if let Some(path) = &export_path {
                s3_url = Some(self.upload_logs_to_s3(profile, region, bucket, &request.log_group_name, path).await?);
            }
        }
        
        // Post to URL if requested
        if let Some(url) = &request.post_to_url {
            self.post_logs_to_url(url, &events).await?;
        }
        
        let result = CloudWatchLogsResult {
            log_group_name: request.log_group_name.clone(),
            events,
            export_path,
            s3_url,
        };
        
        Ok(result)
    }
    
    // Export logs to a file
    async fn export_logs_to_file(&self, log_group_name: &str, events: &[CloudWatchLogEvent], base_path: &str) -> Result<String, AppError> {
        let now = Utc::now();
        let year = now.format("%Y").to_string();
        let month = now.format("%m").to_string();
        let day = now.format("%d").to_string();
        
        let sanitized_group_name = log_group_name.replace("/", "_");
        
        let export_dir = Path::new(base_path)
            .join("logs")
            .join(&sanitized_group_name)
            .join(&year)
            .join(&month)
            .join(&day);
        
        // Create directory structure
        create_dir_all(&export_dir)
            .map_err(|e| AppError::IO(format!("Failed to create export directory: {}", e)))?;
        
        let file_name = format!("logs_{}.json", now.format("%H%M%S"));
        let file_path = export_dir.join(&file_name);
        
        // Create and write to file
        let mut file = File::create(&file_path)
            .map_err(|e| AppError::IO(format!("Failed to create logs file: {}", e)))?;
        
        let json = serde_json::to_string_pretty(events)
            .map_err(|e| AppError::Serialization(format!("Failed to serialize logs: {}", e)))?;
        
        file.write_all(json.as_bytes())
            .map_err(|e| AppError::IO(format!("Failed to write logs to file: {}", e)))?;
        
        info!("Exported logs to file: {:?}", file_path);
        
        Ok(file_path.to_string_lossy().to_string())
    }
    
    // Upload logs to S3
    async fn upload_logs_to_s3(&self, profile: Option<&str>, region: &str, bucket: &str, log_group_name: &str, file_path: &str) -> Result<String, AppError> {
        let client = self.aws_service.create_s3_client(profile, region).await?;
        
        // In a real implementation, this would call put_object with the file content
        
        // Extract just the file name from the path
        let file_name = Path::new(file_path)
            .file_name()
            .ok_or_else(|| AppError::Validation("Invalid file path".to_string()))?
            .to_string_lossy();
        
        // Create S3 key with the same directory structure as local
        let now = Utc::now();
        let year = now.format("%Y").to_string();
        let month = now.format("%m").to_string();
        let day = now.format("%d").to_string();
        
        let sanitized_group_name = log_group_name.replace("/", "_");
        let s3_key = format!("logs/{}/{}/{}/{}/{}", sanitized_group_name, year, month, day, file_name);
        
        info!("Uploading logs to S3: {}/{}", bucket, s3_key);
        
        // Return the S3 URL
        let s3_url = format!("s3://{}/{}", bucket, s3_key);
        
        Ok(s3_url)
    }
    
    // Post logs to an API endpoint
    async fn post_logs_to_url(&self, url: &str, events: &[CloudWatchLogEvent]) -> Result<(), AppError> {
        info!("Posting logs to URL: {}", url);
        
        // In a real implementation, this would make an HTTP POST request
        // For now, just log the action
        
        Ok(())
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
    
    // Add ElastiCache cluster discovery
    pub async fn sync_elasticache_clusters(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_elasticache_client(profile, region).await?;
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
}