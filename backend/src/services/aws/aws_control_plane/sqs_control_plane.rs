use std::sync::Arc;
use aws_sdk_sqs::Client as SqsClient;
use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

pub struct SqsControlPlane {
    aws_service: Arc<AwsService>,
}

impl SqsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_queues(&self, account_id: &str, aws_account_dto: &AwsAccountDto) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_sqs_client(aws_account_dto).await?;

        // List all queues from AWS
        let response = client.list_queues()
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list SQS queues: {}", e)))?;
            
        let mut queues = Vec::new();
        
    
        for queue_url in response.queue_urls() {
            // Extract queue name from URL
            let queue_name = queue_url
                .split('/')
                .last()
                .unwrap_or_default();
                
            // Get queue attributes
            let attributes_response = client.get_queue_attributes()
                .queue_url(queue_url)
                .attribute_names(aws_sdk_sqs::types::QueueAttributeName::All)
                .send()
                .await
                .map_err(|e| AppError::ExternalService(format!("Failed to get attributes for queue {}: {}", queue_url, e)))?;
                
            // Copy attributes to a local HashMap for easier use
            let mut attributes = std::collections::HashMap::new();
            if let Some(attrs) = attributes_response.attributes() {
                for (key, value) in attrs {
                    attributes.insert(key.as_str().to_string(), value.clone());
                }
            }
            
            // Get queue tags
            let tags_response = client.list_queue_tags()
                .queue_url(queue_url)
                .send()
                .await
                .map_err(|e| AppError::ExternalService(format!("Failed to get tags for queue {}: {}", queue_url, e)))?;
                
            // Process tags
            let mut tags_map = serde_json::Map::new();
            let mut name = None;
            
            if let Some(tags) = tags_response.tags() {
                for (key, value) in tags {
                    if key == "Name" {
                        name = Some(value.to_string());
                    }
                    tags_map.insert(key.to_string(), json!(value));
                }
            }
            
            // If no name tag was found, use the queue name
            if name.is_none() {
                name = Some(queue_name.to_string());
            }
            
            // Get queue ARN from our local HashMap
            let queue_arn = attributes.get("QueueArn")
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("arn:aws:sqs:{}:{}:{}", region, account_id, queue_name));
            
            // Build resource data
            let mut resource_data = serde_json::Map::new();
            
            resource_data.insert("queue_url".to_string(), json!(queue_url));
            
            // Parse numeric attributes
            if let Some(retention_period) = attributes.get("MessageRetentionPeriod") {
                if let Ok(value) = retention_period.parse::<i64>() {
                    resource_data.insert("message_retention_period".to_string(), json!(value));
                }
            }
            
            if let Some(visibility_timeout) = attributes.get("VisibilityTimeout") {
                if let Ok(value) = visibility_timeout.parse::<i64>() {
                    resource_data.insert("visibility_timeout".to_string(), json!(value));
                }
            }
            
            if let Some(delay_seconds) = attributes.get("DelaySeconds") {
                if let Ok(value) = delay_seconds.parse::<i64>() {
                    resource_data.insert("delay_seconds".to_string(), json!(value));
                }
            }
            
            if let Some(wait_time) = attributes.get("ReceiveMessageWaitTimeSeconds") {
                if let Ok(value) = wait_time.parse::<i64>() {
                    resource_data.insert("receive_message_wait_time_seconds".to_string(), json!(value));
                }
            }
            
            if let Some(max_size) = attributes.get("MaximumMessageSize") {
                if let Ok(value) = max_size.parse::<i64>() {
                    resource_data.insert("max_message_size".to_string(), json!(value));
                }
            }
            
            // Check boolean attributes
            let is_fifo = queue_name.ends_with(".fifo") || attributes.get("FifoQueue").map(|v| v == "true").unwrap_or(false);
            resource_data.insert("fifo_queue".to_string(), json!(is_fifo));
            
            if let Some(dedup) = attributes.get("ContentBasedDeduplication") {
                resource_data.insert("content_based_deduplication".to_string(), json!(dedup == "true"));
            }
            
            // Create resource DTO
            let queue = AwsResourceDto {
                id: None,
                account_id: account_id.to_string(),
                profile: profile.profile.clone(),
                region: region.to_string(),
                resource_type: "SqsQueue".to_string(),
                resource_id: queue_name.to_string(),
                arn: queue_arn,
                name,
                tags: serde_json::Value::Object(tags_map),
                resource_data: serde_json::Value::Object(resource_data),
            };
            
            queues.push(queue);
        }

        Ok(queues.into_iter().map(|q| q.into()).collect())
    }
}