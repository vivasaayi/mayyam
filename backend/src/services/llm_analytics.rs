use crate::models::{Insight, InsightSeverity, Recommendation, RecommendationPriority};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use serde_json::json;
use std::sync::Arc;
use crate::services::llm_integration::LlmIntegrationService;
use crate::services::data_collection::DataCollectionService;
use crate::repositories::data_source::DataSourceRepository;
use crate::repositories::llm_provider::LlmProviderRepository;
use crate::repositories::prompt_template::PromptTemplateRepository;

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
    pub llm_integration_service: Arc<LlmIntegrationService>,
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
        llm_integration_service: Arc<LlmIntegrationService>,
        data_collection_service: Arc<DataCollectionService>,
        data_source_repo: Arc<DataSourceRepository>,
        llm_provider_repo: Arc<LlmProviderRepository>,
        prompt_template_repo: Arc<PromptTemplateRepository>,
    ) -> Self {
        Self {
            llm_integration_service,
            data_collection_service,
            data_source_repo,
            llm_provider_repo,
            prompt_template_repo,
        }
    }

    pub fn process_section(&self, section: &str, content: &[&str], insights: &mut Vec<Insight>, recommendations: &mut Vec<Recommendation>) {
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
            },
            "recommendations" => {
                recommendations.push(Recommendation {
                    title: "Recommended Action".to_string(),
                    description: text.clone(),
                    priority: RecommendationPriority::Medium,
                    impact: "Moderate".to_string(),
                    action_items: vec![text],
                    estimated_effort: None,
                });
            },
            _ => {}
        }
    }

    pub async fn analyze(&self, _req: ServiceAnalyticsRequest) -> Result<serde_json::Value, crate::errors::AppError> {
        // Stub implementation
        Ok(serde_json::json!({"result": "stub"}))
    }
}