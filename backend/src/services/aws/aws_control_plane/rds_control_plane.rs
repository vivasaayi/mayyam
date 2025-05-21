use std::sync::Arc;
use aws_sdk_rds::Client as RdsClient;
use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

// Control plane implementation for RDS
pub struct RdsControlPlane {
    aws_service: Arc<AwsService>,
}

impl RdsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_instances(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_instances_with_auth(account_id, profile, region, None).await
    }
    
    pub async fn sync_instances_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_rds_client_with_auth(profile, region, account_auth).await?;
        self.sync_instances_with_client(account_id, profile, region, client).await
    }
    
    async fn sync_instances_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, client: RdsClient) -> Result<Vec<AwsResourceModel>, AppError> {
        // Get DB instances from AWS
        let response = client.describe_db_instances()
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to describe RDS instances: {}", e)))?;
            
        let mut instances = Vec::new();
        
        if let Some(db_instances) = response.db_instances() {
            for db_instance in db_instances {
                let db_identifier = db_instance.db_instance_identifier().unwrap_or_default();
                
                // Get resource ARN
                let arn = format!("arn:aws:rds:{}:{}:db:{}", region, account_id, db_identifier);
                
                // Get tags for this instance
                let tags_response = client.list_tags_for_resource()
                    .resource_name(&arn)
                    .send()
                    .await
                    .map_err(|e| AppError::ExternalService(format!("Failed to get tags for RDS instance {}: {}", db_identifier, e)))?;
                
                let mut tags_map = serde_json::Map::new();
                let mut name = None;
                
                if let Some(tag_list) = tags_response.tag_list() {
                    for tag in tag_list {
                        if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                            if key == "Name" {
                                name = Some(value.to_string());
                            }
                            tags_map.insert(key.to_string(), json!(value));
                        }
                    }
                }
                
                // If no name tag was found, use the identifier as name
                if name.is_none() {
                    name = Some(db_identifier.to_string());
                }
                
                // Build resource data
                let mut resource_data = serde_json::Map::new();
                
                // Add basic instance information
                resource_data.insert("db_instance_identifier".to_string(), json!(db_identifier));
                
                if let Some(engine) = db_instance.engine() {
                    resource_data.insert("engine".to_string(), json!(engine));
                }
                
                if let Some(version) = db_instance.engine_version() {
                    resource_data.insert("engine_version".to_string(), json!(version));
                }
                
                if let Some(class) = db_instance.db_instance_class() {
                    resource_data.insert("instance_class".to_string(), json!(class));
                }
                
                let storage = db_instance.allocated_storage();
                resource_data.insert("allocated_storage".to_string(), json!(storage));
                
                // Add endpoint information
                if let Some(endpoint) = db_instance.endpoint() {
                    let mut endpoint_data = serde_json::Map::new();
                    
                    if let Some(address) = endpoint.address() {
                        endpoint_data.insert("address".to_string(), json!(address));
                    }
                    
                    endpoint_data.insert("port".to_string(), json!(endpoint.port()));
                    
                    if let Some(hosted_zone_id) = endpoint.hosted_zone_id() {
                        endpoint_data.insert("hosted_zone_id".to_string(), json!(hosted_zone_id));
                    }
                    
                    resource_data.insert("endpoint".to_string(), json!(endpoint_data));
                }
                
                if let Some(status) = db_instance.db_instance_status() {
                    resource_data.insert("status".to_string(), json!(status));
                }
                
                if let Some(az) = db_instance.availability_zone() {
                    resource_data.insert("availability_zone".to_string(), json!(az));
                }
                
                resource_data.insert("multi_az".to_string(), json!(db_instance.multi_az()));
                
                if let Some(storage_type) = db_instance.storage_type() {
                    resource_data.insert("storage_type".to_string(), json!(storage_type));
                }
                
                resource_data.insert("backup_retention_period".to_string(), json!(db_instance.backup_retention_period()));
                
                // Create resource DTO
                let instance = AwsResourceDto {
                    id: None,
                    account_id: account_id.to_string(),
                    profile: profile.map(|p| p.to_string()),
                    region: region.to_string(),
                    resource_type: "RdsInstance".to_string(),
                    resource_id: db_identifier.to_string(),
                    arn,
                    name,
                    tags: serde_json::Value::Object(tags_map),
                    resource_data: serde_json::Value::Object(resource_data),
                };
                
                instances.push(instance);
            }
        }

        Ok(instances.into_iter().map(|i| i.into()).collect())
    }
}