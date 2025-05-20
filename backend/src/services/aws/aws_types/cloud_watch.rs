use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

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