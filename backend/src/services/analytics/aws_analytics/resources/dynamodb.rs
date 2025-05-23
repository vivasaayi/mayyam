use crate::errors::AppError;
use crate::models::aws_resource;
use crate::services::aws::aws_types::cloud_watch;
use crate::services::analytics::aws_analytics::models::resource_workflows::ResourceAnalysisWorkflow;
use crate::services::analytics::aws_analytics::metrics::MetricsAnalyzer;

pub struct DynamoDbAnalyzer;

impl DynamoDbAnalyzer {
    pub async fn analyze_dynamodb_table(
        _resource: &aws_resource::Model,
        workflow: &ResourceAnalysisWorkflow,
        metrics: &cloud_watch::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut analysis = String::new();

        match workflow {
            ResourceAnalysisWorkflow::Performance => {
                analysis.push_str("# DynamoDB Table Performance Analysis\n\n");

                // Analyze read/write capacity
                if let Some(read_metric) = MetricsAnalyzer::find_metric(metrics, "ConsumedReadCapacityUnits") {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&read_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Read Capacity Usage\n- Average: {:.2} RCUs\n- Peak: {:.2} RCUs\n\n",
                        avg, max
                    ));
                }
                
                // Add write capacity analysis
                if let Some(write_metric) = MetricsAnalyzer::find_metric(metrics, "ConsumedWriteCapacityUnits") {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&write_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Write Capacity Usage\n- Average: {:.2} WCUs\n- Peak: {:.2} WCUs\n\n",
                        avg, max
                    ));
                }
                
                // Add throttling events analysis
                if let Some(read_throttle_metric) = MetricsAnalyzer::find_metric(metrics, "ReadThrottleEvents") {
                    let (_, max) = MetricsAnalyzer::calculate_statistics(&read_throttle_metric.datapoints);
                    if max > 0.0 {
                        analysis.push_str("⚠️ **Read Throttling Detected**\n");
                        analysis.push_str("Consider increasing read capacity or implementing retry logic.\n\n");
                    }
                }
                
                if let Some(write_throttle_metric) = MetricsAnalyzer::find_metric(metrics, "WriteThrottleEvents") {
                    let (_, max) = MetricsAnalyzer::calculate_statistics(&write_throttle_metric.datapoints);
                    if max > 0.0 {
                        analysis.push_str("⚠️ **Write Throttling Detected**\n");
                        analysis.push_str("Consider increasing write capacity or implementing retry logic.\n\n");
                    }
                }
            },
            ResourceAnalysisWorkflow::Cost => {
                analysis.push_str("# DynamoDB Cost Analysis\n\n");
                // Add cost analysis implementation
                analysis.push_str("Cost analysis not yet implemented for DynamoDB\n");
            },
            _ => return Err(AppError::BadRequest(
                "Unsupported workflow type for DynamoDB table".to_string()
            )),
        }

        Ok(analysis)
    }
}