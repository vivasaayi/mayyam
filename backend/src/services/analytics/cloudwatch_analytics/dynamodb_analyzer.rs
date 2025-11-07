use super::cloudwatch_analyzer::CloudWatchAnalyzer;
use crate::errors::AppError;
use crate::models::aws_resource;

pub struct DynamoDbAnalyzer;

impl DynamoDbAnalyzer {
    pub async fn analyze_dynamodb_table(
        analyzer: &CloudWatchAnalyzer,
        resource: &aws_resource::Model,
        workflow: &str,
    ) -> Result<String, AppError> {
        let region = if resource.region.is_empty() {
            "us-east-1"
        } else {
            resource.region.as_str()
        };
        match workflow {
            "unused" | "detect_unused" => {
                let periods = ["7 days", "1 month", "2 months"];
                let mut lines = vec![
                    format!("# DynamoDB: {}", resource.resource_id),
                    String::from("## Unused check (hourly)\n"),
                ];
                for p in periods {
                    let (start, end) = analyzer.parse_time_period(p)?;
                    let unused = analyzer
                        .is_unused_in_window_by_hour(
                            "DynamoDB",
                            &resource.resource_id,
                            region,
                            start,
                            end,
                        )
                        .await?;
                    lines.push(format!(
                        "- {}: {}",
                        p,
                        if unused { "unused" } else { "active" }
                    ));
                }
                Ok(lines.join("\n"))
            }
            _ => Ok("DynamoDB workflow not implemented yet".to_string()),
        }
    }
}
