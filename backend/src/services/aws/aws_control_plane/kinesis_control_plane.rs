use std::sync::Arc;
use aws_sdk_kinesis::Client as KinesisClient;

use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use crate::services::aws::aws_types::kinesis::{
    KinesisCreateStreamRequest, KinesisDeleteStreamRequest, KinesisDescribeStreamRequest,
    KinesisListStreamsRequest, KinesisUpdateShardCountRequest, KinesisAddTagsRequest,
    KinesisRemoveTagsRequest, KinesisUpdateStreamModeRequest, KinesisStartEncryptionRequest,
    KinesisStopEncryptionRequest, KinesisDescribeStreamResponse, KinesisListStreamsResponse,
    KinesisOperationResponse, KinesisTagsResponse, KinesisShardInfo, KinesisHashKeyRange,
    KinesisSequenceNumberRange, KinesisStreamSummary, KinesisStreamModeDetails,
    // New request types
    KinesisRetentionPeriodRequest, KinesisEnhancedMonitoringRequest, KinesisMergeShardsRequest,
    KinesisSplitShardRequest, KinesisResourcePolicyRequest, KinesisStreamConsumerRequest,
    KinesisDescribeStreamConsumerRequest, KinesisListShardsRequest, KinesisTagResourceRequest,
    KinesisPutRecordsRequest, KinesisGetRecordsRequest, KinesisGetShardIteratorRequest,
    // New response types
    KinesisLimitsResponse, KinesisStreamSummaryResponse, KinesisEnhancedMonitoringResponse,
    KinesisResourcePolicyResponse, KinesisStreamConsumerResponse, KinesisListStreamConsumersResponse,
    KinesisListShardsResponse, KinesisPutRecordsResponse, KinesisGetRecordsResponse,
    KinesisGetShardIteratorResponse
};

pub struct KinesisControlPlane {
    aws_service: Arc<AwsService>,
}

impl KinesisControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_streams(&self, account_id: &str, profile: &AwsAccountDto, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_streams_with_auth(account_id, profile, region, None).await
    }
    
    pub async fn sync_streams_with_auth(&self, account_id: &str, profile: &AwsAccountDto, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_kinesis_client_with_auth(profile, region).await?;
        self.sync_streams_with_client(account_id, profile, region, client).await
    }
    
    async fn sync_streams_with_client(&self, account_id: &str, profile: &AwsAccountDto, region: &str, client: KinesisClient) -> Result<Vec<AwsResourceModel>, AppError> {
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

    pub async fn create_stream(&self, aws_account_dto: &AwsAccountDto, region: &str, request: &KinesisCreateStreamRequest) -> Result<KinesisOperationResponse, AppError> {
        let client = self.aws_service.create_kinesis_client(aws_account_dto, region).await?;

        let shard_count = request.shard_count
            .ok_or_else(|| AppError::ExternalService("Missing shard_count in create stream request".to_string()))?;

        client.create_stream()
            .stream_name(&request.stream_name)
            .shard_count(shard_count)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to create Kinesis stream: {}", e)))?;

        // Wait a moment for the stream to appear in AWS
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        // Try to get the current stream status
        let status = match self.get_stream_status(&client, &request.stream_name).await {
            Ok(status) => status,
            Err(_) => "CREATING".to_string(), // Fallback if describe fails
        };

        let mut details = std::collections::HashMap::new();
        details.insert("shard_count".to_string(), serde_json::json!(shard_count));
        details.insert("account_id".to_string(), serde_json::json!("FIX_ME"));

        Ok(KinesisOperationResponse {
            stream_name: request.stream_name.clone(),
            stream_arn: Some("FIX_ME".to_string()),
            status,
            details,
        })
    }

    pub async fn delete_stream(&self, profile: &AwsAccountDto, region: &str, request: &KinesisDeleteStreamRequest) -> Result<KinesisOperationResponse, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        client.delete_stream()
            .stream_name(&request.stream_name)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to delete Kinesis stream: {}", e)))?;

        // Get actual stream status after deletion initiation
        let status = match self.get_stream_status(&client, &request.stream_name).await {
            Ok(status) => status,
            Err(_) => "DELETING".to_string(), // Fallback if describe fails (expected during deletion)
        };

        Ok(KinesisOperationResponse {
            stream_name: request.stream_name.clone(),
            stream_arn: None,
            status,
            details: std::collections::HashMap::new(),
        })
    }

    // Helper method to get stream status
    // async fn get_stream_status(&self, client: &KinesisClient, stream_name: &str) -> Result<String, AppError> {
    //     let response = client.describe_stream_summary()
    //         .stream_name(stream_name)
    //         .send()
    //         .await
    //         .map_err(|e| AppError::ExternalService(format!("Failed to get stream status: {}", e)))?;
    //
    //     let status = response.stream_description_summary()
    //         .and_then(|desc| desc.stream_status())
    //         .map(|s| s.as_str().to_string())
    //         .ok_or_else(|| AppError::ExternalService("Missing stream status in response".to_string()))?;
    //
    //     Ok(status)
    // }

    pub async fn describe_stream(&self, aws_account_dto: AwsAccountDto, region: &str, request: &KinesisDescribeStreamRequest) -> Result<KinesisDescribeStreamResponse, AppError> {
        let client = self.aws_service.create_kinesis_client(aws_account_dto, region).await?;

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

            let stream_mode_details = KinesisStreamModeDetails {
                stream_mode: stream_desc.stream_mode_details()
                    .and_then(|smd| smd.stream_mode())
                    .map(|sm| sm.as_str())
                    .unwrap_or("PROVISIONED")
                    .to_string(),
            };

            let shards: Vec<KinesisShardInfo> = stream_desc.shards()
                .unwrap_or_default()
                .iter()
                .map(|shard| {
                    KinesisShardInfo {
                        shard_id: shard.shard_id().unwrap_or("unknown").to_string(),
                        hash_key_range: KinesisHashKeyRange {
                            starting_hash_key: shard.hash_key_range()
                                .and_then(|hkr| hkr.starting_hash_key())
                                .unwrap_or("0")
                                .to_string(),
                            ending_hash_key: shard.hash_key_range()
                                .and_then(|hkr| hkr.ending_hash_key())
                                .unwrap_or("340282366920938463463374607431768211455")
                                .to_string(),
                        },
                        sequence_number_range: KinesisSequenceNumberRange {
                            starting_sequence_number: shard.sequence_number_range()
                                .and_then(|snr| snr.starting_sequence_number())
                                .unwrap_or("0")
                                .to_string(),
                            ending_sequence_number: shard.sequence_number_range()
                                .and_then(|snr| snr.ending_sequence_number())
                                .map(|s| s.to_string()),
                        },
                        parent_shard_id: shard.parent_shard_id().map(|s| s.to_string()),
                        adjacent_parent_shard_id: shard.adjacent_parent_shard_id().map(|s| s.to_string()),
                    }
                })
                .collect();

            let creation_timestamp = stream_desc.stream_creation_timestamp()
                .and_then(|ts| ts.fmt(aws_smithy_types::date_time::Format::DateTime).ok());

            Ok(KinesisDescribeStreamResponse {
                stream_name: request.stream_name.clone(),
                stream_arn: stream_arn.to_string(),
                stream_status: stream_status.to_string(),
                stream_mode_details,
                shards,
                retention_period_hours: stream_desc.retention_period_hours().unwrap_or(24),
                encryption_type: stream_desc.encryption_type()
                    .map(|et| et.as_str())
                    .unwrap_or("NONE")
                    .to_string(),
                creation_timestamp,
            })
        } else {
            Err(AppError::ExternalService("No stream description found in AWS response".to_string()))
        }
    }

    pub async fn list_streams(&self, profile: &AwsAccountDto, region: &str, request: &KinesisListStreamsRequest) -> Result<KinesisListStreamsResponse, AppError> {
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

        let stream_summaries: Vec<KinesisStreamSummary> = response.stream_summaries()
            .unwrap_or_default()
            .iter()
            .map(|summary| {
                KinesisStreamSummary {
                    stream_name: summary.stream_name().unwrap_or("unknown").to_string(),
                    stream_arn: summary.stream_arn().unwrap_or("unknown").to_string(),
                    stream_status: summary.stream_status()
                        .map(|s| s.as_str().to_string())
                        .unwrap_or("UNKNOWN".to_string()),
                    stream_mode_details: KinesisStreamModeDetails {
                        stream_mode: summary.stream_mode_details()
                            .and_then(|smd| smd.stream_mode())
                            .map(|sm| sm.as_str().to_string())
                            .unwrap_or("PROVISIONED".to_string()),
                    },
                    stream_creation_timestamp: summary.stream_creation_timestamp()
                        .and_then(|ts| ts.fmt(aws_smithy_types::date_time::Format::DateTime).ok()),
                }
            })
            .collect();

        Ok(KinesisListStreamsResponse {
            stream_names: response.stream_names().unwrap_or_default().iter().map(|s| s.to_string()).collect(),
            has_more_streams: response.has_more_streams().unwrap_or(false),
            next_token: response.next_token().map(|s| s.to_string()),
            stream_summaries,
        })
    }

    pub async fn update_shard_count(&self, profile: &AwsAccountDto, region: &str, request: &KinesisUpdateShardCountRequest) -> Result<KinesisOperationResponse, AppError> {
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

        // Get actual stream status after operation
        let status = match self.get_stream_status(&client, &request.stream_name).await {
            Ok(status) => status,
            Err(_) => "UPDATING".to_string(),
        };

        let mut details = std::collections::HashMap::new();
        details.insert("target_shard_count".to_string(), serde_json::json!(request.target_shard_count));
        details.insert("scaling_type".to_string(), serde_json::json!(request.scaling_type));

        Ok(KinesisOperationResponse {
            stream_name: request.stream_name.clone(),
            stream_arn: None,
            status,
            details,
        })
    }

    pub async fn add_tags_to_stream(&self, profile: &AwsAccountDto, region: &str, request: &KinesisAddTagsRequest) -> Result<KinesisTagsResponse, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        client.add_tags_to_stream()
            .stream_name(&request.stream_name)
            .set_tags(Some(request.tags.clone()))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to add tags to stream: {}", e)))?;

        // Get actual stream status after operation
        let status = match self.get_stream_status(&client, &request.stream_name).await {
            Ok(status) => status,
            Err(_) => "ACTIVE".to_string(), // Tagging typically doesn't change stream status
        };

        Ok(KinesisTagsResponse {
            stream_name: request.stream_name.clone(),
            tags: request.tags.clone(),
        })
    }

    pub async fn remove_tags_from_stream(&self, profile: &AwsAccountDto, region: &str, request: &KinesisRemoveTagsRequest) -> Result<KinesisOperationResponse, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        client.remove_tags_from_stream()
            .stream_name(&request.stream_name)
            .set_tag_keys(Some(request.tag_keys.clone()))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to remove tags from stream: {}", e)))?;

        // Get actual stream status after operation
        let status = match self.get_stream_status(&client, &request.stream_name).await {
            Ok(status) => status,
            Err(_) => "ACTIVE".to_string(), // Tag removal typically doesn't change stream status
        };

        let mut details = std::collections::HashMap::new();
        details.insert("tags_removed".to_string(), serde_json::json!(request.tag_keys.len()));
        details.insert("tag_keys".to_string(), serde_json::json!(request.tag_keys));

        Ok(KinesisOperationResponse {
            stream_name: request.stream_name.clone(),
            stream_arn: None,
            status,
            details,
        })
    }

    pub async fn list_tags_for_stream(&self, profile: &AwsAccountDto, region: &str, stream_name: &str) -> Result<KinesisTagsResponse, AppError> {
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

        Ok(KinesisTagsResponse {
            stream_name: stream_name.to_string(),
            tags,
        })
    }

    pub async fn update_stream_mode(&self, profile: &AwsAccountDto, region: &str, request: &KinesisUpdateStreamModeRequest) -> Result<KinesisOperationResponse, AppError> {
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

        // Get actual stream status after operation
        let status = match self.get_stream_status(&client, &request.stream_name).await {
            Ok(status) => status,
            Err(_) => "UPDATING".to_string(),
        };

        let mut details = std::collections::HashMap::new();
        details.insert("stream_mode".to_string(), serde_json::json!(request.stream_mode_details.stream_mode));

        Ok(KinesisOperationResponse {
            stream_name: request.stream_name.clone(),
            stream_arn: None,
            status,
            details,
        })
    }

    pub async fn start_stream_encryption(&self, profile: &AwsAccountDto, region: &str, request: &KinesisStartEncryptionRequest) -> Result<KinesisOperationResponse, AppError> {
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

        // Get actual stream status after operation
        let status = match self.get_stream_status(&client, &request.stream_name).await {
            Ok(status) => status,
            Err(_) => "UPDATING".to_string(),
        };

        let mut details = std::collections::HashMap::new();
        details.insert("encryption_type".to_string(), serde_json::json!(request.encryption_type));
        details.insert("key_id".to_string(), serde_json::json!(request.key_id));

        Ok(KinesisOperationResponse {
            stream_name: request.stream_name.clone(),
            stream_arn: None,
            status,
            details,
        })
    }

    pub async fn stop_stream_encryption(&self, profile: &AwsAccountDto, region: &str, request: &KinesisStopEncryptionRequest) -> Result<KinesisOperationResponse, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        client.stop_stream_encryption()
            .stream_name(&request.stream_name)
            .encryption_type(aws_sdk_kinesis::types::EncryptionType::Kms)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to stop stream encryption: {}", e)))?;

        // Get actual stream status after operation
        let status = match self.get_stream_status(&client, &request.stream_name).await {
            Ok(status) => status,
            Err(_) => "UPDATING".to_string(),
        };

        let mut details = std::collections::HashMap::new();
        details.insert("encryption_action".to_string(), serde_json::json!("STOPPED"));

        Ok(KinesisOperationResponse {
            stream_name: request.stream_name.clone(),
            stream_arn: None,
            status,
            details,
        })
    }

    // Additional Control Plane Operations
    pub async fn describe_limits(&self, profile: &AwsAccountDto, region: &str) -> Result<KinesisLimitsResponse, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        let response = client.describe_limits()
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to describe limits: {}", e)))?;

        Ok(KinesisLimitsResponse {
            shard_limit: response.shard_limit().unwrap_or(0),
            open_shard_count: response.open_shard_count().unwrap_or(0),
            on_demand_stream_count: response.on_demand_stream_count().unwrap_or(0),
            on_demand_stream_count_limit: response.on_demand_stream_count_limit().unwrap_or(0),
        })
    }

    pub async fn describe_stream_summary(&self, profile: &AwsAccountDto, region: &str, stream_name: &str) -> Result<KinesisStreamSummaryResponse, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        let response = client.describe_stream_summary()
            .stream_name(stream_name)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to describe stream summary: {}", e)))?;

        let summary = response.stream_description_summary()
            .ok_or_else(|| AppError::ExternalService("No stream summary found".to_string()))?;

        Ok(KinesisStreamSummaryResponse {
            stream_name: summary.stream_name().unwrap_or("").to_string(),
            stream_arn: summary.stream_arn().unwrap_or("").to_string(),
            stream_status: summary.stream_status().unwrap().as_str().to_string(),
            stream_mode_details: KinesisStreamModeDetails {
                stream_mode: summary.stream_mode_details()
                    .and_then(|smd| smd.stream_mode())
                    .map(|sm| sm.as_str().to_string())
                    .unwrap_or_else(|| "PROVISIONED".to_string()),
            },
            stream_creation_timestamp: summary.stream_creation_timestamp()
                .and_then(|ts| ts.fmt(aws_smithy_types::date_time::Format::DateTime).ok())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
        })
    }

    pub async fn increase_stream_retention_period(&self, profile: &AwsAccountDto, region: &str, request: &KinesisRetentionPeriodRequest) -> Result<KinesisOperationResponse, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        client.increase_stream_retention_period()
            .stream_name(&request.stream_name)
            .retention_period_hours(request.retention_period_hours)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to increase retention period: {}", e)))?;

        let status = match self.get_stream_status(&client, &request.stream_name).await {
            Ok(status) => status,
            Err(_) => "UPDATING".to_string(),
        };

        let mut details = std::collections::HashMap::new();
        details.insert("retention_period_hours".to_string(), serde_json::json!(request.retention_period_hours));

        Ok(KinesisOperationResponse {
            stream_name: request.stream_name.clone(),
            stream_arn: None,
            status,
            details,
        })
    }

    pub async fn decrease_stream_retention_period(&self, profile: &AwsAccountDto, region: &str, request: &KinesisRetentionPeriodRequest) -> Result<KinesisOperationResponse, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        client.decrease_stream_retention_period()
            .stream_name(&request.stream_name)
            .retention_period_hours(request.retention_period_hours)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to decrease retention period: {}", e)))?;

        let status = match self.get_stream_status(&client, &request.stream_name).await {
            Ok(status) => status,
            Err(_) => "UPDATING".to_string(),
        };

        let mut details = std::collections::HashMap::new();
        details.insert("retention_period_hours".to_string(), serde_json::json!(request.retention_period_hours));

        Ok(KinesisOperationResponse {
            stream_name: request.stream_name.clone(),
            stream_arn: None,
            status,
            details,
        })
    }

    pub async fn enable_enhanced_monitoring(&self, profile: &AwsAccountDto, region: &str, request: &KinesisEnhancedMonitoringRequest) -> Result<KinesisEnhancedMonitoringResponse, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        let shard_level_metrics: Vec<aws_sdk_kinesis::types::MetricsName> = request.shard_level_metrics
            .iter()
            .filter_map(|metric| match metric.as_str() {
                "IncomingRecords" => Some(aws_sdk_kinesis::types::MetricsName::IncomingRecords),
                "IncomingBytes" => Some(aws_sdk_kinesis::types::MetricsName::IncomingBytes),
                "OutgoingRecords" => Some(aws_sdk_kinesis::types::MetricsName::OutgoingRecords),
                "OutgoingBytes" => Some(aws_sdk_kinesis::types::MetricsName::OutgoingBytes),
                "WriteProvisionedThroughputExceeded" => Some(aws_sdk_kinesis::types::MetricsName::WriteProvisionedThroughputExceeded),
                "ReadProvisionedThroughputExceeded" => Some(aws_sdk_kinesis::types::MetricsName::ReadProvisionedThroughputExceeded),
                "IteratorAgeMilliseconds" => Some(aws_sdk_kinesis::types::MetricsName::IteratorAgeMilliseconds),
                "All" => Some(aws_sdk_kinesis::types::MetricsName::All),
                _ => None,
            })
            .collect();

        let response = client.enable_enhanced_monitoring()
            .stream_name(&request.stream_name)
            .set_shard_level_metrics(Some(shard_level_metrics))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to enable enhanced monitoring: {}", e)))?;

        Ok(KinesisEnhancedMonitoringResponse {
            stream_name: request.stream_name.clone(),
            current_shard_level_metrics: response.current_shard_level_metrics()
                .unwrap_or_default()
                .iter()
                .map(|metric| metric.as_str().to_string())
                .collect(),
            desired_shard_level_metrics: response.desired_shard_level_metrics()
                .unwrap_or_default()
                .iter()
                .map(|metric| metric.as_str().to_string())
                .collect(),
            stream_arn: response.stream_arn().map(|s| s.to_string()),
        })
    }

    pub async fn disable_enhanced_monitoring(&self, profile: &AwsAccountDto, region: &str, request: &KinesisEnhancedMonitoringRequest) -> Result<KinesisEnhancedMonitoringResponse, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        let shard_level_metrics: Vec<aws_sdk_kinesis::types::MetricsName> = request.shard_level_metrics
            .iter()
            .filter_map(|metric| match metric.as_str() {
                "IncomingRecords" => Some(aws_sdk_kinesis::types::MetricsName::IncomingRecords),
                "IncomingBytes" => Some(aws_sdk_kinesis::types::MetricsName::IncomingBytes),
                "OutgoingRecords" => Some(aws_sdk_kinesis::types::MetricsName::OutgoingRecords),
                "OutgoingBytes" => Some(aws_sdk_kinesis::types::MetricsName::OutgoingBytes),
                "WriteProvisionedThroughputExceeded" => Some(aws_sdk_kinesis::types::MetricsName::WriteProvisionedThroughputExceeded),
                "ReadProvisionedThroughputExceeded" => Some(aws_sdk_kinesis::types::MetricsName::ReadProvisionedThroughputExceeded),
                "IteratorAgeMilliseconds" => Some(aws_sdk_kinesis::types::MetricsName::IteratorAgeMilliseconds),
                "All" => Some(aws_sdk_kinesis::types::MetricsName::All),
                _ => None,
            })
            .collect();

        let response = client.disable_enhanced_monitoring()
            .stream_name(&request.stream_name)
            .set_shard_level_metrics(Some(shard_level_metrics))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to disable enhanced monitoring: {}", e)))?;

        Ok(KinesisEnhancedMonitoringResponse {
            stream_name: request.stream_name.clone(),
            current_shard_level_metrics: response.current_shard_level_metrics()
                .unwrap_or_default()
                .iter()
                .map(|metric| metric.as_str().to_string())
                .collect(),
            desired_shard_level_metrics: response.desired_shard_level_metrics()
                .unwrap_or_default()
                .iter()
                .map(|metric| metric.as_str().to_string())
                .collect(),
            stream_arn: response.stream_arn().map(|s| s.to_string()),
        })
    }

    pub async fn list_shards(&self, profile: &AwsAccountDto, region: &str, request: &KinesisListShardsRequest) -> Result<KinesisListShardsResponse, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;

        let mut list_shards_request = client.list_shards();

        if let Some(stream_name) = &request.stream_name {
            list_shards_request = list_shards_request.stream_name(stream_name);
        }
        if let Some(stream_arn) = &request.stream_arn {
            list_shards_request = list_shards_request.stream_arn(stream_arn);
        }
        if let Some(next_token) = &request.next_token {
            list_shards_request = list_shards_request.next_token(next_token);
        }
        if let Some(exclusive_start_shard_id) = &request.exclusive_start_shard_id {
            list_shards_request = list_shards_request.exclusive_start_shard_id(exclusive_start_shard_id);
        }
        if let Some(max_results) = request.max_results {
            list_shards_request = list_shards_request.max_results(max_results);
        }

        let response = list_shards_request
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list shards: {}", e)))?;

        let shards = response.shards()
            .unwrap_or_default()
            .iter()
            .map(|shard| KinesisShardInfo {
                shard_id: shard.shard_id().unwrap_or("").to_string(),
                hash_key_range: KinesisHashKeyRange {
                    starting_hash_key: shard.hash_key_range()
                        .and_then(|hkr| hkr.starting_hash_key())
                        .unwrap_or("")
                        .to_string(),
                    ending_hash_key: shard.hash_key_range()
                        .and_then(|hkr| hkr.ending_hash_key())
                        .unwrap_or("")
                        .to_string(),
                },
                sequence_number_range: KinesisSequenceNumberRange {
                    starting_sequence_number: shard.sequence_number_range()
                        .and_then(|snr| snr.starting_sequence_number())
                        .unwrap_or("")
                        .to_string(),
                    ending_sequence_number: shard.sequence_number_range()
                        .and_then(|snr| snr.ending_sequence_number())
                        .map(|s| s.to_string()),
                },
                parent_shard_id: shard.parent_shard_id().map(|s| s.to_string()),
                adjacent_parent_shard_id: shard.adjacent_parent_shard_id().map(|s| s.to_string()),
            })
            .collect();

        Ok(KinesisListShardsResponse {
            shards,
            next_token: response.next_token().map(|s| s.to_string()),
            stream_name: request.stream_name.clone(),
            stream_arn: request.stream_arn.clone(),
            stream_creation_timestamp: request.stream_creation_timestamp.clone(),
        })
    }
}