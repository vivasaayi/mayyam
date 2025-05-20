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