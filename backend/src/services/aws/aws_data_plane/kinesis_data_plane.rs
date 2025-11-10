// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::services::aws::aws_types::cloud_watch::{
    CloudWatchMetricsRequest, CloudWatchMetricsResult,
};
use crate::services::aws::aws_types::kinesis::{
    KinesisGetRecordsRequest, KinesisGetRecordsResponse, KinesisGetShardIteratorRequest,
    KinesisGetShardIteratorResponse, KinesisPutRecordRequest, KinesisPutRecordsRequest,
    KinesisPutRecordsResponse,
};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use aws_sdk_kinesis::primitives::Blob;
use aws_sdk_kinesis::types::PutRecordsRequestEntry;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::json;
use std::sync::Arc;

// Data plane implementation for Kinesis
pub struct KinesisDataPlane {
    aws_service: Arc<AwsService>,
}

impl KinesisDataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn put_record(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisPutRecordRequest,
    ) -> Result<serde_json::Value, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        // Actually call AWS Kinesis put_record API
        let response = client
            .put_record()
            .stream_name(&request.stream_name)
            .data(Blob::new(request.data.as_bytes()))
            .partition_key(&request.partition_key)
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to put record to Kinesis stream: {}", e))
            })?;

        let sequence_number = response.sequence_number();
        let shard_id = response.shard_id();

        Ok(json!({
            "sequence_number": sequence_number,
            "shard_id": shard_id,
            "encryption_type": response.encryption_type().map(|et| et.as_str()).unwrap_or("NONE")
        }))
    }

    pub async fn get_stream_metrics(
        &self,
        _request: &CloudWatchMetricsRequest,
    ) -> Result<CloudWatchMetricsResult, AppError> {
        return Err(AppError::ExternalService(
            "get_stream_metrics not implemented - use CloudWatch data plane directly".to_string(),
        ));
    }

    pub async fn put_records(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisPutRecordsRequest,
    ) -> Result<KinesisPutRecordsResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        // Convert records to AWS SDK format
        let mut records = Vec::new();
        for record in &request.records {
            let data_blob = match BASE64.decode(&record.data) {
                Ok(decoded) => Blob::new(decoded),
                Err(_) => Blob::new(record.data.as_bytes()), // Fallback to raw bytes if not base64
            };

            let entry = PutRecordsRequestEntry::builder()
                .data(data_blob)
                .partition_key(&record.partition_key)
                .build()?;

            records.push(entry);
        }

        let stream_name = request
            .stream_name
            .as_ref()
            .ok_or_else(|| AppError::BadRequest("stream_name is required".to_string()))?;

        let response = client
            .put_records()
            .stream_name(stream_name)
            .set_records(Some(records))
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to put records to Kinesis stream: {}", e))
            })?;

        // Convert response
        let mut result_records = Vec::new();

        for record in response.records() {
            result_records.push(
                crate::services::aws::aws_types::kinesis::KinesisPutRecordsResultEntry {
                    sequence_number: record.sequence_number().map(|s| s.to_string()),
                    shard_id: record.shard_id().map(|s| s.to_string()),
                    error_code: record.error_code().map(|s| s.to_string()),
                    error_message: record.error_message().map(|s| s.to_string()),
                },
            );
        }

        Ok(KinesisPutRecordsResponse {
            failed_record_count: response.failed_record_count().unwrap_or(0),
            records: result_records,
            encryption_type: response.encryption_type().map(|et| et.as_str().to_string()),
        })
    }

    pub async fn get_records(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisGetRecordsRequest,
    ) -> Result<KinesisGetRecordsResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        let mut get_records_request = client.get_records().shard_iterator(&request.shard_iterator);

        if let Some(limit) = request.limit {
            get_records_request = get_records_request.limit(limit);
        }

        let response = get_records_request.send().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to get records from Kinesis stream: {}", e))
        })?;

        // Convert records
        let mut result_records = Vec::new();

        for record in response.records() {
            let data = record.data();

            result_records.push(crate::services::aws::aws_types::kinesis::KinesisRecord {
                sequence_number: record.sequence_number().to_string(),
                data: BASE64.encode(data.as_ref()),
                partition_key: record.partition_key().to_string(),
                approximate_arrival_timestamp: record
                    .approximate_arrival_timestamp()
                    .map(|ts| ts.secs().to_string())
                    .unwrap_or_else(|| "0".to_string()),
                encryption_type: record.encryption_type().map(|et| et.as_str().to_string()),
            });
        }

        Ok(KinesisGetRecordsResponse {
            records: result_records,
            next_shard_iterator: response.next_shard_iterator().map(|s| s.to_string()),
            millis_behind_latest: Some(response.millis_behind_latest().unwrap_or(0)),
            child_shards: None, // TODO: Convert child shards if present
        })
    }

    pub async fn get_shard_iterator(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisGetShardIteratorRequest,
    ) -> Result<KinesisGetShardIteratorResponse, AppError> {
        let client = self
            .aws_service
            .create_kinesis_client(aws_account_dto)
            .await?;

        let shard_iterator_type = match request.shard_iterator_type.as_str() {
            "TRIM_HORIZON" => aws_sdk_kinesis::types::ShardIteratorType::TrimHorizon,
            "LATEST" => aws_sdk_kinesis::types::ShardIteratorType::Latest,
            "AT_SEQUENCE_NUMBER" => aws_sdk_kinesis::types::ShardIteratorType::AtSequenceNumber,
            "AFTER_SEQUENCE_NUMBER" => {
                aws_sdk_kinesis::types::ShardIteratorType::AfterSequenceNumber
            }
            "AT_TIMESTAMP" => aws_sdk_kinesis::types::ShardIteratorType::AtTimestamp,
            _ => {
                return Err(AppError::BadRequest(format!(
                    "Invalid shard iterator type: {}",
                    request.shard_iterator_type
                )))
            }
        };

        let stream_name = request
            .stream_name
            .as_ref()
            .ok_or_else(|| AppError::BadRequest("stream_name is required".to_string()))?;

        let mut get_shard_iterator_request = client
            .get_shard_iterator()
            .stream_name(stream_name)
            .shard_id(&request.shard_id)
            .shard_iterator_type(shard_iterator_type);

        if let Some(sequence_number) = &request.starting_sequence_number {
            get_shard_iterator_request =
                get_shard_iterator_request.starting_sequence_number(sequence_number);
        }

        if let Some(timestamp_str) = &request.timestamp {
            // Parse timestamp string as epoch seconds
            if let Ok(timestamp_secs) = timestamp_str.parse::<i64>() {
                get_shard_iterator_request = get_shard_iterator_request.timestamp(
                    aws_sdk_kinesis::primitives::DateTime::from_secs(timestamp_secs),
                );
            }
        }

        let response = get_shard_iterator_request.send().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to get shard iterator: {}", e))
        })?;

        Ok(KinesisGetShardIteratorResponse {
            shard_iterator: response.shard_iterator().unwrap_or("").to_string(),
        })
    }

    // Additional Kinesis-specific data plane operations would go here
    // - list_shards
    // - merge_shards
    // - split_shard
}
