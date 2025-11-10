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


use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchMetricMathRequest {
    pub resource_id: String,
    pub resource_type: String,
    pub region: String,
    pub metrics: Vec<String>,
    pub period: i32,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub math_expressions: Vec<MetricMathExpression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricMathExpression {
    pub id: String,
    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchAnomalyDetectionRequest {
    pub resource_id: String,
    pub resource_type: String,
    pub region: String,
    pub metric_name: String,
    pub period: i32,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub standard_deviation: Option<f64>, // Confidence band width in standard deviations
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchAnomalyResult {
    pub baseline_values: Vec<CloudWatchDatapoint>,
    pub upper_band: Vec<CloudWatchDatapoint>,
    pub lower_band: Vec<CloudWatchDatapoint>,
    pub anomalies: Vec<CloudWatchAnomaly>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchAnomaly {
    pub timestamp: DateTime<Utc>,
    pub expected_value: f64,
    pub actual_value: f64,
    pub deviation: f64, // How many standard deviations away from expected
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchDashboardRequest {
    pub dashboard_name: String,
    pub widgets: Vec<CloudWatchWidget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchWidget {
    pub widget_type: String, // "metric", "text", "alarm", etc.
    pub title: String,
    pub width: i32,
    pub height: i32,
    pub x_position: i32,
    pub y_position: i32,
    pub properties: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchAlarmRequest {
    pub alarm_name: String,
    pub alarm_description: Option<String>,
    pub metric_namespace: String,
    pub metric_name: String,
    pub dimensions: Vec<CloudWatchDimension>,
    pub threshold: f64,
    pub comparison_operator: String,
    pub evaluation_periods: i32,
    pub period: i32,
    pub statistic: String,
    pub actions_enabled: bool,
    pub alarm_actions: Vec<String>,
    pub ok_actions: Vec<String>,
    pub insufficient_data_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchCompositeAlarmRequest {
    pub alarm_name: String,
    pub alarm_description: Option<String>,
    pub alarm_rule: String,
    pub actions_enabled: bool,
    pub alarm_actions: Vec<String>,
    pub ok_actions: Vec<String>,
    pub insufficient_data_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchDimension {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchLogEvent {
    pub timestamp: DateTime<Utc>,
    pub message: String,
    pub ingestion_time: Option<DateTime<Utc>>,
    pub log_stream_name: Option<String>,
    pub event_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchLogsResponse {
    pub events: Vec<CloudWatchLogEvent>,
    pub next_token: Option<String>,
    pub log_group_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchLogGroup {
    pub log_group_name: String,
    pub creation_time: DateTime<Utc>,
    pub retention_in_days: Option<i32>,
    pub stored_bytes: i64,
    pub arn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchLogStream {
    pub log_stream_name: String,
    pub creation_time: DateTime<Utc>,
    pub first_event_timestamp: Option<DateTime<Utc>>,
    pub last_event_timestamp: Option<DateTime<Utc>>,
    pub stored_bytes: i64,
    pub arn: String,
}
