use serde::{Deserialize, Serialize};

// Kinesis-specific types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KinesisStreamInfo {
    pub stream_name: String,
    pub stream_status: String,
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