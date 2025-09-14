use std::sync::Arc;
use actix_web::{web, HttpResponse, Result};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use tracing::{info, error};
use crate::services::llm::{AnalysisType, TimeRange};
use chrono::Utc;

use crate::services::llm::{LlmAnalyticsService, ServiceAnalyticsRequest};
use crate::models::data_source::{ResourceType, SourceType};

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyticsRequest {
    pub data_source_id: Uuid,
    pub analysis_type: String,
    pub prompt_template_id: Option<Uuid>,
    pub llm_provider_id: Option<Uuid>,
    pub custom_prompt: Option<String>,
    pub variables: Option<serde_json::Value>,
    pub time_range_hours: Option<i32>,
}

impl From<AnalyticsRequest> for ServiceAnalyticsRequest {
    fn from(req: AnalyticsRequest) -> Self {
        ServiceAnalyticsRequest {
            resource_id: String::new(), // Not present in controller DTO
            resource_type: ResourceType::RDS, // Placeholder, should be mapped properly
            data_source_ids: vec![req.data_source_id],
            analysis_type: AnalysisType::Custom(req.analysis_type),
            time_range: TimeRange {
                start_time: Utc::now() - chrono::Duration::hours(req.time_range_hours.unwrap_or(24) as i64),
                end_time: Utc::now(),
            },
            custom_prompts: req.custom_prompt.map(|c| vec![c]),
            llm_provider_id: req.llm_provider_id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceAnalyticsRequest {
    pub resource_type: ResourceType,
    pub source_type: SourceType,
    pub resource_id: String,
    pub analysis_type: String,
    pub llm_provider_id: Option<Uuid>,
    pub time_range_hours: Option<i32>,
}

impl From<ResourceAnalyticsRequest> for ServiceAnalyticsRequest {
    fn from(req: ResourceAnalyticsRequest) -> Self {
        ServiceAnalyticsRequest {
            resource_id: req.resource_id,
            resource_type: req.resource_type,
            data_source_ids: vec![], // Not present in controller DTO
            analysis_type: AnalysisType::Custom(req.analysis_type),
            time_range: TimeRange {
                start_time: Utc::now() - chrono::Duration::hours(req.time_range_hours.unwrap_or(24) as i64),
                end_time: Utc::now(),
            },
            custom_prompts: None,
            llm_provider_id: req.llm_provider_id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchAnalyticsRequest {
    pub data_source_ids: Vec<Uuid>,
    pub analysis_type: String,
    pub llm_provider_id: Option<Uuid>,
    pub parallel_execution: Option<bool>,
}

impl BatchAnalyticsRequest {
    pub fn to_service_requests(&self) -> Vec<ServiceAnalyticsRequest> {
        self.data_source_ids.iter().map(|ds_id| ServiceAnalyticsRequest {
            resource_id: String::new(),
            resource_type: ResourceType::RDS, // Placeholder
            data_source_ids: vec![*ds_id],
            analysis_type: AnalysisType::Custom(self.analysis_type.clone()),
            time_range: TimeRange {
                start_time: Utc::now() - chrono::Duration::hours(24),
                end_time: Utc::now(),
            },
            custom_prompts: None,
            llm_provider_id: self.llm_provider_id,
        }).collect()
    }
}

pub struct LlmAnalyticsController {
    llm_analytics_service: Arc<LlmAnalyticsService>,
}

impl LlmAnalyticsController {
    pub fn new(llm_analytics_service: Arc<LlmAnalyticsService>) -> Self {
        Self {
            llm_analytics_service,
        }
    }

    /// Execute LLM-powered analytics on a specific data source
    pub async fn execute_analytics(
        &self,
        request: web::Json<AnalyticsRequest>,
    ) -> Result<HttpResponse> {
        info!("Received analytics request: {:?}", request);
        let service_request = ServiceAnalyticsRequest::from(request.into_inner());
        match self.llm_analytics_service.analyze(service_request).await {
            Ok(response) => {
                info!("Analytics completed successfully");
                Ok(HttpResponse::Ok().json(response))
            }
            Err(e) => {
                error!("Failed to execute analytics: {:?}", e);
                Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Failed to execute analytics",
                    "details": e.to_string()
                })))
            }
        }
    }

    /// Execute analytics for a specific resource type and source
    pub async fn analyze_resource(
        &self,
        request: web::Json<ResourceAnalyticsRequest>,
    ) -> Result<HttpResponse> {
        info!("Received resource analytics request: {:?}", request);
        let service_request = ServiceAnalyticsRequest::from(request.into_inner());
        match self.llm_analytics_service.analyze(service_request).await {
            Ok(response) => {
                info!("Resource analysis completed successfully");
                Ok(HttpResponse::Ok().json(response))
            }
            Err(e) => {
                error!("Failed to analyze resource: {:?}", e);
                Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Failed to analyze resource",
                    "details": e.to_string()
                })))
            }
        }
    }

    /// Execute batch analytics across multiple data sources
    pub async fn execute_batch_analytics(
        &self,
        request: web::Json<BatchAnalyticsRequest>,
    ) -> Result<HttpResponse> {
        info!("Received batch analytics request: {:?}", request);
        let service_requests = request.to_service_requests();
        let mut results = Vec::new();
        let mut failed_analyses = Vec::new();
        let batch_id = Uuid::new_v4();
        let start = std::time::Instant::now();
        for req in service_requests {
            match self.llm_analytics_service.analyze(req).await {
                Ok(res) => results.push(res),
                Err(e) => failed_analyses.push(e.to_string()),
            }
        }
        let total_execution_time_ms = start.elapsed().as_millis() as u64;
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "batch_id": batch_id,
            "results": results,
            "failed_analyses": failed_analyses,
            "total_execution_time_ms": total_execution_time_ms
        })))
    }

    /// Get analytics execution history (stub)
    pub async fn get_analytics_history(
        &self,
        _query: web::Query<serde_json::Value>,
    ) -> Result<HttpResponse> {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "history": []
        })))
    }

    /// Get available analysis types for a resource type (stub)
    pub async fn get_analysis_types(
        &self,
        _path: web::Path<String>,
    ) -> Result<HttpResponse> {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "types": []
        })))
    }

    /// Get analytics performance metrics (stub)
    pub async fn get_analytics_metrics(&self) -> Result<HttpResponse> {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "metrics": []
        })))
    }

    /// Cancel a running analytics job (stub)
    pub async fn cancel_analytics(
        &self,
        _path: web::Path<Uuid>,
    ) -> Result<HttpResponse> {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": "Analytics job cancelled successfully"
        })))
    }
}
