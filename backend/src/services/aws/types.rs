use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// Common Request/Response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSyncRequest {
    pub account_id: String,
    pub profile: Option<String>,
    pub region: String,
    pub resource_types: Option<Vec<String>>,
    // Authentication fields
    pub use_role: bool,
    pub role_arn: Option<String>,
    pub external_id: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSyncResponse {
    pub summary: Vec<ResourceTypeSyncSummary>,
    pub total_resources: usize,
    pub sync_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTypeSyncSummary {
    pub resource_type: String,
    pub count: usize,
    pub status: String,
    pub details: Option<String>,
}

// CloudWatch Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchMetricsRequest {
    pub resource_id: String,
    pub resource_type: String,
    pub region: String,
    pub metrics: Vec<String>,
    pub period: i32,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchMetricsResult {
    pub resource_id: String,
    pub resource_type: String,
    pub metrics: Vec<CloudWatchMetricData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchMetricData {
    pub namespace: String,
    pub metric_name: String,
    pub unit: String,
    pub datapoints: Vec<CloudWatchDatapoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchDatapoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchLogsRequest {
    pub log_group_name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub filter_pattern: Option<String>,
    pub export_path: Option<String>,
    pub upload_to_s3: Option<bool>,
    pub s3_bucket: Option<String>,
    pub post_to_url: Option<String>,
}

// SNS Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnsPublishRequest {
    pub topic_arn: String,
    pub message: String,
    pub subject: Option<String>,
    pub message_attributes: Option<serde_json::Value>,
}

// Lambda Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaInvokeRequest {
    pub function_name: String,
    pub payload: serde_json::Value,
    pub invocation_type: Option<String>,
    pub client_context: Option<String>,
    pub qualifier: Option<String>,
}

// OpenSearch Types 
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenSearchClusterHealthRequest {
    pub domain_name: String,
    pub index: Option<String>,
    pub level: Option<String>,
    pub wait_for_status: Option<String>,
    pub timeout: Option<String>,
}
