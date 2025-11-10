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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchMetricsRequest {
    pub resource_type: String,
    pub resource_id: String,
    pub region: String,
    pub metrics: Vec<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub period: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchLogsRequest {
    pub log_group_name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub filter_pattern: Option<String>,
    pub limit: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchDatapoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchMetricData {
    pub namespace: String,
    pub metric_name: String,
    pub unit: String,
    pub datapoints: Vec<CloudWatchDatapoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchMetricsResult {
    pub resource_id: String,
    pub resource_type: String,
    pub metrics: Vec<CloudWatchMetricData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchAlarmDetails {
    pub alarm_name: String,
    pub namespace: String,
    pub metric_name: String,
    pub threshold: f64,
    pub comparison_operator: String,
    pub evaluation_periods: i32,
    pub period: i32,
    pub statistic: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardWidgetConfig {
    pub title: String,
    pub width: i32,
    pub height: i32,
    pub view: String,
}

// Helper functions for time conversion
pub(crate) fn to_aws_datetime(dt: &DateTime<Utc>) -> aws_sdk_cloudwatch::primitives::DateTime {
    aws_sdk_cloudwatch::primitives::DateTime::from_secs(dt.timestamp())
}

pub(crate) fn from_aws_datetime(dt: &aws_sdk_cloudwatch::primitives::DateTime) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(dt.secs(), 0).unwrap_or_else(|| Utc::now())
}
