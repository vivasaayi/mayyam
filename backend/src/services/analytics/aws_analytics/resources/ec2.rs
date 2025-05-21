use crate::errors::AppError;
use crate::models::aws_resource;
use crate::services::aws::aws_types::cloud_watch;
use crate::services::analytics::aws_analytics::models::resource_workflows::ResourceAnalysisWorkflow;
use crate::services::analytics::aws_analytics::metrics::MetricsAnalyzer;

pub struct Ec2Analyzer;

impl Ec2Analyzer {
    pub async fn analyze_ec2_instance(
        _resource: &aws_resource::Model,
        workflow: &ResourceAnalysisWorkflow,
        metrics: &cloud_watch::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut analysis = String::new();

        match workflow {
            ResourceAnalysisWorkflow::Performance => {
                analysis.push_str("# EC2 Instance Performance Analysis\n\n");

                // Analyze CPU metrics
                if let Some(cpu_metric) = MetricsAnalyzer::find_metric(metrics, "CPUUtilization") {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&cpu_metric.datapoints);
                    analysis.push_str(&format!(
                        "## CPU Utilization\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                        avg, max
                    ));

                    // Add recommendations based on CPU usage
                    if max > 80.0 {
                        analysis.push_str("⚠️ **High CPU Usage Detected**\n");
                        analysis.push_str("Consider:\n");
                        analysis.push_str("1. Scaling up the instance type\n");
                        analysis.push_str("2. Using auto-scaling groups\n");
                        analysis.push_str("3. Analyzing resource-intensive processes\n\n");
                    }
                }

                // Analyze memory metrics if available
                if let Some(mem_metric) = MetricsAnalyzer::find_metric(metrics, "MemoryUtilization") {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&mem_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Memory Utilization\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                        avg, max
                    ));
                }

                // Network analysis
                MetricsAnalyzer::analyze_network_metrics(&mut analysis, metrics);
            },
            ResourceAnalysisWorkflow::Cost => {
                analysis.push_str("# EC2 Instance Cost Analysis\n\n");
                // Implement cost analysis...
                analysis.push_str("Cost analysis not yet implemented\n");
            },
            _ => {
                return Err(AppError::BadRequest(
                    "Unsupported workflow type for EC2 instance".to_string()
                ));
            }
        }

        Ok(analysis)
    }

    pub async fn answer_ec2_question(
        _resource: &aws_resource::Model,
        question: &str,
        metrics: &cloud_watch::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut answer = String::new();

        // Convert question to lowercase for easier matching
        let question = question.to_lowercase();

        if question.contains("cpu") || question.contains("processor") {
            if let Some(cpu_metric) = MetricsAnalyzer::find_metric(metrics, "CPUUtilization") {
                let (avg, max) = MetricsAnalyzer::calculate_statistics(&cpu_metric.datapoints);
                answer.push_str(&format!(
                    "The EC2 instance's CPU usage:\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                    avg, max
                ));
            }
        }

        if question.contains("memory") || question.contains("ram") {
            if let Some(mem_metric) = MetricsAnalyzer::find_metric(metrics, "MemoryUtilization") {
                let (avg, max) = MetricsAnalyzer::calculate_statistics(&mem_metric.datapoints);
                answer.push_str(&format!(
                    "Memory utilization:\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                    avg, max
                ));
            }
        }

        if answer.is_empty() {
            answer = "I apologize, but I don't have enough information to answer that specific question about the EC2 instance.".to_string();
        }

        Ok(answer)
    }
}