use std::sync::Arc;
use aws_sdk_s3::Client as S3Client;

use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

// Control plane implementation for S3
pub struct S3ControlPlane {
    aws_service: Arc<AwsService>,
}

impl S3ControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_buckets(&self, account_id: &str, profile: &AwsAccountDto, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_buckets_with_auth(account_id, profile, region, None).await
    }
    
    pub async fn sync_buckets_with_auth(&self, account_id: &str, profile: &AwsAccountDto, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_s3_client_with_auth(profile, region).await?;
        self.sync_buckets_with_client(account_id, profile, region, client).await
    }
    
    async fn sync_buckets_with_client(&self, account_id: &str, profile: &AwsAccountDto, region: &str, client: S3Client) -> Result<Vec<AwsResourceModel>, AppError> {
        // Get buckets from AWS
        let response = client.list_buckets()
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list S3 buckets: {}", e)))?;
            
        let mut buckets = Vec::new();
        
        if let Some(aws_buckets) = response.buckets() {
            for aws_bucket in aws_buckets {
                let bucket_name = aws_bucket.name().unwrap_or_default();
                
                // Get bucket location/region
                let location_resp = client.get_bucket_location()
                    .bucket(bucket_name)
                    .send()
                    .await
                    .map_err(|e| AppError::ExternalService(format!("Failed to get bucket location for {}: {}", bucket_name, e)))?;
                    
                let bucket_region = location_resp.location_constraint()
                    .map(|c| c.as_str())
                    .unwrap_or(region);
                
                if bucket_region != region && bucket_region != "us-east-1" {
                    // Skip buckets that don't match our region
                    continue;
                }
                
                // Get bucket tags if available
                let mut tags_map = serde_json::Map::new();
                
                match client.get_bucket_tagging()
                    .bucket(bucket_name)
                    .send()
                    .await {
                        Ok(tagging_resp) => {
                            if let Some(tag_set) = tagging_resp.tag_set() {
                                for tag in tag_set {
                                    if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                                        tags_map.insert(key.to_string(), json!(value));
                                    }
                                }
                            }
                        },
                        Err(_) => {
                            // Bucket might not have tags, that's okay
                        }
                    };
                
                // Check versioning status
                let versioning_resp = client.get_bucket_versioning()
                    .bucket(bucket_name)
                    .send()
                    .await
                    .map_err(|e| AppError::ExternalService(format!("Failed to get versioning for {}: {}", bucket_name, e)))?;
                    
                let versioning_enabled = versioning_resp.status().map(|s| s.as_str() == "Enabled").unwrap_or(false);
                
                // Check lifecycle rules if available
                let mut lifecycle_rules = Vec::new();
                
                match client.get_bucket_lifecycle_configuration()
                    .bucket(bucket_name)
                    .send()
                    .await {
                        Ok(lifecycle_resp) => {
                            if let Some(rules) = lifecycle_resp.rules() {
                                for rule in rules {
                                    let mut rule_data = serde_json::Map::new();
                                    
                                    if let Some(id) = rule.id() {
                                        rule_data.insert("id".to_string(), json!(id));
                                    }
                                    
                                    if let Some(prefix) = rule.prefix() {
                                        rule_data.insert("prefix".to_string(), json!(prefix));
                                    }
                                    
                                    if let Some(status) = rule.status().map(|s| s.as_str()) {
                                        rule_data.insert("status".to_string(), json!(status));
                                    }
                                    
                                    // Process transitions
                                    if let Some(transitions) = rule.transitions() {
                                        for transition in transitions {
                                            // Days is not an Option in the AWS SDK
                                    rule_data.insert("transition_days".to_string(), json!(transition.days()));
                                            
                                            if let Some(storage_class) = transition.storage_class().map(|s| s.as_str()) {
                                                rule_data.insert("storage_class".to_string(), json!(storage_class));
                                            }
                                        }
                                    }
                                    
                                    lifecycle_rules.push(serde_json::Value::Object(rule_data));
                                }
                            }
                        },
                        Err(_) => {
                            // Bucket might not have lifecycle rules, that's okay
                        }
                    };
                
                // Build resource data
                let mut resource_data = serde_json::Map::new();
                
                if let Some(creation_date) = aws_bucket.creation_date() {
                    // Properly handle the Result from fmt
                    if let Ok(formatted_date) = creation_date.fmt(aws_smithy_types::date_time::Format::DateTime) {
                        resource_data.insert("creation_date".to_string(), json!(formatted_date));
                    }
                }
                
                resource_data.insert("region".to_string(), json!(bucket_region));
                resource_data.insert("versioning_enabled".to_string(), json!(versioning_enabled));
                
                if !lifecycle_rules.is_empty() {
                    resource_data.insert("lifecycle_rules".to_string(), json!(lifecycle_rules));
                }
                
                // Create resource DTO
                let bucket = AwsResourceDto {
                    id: None,
                    account_id: account_id.to_string(),
                    profile: profile.map(|p| p.to_string()),
                    region: region.to_string(),
                    resource_type: "S3Bucket".to_string(),
                    resource_id: bucket_name.to_string(),
                    arn: format!("arn:aws:s3:::{}", bucket_name),
                    name: Some(bucket_name.to_string()),
                    tags: serde_json::Value::Object(tags_map),
                    resource_data: serde_json::Value::Object(resource_data),
                };
                
                buckets.push(bucket);
            }
        }

        Ok(buckets.into_iter().map(|b| b.into()).collect())
    }
}