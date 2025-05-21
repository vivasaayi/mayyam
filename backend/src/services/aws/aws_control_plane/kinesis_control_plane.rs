use std::sync::Arc;
use aws_sdk_kinesis::Client as KinesisClient;

use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

// Control plane implementation for Kinesis
pub struct KinesisControlPlane {
    aws_service: Arc<AwsService>,
}

impl KinesisControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_streams(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_streams_with_auth(account_id, profile, region, None).await
    }
    
    pub async fn sync_streams_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_kinesis_client_with_auth(profile, region, account_auth).await?;
        self.sync_streams_with_client(account_id, profile, region, client).await
    }
    
    async fn sync_streams_with_client(&self, account_id: &str, profile: Option<&str>, region: &str, client: KinesisClient) -> Result<Vec<AwsResourceModel>, AppError> {
        // List streams from AWS
        let response = client.list_streams()
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list Kinesis streams: {}", e)))?;
            
        let stream_names = response.stream_names().unwrap_or_default();
        
        let mut streams = Vec::new();
        
        for stream_name in stream_names {
            // Get detailed information for each stream
            let describe_resp = client.describe_stream_summary()
                .stream_name(stream_name)
                .send()
                .await
                .map_err(|e| AppError::ExternalService(format!("Failed to describe Kinesis stream {}: {}", stream_name, e)))?;
                
            let stream_desc = describe_resp.stream_description_summary()
                .ok_or_else(|| AppError::ExternalService(format!("No description found for stream {}", stream_name)))?;
                
            // Get tags for the stream
            let arn = format!("arn:aws:kinesis:{}:{}:stream/{}", region, account_id, stream_name);
            
            let tags_response = client.list_tags_for_stream()
                .stream_name(stream_name)
                .send()
                .await
                .map_err(|e| AppError::ExternalService(format!("Failed to get tags for stream {}: {}", stream_name, e)))?;
                
            let mut tags_map = serde_json::Map::new();
            let mut name = None;
            
            if let Some(tags) = tags_response.tags() {
                for tag in tags {
                    if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                        if key == "Name" {
                            name = Some(value.to_string());
                        }
                        tags_map.insert(key.to_string(), json!(value));
                    }
                }
            }
            
            // If no name tag was found, use the stream name as name
            if name.is_none() {
                name = Some(stream_name.to_string());
            }
            
            // Get enhanced monitoring information
            // Process enhanced monitoring data
            // stream_desc.enhanced_monitoring() returns a slice of EnhancedMetrics
            let enhanced_monitoring: Vec<serde_json::Value> = if let Some(monitoring) = stream_desc.enhanced_monitoring() {
                monitoring.iter()
                    .map(|e_metrics| {
                        // Each e_metrics is an EnhancedMetrics object
                        if let Some(shard_metrics) = e_metrics.shard_level_metrics() {
                            let metrics_vec: Vec<String> = shard_metrics.iter()
                                .map(|m| m.as_str().to_string())
                                .collect();
                            json!(metrics_vec)
                        } else {
                            json!([])
                        }
                    })
                    .collect()
            } else {
                vec![]
            };
                
            // Build resource data
            let mut resource_data = serde_json::Map::new();
            
            resource_data.insert("stream_name".to_string(), json!(stream_name));
            
            if let Some(status) = stream_desc.stream_status().map(|s| s.as_str()) {
                resource_data.insert("stream_status".to_string(), json!(status));
            }
            
            if let Some(retention) = stream_desc.retention_period_hours() {
                resource_data.insert("retention_period_hours".to_string(), json!(retention));
            }
            
            if let Some(shard_count) = stream_desc.open_shard_count() {
                resource_data.insert("shard_count".to_string(), json!(shard_count));
            }
            
            if let Some(open_shard_count) = stream_desc.open_shard_count() {
                resource_data.insert("open_shard_count".to_string(), json!(open_shard_count));
            }
            
            if !enhanced_monitoring.is_empty() {
                resource_data.insert("enhanced_monitoring".to_string(), json!(enhanced_monitoring));
            }
            
            if let Some(encryption) = stream_desc.encryption_type().map(|e| e.as_str()) {
                resource_data.insert("encryption_type".to_string(), json!(encryption));
            }
            
            if let Some(creation_time) = stream_desc.stream_creation_timestamp() {
                if let Ok(formatted_time) = creation_time.fmt(aws_smithy_types::date_time::Format::DateTime) {
                    resource_data.insert("creation_timestamp".to_string(), json!(formatted_time));
                } else {
                    // Fall back to seconds since epoch
                    resource_data.insert("creation_timestamp".to_string(), 
                        json!(creation_time.as_secs_f64().to_string()));
                }
            }
            
            // Create resource DTO
            let stream = AwsResourceDto {
                id: None,
                account_id: account_id.to_string(),
                profile: profile.map(|p| p.to_string()),
                region: region.to_string(),
                resource_type: "KinesisStream".to_string(),
                resource_id: stream_name.to_string(),
                arn,
                name,
                tags: serde_json::Value::Object(tags_map),
                resource_data: serde_json::Value::Object(resource_data),
            };
            
            streams.push(stream);
        }

        Ok(streams.into_iter().map(|s| s.into()).collect())
    }
}