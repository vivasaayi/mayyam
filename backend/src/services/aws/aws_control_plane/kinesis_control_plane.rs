use std::sync::Arc;
use aws_sdk_kinesis::Client as KinesisClient;

use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use crate::services::aws::aws_types::kinesis::{
    KinesisCreateStreamRequest, KinesisDeleteStreamRequest, KinesisDescribeStreamRequest,
    KinesisListStreamsRequest, KinesisUpdateShardCountRequest, KinesisAddTagsRequest,
    KinesisRemoveTagsRequest, KinesisUpdateStreamModeRequest, KinesisStartEncryptionRequest,
    KinesisStopEncryptionRequest
};

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

    // Control plane operations
    pub async fn create_stream(&self, profile: Option<&str>, region: &str, request: &KinesisCreateStreamRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        let shard_count = request.shard_count
            .ok_or_else(|| AppError::ExternalService("Missing shard_count in create stream request".to_string()))?;

        client.create_stream()
            .stream_name(&request.stream_name)
            .shard_count(shard_count)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kinesis stream: {}", e)))?;

        Ok(json!({
            "stream_name": request.stream_name,
            "stream_arn": format!("arn:aws:kinesis:{}:123456789012:stream/{}", region, request.stream_name),
            "status": "CREATING"
        }))
    }

    pub async fn delete_stream(&self, profile: Option<&str>, region: &str, request: &KinesisDeleteStreamRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        client.delete_stream()
            .stream_name(&request.stream_name)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to delete Kinesis stream: {}", e)))?;

        Ok(json!({
            "stream_name": request.stream_name,
            "status": "DELETING"
        }))
    }

    pub async fn describe_stream(&self, profile: Option<&str>, region: &str, request: &KinesisDescribeStreamRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        let response = client.describe_stream()
            .stream_name(&request.stream_name)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to describe Kinesis stream: {}", e)))?;

        if let Some(stream_desc) = response.stream_description() {
            let stream_arn = stream_desc.stream_arn()
                .ok_or_else(|| AppError::ExternalService("Missing stream ARN in describe response".to_string()))?;
            let stream_status = stream_desc.stream_status()
                .map(|s| s.as_str())
                .ok_or_else(|| AppError::ExternalService("Missing stream status in describe response".to_string()))?;

            Ok(json!({
                "stream_name": request.stream_name,
                "stream_arn": stream_arn,
                "stream_status": stream_status,
                "stream_mode_details": {
                    "stream_mode": stream_desc.stream_mode_details()
                        .and_then(|smd| smd.stream_mode())
                        .map(|sm| sm.as_str())
                        .unwrap_or("PROVISIONED")
                },
                "shards": stream_desc.shards().unwrap_or_default().iter().map(|shard| {
                    json!({
                        "shard_id": shard.shard_id().unwrap_or("unknown"),
                        "hash_key_range": {
                            "starting_hash_key": shard.hash_key_range()
                                .and_then(|hkr| hkr.starting_hash_key())
                                .unwrap_or("0"),
                            "ending_hash_key": shard.hash_key_range()
                                .and_then(|hkr| hkr.ending_hash_key())
                                .unwrap_or("340282366920938463463374607431768211455")
                        },
                        "sequence_number_range": {
                            "starting_sequence_number": shard.sequence_number_range()
                                .and_then(|snr| snr.starting_sequence_number())
                                .unwrap_or("0")
                        }
                    })
                }).collect::<Vec<_>>(),
                "retention_period_hours": stream_desc.retention_period_hours().unwrap_or(24),
                "encryption_type": stream_desc.encryption_type().map(|et| et.as_str()).unwrap_or("NONE")
            }))
        } else {
            Err(AppError::ExternalService("No stream description found in AWS response".to_string()))
        }
    }

    pub async fn list_streams(&self, profile: Option<&str>, region: &str, request: &KinesisListStreamsRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        let mut list_builder = client.list_streams();

        if let Some(next_token) = &request.next_token {
            list_builder = list_builder.next_token(next_token);
        }

        // Note: max_results parameter is not supported in the current AWS SDK version
        // if let Some(max_results) = request.max_results {
        //     list_builder = list_builder.max_results(max_results as i32);
        // }

        let response = list_builder
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list Kinesis streams: {}", e)))?;

        Ok(json!({
            "stream_names": response.stream_names().unwrap_or_default(),
            "has_more_streams": response.has_more_streams().unwrap_or(false),
            "next_token": response.next_token(),
            "stream_summaries": response.stream_summaries().unwrap_or_default().iter().map(|summary| {
                json!({
                    "stream_name": summary.stream_name(),
                    "stream_arn": summary.stream_arn(),
                    "stream_status": summary.stream_status().map(|s| s.as_str()),
                    "stream_mode_details": {
                        "stream_mode": summary.stream_mode_details()
                            .and_then(|smd| smd.stream_mode())
                            .map(|sm| sm.as_str())
                    },
                    "stream_creation_timestamp": summary.stream_creation_timestamp()
                        .map(|ts| format!("{:?}", ts))
                })
            }).collect::<Vec<_>>()
        }))
    }

    pub async fn update_shard_count(&self, profile: Option<&str>, region: &str, request: &KinesisUpdateShardCountRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        let scaling_type = match request.scaling_type.as_str() {
            "UNIFORM_SCALING" => aws_sdk_kinesis::types::ScalingType::UniformScaling,
            _ => return Err(AppError::ExternalService("Invalid scaling type. Must be UNIFORM_SCALING".to_string())),
        };

        client.update_shard_count()
            .stream_name(&request.stream_name)
            .target_shard_count(request.target_shard_count)
            .scaling_type(scaling_type)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to update shard count: {}", e)))?;

        Ok(json!({
            "stream_name": request.stream_name,
            "target_shard_count": request.target_shard_count,
            "scaling_type": request.scaling_type,
            "status": "UPDATING"
        }))
    }

    pub async fn add_tags_to_stream(&self, profile: Option<&str>, region: &str, request: &KinesisAddTagsRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        client.add_tags_to_stream()
            .stream_name(&request.stream_name)
            .set_tags(Some(request.tags.clone()))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to add tags to stream: {}", e)))?;

        Ok(json!({
            "stream_name": request.stream_name,
            "tags_added": request.tags.len(),
            "status": "SUCCESS"
        }))
    }

    pub async fn remove_tags_from_stream(&self, profile: Option<&str>, region: &str, request: &KinesisRemoveTagsRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        client.remove_tags_from_stream()
            .stream_name(&request.stream_name)
            .set_tag_keys(Some(request.tag_keys.clone()))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to remove tags from stream: {}", e)))?;

        Ok(json!({
            "stream_name": request.stream_name,
            "tags_removed": request.tag_keys.len(),
            "status": "SUCCESS"
        }))
    }

    pub async fn list_tags_for_stream(&self, profile: Option<&str>, region: &str, stream_name: &str) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        let response = client.list_tags_for_stream()
            .stream_name(stream_name)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list tags for stream: {}", e)))?;

        let tags: std::collections::HashMap<String, String> = if let Some(tags_list) = response.tags() {
            tags_list.iter()
                .filter_map(|tag| {
                    if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                        Some((key.to_string(), value.to_string()))
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            std::collections::HashMap::new()
        };

        Ok(json!({
            "stream_name": stream_name,
            "tags": tags
        }))
    }

    pub async fn update_stream_mode(&self, profile: Option<&str>, region: &str, request: &KinesisUpdateStreamModeRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        let stream_mode = match request.stream_mode_details.stream_mode.as_str() {
            "PROVISIONED" => aws_sdk_kinesis::types::StreamMode::Provisioned,
            "ON_DEMAND" => aws_sdk_kinesis::types::StreamMode::OnDemand,
            _ => return Err(AppError::ExternalService("Invalid stream mode. Must be PROVISIONED or ON_DEMAND".to_string())),
        };

        let stream_mode_details = aws_sdk_kinesis::types::StreamModeDetails::builder()
            .stream_mode(stream_mode)
            .build();

        client.update_stream_mode()
            .stream_arn(&request.stream_name)
            .stream_mode_details(stream_mode_details)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to update stream mode: {}", e)))?;

        Ok(json!({
            "stream_name": request.stream_name,
            "stream_mode": request.stream_mode_details.stream_mode,
            "status": "UPDATING"
        }))
    }

    pub async fn start_stream_encryption(&self, profile: Option<&str>, region: &str, request: &KinesisStartEncryptionRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        let encryption_type = match request.encryption_type.as_str() {
            "KMS" => aws_sdk_kinesis::types::EncryptionType::Kms,
            _ => return Err(AppError::ExternalService("Invalid encryption type. Must be KMS".to_string())),
        };

        client.start_stream_encryption()
            .stream_name(&request.stream_name)
            .encryption_type(encryption_type)
            .key_id(&request.key_id)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to start stream encryption: {}", e)))?;

        Ok(json!({
            "stream_name": request.stream_name,
            "encryption_type": request.encryption_type,
            "key_id": request.key_id,
            "status": "ENCRYPTING"
        }))
    }

    pub async fn stop_stream_encryption(&self, profile: Option<&str>, region: &str, request: &KinesisStopEncryptionRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        client.stop_stream_encryption()
            .stream_name(&request.stream_name)
            .encryption_type(aws_sdk_kinesis::types::EncryptionType::Kms)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to stop stream encryption: {}", e)))?;

        Ok(json!({
            "stream_name": request.stream_name,
            "status": "DECRYPTING"
        }))
    }
}