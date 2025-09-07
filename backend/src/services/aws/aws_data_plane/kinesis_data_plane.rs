use std::sync::Arc;
use serde_json::json;
use crate::errors::AppError;
use crate::services::aws::aws_types::kinesis::KinesisPutRecordRequest;
use crate::services::aws::aws_types::cloud_watch::{CloudWatchMetricsRequest, CloudWatchMetricsResult};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use aws_sdk_kinesis::primitives::Blob;

// Data plane implementation for Kinesis
pub struct KinesisDataPlane {
    aws_service: Arc<AwsService>,
}

impl KinesisDataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn put_record(&self, profile: Option<&str>, region: &str, request: &KinesisPutRecordRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_kinesis_client(profile, region).await?;
        
        // Actually call AWS Kinesis put_record API
        let response = client.put_record()
            .stream_name(&request.stream_name)
            .data(Blob::new(request.data.as_bytes()))
            .partition_key(&request.partition_key)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to put record to Kinesis stream: {}", e)))?;
        
        let sequence_number = response.sequence_number()
            .ok_or_else(|| AppError::ExternalService("Missing sequence number in put_record response".to_string()))?;
        let shard_id = response.shard_id()
            .ok_or_else(|| AppError::ExternalService("Missing shard ID in put_record response".to_string()))?;

        Ok(json!({
            "sequence_number": sequence_number,
            "shard_id": shard_id,
            "encryption_type": response.encryption_type().map(|et| et.as_str()).unwrap_or("NONE")
        }))
    }

    pub async fn get_stream_metrics(&self, _request: &CloudWatchMetricsRequest) -> Result<CloudWatchMetricsResult, AppError> {
        return Err(AppError::ExternalService("get_stream_metrics not implemented - use CloudWatch data plane directly".to_string()));
    }

    // Additional Kinesis-specific data plane operations would go here
    // For example:
    // - get_records
    // - list_shards
    // - merge_shards
    // - split_shard
}