use super::cloudwatch_analyzer::CloudWatchAnalyzer;
use crate::errors::AppError;
use crate::models::aws_resource::Model as AwsResource;
use chrono::{DateTime, Utc};
use serde_json::json;
use std::sync::Arc;

pub struct KinesisAnalyzer;

impl KinesisAnalyzer {
    pub async fn analyze_kinesis_stream(
        analyzer: &CloudWatchAnalyzer,
        resource: &AwsResource,
        workflow: &str,
    ) -> Result<String, AppError> {
        let time_periods = vec![
            "6 hours", "1 day", "3 days", "7 days", "2 weeks", "1 month", "2 months",
        ];

        match workflow {
            "unused" => {
                let stream_name = resource.name.as_ref().unwrap_or(&resource.resource_id);
                // Use the resource's actual region and an hourly-aware unused check to avoid masking peaks
                let mut unused_analysis = serde_json::json!({});
                for period in &time_periods {
                    let (start_time, end_time) = analyzer.parse_time_period(period)?;
                    let is_unused = analyzer
                        .is_unused_in_window_by_hour(
                            "Kinesis",
                            &stream_name,
                            &resource.region,
                            start_time,
                            end_time,
                        )
                        .await?;
                    unused_analysis[*period] = serde_json::json!({
                        "unused": is_unused,
                        "period": period,
                        "start_time": start_time,
                        "end_time": end_time
                    });
                }

                let mut result = "# Kinesis Stream Unused Analysis\n\n".to_string();

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
                    "Kinesis",
                    &resource.resource_id,
                    &resource.region,
                    now - chrono::Duration::days(7),
                    now,
                ).await?;

                let mut result = "# Kinesis Stream Usage Classification\n\n".to_string();
                result.push_str(&format!("**Usage Score**: {}/10\n\n", score));

                let description = match score {
                    1..=3 => "Very Low Usage - Consider reducing shard count",
                    4..=6 => "Moderate Usage - Monitor for optimization opportunities",
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
                    "Kinesis",
                    &resource.resource_id,
                    &resource.region,
                    now - chrono::Duration::days(7),
                    now,
                ).await?;

                let mut result = "# Kinesis Stream Usage Patterns\n\n".to_string();
                result.push_str(&patterns);
                Ok(result)
            },
            "scaling" => {
                let now = Utc::now();
                let score = analyzer.classify_resource_usage(
                    "Kinesis",
                    &resource.resource_id,
                    &resource.region,
                    now - chrono::Duration::days(7),
                    now,
                ).await?;

                let mut result = "# Kinesis Stream Scaling Recommendations\n\n".to_string();

                if score <= 3 {
                    result.push_str("**Recommendation**: Consider reducing the number of shards to optimize costs.\n");
                    result.push_str("- Current usage is very low\n");
                    result.push_str("- Monitor for at least 2 weeks before scaling down\n");
                } else if score >= 8 {
                    result.push_str("**Recommendation**: Consider increasing shards for better throughput.\n");
                    result.push_str("- Current usage is high\n");
                    result.push_str("- Monitor ProvisionedThroughputExceeded metrics\n");
                } else {
                    result.push_str("**Recommendation**: Current shard configuration appears optimal.\n");
                    result.push_str("- Usage is balanced\n");
                    result.push_str("- Continue monitoring performance\n");
                }

                Ok(result)
            },
            _ => Ok("# Unknown Workflow\n\nPlease specify a valid workflow: unused, classification, patterns, or scaling".to_string()),
        }
    }
}
