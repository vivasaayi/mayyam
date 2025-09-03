use std::sync::Arc;
use chrono::{DateTime, Utc, Duration};
use serde_json::json;
use crate::errors::AppError;
use crate::services::llm_integration::LlmIntegrationService;
use crate::services::aws::aws_data_plane::cloudwatch::metrics::CloudWatchMetrics;
use crate::services::aws::aws_data_plane::cloudwatch::types::CloudWatchMetricsRequest;

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
            let is_unused = self.check_resource_unused(resource_type, resource_id, region, start_time, end_time).await?;

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
        let metrics = self.get_resource_metrics(resource_type, resource_id, region, start_time, end_time).await?;

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
        let metrics = self.get_resource_metrics(resource_type, resource_id, region, start_time, end_time).await?;

        // Use LLM to analyze patterns
        let _prompt = self.generate_pattern_analysis_prompt(resource_type, resource_id, &metrics)?;

        // For now, return a placeholder - would need provider ID
        Ok("Pattern analysis requires LLM provider configuration".to_string())
    }

    fn parse_time_period(&self, period: &str) -> Result<(DateTime<Utc>, DateTime<Utc>), AppError> {
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
            _ => return Err(AppError::BadRequest(format!("Invalid time period: {}", period))),
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
        let metrics = self.get_resource_metrics(resource_type, resource_id, region, start_time, end_time).await?;

        // Check if all key metrics are zero or very low
        match resource_type {
            "Kinesis" => {
                let has_throughput = metrics.iter().any(|m|
                    (m.metric_name == "IncomingBytes" || m.metric_name == "OutgoingBytes") 
                    && m.datapoints.iter().any(|d| d.value > 0.0)
                );
                Ok(!has_throughput)
            },
            "SQS" => {
                let has_messages = metrics.iter().any(|m|
                    m.metric_name == "NumberOfMessagesSent" 
                    && m.datapoints.iter().any(|d| d.value > 0.0)
                );
                Ok(!has_messages)
            },
            "RDS" => {
                let has_connections = metrics.iter().any(|m|
                    m.metric_name == "DatabaseConnections" 
                    && m.datapoints.iter().any(|d| d.value > 0.0)
                );
                let has_cpu = metrics.iter().any(|m|
                    m.metric_name == "CPUUtilization" 
                    && m.datapoints.iter().any(|d| d.value > 5.0)
                );
                Ok(!has_connections && !has_cpu)
            },
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
    ) -> Result<Vec<crate::services::aws::aws_data_plane::cloudwatch::types::CloudWatchMetricData>, AppError> {
        let request = CloudWatchMetricsRequest {
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            metrics: self.get_metrics_for_resource_type(resource_type),
            start_time,
            end_time,
            period: 300, // 5 minutes
        };

        let result = self.cloudwatch_service.get_metrics(None, region, &request).await?;
        Ok(result.metrics)
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

    fn calculate_usage_score(&self, resource_type: &str, metrics: &[crate::services::aws::aws_data_plane::cloudwatch::types::CloudWatchMetricData]) -> Result<i32, AppError> {
        match resource_type {
            "Kinesis" => {
                let avg_throughput = metrics.iter()
                    .filter(|m| m.metric_name == "IncomingBytes" || m.metric_name == "OutgoingBytes")
                    .map(|m| m.datapoints.iter().map(|d| d.value).sum::<f64>() / m.datapoints.len() as f64)
                    .sum::<f64>();

                // Scale: < 1MB/s = 1, > 100MB/s = 10
                let score = ((avg_throughput / 1000000.0).log10() * 3.0).clamp(1.0, 10.0) as i32;
                Ok(score)
            },
            "SQS" => {
                let avg_messages = metrics.iter()
                    .filter(|m| m.metric_name == "NumberOfMessagesSent")
                    .map(|m| m.datapoints.iter().map(|d| d.value).sum::<f64>() / m.datapoints.len() as f64)
                    .sum::<f64>();

                // Scale: < 1 msg/min = 1, > 1000 msg/min = 10
                let score = ((avg_messages / 60.0).log10() * 2.0).clamp(1.0, 10.0) as i32;
                Ok(score)
            },
            "RDS" => {
                let avg_cpu = metrics.iter()
                    .filter(|m| m.metric_name == "CPUUtilization")
                    .map(|m| m.datapoints.iter().map(|d| d.value).sum::<f64>() / m.datapoints.len() as f64)
                    .sum::<f64>();

                let avg_connections = metrics.iter()
                    .filter(|m| m.metric_name == "DatabaseConnections")
                    .map(|m| m.datapoints.iter().map(|d| d.value).sum::<f64>() / m.datapoints.len() as f64)
                    .sum::<f64>();

                // Combined score based on CPU and connections
                let cpu_score = (avg_cpu / 10.0).clamp(1.0, 10.0);
                let conn_score = (avg_connections / 10.0).clamp(1.0, 10.0);
                let combined = (cpu_score + conn_score) / 2.0;

                Ok(combined as i32)
            },
            _ => Ok(5), // Default medium usage
        }
    }

    fn generate_pattern_analysis_prompt(
        &self,
        resource_type: &str,
        resource_id: &str,
        metrics: &[crate::services::aws::aws_data_plane::cloudwatch::types::CloudWatchMetricData],
    ) -> Result<String, AppError> {
        let mut prompt = format!(
            "Analyze usage patterns for {} resource {}:\n\nMetrics:\n",
            resource_type, resource_id
        );

        for metric in metrics.iter().take(20) {
            prompt.push_str(&format!("- {}: {} data points\n", metric.metric_name, metric.datapoints.len()));
            for point in metric.datapoints.iter().take(5) {
                prompt.push_str(&format!("  {}: {}\n", point.timestamp, point.value));
            }
        }

        prompt.push_str("\nIdentify patterns such as:\n- Seasonal spikes\n- Steady usage\n- Burst patterns\n- Idle periods\n- Peak hours\n");

        Ok(prompt)
    }
}
