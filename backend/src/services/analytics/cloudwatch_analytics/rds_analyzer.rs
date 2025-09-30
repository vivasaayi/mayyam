use super::cloudwatch_analyzer::CloudWatchAnalyzer;
use crate::errors::AppError;
use crate::models::aws_resource::Model as AwsResource;
use chrono::{DateTime, Utc};
use serde_json::json;
use std::sync::Arc;

pub struct RdsAnalyzer;

impl RdsAnalyzer {
    pub async fn analyze_rds_instance(
        analyzer: &CloudWatchAnalyzer,
        resource: &AwsResource,
        workflow: &str,
    ) -> Result<String, AppError> {
        let time_periods = vec![
            "6 hours", "1 day", "3 days", "7 days", "2 weeks", "1 month", "2 months",
        ];

        match workflow {
            "unused" => {
                let unused_analysis = analyzer.analyze_unused_resource(
                    "RDS",
                    &resource.resource_id,
                    "us-east-1",
                    &time_periods,
                ).await?;

                let mut result = "# RDS Instance Unused Analysis\n\n".to_string();

                for period in time_periods {
                    if let Some(analysis) = unused_analysis.get(period) {
                        let status = if analysis["unused"].as_bool().unwrap_or(false) {
                            "✅ Unused"
                        } else {
                            "❌ In Use"
                        };
                        result.push_str(&format!("- **{}**: {}\n", period, status));
                    }
                }

                Ok(result)
            },
            "classification" => {
                let now = Utc::now();
                let score = analyzer.classify_resource_usage(
                    "RDS",
                    &resource.resource_id,
                    &resource.region,
                    now - chrono::Duration::days(7),
                    now,
                ).await?;

                let mut result = "# RDS Instance Usage Classification\n\n".to_string();
                result.push_str(&format!("**Usage Score**: {}/10\n\n", score));

                let description = match score {
                    1..=3 => "Very Low Usage - Consider rightsizing or serverless options",
                    4..=6 => "Moderate Usage - Standard performance",
                    7..=8 => "High Usage - Good utilization",
                    9..=10 => "Very High Usage - May need scaling up",
                    _ => "Unknown usage level",
                };

                result.push_str(&format!("**Analysis**: {}\n", description));
                Ok(result)
            },
            "patterns" => {
                let now = Utc::now();
                let patterns = analyzer.detect_usage_patterns(
                    "RDS",
                    &resource.resource_id,
                    &resource.region,
                    now - chrono::Duration::days(7),
                    now,
                ).await?;

                let mut result = "# RDS Instance Usage Patterns\n\n".to_string();
                result.push_str(&patterns);
                Ok(result)
            },
            "scaling" => {
                let now = Utc::now();
                let score = analyzer.classify_resource_usage(
                    "RDS",
                    &resource.resource_id,
                    &resource.region,
                    now - chrono::Duration::days(7),
                    now,
                ).await?;

                let mut result = "# RDS Instance Scaling Recommendations\n\n".to_string();

                if score <= 3 {
                    result.push_str("**Recommendation**: Consider rightsizing to smaller instance type.\n");
                    result.push_str("- Current usage is very low\n");
                    result.push_str("- Evaluate Aurora Serverless or smaller instance classes\n");
                } else if score >= 8 {
                    result.push_str("**Recommendation**: Consider scaling up instance type.\n");
                    result.push_str("- Current usage is high\n");
                    result.push_str("- Monitor CPU, memory, and IOPS metrics\n");
                    result.push_str("- Consider read replicas for read-heavy workloads\n");
                } else {
                    result.push_str("**Recommendation**: Current instance size appears appropriate.\n");
                    result.push_str("- Usage is balanced\n");
                    result.push_str("- Continue monitoring performance metrics\n");
                }

                Ok(result)
            },
            _ => Ok("# Unknown Workflow\n\nPlease specify a valid workflow: unused, classification, patterns, or scaling".to_string()),
        }
    }

    pub async fn answer_rds_question(
        resource: &AwsResource,
        question: &str,
        metrics: &crate::services::aws::aws_types::cloud_watch::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        // Simple question answering based on RDS metrics
        let response = format!(
            "# RDS Question Analysis: {}\n\n**Resource**: {}\n\n",
            question, resource.resource_id
        );

        // Add basic analysis based on available metrics
        for metric in &metrics.metrics {
            match metric.metric_name.as_str() {
                "CPUUtilization" => {
                    if let Some(avg) = metric
                        .datapoints
                        .iter()
                        .map(|d| d.value)
                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                    {
                        let response = format!("{}**CPU Utilization**: {:.2}%\n", response, avg);
                    }
                }
                "DatabaseConnections" => {
                    if let Some(avg) = metric
                        .datapoints
                        .iter()
                        .map(|d| d.value)
                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                    {
                        let response =
                            format!("{}**Database Connections**: {:.0}\n", response, avg);
                    }
                }
                _ => {}
            }
        }

        Ok(format!("{}**Analysis**: This is a basic RDS analysis. For more detailed insights, consider using the full analytics workflow.\n", response))
    }
}
