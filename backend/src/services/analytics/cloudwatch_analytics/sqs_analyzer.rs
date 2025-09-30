use super::cloudwatch_analyzer::CloudWatchAnalyzer;
use crate::errors::AppError;
use crate::models::aws_resource::Model as AwsResource;
use chrono::{DateTime, Utc};
use serde_json::json;
use std::sync::Arc;

pub struct SqsAnalyzer;

impl SqsAnalyzer {
    pub async fn analyze_sqs_queue(
        analyzer: &CloudWatchAnalyzer,
        resource: &AwsResource,
        workflow: &str,
    ) -> Result<String, AppError> {
        let time_periods = vec![
            "6 hours", "1 day", "3 days", "7 days", "2 weeks", "1 month", "2 months",
        ];

        match workflow {
            "unused" => {
                let queue_name = resource.name.as_ref().unwrap_or(&resource.resource_id);
                let unused_analysis = analyzer.analyze_unused_resource(
                    "SQS",
                    &queue_name,
                    "us-east-1",
                    &time_periods,
                ).await?;

                let mut result = "# SQS Queue Unused Analysis\n\n".to_string();

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
                    "SQS",
                    &resource.resource_id,
                    &resource.region,
                    now - chrono::Duration::days(7),
                    now,
                ).await?;

                let mut result = "# SQS Queue Usage Classification\n\n".to_string();
                result.push_str(&format!("**Usage Score**: {}/10\n\n", score));

                let description = match score {
                    1..=3 => "Very Low Usage - Consider reviewing queue purpose",
                    4..=6 => "Moderate Usage - Standard queue performance",
                    7..=8 => "High Usage - Good utilization",
                    9..=10 => "Very High Usage - May need performance optimization",
                    _ => "Unknown usage level",
                };

                result.push_str(&format!("**Analysis**: {}\n", description));
                Ok(result)
            },
            "patterns" => {
                let now = Utc::now();
                let patterns = analyzer.detect_usage_patterns(
                    "SQS",
                    &resource.resource_id,
                    &resource.region,
                    now - chrono::Duration::days(7),
                    now,
                ).await?;

                let mut result = "# SQS Queue Usage Patterns\n\n".to_string();
                result.push_str(&patterns);
                Ok(result)
            },
            _ => Ok("# Unknown Workflow\n\nPlease specify a valid workflow: unused, classification, or patterns".to_string()),
        }
    }
}
