use aws_sdk_kinesis::types::StreamDescription;
use aws_sdk_kinesis::Client as KinesisClient;
use std::sync::Arc;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::aws_types::kinesis::{
    KinesisAddTagsRequest,
    KinesisCreateStreamRequest,
    KinesisDeleteStreamRequest,
    KinesisDescribeStreamConsumerRequest,
    KinesisDescribeStreamRequest,
    KinesisDescribeStreamResponse,
    KinesisEnhancedMonitoringRequest,
    KinesisEnhancedMonitoringResponse,
    KinesisGetRecordsRequest,
    KinesisGetRecordsResponse,
    KinesisGetShardIteratorRequest,
    KinesisGetShardIteratorResponse,
    KinesisHashKeyRange,
    // New response types
    KinesisLimitsResponse,
    KinesisListShardsRequest,
    KinesisListShardsResponse,
    KinesisListStreamConsumersResponse,
    KinesisListStreamsRequest,
    KinesisListStreamsResponse,
    KinesisMergeShardsRequest,
    KinesisOperationResponse,
    KinesisPutRecordsRequest,
    KinesisPutRecordsResponse,
    KinesisRemoveTagsRequest,
    KinesisResourcePolicyRequest,
    KinesisResourcePolicyResponse,
    // New request types
    KinesisRetentionPeriodRequest,
    KinesisSequenceNumberRange,
    KinesisShard,
    KinesisShardInfo,
    KinesisSplitShardRequest,
    KinesisStartEncryptionRequest,
    KinesisStopEncryptionRequest,
    KinesisStreamConsumerRequest,
    KinesisStreamConsumerResponse,
    KinesisStreamModeDetails,
    KinesisStreamSummary,
    KinesisStreamSummaryResponse,
    KinesisTagResourceRequest,
    KinesisTagsResponse,
    KinesisUpdateShardCountRequest,
    KinesisUpdateStreamModeRequest,
};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use serde_json::json;

pub struct KinesisControlPlane {
    aws_service: Arc<AwsService>,
}

impl KinesisControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_streams(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Kinesis streams for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );
        let client: KinesisClient = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;
        // List streams from AWS
        let response = client.list_streams().send().await.map_err(|e| {
            error!("Failed to list Kinesis streams: {}", &e);
            let inner_aws_error = e.into_service_error();
            error!("Error raw response: {:?}", &inner_aws_error);
            AppError::ExternalService(format!(
                "Failed to list Kinesis streams: {}",
                inner_aws_error
            ))
        })?;

        let stream_names = response.stream_names();

        let mut streams = Vec::new();

        debug!("Fetched {} Kinesis streams", streams.len());

        for stream_name in stream_names {
            debug!("Found Kinesis stream: {}", &stream_name);
            // Get detailed information for each stream
            let describe_resp = client
                .describe_stream_summary()
                .stream_name(stream_name)
                .send()
                .await
                .map_err(|e| {
                    error!("Failed to describe Kinesis stream {}: {}", stream_name, &e);
                    let inner_aws_error = e.into_service_error();
                    error!("Error raw response: {:?}", &inner_aws_error);

                    AppError::ExternalService(format!(
                        "Failed to describe Kinesis stream {}: {}",
                        stream_name, inner_aws_error
                    ))
                })?;

            let stream_desc = describe_resp.stream_description_summary().ok_or_else(|| {
                AppError::ExternalService(format!(
                    "No description found for stream {}",
                    stream_name
                ))
            })?;

            let tags = self
                .list_tags_for_stream(aws_account_dto, stream_name)
                .await?;

            // If no name tag was found, use the stream name as name
            // if name.is_none() {
            //     name = Some(stream_name.to_string());
            // }

            // Get enhanced monitoring information
            // Process enhanced monitoring data
            // stream_desc.enhanced_monitoring() returns a slice of EnhancedMetrics
            // let enhanced_monitoring: Vec<serde_json::Value> = if let Some(monitoring) = stream_desc.enhanced_monitoring() {
            //     monitoring.iter()
            //         .map(|e_metrics| {
            //             // Each e_metrics is an EnhancedMetrics object
            //             if let Some(shard_metrics) = e_metrics.shard_level_metrics() {
            //                 let metrics_vec: Vec<String> = shard_metrics.iter()
            //                     .map(|m| m.as_str().to_string())
            //                     .collect();
            //                 json!(metrics_vec)
            //             } else {
            //                 json!([])
            //             }
            //         })
            //         .collect()
            // } else {
            //     vec![]
            // };

            // Build resource data
            let mut resource_data = serde_json::Map::new();

            resource_data.insert("stream_name".to_string(), json!(stream_name));

            // if let Some(status) = stream_desc.stream_status().map(|s| s.as_str()) {
            //     resource_data.insert("stream_status".to_string(), json!(status));
            // }

            // if let Some(retention) = stream_desc.retention_period_hours() {
            //     resource_data.insert("retention_period_hours".to_string(), json!(retention));
            // }

            // if let Some(shard_count) = stream_desc.open_shard_count() {
            //     resource_data.insert("shard_count".to_string(), json!(shard_count));
            // }

            // if let Some(open_shard_count) = stream_desc.open_shard_count() {
            //     resource_data.insert("open_shard_count".to_string(), json!(open_shard_count));
            // }

            // if !enhanced_monitoring.is_empty() {
            //     resource_data.insert("enhanced_monitoring".to_string(), json!(enhanced_monitoring));
            // }

            if let Some(encryption) = stream_desc.encryption_type().map(|e| e.as_str()) {
                resource_data.insert("encryption_type".to_string(), json!(encryption));
            }

            // if let Some(creation_time) = stream_desc.stream_creation_timestamp() {
            //     if let Ok(formatted_time) = creation_time.fmt(aws_smithy_types::date_time::Format::DateTime) {
            //         resource_data.insert("creation_timestamp".to_string(), json!(formatted_time));
            //     } else {
            //         // Fall back to seconds since epoch
            //         resource_data.insert("creation_timestamp".to_string(),
            //             json!(creation_time.as_secs_f64().to_string()));
            //     }
            // }

            // Create resource DTO
            let stream = AwsResourceDto {
                id: None,
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone().to_string(),
                resource_type: "KinesisStream".to_string(),
                resource_id: stream_name.to_string(),
                arn: stream_desc.stream_arn.clone(),
                name: Some("".to_string()),
                tags: serde_json::Value::Null,
                resource_data: serde_json::Value::Object(resource_data),
                sync_id: Some(sync_id),
            };

            streams.push(stream);
        }

        Ok(streams.into_iter().map(|s| s.into()).collect())
    }

    pub async fn return_current_status_as_response(
        &self,
        aws_account_dto: &AwsAccountDto,
        stream_name: &str,
    ) -> Result<KinesisOperationResponse, AppError> {
        let stream_details = self
            .describe_stream(
                aws_account_dto,
                &KinesisDescribeStreamRequest {
                    stream_name: stream_name.to_string(),
                },
            )
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to describe Kinesis stream: {}", e))
            })?;

        Ok(KinesisOperationResponse {
            stream_name: stream_name.to_string(),
            stream_arn: Some(stream_details.stream_arn),
            status: stream_details.stream_status.to_string(),
            details: std::collections::HashMap::new(),
        })
    }

    pub async fn create_stream(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisCreateStreamRequest,
    ) -> Result<KinesisOperationResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        let shard_count = request.shard_count.ok_or_else(|| {
            AppError::ExternalService("Missing shard_count in create stream request".to_string())
        })?;

        client
            .create_stream()
            .stream_name(&request.stream_name)
            .shard_count(shard_count)
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to create Kinesis stream: {}", e))
            })?;

        // Wait a moment for the stream to appear in AWS
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        self.return_current_status_as_response(aws_account_dto, &request.stream_name)
            .await
    }

    pub async fn delete_stream(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisDeleteStreamRequest,
    ) -> Result<KinesisOperationResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        client
            .delete_stream()
            .stream_name(&request.stream_name)
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to delete Kinesis stream: {}", e))
            })?;

        self.return_current_status_as_response(aws_account_dto, &request.stream_name)
            .await
    }

    pub async fn describe_stream(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisDescribeStreamRequest,
    ) -> Result<StreamDescription, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        let response = client
            .describe_stream()
            .stream_name(&request.stream_name)
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to describe Kinesis stream: {}", e))
            })?;

        Ok(response.stream_description.unwrap())
    }

    pub async fn list_streams(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisListStreamsRequest,
    ) -> Result<KinesisListStreamsResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        let mut list_builder = client.list_streams();

        if let Some(next_token) = &request.next_token {
            list_builder = list_builder.next_token(next_token);
        }

        // Note: max_results parameter is not supported in the current AWS SDK version
        // if let Some(max_results) = request.max_results {
        //     list_builder = list_builder.max_results(max_results as i32);
        // }

        let response = list_builder.send().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to list Kinesis streams: {}", e))
        })?;

        Ok(KinesisListStreamsResponse {
            stream_names: response.stream_names().to_vec(),
            has_more_streams: response.has_more_streams(),
            next_token: response.next_token().map(|s| s.to_string()),
            stream_summaries: vec![], //response.stream_summaries().to_vec(),
        })
    }

    pub async fn update_shard_count(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisUpdateShardCountRequest,
    ) -> Result<KinesisOperationResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        let scaling_type = match request.scaling_type.as_str() {
            "UNIFORM_SCALING" => aws_sdk_kinesis::types::ScalingType::UniformScaling,
            _ => {
                return Err(AppError::ExternalService(
                    "Invalid scaling type. Must be UNIFORM_SCALING".to_string(),
                ))
            }
        };

        client
            .update_shard_count()
            .stream_name(&request.stream_name)
            .target_shard_count(request.target_shard_count)
            .scaling_type(scaling_type)
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to update shard count: {}", e))
            })?;

        let mut details = std::collections::HashMap::new();
        details.insert(
            "target_shard_count".to_string(),
            serde_json::json!(request.target_shard_count),
        );
        details.insert(
            "scaling_type".to_string(),
            serde_json::json!(request.scaling_type),
        );

        self.return_current_status_as_response(aws_account_dto, &request.stream_name)
            .await
    }

    pub async fn add_tags_to_stream(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisAddTagsRequest,
    ) -> Result<KinesisTagsResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        client
            .add_tags_to_stream()
            .stream_name(&request.stream_name)
            .set_tags(Some(request.tags.clone()))
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to add tags to stream: {}", e))
            })?;

        Ok(KinesisTagsResponse {
            stream_name: request.stream_name.clone(),
            tags: request.tags.clone(),
        })
    }

    pub async fn remove_tags_from_stream(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisRemoveTagsRequest,
    ) -> Result<KinesisOperationResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        client
            .remove_tags_from_stream()
            .stream_name(&request.stream_name)
            .set_tag_keys(Some(request.tag_keys.clone()))
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to remove tags from stream: {}", e))
            })?;

        let mut details = std::collections::HashMap::new();
        details.insert(
            "tags_removed".to_string(),
            serde_json::json!(request.tag_keys.len()),
        );
        details.insert("tag_keys".to_string(), serde_json::json!(request.tag_keys));

        Ok(KinesisOperationResponse {
            stream_name: request.stream_name.clone(),
            stream_arn: None,
            status: "FIX_ME".to_string(),
            details,
        })
    }

    pub async fn list_tags_for_stream(
        &self,
        aws_account_dto: &AwsAccountDto,
        stream_name: &str,
    ) -> Result<KinesisTagsResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        let response = client
            .list_tags_for_stream()
            .stream_name(stream_name)
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to list tags for stream: {}", e))
            })?;

        let tags: std::collections::HashMap<String, String> = response
            .tags()
            .iter()
            .filter_map(|tag| {
                Some((
                    tag.key().to_string(),
                    tag.value().unwrap_or_else(|| "").to_string(),
                ))
            })
            .collect();

        Ok(KinesisTagsResponse {
            stream_name: stream_name.to_string(),
            tags,
        })
    }

    pub async fn update_stream_mode(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisUpdateStreamModeRequest,
    ) -> Result<KinesisOperationResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        let stream_mode = match request.stream_mode_details.stream_mode.as_str() {
            "PROVISIONED" => aws_sdk_kinesis::types::StreamMode::Provisioned,
            "ON_DEMAND" => aws_sdk_kinesis::types::StreamMode::OnDemand,
            _ => {
                return Err(AppError::ExternalService(
                    "Invalid stream mode. Must be PROVISIONED or ON_DEMAND".to_string(),
                ))
            }
        };

        let stream_mode_details = aws_sdk_kinesis::types::StreamModeDetails::builder()
            .stream_mode(stream_mode)
            .build()
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to build StreamModeDetails: {}", e))
            })?;

        client
            .update_stream_mode()
            .stream_arn(&request.stream_name)
            .stream_mode_details(stream_mode_details)
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to update stream mode: {}", e))
            })?;

        let mut details = std::collections::HashMap::new();
        details.insert(
            "stream_mode".to_string(),
            serde_json::json!(request.stream_mode_details.stream_mode),
        );

        self.return_current_status_as_response(aws_account_dto, &request.stream_name)
            .await
    }

    pub async fn start_stream_encryption(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisStartEncryptionRequest,
    ) -> Result<KinesisOperationResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        let encryption_type = match request.encryption_type.as_str() {
            "KMS" => aws_sdk_kinesis::types::EncryptionType::Kms,
            _ => {
                return Err(AppError::ExternalService(
                    "Invalid encryption type. Must be KMS".to_string(),
                ))
            }
        };

        client
            .start_stream_encryption()
            .stream_name(&request.stream_name)
            .encryption_type(encryption_type)
            .key_id(&request.key_id)
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to start stream encryption: {}", e))
            })?;

        let mut details = std::collections::HashMap::new();
        details.insert(
            "encryption_type".to_string(),
            serde_json::json!(request.encryption_type),
        );
        details.insert("key_id".to_string(), serde_json::json!(request.key_id));

        Ok(KinesisOperationResponse {
            stream_name: request.stream_name.clone(),
            stream_arn: None,
            status: "FIX_ME".to_string(),
            details,
        })
    }

    pub async fn stop_stream_encryption(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisStopEncryptionRequest,
    ) -> Result<KinesisOperationResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        client
            .stop_stream_encryption()
            .stream_name(&request.stream_name)
            .encryption_type(aws_sdk_kinesis::types::EncryptionType::Kms)
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to stop stream encryption: {}", e))
            })?;

        self.return_current_status_as_response(aws_account_dto, &request.stream_name)
            .await
    }

    // Additional Control Plane Operations
    pub async fn describe_limits(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<KinesisLimitsResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        let response =
            client.describe_limits().send().await.map_err(|e| {
                AppError::ExternalService(format!("Failed to describe limits: {}", e))
            })?;

        Ok(KinesisLimitsResponse {
            shard_limit: response.shard_limit(),
            open_shard_count: response.open_shard_count(),
            on_demand_stream_count: response.on_demand_stream_count(),
            on_demand_stream_count_limit: response.on_demand_stream_count_limit(),
        })
    }

    pub async fn increase_stream_retention_period(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisRetentionPeriodRequest,
    ) -> Result<KinesisOperationResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        client
            .increase_stream_retention_period()
            .stream_name(&request.stream_name)
            .retention_period_hours(request.retention_period_hours)
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to increase retention period: {}", e))
            })?;

        self.return_current_status_as_response(aws_account_dto, &request.stream_name)
            .await
    }

    pub async fn decrease_stream_retention_period(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisRetentionPeriodRequest,
    ) -> Result<KinesisOperationResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        client
            .decrease_stream_retention_period()
            .stream_name(&request.stream_name)
            .retention_period_hours(request.retention_period_hours)
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to decrease retention period: {}", e))
            })?;

        self.return_current_status_as_response(aws_account_dto, &request.stream_name)
            .await
    }

    pub async fn enable_enhanced_monitoring(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisEnhancedMonitoringRequest,
    ) -> Result<KinesisEnhancedMonitoringResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        let shard_level_metrics: Vec<aws_sdk_kinesis::types::MetricsName> = request
            .shard_level_metrics
            .iter()
            .filter_map(|metric| match metric.as_str() {
                "IncomingRecords" => Some(aws_sdk_kinesis::types::MetricsName::IncomingRecords),
                "IncomingBytes" => Some(aws_sdk_kinesis::types::MetricsName::IncomingBytes),
                "OutgoingRecords" => Some(aws_sdk_kinesis::types::MetricsName::OutgoingRecords),
                "OutgoingBytes" => Some(aws_sdk_kinesis::types::MetricsName::OutgoingBytes),
                "WriteProvisionedThroughputExceeded" => {
                    Some(aws_sdk_kinesis::types::MetricsName::WriteProvisionedThroughputExceeded)
                }
                "ReadProvisionedThroughputExceeded" => {
                    Some(aws_sdk_kinesis::types::MetricsName::ReadProvisionedThroughputExceeded)
                }
                "IteratorAgeMilliseconds" => {
                    Some(aws_sdk_kinesis::types::MetricsName::IteratorAgeMilliseconds)
                }
                "All" => Some(aws_sdk_kinesis::types::MetricsName::All),
                _ => None,
            })
            .collect();

        let response = client
            .enable_enhanced_monitoring()
            .stream_name(&request.stream_name)
            .set_shard_level_metrics(Some(shard_level_metrics))
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to enable enhanced monitoring: {}", e))
            })?;

        Ok(KinesisEnhancedMonitoringResponse {
            stream_name: request.stream_name.clone(),
            current_shard_level_metrics: vec![], //response.current_shard_level_metrics(),
            desired_shard_level_metrics: vec![], //response.desired_shard_level_metrics(),
            stream_arn: response.stream_arn().map(|s| s.to_string()),
        })
    }

    pub async fn disable_enhanced_monitoring(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisEnhancedMonitoringRequest,
    ) -> Result<KinesisEnhancedMonitoringResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        let shard_level_metrics: Vec<aws_sdk_kinesis::types::MetricsName> = request
            .shard_level_metrics
            .iter()
            .filter_map(|metric| match metric.as_str() {
                "IncomingRecords" => Some(aws_sdk_kinesis::types::MetricsName::IncomingRecords),
                "IncomingBytes" => Some(aws_sdk_kinesis::types::MetricsName::IncomingBytes),
                "OutgoingRecords" => Some(aws_sdk_kinesis::types::MetricsName::OutgoingRecords),
                "OutgoingBytes" => Some(aws_sdk_kinesis::types::MetricsName::OutgoingBytes),
                "WriteProvisionedThroughputExceeded" => {
                    Some(aws_sdk_kinesis::types::MetricsName::WriteProvisionedThroughputExceeded)
                }
                "ReadProvisionedThroughputExceeded" => {
                    Some(aws_sdk_kinesis::types::MetricsName::ReadProvisionedThroughputExceeded)
                }
                "IteratorAgeMilliseconds" => {
                    Some(aws_sdk_kinesis::types::MetricsName::IteratorAgeMilliseconds)
                }
                "All" => Some(aws_sdk_kinesis::types::MetricsName::All),
                _ => None,
            })
            .collect();

        let response = client
            .disable_enhanced_monitoring()
            .stream_name(&request.stream_name)
            .set_shard_level_metrics(Some(shard_level_metrics))
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to disable enhanced monitoring: {}", e))
            })?;

        Ok(KinesisEnhancedMonitoringResponse {
            stream_name: request.stream_name.clone(),
            current_shard_level_metrics: vec![], //response.current_shard_level_metrics(),
            desired_shard_level_metrics: vec![], //response.desired_shard_level_metrics(),
            stream_arn: response.stream_arn().map(|s| s.to_string()),
        })
    }

    pub async fn list_shards(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisListShardsRequest,
    ) -> Result<KinesisListShardsResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

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
            list_shards_request =
                list_shards_request.exclusive_start_shard_id(exclusive_start_shard_id);
        }
        if let Some(max_results) = request.max_results {
            list_shards_request = list_shards_request.max_results(max_results);
        }

        let response = list_shards_request
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list shards: {}", e)))?;

        let shards = response.shards();

        Ok(KinesisListShardsResponse {
            shards: shards.into_iter().map(|shard| KinesisShard {
                shard_id: shard.shard_id().to_string(),
                parent_shard_id: shard.parent_shard_id().map(|s| s.to_string()),
                adjacent_parent_shard_id: shard.adjacent_parent_shard_id().map(|s| s.to_string()),
                hash_key_range: KinesisHashKeyRange {
                    starting_hash_key: shard.hash_key_range().expect("Hash key range should be present").starting_hash_key().to_string(),
                    ending_hash_key: shard.hash_key_range().expect("Hash key range should be present").ending_hash_key().to_string(),
                },
                sequence_number_range: KinesisSequenceNumberRange {
                    starting_sequence_number: shard.sequence_number_range().expect("Sequence number range should be present").starting_sequence_number().to_string(),
                    ending_sequence_number: shard.sequence_number_range().expect("Sequence number range should be present").ending_sequence_number().map(|s| s.to_string()),
                },
            }).collect(),
            next_token: response.next_token().map(|s| s.to_string()),
            stream_name: None, // Not provided in ListShards response
            stream_arn: None,  // Not provided in ListShards response
            stream_creation_timestamp: None, // Not provided in ListShards response
        })
    }
}
