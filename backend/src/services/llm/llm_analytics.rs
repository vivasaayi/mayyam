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
use crate::models::llm_provider::LlmProviderModel;
use crate::models::{Insight, InsightSeverity, Recommendation, RecommendationPriority};
use crate::repositories::data_source::DataSourceRepository;
use crate::repositories::llm_provider::LlmProviderRepository;
use crate::repositories::prompt_template::PromptTemplateRepository;
use crate::services::data_collection::DataCollectionService;
use crate::services::llm::interface::UnifiedLlmRequest;
use crate::services::llm::manager::UnifiedLlmManager;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalysisType {
    Custom(String),
    // Add more variants as needed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
}

pub struct LlmAnalyticsService {
    pub llm_manager: Arc<UnifiedLlmManager>,
    pub data_collection_service: Arc<DataCollectionService>,
    pub data_source_repo: Arc<DataSourceRepository>,
    pub llm_provider_repo: Arc<LlmProviderRepository>,
    pub prompt_template_repo: Arc<PromptTemplateRepository>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAnalyticsRequest {
    pub resource_id: String,
    pub resource_type: crate::models::data_source::ResourceType,
    pub data_source_ids: Vec<uuid::Uuid>,
    pub analysis_type: AnalysisType,
    pub time_range: TimeRange,
    pub custom_prompts: Option<Vec<String>>,
    pub llm_provider_id: Option<uuid::Uuid>,
}

impl LlmAnalyticsService {
    pub fn new(
        llm_manager: Arc<UnifiedLlmManager>,
        data_collection_service: Arc<DataCollectionService>,
        data_source_repo: Arc<DataSourceRepository>,
        llm_provider_repo: Arc<LlmProviderRepository>,
        prompt_template_repo: Arc<PromptTemplateRepository>,
    ) -> Self {
        Self {
            llm_manager,
            data_collection_service,
            data_source_repo,
            llm_provider_repo,
            prompt_template_repo,
        }
    }

    pub fn process_section(
        &self,
        section: &str,
        content: &[&str],
        insights: &mut Vec<Insight>,
        recommendations: &mut Vec<Recommendation>,
    ) {
        let text = content.join(" ");
        match section {
            "insights" => {
                insights.push(Insight {
                    title: "Analysis Finding".to_string(),
                    description: text.clone(),
                    severity: InsightSeverity::Medium,
                    category: "General".to_string(),
                    metrics_involved: vec![],
                    supporting_data: json!({}),
                });
            }
            "recommendations" => {
                recommendations.push(Recommendation {
                    title: "Recommended Action".to_string(),
                    description: text.clone(),
                    priority: RecommendationPriority::Medium,
                    impact: "Moderate".to_string(),
                    action_items: vec![text],
                    estimated_effort: None,
                });
            }
            _ => {}
        }
    }

    pub async fn analyze(
        &self,
        req: ServiceAnalyticsRequest,
    ) -> Result<serde_json::Value, crate::errors::AppError> {
        // Get data from data sources
        let mut all_metrics = Vec::new();

        for data_source_id in &req.data_source_ids {
            let data_request = crate::services::data_collection::DataCollectionRequest {
                data_source_id: *data_source_id,
                resource_ids: vec![req.resource_id.clone()],
                metric_names: self.get_metrics_for_resource_type(&req.resource_type),
                start_time: req.time_range.start_time,
                end_time: req.time_range.end_time,
                period: Some(300), // 5 minutes
            };

            match self
                .data_collection_service
                .collect_data(data_request)
                .await
            {
                Ok(response) => {
                    all_metrics.extend(response.metrics);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to collect data from source {}: {}",
                        data_source_id,
                        e
                    );
                }
            }
        }

        if all_metrics.is_empty() {
            return Ok(serde_json::json!({
                "error": "No metrics data available for analysis"
            }));
        }

        // Generate prompt based on analysis type and resource type
        let prompt = self.generate_analysis_prompt(&req, &all_metrics)?;

        // Get LLM provider (use default if not specified)
        let provider_id = req.llm_provider_id.unwrap_or_else(|| {
            // TODO: Get default provider from config
            uuid::Uuid::new_v4() // Placeholder
        });

        // Call LLM using unified interface
        let llm_request = UnifiedLlmRequest {
            prompt,
            system_prompt: Some("You are an expert AWS cloud analyst. Provide detailed, actionable insights based on the metrics data provided.".to_string()),
            temperature: Some(0.3),
            max_tokens: Some(2000),
            top_p: None,
            top_k: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            enable_thinking: None,
            stream: None,
            extra_params: None,
            context: None,
        };

        let llm_response = self.llm_manager.generate_smart(llm_request).await?;

        // Parse LLM response and structure it
        let analysis_result = self.parse_llm_response(&llm_response.response.content)?;

        Ok(serde_json::json!({
            "resource_id": req.resource_id,
            "resource_type": format!("{:?}", req.resource_type),
            "analysis_type": format!("{:?}", req.analysis_type),
            "time_range": {
                "start": req.time_range.start_time,
                "end": req.time_range.end_time
            },
            "analysis_result": analysis_result,
            "llm_provider": llm_response.response.provider,
            "timestamp": llm_response.response.timestamp,
            "raw_response": llm_response.response.content
        }))
    }

    fn get_metrics_for_resource_type(
        &self,
        resource_type: &crate::models::data_source::ResourceType,
    ) -> Vec<String> {
        match resource_type {
            crate::models::data_source::ResourceType::Kinesis => vec![
                "IncomingBytes".to_string(),
                "OutgoingBytes".to_string(),
                "IncomingRecords".to_string(),
                "OutgoingRecords".to_string(),
                "ReadProvisionedThroughputExceeded".to_string(),
                "WriteProvisionedThroughputExceeded".to_string(),
            ],
            crate::models::data_source::ResourceType::SQS => vec![
                "NumberOfMessagesSent".to_string(),
                "NumberOfMessagesReceived".to_string(),
                "ApproximateNumberOfMessagesVisible".to_string(),
                "ApproximateAgeOfOldestMessage".to_string(),
                "NumberOfEmptyReceives".to_string(),
            ],
            crate::models::data_source::ResourceType::RDS => vec![
                "CPUUtilization".to_string(),
                "DatabaseConnections".to_string(),
                "ReadIOPS".to_string(),
                "WriteIOPS".to_string(),
                "FreeStorageSpace".to_string(),
                "ReadLatency".to_string(),
                "WriteLatency".to_string(),
            ],
            _ => vec![
                "CPUUtilization".to_string(),
                "MemoryUtilization".to_string(),
            ],
        }
    }

    fn generate_analysis_prompt(
        &self,
        req: &ServiceAnalyticsRequest,
        metrics: &[crate::services::data_collection::MetricData],
    ) -> Result<String, crate::errors::AppError> {
        let mut prompt = format!(
            "Analyze the following AWS {} resource metrics and provide insights:\n\n",
            format!("{:?}", req.resource_type).to_lowercase()
        );

        prompt.push_str(&format!("Resource ID: {}\n", req.resource_id));
        prompt.push_str(&format!(
            "Time Range: {} to {}\n\n",
            req.time_range.start_time, req.time_range.end_time
        ));

        prompt.push_str("Metrics Data:\n");
        for metric in metrics.iter().take(50) {
            // Limit to first 50 metrics to avoid token limits
            prompt.push_str(&format!(
                "- {}: {} {} at {}\n",
                metric.metric_name,
                metric.value,
                metric.unit,
                metric.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }

        if metrics.len() > 50 {
            prompt.push_str(&format!("... and {} more metrics\n", metrics.len() - 50));
        }

        prompt.push_str("\nPlease provide:\n");
        prompt.push_str("1. Summary of current performance\n");
        prompt.push_str("2. Key insights and anomalies\n");
        prompt.push_str("3. Recommendations for optimization\n");
        prompt.push_str("4. Potential issues or bottlenecks\n");

        match req.analysis_type {
            AnalysisType::Custom(ref custom_type) => {
                prompt.push_str(&format!("5. Specific analysis for: {}\n", custom_type));
            }
        }

        Ok(prompt)
    }

    fn parse_llm_response(
        &self,
        response: &str,
    ) -> Result<serde_json::Value, crate::errors::AppError> {
        // Try to parse as JSON first
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(response) {
            return Ok(json);
        }

        // If not JSON, parse as markdown/text and structure it
        let mut insights = Vec::new();
        let mut recommendations = Vec::new();

        let sections = response.split("\n## ").collect::<Vec<&str>>();

        for section in sections {
            if section.to_lowercase().contains("insight")
                || section.to_lowercase().contains("finding")
            {
                insights.push(section.to_string());
            } else if section.to_lowercase().contains("recommendation")
                || section.to_lowercase().contains("action")
            {
                recommendations.push(section.to_string());
            }
        }

        Ok(serde_json::json!({
            "insights": insights,
            "recommendations": recommendations,
            "full_response": response
        }))
    }
}
