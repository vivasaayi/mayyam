use aws_sdk_kinesis::types::StreamSummary;
use serde::{Deserialize, Serialize};

// Kinesis-specific types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisStreamInfo {
    pub stre    pub adjacent_parent_shard_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisListShardsResponse {    pub stream_status: String,
    pub retention_period_hours: i32,
    pub shard_count: i32,
    pub enhanced_monitoring: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisPutRecordRequest {
    pub stream_name: String,
    pub data: String,
    pub partition_key: String,
    pub sequence_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisCreateStreamRequest {
    pub stream_name: String,
    pub shard_count: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisDeleteStreamRequest {
    pub stream_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisDescribeStreamRequest {
    pub stream_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisListStreamsRequest {
    pub next_token: Option<String>,
    pub max_results: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisUpdateShardCountRequest {
    pub stream_name: String,
    pub target_shard_count: i32,
    pub scaling_type: String, // UNIFORM_SCALING
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisAddTagsRequest {
    pub stream_name: String,
    pub tags: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisRemoveTagsRequest {
    pub stream_name: String,
    pub tag_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisUpdateStreamModeRequest {
    pub stream_name: String,
    pub stream_mode_details: KinesisStreamModeDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisStreamModeDetails {
    pub stream_mode: String, // PROVISIONED or ON_DEMAND
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisStartEncryptionRequest {
    pub stream_name: String,
    pub encryption_type: String, // KMS
    pub key_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisStopEncryptionRequest {
    pub stream_name: String,
}

// Additional Control Plane Request Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisRetentionPeriodRequest {
    pub stream_name: String,
    pub retention_period_hours: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisEnhancedMonitoringRequest {
    pub stream_name: String,
    pub shard_level_metrics: Vec<String>, // e.g., ["IncomingRecords", "OutgoingRecords"]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisMergeShardsRequest {
    pub stream_name: String,
    pub shard_to_merge: String,
    pub adjacent_shard_to_merge: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisSplitShardRequest {
    pub stream_name: String,
    pub shard_to_split: String,
    pub new_starting_hash_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisResourcePolicyRequest {
    pub resource_arn: String,
    pub policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisStreamConsumerRequest {
    pub stream_arn: String,
    pub consumer_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisDescribeStreamConsumerRequest {
    pub stream_arn: Option<String>,
    pub consumer_name: Option<String>,
    pub consumer_arn: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisListShardsRequest {
    pub stream_name: Option<String>,
    pub stream_arn: Option<String>,
    pub next_token: Option<String>,
    pub exclusive_start_shard_id: Option<String>,
    pub max_results: Option<i32>,
    pub stream_creation_timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisTagResourceRequest {
    pub resource_arn: String,
    pub tags: std::collections::HashMap<String, String>,
}

// Data Plane Request Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisPutRecordsRequest {
    pub stream_name: Option<String>,
    pub stream_arn: Option<String>,
    pub records: Vec<KinesisPutRecordRequestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisPutRecordRequestEntry {
    pub data: String, // Base64 encoded data
    pub explicit_hash_key: Option<String>,
    pub partition_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisGetRecordsRequest {
    pub shard_iterator: String,
    pub limit: Option<i32>,
    pub stream_arn: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisGetShardIteratorRequest {
    pub stream_name: Option<String>,
    pub stream_arn: Option<String>,
    pub shard_id: String,
    pub shard_iterator_type: String, // TRIM_HORIZON, LATEST, AT_SEQUENCE_NUMBER, AFTER_SEQUENCE_NUMBER, AT_TIMESTAMP
    pub starting_sequence_number: Option<String>,
    pub timestamp: Option<String>,
}

// Response types for typed returns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisDescribeStreamResponse {
    pub stream_name: String,
    pub stream_arn: String,
    pub stream_status: String,
    pub stream_mode_details: KinesisStreamModeDetails,
    pub shards: Vec<KinesisShardInfo>,
    pub retention_period_hours: i32,
    pub encryption_type: String,
    pub creation_timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisShardInfo {
    pub shard_id: String,
    pub hash_key_range: KinesisHashKeyRange,
    pub sequence_number_range: KinesisSequenceNumberRange,
    pub parent_shard_id: Option<String>,
    pub adjacent_parent_shard_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisHashKeyRange {
    pub starting_hash_key: String,
    pub ending_hash_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisSequenceNumberRange {
    pub starting_sequence_number: String,
    pub ending_sequence_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisStreamSummary {
    pub stream_name: String,
    pub stream_arn: String,
    pub stream_status: String,
    pub stream_mode_details: Option<KinesisStreamModeDetails>,
    pub stream_creation_timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisListStreamsResponse {
    pub stream_names: Vec<String>,
    pub has_more_streams: bool,
    pub next_token: Option<String>,
    pub stream_summaries: Vec<KinesisStreamSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisOperationResponse {
    pub stream_name: String,
    pub stream_arn: Option<String>,
    pub status: String,
    pub details: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisTagsResponse {
    pub stream_name: String,
    pub tags: std::collections::HashMap<String, String>,
}

// Additional Response Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisLimitsResponse {
    pub shard_limit: i32,
    pub open_shard_count: i32,
    pub on_demand_stream_count: i32,
    pub on_demand_stream_count_limit: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisStreamSummaryResponse {
    pub stream_name: String,
    pub stream_arn: String,
    pub stream_status: String,
    pub stream_mode_details: KinesisStreamModeDetails,
    pub stream_creation_timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisEnhancedMonitoringResponse {
    pub stream_name: String,
    pub current_shard_level_metrics: Vec<crate::types::MetricsName>,
    pub desired_shard_level_metrics: Vec<crate::types::MetricsName>,
    pub stream_arn: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisResourcePolicyResponse {
    pub resource_arn: String,
    pub policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisStreamConsumerResponse {
    pub consumer_name: String,
    pub consumer_arn: String,
    pub consumer_status: String,
    pub consumer_creation_timestamp: String,
    pub stream_arn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisListStreamConsumersResponse {
    pub consumers: Vec<KinesisConsumerSummary>,
    pub next_token: Option<String>,
    pub stream_arn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisConsumerSummary {
    pub consumer_name: String,
    pub consumer_arn: String,
    pub consumer_status: String,
    pub consumer_creation_timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisShard {
    pub shard_id: String,
    pub parent_shard_id: Option<String>,
    pub adjacent_parent_shard_id: Option<String>,
    pub hash_key_range: KinesisHashKeyRange,
    pub sequence_number_range: KinesisSequenceNumberRange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisHashKeyRange {
    pub starting_hash_key: String,
    pub ending_hash_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisSequenceNumberRange {
    pub starting_sequence_number: String,
    pub ending_sequence_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisListShardsResponse {
    pub shards: Vec<KinesisShard>,
    pub next_token: Option<String>,
    pub stream_name: Option<String>,
    pub stream_arn: Option<String>,
    pub stream_creation_timestamp: Option<String>,
}

// Data Plane Response Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisPutRecordsResponse {
    pub failed_record_count: i32,
    pub records: Vec<KinesisPutRecordsResultEntry>,
    pub encryption_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisPutRecordsResultEntry {
    pub sequence_number: Option<String>,
    pub shard_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisGetRecordsResponse {
    pub records: Vec<KinesisRecord>,
    pub next_shard_iterator: Option<String>,
    pub millis_behind_latest: Option<i64>,
    pub child_shards: Option<Vec<KinesisChildShard>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisRecord {
    pub sequence_number: String,
    pub approximate_arrival_timestamp: String,
    pub data: String, // Base64 encoded
    pub partition_key: String,
    pub encryption_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisChildShard {
    pub shard_id: String,
    pub parent_shards: Vec<String>,
    pub hash_key_range: KinesisHashKeyRange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisGetShardIteratorResponse {
    pub shard_iterator: String,
}