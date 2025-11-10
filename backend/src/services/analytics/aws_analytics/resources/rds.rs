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
use crate::models::aws_resource;
use crate::services::analytics::aws_analytics::metrics::MetricsAnalyzer;
use crate::services::analytics::aws_analytics::models::resource_workflows::ResourceAnalysisWorkflow;
use crate::services::aws::aws_types::cloud_watch;

pub struct RdsAnalyzer;

impl RdsAnalyzer {
    pub async fn analyze_rds_instance(
        _resource: &aws_resource::Model,
        workflow: &ResourceAnalysisWorkflow,
        metrics: &cloud_watch::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut analysis = String::new();

        match workflow {
            ResourceAnalysisWorkflow::Performance => {
                analysis.push_str("# RDS Instance Performance Analysis\n\n");

                // CPU analysis
                if let Some(cpu_metric) = MetricsAnalyzer::find_metric(metrics, "CPUUtilization") {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&cpu_metric.datapoints);
                    analysis.push_str(&format!(
                        "## CPU Utilization\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                        avg, max
                    ));
                }

                // Memory analysis
                if let Some(mem_metric) = MetricsAnalyzer::find_metric(metrics, "FreeableMemory") {
                    let (avg, _) = MetricsAnalyzer::calculate_statistics(&mem_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Memory Usage\n- Average Free Memory: {:.2} MB\n\n",
                        avg / (1024.0 * 1024.0)
                    ));
                }
            }
            ResourceAnalysisWorkflow::Storage => {
                analysis.push_str("# RDS Storage Analysis\n\n");

                // Storage usage analysis
                if let Some(storage_metric) =
                    MetricsAnalyzer::find_metric(metrics, "FreeStorageSpace")
                {
                    let (avg, _) =
                        MetricsAnalyzer::calculate_statistics(&storage_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Storage Usage\n- Average Free Space: {:.2} GB\n\n",
                        avg / (1024.0 * 1024.0 * 1024.0)
                    ));
                }
            }
            _ => {
                return Err(AppError::BadRequest(
                    "Unsupported workflow type for RDS instance".to_string(),
                ))
            }
        }

        Ok(analysis)
    }

    pub async fn answer_rds_question(
        _resource: &aws_resource::Model,
        question: &str,
        metrics: &cloud_watch::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut answer = String::new();

        let question = question.to_lowercase();

        if question.contains("cpu") {
            if let Some(cpu_metric) = MetricsAnalyzer::find_metric(metrics, "CPUUtilization") {
                let (avg, max) = MetricsAnalyzer::calculate_statistics(&cpu_metric.datapoints);
                answer.push_str(&format!(
                    "The RDS instance's CPU usage:\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                    avg, max
                ));
            }
        }

        if question.contains("memory") || question.contains("ram") {
            if let Some(mem_metric) = MetricsAnalyzer::find_metric(metrics, "FreeableMemory") {
                let (avg, _) = MetricsAnalyzer::calculate_statistics(&mem_metric.datapoints);
                answer.push_str(&format!(
                    "Memory status:\n- Average Free Memory: {:.2} MB\n\n",
                    avg / (1024.0 * 1024.0)
                ));
            }
        }

        if answer.is_empty() {
            answer = "I apologize, but I don't have enough information to answer that specific question about the RDS instance.".to_string();
        }

        Ok(answer)
    }
}
