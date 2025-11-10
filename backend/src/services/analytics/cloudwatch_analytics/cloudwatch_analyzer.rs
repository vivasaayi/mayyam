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
use crate::services::aws::aws_data_plane::cloudwatch::{
    CloudWatchMetricData, CloudWatchMetrics, CloudWatchMetricsRequest, CloudWatchService,
};
use crate::services::llm::LlmIntegrationService;
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct CloudWatchAnalyzer {
    llm_service: Arc<LlmIntegrationService>,
    cloudwatch_service: Arc<CloudWatchService>,
}

impl CloudWatchAnalyzer {
    pub fn new(
        llm_service: Arc<LlmIntegrationService>,
        cloudwatch_service: Arc<CloudWatchService>,
    ) -> Self {
        Self {
            llm_service,
            cloudwatch_service,
        }
    }

    pub async fn analyze_unused_resource(
        &self,
        resource_type: &str,
        resource_id: &str,
        region: &str,
        time_periods: &[&str],
    ) -> Result<serde_json::Value, AppError> {
        let mut results = json!({});

        for period in time_periods {
            let (start_time, end_time) = self.parse_time_period(period)?;
            let is_unused = self
                .check_resource_unused(resource_type, resource_id, region, start_time, end_time)
                .await?;

            results[period] = json!({
                "unused": is_unused,
                "period": period,
                "start_time": start_time,
                "end_time": end_time
            });
        }

        Ok(results)
    }

    pub async fn classify_resource_usage(
        &self,
        resource_type: &str,
        resource_id: &str,
        region: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<i32, AppError> {
        // Get metrics for classification
        let metrics = self
            .get_resource_metrics(resource_type, resource_id, region, start_time, end_time)
            .await?;

        // Calculate usage score (1-10 scale)
        let score = self.calculate_usage_score(resource_type, &metrics)?;

        Ok(score)
    }

    pub async fn detect_usage_patterns(
        &self,
        resource_type: &str,
        resource_id: &str,
        region: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<String, AppError> {
        let metrics = self
            .get_resource_metrics(resource_type, resource_id, region, start_time, end_time)
            .await?;

        // Use LLM to analyze patterns
        let _prompt =
            self.generate_pattern_analysis_prompt(resource_type, resource_id, &metrics)?;

        // For now, return a placeholder - would need provider ID
        Ok("Pattern analysis requires LLM provider configuration".to_string())
    }

    pub fn parse_time_period(
        &self,
        period: &str,
    ) -> Result<(DateTime<Utc>, DateTime<Utc>), AppError> {
        let now = Utc::now();
        let end_time = now;
        let start_time = match period {
            "6 hours" => now - Duration::hours(6),
            "1 day" => now - Duration::days(1),
            "3 days" => now - Duration::days(3),
            "7 days" => now - Duration::days(7),
            "2 weeks" => now - Duration::weeks(2),
            "1 month" => now - Duration::days(30),
            "2 months" => now - Duration::days(60),
            _ => {
                return Err(AppError::BadRequest(format!(
                    "Invalid time period: {}",
                    period
                )))
            }
        };

        Ok((start_time, end_time))
    }

    async fn check_resource_unused(
        &self,
        resource_type: &str,
        resource_id: &str,
        region: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<bool, AppError> {
        let metrics = self
            .get_resource_metrics(resource_type, resource_id, region, start_time, end_time)
            .await?;

        // Check if all key metrics are zero or very low
        match resource_type {
            "Kinesis" => {
                let has_throughput = metrics.iter().any(|m| {
                    (m.metric_name == "IncomingBytes" || m.metric_name == "OutgoingBytes")
                        && m.datapoints.iter().any(|d| d.value > 0.0)
                });
                Ok(!has_throughput)
            }
            "SQS" => {
                let has_messages = metrics.iter().any(|m| {
                    m.metric_name == "NumberOfMessagesSent"
                        && m.datapoints.iter().any(|d| d.value > 0.0)
                });
                Ok(!has_messages)
            }
            "RDS" => {
                let has_connections = metrics.iter().any(|m| {
                    m.metric_name == "DatabaseConnections"
                        && m.datapoints.iter().any(|d| d.value > 0.0)
                });
                let has_cpu = metrics.iter().any(|m| {
                    m.metric_name == "CPUUtilization" && m.datapoints.iter().any(|d| d.value > 5.0)
                });
                Ok(!has_connections && !has_cpu)
            }
            _ => Ok(false),
        }
    }

    async fn get_resource_metrics(
        &self,
        resource_type: &str,
        resource_id: &str,
        region: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<CloudWatchMetricData>, AppError> {
        let request = CloudWatchMetricsRequest {
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            region: region.to_string(),
            metrics: self.get_metrics_for_resource_type(resource_type),
            start_time,
            end_time,
            period: 300, // 5 minutes
        };

        // Use the provided region instead of default to ensure correct CloudWatch reads
        let aws_account_dto = AwsAccountDto::new_with_profile("", region);

        let result = self
            .cloudwatch_service
            .get_metrics(&aws_account_dto, &request)
            .await?;
        Ok(result.metrics)
    }

    /// Map generic resource types to CloudWatch-specific names for namespace/dimensions
    pub fn map_to_cw_resource_type(&self, resource_type: &str) -> String {
        match resource_type {
            "Kinesis" => "KinesisStream".to_string(),
            "SQS" => "SqsQueue".to_string(),
            "RDS" => "RdsInstance".to_string(),
            "DynamoDB" => "DynamoDbTable".to_string(),
            "S3" => "S3Bucket".to_string(),
            other => other.to_string(),
        }
    }

    /// Hourly-window unused detection using Sum/Max statistics to avoid peak masking
    pub async fn is_unused_in_window_by_hour(
        &self,
        resource_type: &str,
        resource_id: &str,
        region: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<bool, AppError> {
        let cw_type = self.map_to_cw_resource_type(resource_type);

        // Build CloudWatch client context
        let aws_account_dto = AwsAccountDto::new_with_profile("", region);
        let namespace = self
            .cloudwatch_service
            .get_namespace_for_resource_type(&cw_type);
        let dimensions = self
            .cloudwatch_service
            .create_dimensions_for_resource(&cw_type, resource_id);

        // Helper to fetch hourly datapoints for a single metric/statistic
        async fn fetch_stat(
            svc: &CloudWatchService,
            dto: &AwsAccountDto,
            namespace: &str,
            metric: &str,
            dims: Vec<aws_sdk_cloudwatch::types::Dimension>,
            start: DateTime<Utc>,
            end: DateTime<Utc>,
            stat: aws_sdk_cloudwatch::types::Statistic,
        ) -> Result<
            Vec<crate::services::aws::aws_data_plane::cloudwatch::CloudWatchDatapoint>,
            AppError,
        > {
            svc.get_metric_statistics(
                dto,
                namespace,
                metric,
                dims,
                start,
                end,
                3600, // 1 hour
                vec![stat],
            )
            .await
        }

        // Choose metrics per resource type
        match resource_type {
            "Kinesis" => {
                use aws_sdk_cloudwatch::types::Statistic;
                let incoming_records_sum = fetch_stat(
                    &self.cloudwatch_service,
                    &aws_account_dto,
                    namespace,
                    "IncomingRecords",
                    dimensions.clone(),
                    start_time,
                    end_time,
                    Statistic::Sum,
                )
                .await?;
                let incoming_bytes_max = fetch_stat(
                    &self.cloudwatch_service,
                    &aws_account_dto,
                    namespace,
                    "IncomingBytes",
                    dimensions,
                    start_time,
                    end_time,
                    Statistic::Maximum,
                )
                .await?;

                let any_activity = incoming_records_sum.iter().any(|dp| dp.value > 0.0)
                    || incoming_bytes_max.iter().any(|dp| dp.value > 0.0);
                Ok(!any_activity)
            }
            "SQS" => {
                use aws_sdk_cloudwatch::types::Statistic;
                let sent_sum = fetch_stat(
                    &self.cloudwatch_service,
                    &aws_account_dto,
                    namespace,
                    "NumberOfMessagesSent",
                    dimensions.clone(),
                    start_time,
                    end_time,
                    Statistic::Sum,
                )
                .await?;
                let received_max = fetch_stat(
                    &self.cloudwatch_service,
                    &aws_account_dto,
                    namespace,
                    "NumberOfMessagesReceived",
                    dimensions,
                    start_time,
                    end_time,
                    Statistic::Maximum,
                )
                .await?;

                let any_activity = sent_sum.iter().any(|dp| dp.value > 0.0)
                    || received_max.iter().any(|dp| dp.value > 0.0);
                Ok(!any_activity)
            }
            "RDS" => {
                use aws_sdk_cloudwatch::types::Statistic;
                let cpu_max = fetch_stat(
                    &self.cloudwatch_service,
                    &aws_account_dto,
                    namespace,
                    "CPUUtilization",
                    dimensions.clone(),
                    start_time,
                    end_time,
                    Statistic::Maximum,
                )
                .await?;
                let conn_sum = fetch_stat(
                    &self.cloudwatch_service,
                    &aws_account_dto,
                    namespace,
                    "DatabaseConnections",
                    dimensions,
                    start_time,
                    end_time,
                    Statistic::Sum,
                )
                .await?;

                let any_activity = cpu_max.iter().any(|dp| dp.value > 5.0)
                    || conn_sum.iter().any(|dp| dp.value > 0.0);
                Ok(!any_activity)
            }
            "DynamoDB" | "DynamoDbTable" => {
                use aws_sdk_cloudwatch::types::Statistic;
                let read_sum = fetch_stat(
                    &self.cloudwatch_service,
                    &aws_account_dto,
                    namespace,
                    "ConsumedReadCapacityUnits",
                    dimensions.clone(),
                    start_time,
                    end_time,
                    Statistic::Sum,
                )
                .await?;
                let write_sum = fetch_stat(
                    &self.cloudwatch_service,
                    &aws_account_dto,
                    namespace,
                    "ConsumedWriteCapacityUnits",
                    dimensions,
                    start_time,
                    end_time,
                    Statistic::Sum,
                )
                .await?;

                let any_activity = read_sum.iter().any(|dp| dp.value > 0.0)
                    || write_sum.iter().any(|dp| dp.value > 0.0);
                Ok(!any_activity)
            }
            "S3" | "S3Bucket" => {
                use aws_sdk_cloudwatch::types::Statistic;
                // S3 request metrics often require FilterId="EntireBucket" dimension
                let mut req_dims = dimensions.clone();
                req_dims.push(
                    aws_sdk_cloudwatch::types::Dimension::builder()
                        .name("FilterId")
                        .value("EntireBucket")
                        .build(),
                );

                let get_sum = fetch_stat(
                    &self.cloudwatch_service,
                    &aws_account_dto,
                    namespace,
                    "GetRequests",
                    req_dims.clone(),
                    start_time,
                    end_time,
                    Statistic::Sum,
                )
                .await?;
                let put_sum = fetch_stat(
                    &self.cloudwatch_service,
                    &aws_account_dto,
                    namespace,
                    "PutRequests",
                    req_dims.clone(),
                    start_time,
                    end_time,
                    Statistic::Sum,
                )
                .await?;
                let delete_sum = fetch_stat(
                    &self.cloudwatch_service,
                    &aws_account_dto,
                    namespace,
                    "DeleteRequests",
                    req_dims,
                    start_time,
                    end_time,
                    Statistic::Sum,
                )
                .await?;

                // If request metrics are disabled (empty datapoints), avoid false positives
                let no_data = get_sum.is_empty() && put_sum.is_empty() && delete_sum.is_empty();
                if no_data {
                    return Ok(false);
                }

                let any_activity = get_sum.iter().any(|dp| dp.value > 0.0)
                    || put_sum.iter().any(|dp| dp.value > 0.0)
                    || delete_sum.iter().any(|dp| dp.value > 0.0);
                Ok(!any_activity)
            }
            // Default fallback uses original average-based check
            _ => {
                self.check_resource_unused(resource_type, resource_id, region, start_time, end_time)
                    .await
            }
        }
    }

    fn get_metrics_for_resource_type(&self, resource_type: &str) -> Vec<String> {
        match resource_type {
            "Kinesis" => vec![
                "IncomingBytes".to_string(),
                "OutgoingBytes".to_string(),
                "IncomingRecords".to_string(),
                "OutgoingRecords".to_string(),
            ],
            "SQS" => vec![
                "NumberOfMessagesSent".to_string(),
                "NumberOfMessagesReceived".to_string(),
                "ApproximateNumberOfMessagesVisible".to_string(),
            ],
            "RDS" => vec![
                "CPUUtilization".to_string(),
                "DatabaseConnections".to_string(),
                "ReadIOPS".to_string(),
                "WriteIOPS".to_string(),
            ],
            _ => vec![],
        }
    }

    fn calculate_usage_score(
        &self,
        resource_type: &str,
        metrics: &[CloudWatchMetricData],
    ) -> Result<i32, AppError> {
        match resource_type {
            "Kinesis" => {
                let avg_throughput = metrics
                    .iter()
                    .filter(|m| {
                        m.metric_name == "IncomingBytes" || m.metric_name == "OutgoingBytes"
                    })
                    .map(|m| {
                        m.datapoints.iter().map(|d| d.value).sum::<f64>()
                            / m.datapoints.len() as f64
                    })
                    .sum::<f64>();

                // Scale: < 1MB/s = 1, > 100MB/s = 10
                let score = ((avg_throughput / 1000000.0).log10() * 3.0).clamp(1.0, 10.0) as i32;
                Ok(score)
            }
            "SQS" => {
                let avg_messages = metrics
                    .iter()
                    .filter(|m| m.metric_name == "NumberOfMessagesSent")
                    .map(|m| {
                        m.datapoints.iter().map(|d| d.value).sum::<f64>()
                            / m.datapoints.len() as f64
                    })
                    .sum::<f64>();

                // Scale: < 1 msg/min = 1, > 1000 msg/min = 10
                let score = ((avg_messages / 60.0).log10() * 2.0).clamp(1.0, 10.0) as i32;
                Ok(score)
            }
            "RDS" => {
                let avg_cpu = metrics
                    .iter()
                    .filter(|m| m.metric_name == "CPUUtilization")
                    .map(|m| {
                        m.datapoints.iter().map(|d| d.value).sum::<f64>()
                            / m.datapoints.len() as f64
                    })
                    .sum::<f64>();

                let avg_connections = metrics
                    .iter()
                    .filter(|m| m.metric_name == "DatabaseConnections")
                    .map(|m| {
                        m.datapoints.iter().map(|d| d.value).sum::<f64>()
                            / m.datapoints.len() as f64
                    })
                    .sum::<f64>();

                // Combined score based on CPU and connections
                let cpu_score = (avg_cpu / 10.0).clamp(1.0, 10.0);
                let conn_score = (avg_connections / 10.0).clamp(1.0, 10.0);
                let combined = (cpu_score + conn_score) / 2.0;

                Ok(combined as i32)
            }
            _ => Ok(5), // Default medium usage
        }
    }

    fn generate_pattern_analysis_prompt(
        &self,
        resource_type: &str,
        resource_id: &str,
        metrics: &[CloudWatchMetricData],
    ) -> Result<String, AppError> {
        let mut prompt = format!(
            "Analyze usage patterns for {} resource {}:\n\nMetrics:\n",
            resource_type, resource_id
        );

        for metric in metrics.iter().take(20) {
            prompt.push_str(&format!(
                "- {}: {} data points\n",
                metric.metric_name,
                metric.datapoints.len()
            ));
            for point in metric.datapoints.iter().take(5) {
                prompt.push_str(&format!("  {}: {}\n", point.timestamp, point.value));
            }
        }

        prompt.push_str("\nIdentify patterns such as:\n- Seasonal spikes\n- Steady usage\n- Burst patterns\n- Idle periods\n- Peak hours\n");

        Ok(prompt)
    }
}
