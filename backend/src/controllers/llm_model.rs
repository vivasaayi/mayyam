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

use actix_web::{web, HttpResponse, Result as ActixResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::repositories::llm_model::LlmProviderModelRepository;
use crate::repositories::prompt_template::PromptTemplateRepository;
use crate::services::analytics::ai_llm_analytics::agent_inventory::{
    agent_control_workflow, agent_governance_workflow, agent_inventory_items_from_models,
    evaluate_agent_inventory, RESOURCE_TYPE as AGENT_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::ai_spend_report::ai_spend_report_bundle;
use crate::services::analytics::ai_llm_analytics::error_rate_inventory::{
    error_rate_inventory_item_from_model, evaluate_error_rate_inventory,
    RESOURCE_TYPE as ERROR_RATE_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::evaluation_dataset_inventory::{
    evaluate_evaluation_dataset_inventory, evaluation_dataset_inventory_item_from_model,
    RESOURCE_TYPE as EVALUATION_DATASET_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::grounding_score_inventory::{
    evaluate_grounding_score_inventory, grounding_score_inventory_item_from_model,
    RESOURCE_TYPE as GROUNDING_SCORE_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::hallucination_feedback_inventory::{
    evaluate_hallucination_feedback_inventory, hallucination_feedback_inventory_item_from_model,
    RESOURCE_TYPE as HALLUCINATION_FEEDBACK_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::latency_inventory::{
    evaluate_latency_inventory, latency_inventory_item_from_model,
    RESOURCE_TYPE as LATENCY_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::model_cost_inventory::{
    evaluate_model_cost_inventory, model_cost_inventory_item_from_model,
    RESOURCE_TYPE as MODEL_COST_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::model_inventory::{
    evaluate_model_inventory, model_inventory_item_from_model, RESOURCE_TYPE as MODEL_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::prompt_injection_inventory::{
    evaluate_prompt_injection_inventory, prompt_injection_inventory_item_from_model,
    RESOURCE_TYPE as PROMPT_INJECTION_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::response_quality_score_inventory::{
    evaluate_response_quality_score_inventory, response_quality_score_inventory_item_from_model,
    RESOURCE_TYPE as RESPONSE_QUALITY_SCORE_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::sensitive_data_leakage_inventory::{
    evaluate_sensitive_data_leakage_inventory, sensitive_data_leakage_inventory_item_from_model,
    RESOURCE_TYPE as SENSITIVE_DATA_LEAKAGE_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::token_usage_inventory::{
    evaluate_token_usage_inventory, token_usage_inventory_item_from_model,
    RESOURCE_TYPE as TOKEN_USAGE_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::tool_call_trace_inventory::{
    evaluate_tool_call_trace_inventory, tool_call_trace_inventory_item_from_model,
    RESOURCE_TYPE as TOOL_CALL_TRACE_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::tool_call_trace_replay_workflow::tool_call_trace_replay_workflow;
use crate::services::analytics::ai_llm_analytics::unsafe_tool_call_inventory::{
    evaluate_unsafe_tool_call_inventory, unsafe_tool_call_inventory_item_from_model,
    RESOURCE_TYPE as UNSAFE_TOOL_CALL_RESOURCE_TYPE,
};
use crate::services::aws::inventory::types::{Pillar, DEFAULT_STALE_AFTER_HOURS};

#[derive(Debug, Deserialize)]
pub struct CreateModelRequest {
    pub model_name: String,
    pub model_config: serde_json::Value,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateModelRequest {
    pub model_name: Option<String>,
    pub model_config: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ModelListResponse<T> {
    pub models: Vec<T>,
    pub total: usize,
}

pub struct LlmModelController {
    repo: Arc<LlmProviderModelRepository>,
    prompt_template_repo: Option<Arc<PromptTemplateRepository>>,
}

impl LlmModelController {
    pub fn new(repo: Arc<LlmProviderModelRepository>) -> Self {
        Self {
            repo,
            prompt_template_repo: None,
        }
    }

    pub fn with_prompt_template_repository(
        repo: Arc<LlmProviderModelRepository>,
        prompt_template_repo: Arc<PromptTemplateRepository>,
    ) -> Self {
        Self {
            repo,
            prompt_template_repo: Some(prompt_template_repo),
        }
    }

    pub async fn list(
        controller: web::Data<LlmModelController>,
        path: web::Path<Uuid>,
    ) -> ActixResult<HttpResponse> {
        let models = controller
            .repo
            .list_by_provider(*path)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let dtos: Vec<crate::models::llm_model::LlmProviderModelDto> =
            models.into_iter().map(Into::into).collect();
        Ok(HttpResponse::Ok().json(ModelListResponse {
            total: dtos.len(),
            models: dtos,
        }))
    }

    pub async fn create(
        controller: web::Data<LlmModelController>,
        path: web::Path<Uuid>,
        req: web::Json<CreateModelRequest>,
    ) -> ActixResult<HttpResponse> {
        let model = controller
            .repo
            .create(
                *path,
                req.model_name.clone(),
                req.model_config.clone(),
                req.enabled.unwrap_or(true),
            )
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        Ok(HttpResponse::Ok().json(crate::models::llm_model::LlmProviderModelDto::from(model)))
    }

    pub async fn update(
        controller: web::Data<LlmModelController>,
        path: web::Path<(Uuid, Uuid)>,
        req: web::Json<UpdateModelRequest>,
    ) -> ActixResult<HttpResponse> {
        let (_provider_id, model_id) = path.into_inner();
        let model = controller
            .repo
            .update(
                model_id,
                req.model_name.clone(),
                req.model_config.clone(),
                req.enabled,
            )
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        Ok(HttpResponse::Ok().json(crate::models::llm_model::LlmProviderModelDto::from(model)))
    }

    pub async fn delete(
        controller: web::Data<LlmModelController>,
        path: web::Path<(Uuid, Uuid)>,
    ) -> ActixResult<HttpResponse> {
        let (_provider_id, model_id) = path.into_inner();
        controller
            .repo
            .delete(model_id)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        Ok(HttpResponse::NoContent().finish())
    }

    pub async fn toggle(
        controller: web::Data<LlmModelController>,
        path: web::Path<(Uuid, Uuid)>,
        enabled: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let (_provider_id, model_id) = path.into_inner();
        let en = enabled.get("enabled").map(|v| v == "true").unwrap_or(true);
        let model = controller
            .repo
            .set_enabled(model_id, en)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        Ok(HttpResponse::Ok().json(crate::models::llm_model::LlmProviderModelDto::from(model)))
    }

    pub async fn model_inventory_pillar_reports(
        controller: web::Data<LlmModelController>,
        query: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let pillars = parse_model_inventory_pillars(query.get("pillar"))
            .map_err(actix_web::error::ErrorBadRequest)?;
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let items: Vec<_> = models.iter().map(model_inventory_item_from_model).collect();
        let now = chrono::Utc::now();
        let reports: Vec<_> = pillars
            .into_iter()
            .map(|pillar| evaluate_model_inventory(&items, pillar, now))
            .collect();

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": MODEL_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
            "reports": reports,
        })))
    }

    pub async fn agent_inventory_pillar_reports(
        controller: web::Data<LlmModelController>,
        query: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let pillars = parse_agent_inventory_pillars(query.get("pillar"))
            .map_err(actix_web::error::ErrorBadRequest)?;
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let prompts = match &controller.prompt_template_repo {
            Some(repo) => repo
                .find_all()
                .await
                .map_err(|e| actix_web::error::ErrorInternalServerError(e))?,
            None => Vec::new(),
        };
        let items = agent_inventory_items_from_models(&models, &prompts);
        let now = chrono::Utc::now();
        let reports: Vec<_> = pillars
            .into_iter()
            .map(|pillar| evaluate_agent_inventory(&items, pillar, now))
            .collect();
        let control_workflow = agent_control_workflow(&items, &reports);
        let governance_workflow = agent_governance_workflow(&items, &reports);

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": AGENT_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
            "control_workflow": control_workflow,
            "governance_workflow": governance_workflow,
            "reports": reports,
        })))
    }

    pub async fn latency_inventory_pillar_reports(
        controller: web::Data<LlmModelController>,
        query: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let pillars = parse_latency_inventory_pillars(query.get("pillar"))
            .map_err(actix_web::error::ErrorBadRequest)?;
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let items: Vec<_> = models
            .iter()
            .map(latency_inventory_item_from_model)
            .collect();
        let now = chrono::Utc::now();
        let reports: Vec<_> = pillars
            .into_iter()
            .map(|pillar| evaluate_latency_inventory(&items, pillar, now))
            .collect();

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": LATENCY_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
            "reports": reports,
        })))
    }

    pub async fn error_rate_inventory_pillar_reports(
        controller: web::Data<LlmModelController>,
        query: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let pillars = parse_error_rate_inventory_pillars(query.get("pillar"))
            .map_err(actix_web::error::ErrorBadRequest)?;
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let items: Vec<_> = models
            .iter()
            .map(error_rate_inventory_item_from_model)
            .collect();
        let now = chrono::Utc::now();
        let reports: Vec<_> = pillars
            .into_iter()
            .map(|pillar| evaluate_error_rate_inventory(&items, pillar, now))
            .collect();

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": ERROR_RATE_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
            "reports": reports,
        })))
    }

    pub async fn token_usage_inventory_pillar_reports(
        controller: web::Data<LlmModelController>,
        query: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let pillars = parse_token_usage_inventory_pillars(query.get("pillar"))
            .map_err(actix_web::error::ErrorBadRequest)?;
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let items: Vec<_> = models
            .iter()
            .map(token_usage_inventory_item_from_model)
            .collect();
        let now = chrono::Utc::now();
        let reports: Vec<_> = pillars
            .into_iter()
            .map(|pillar| evaluate_token_usage_inventory(&items, pillar, now))
            .collect();

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": TOKEN_USAGE_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
            "reports": reports,
        })))
    }

    pub async fn model_cost_inventory_pillar_reports(
        controller: web::Data<LlmModelController>,
        query: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let pillars = parse_model_cost_inventory_pillars(query.get("pillar"))
            .map_err(actix_web::error::ErrorBadRequest)?;
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let items: Vec<_> = models
            .iter()
            .map(model_cost_inventory_item_from_model)
            .collect();
        let now = chrono::Utc::now();
        let reports: Vec<_> = pillars
            .into_iter()
            .map(|pillar| evaluate_model_cost_inventory(&items, pillar, now))
            .collect();

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": MODEL_COST_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
            "reports": reports,
        })))
    }

    pub async fn ai_spend_report(
        controller: web::Data<LlmModelController>,
    ) -> ActixResult<HttpResponse> {
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let model_cost_items: Vec<_> = models
            .iter()
            .map(model_cost_inventory_item_from_model)
            .collect();
        let token_usage_items: Vec<_> = models
            .iter()
            .map(token_usage_inventory_item_from_model)
            .collect();
        let now = chrono::Utc::now();
        let report = ai_spend_report_bundle(&model_cost_items, &token_usage_items, now);

        Ok(HttpResponse::Ok().json(report))
    }

    pub async fn prompt_injection_inventory_pillar_reports(
        controller: web::Data<LlmModelController>,
        query: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let pillars = parse_prompt_injection_inventory_pillars(query.get("pillar"))
            .map_err(actix_web::error::ErrorBadRequest)?;
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let items: Vec<_> = models
            .iter()
            .map(prompt_injection_inventory_item_from_model)
            .collect();
        let now = chrono::Utc::now();
        let reports: Vec<_> = pillars
            .into_iter()
            .map(|pillar| evaluate_prompt_injection_inventory(&items, pillar, now))
            .collect();

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": PROMPT_INJECTION_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
            "reports": reports,
        })))
    }

    pub async fn sensitive_data_leakage_inventory_pillar_reports(
        controller: web::Data<LlmModelController>,
        query: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let pillars = parse_sensitive_data_leakage_inventory_pillars(query.get("pillar"))
            .map_err(actix_web::error::ErrorBadRequest)?;
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let items: Vec<_> = models
            .iter()
            .map(sensitive_data_leakage_inventory_item_from_model)
            .collect();
        let now = chrono::Utc::now();
        let reports: Vec<_> = pillars
            .into_iter()
            .map(|pillar| evaluate_sensitive_data_leakage_inventory(&items, pillar, now))
            .collect();

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": SENSITIVE_DATA_LEAKAGE_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
            "reports": reports,
        })))
    }

    pub async fn tool_call_trace_inventory_pillar_reports(
        controller: web::Data<LlmModelController>,
        query: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let pillars = parse_tool_call_trace_inventory_pillars(query.get("pillar"))
            .map_err(actix_web::error::ErrorBadRequest)?;
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let items: Vec<_> = models
            .iter()
            .map(tool_call_trace_inventory_item_from_model)
            .collect();
        let now = chrono::Utc::now();
        let reports: Vec<_> = pillars
            .into_iter()
            .map(|pillar| evaluate_tool_call_trace_inventory(&items, pillar, now))
            .collect();
        let replay_workflow = tool_call_trace_replay_workflow(&items, &reports);

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": TOOL_CALL_TRACE_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
            "reports": reports,
            "replay_workflow": replay_workflow,
        })))
    }

    pub async fn unsafe_tool_call_inventory_pillar_reports(
        controller: web::Data<LlmModelController>,
        query: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let pillars = parse_unsafe_tool_call_inventory_pillars(query.get("pillar"))
            .map_err(actix_web::error::ErrorBadRequest)?;
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let items: Vec<_> = models
            .iter()
            .map(unsafe_tool_call_inventory_item_from_model)
            .collect();
        let now = chrono::Utc::now();
        let reports: Vec<_> = pillars
            .into_iter()
            .map(|pillar| evaluate_unsafe_tool_call_inventory(&items, pillar, now))
            .collect();

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": UNSAFE_TOOL_CALL_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
            "reports": reports,
        })))
    }

    pub async fn evaluation_dataset_inventory_pillar_reports(
        controller: web::Data<LlmModelController>,
        query: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let pillars = parse_evaluation_dataset_inventory_pillars(query.get("pillar"))
            .map_err(actix_web::error::ErrorBadRequest)?;
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let items: Vec<_> = models
            .iter()
            .map(evaluation_dataset_inventory_item_from_model)
            .collect();
        let now = chrono::Utc::now();
        let reports: Vec<_> = pillars
            .into_iter()
            .map(|pillar| evaluate_evaluation_dataset_inventory(&items, pillar, now))
            .collect();

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": EVALUATION_DATASET_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
            "reports": reports,
        })))
    }

    pub async fn response_quality_score_inventory_pillar_reports(
        controller: web::Data<LlmModelController>,
        query: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let pillars = parse_response_quality_score_inventory_pillars(query.get("pillar"))
            .map_err(actix_web::error::ErrorBadRequest)?;
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let items: Vec<_> = models
            .iter()
            .map(response_quality_score_inventory_item_from_model)
            .collect();
        let now = chrono::Utc::now();
        let reports: Vec<_> = pillars
            .into_iter()
            .map(|pillar| evaluate_response_quality_score_inventory(&items, pillar, now))
            .collect();

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": RESPONSE_QUALITY_SCORE_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
            "reports": reports,
        })))
    }

    pub async fn grounding_score_inventory_pillar_reports(
        controller: web::Data<LlmModelController>,
        query: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let pillars = parse_grounding_score_inventory_pillars(query.get("pillar"))
            .map_err(actix_web::error::ErrorBadRequest)?;
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let items: Vec<_> = models
            .iter()
            .map(grounding_score_inventory_item_from_model)
            .collect();
        let now = chrono::Utc::now();
        let reports: Vec<_> = pillars
            .into_iter()
            .map(|pillar| evaluate_grounding_score_inventory(&items, pillar, now))
            .collect();

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": GROUNDING_SCORE_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
            "reports": reports,
        })))
    }

    pub async fn hallucination_feedback_inventory_pillar_reports(
        controller: web::Data<LlmModelController>,
        query: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let pillars = parse_hallucination_feedback_inventory_pillars(query.get("pillar"))
            .map_err(actix_web::error::ErrorBadRequest)?;
        let models = controller
            .repo
            .list_all()
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let items: Vec<_> = models
            .iter()
            .map(hallucination_feedback_inventory_item_from_model)
            .collect();
        let now = chrono::Utc::now();
        let reports: Vec<_> = pillars
            .into_iter()
            .map(|pillar| evaluate_hallucination_feedback_inventory(&items, pillar, now))
            .collect();

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": HALLUCINATION_FEEDBACK_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
            "reports": reports,
        })))
    }
}

fn parse_model_inventory_pillars(pillar: Option<&String>) -> Result<Vec<Pillar>, String> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        format!(
                            "Unsupported pillar '{}' for AI/LLM model inventory; supported pillars are cost, resilience, and security",
                            part
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(
                    "At least one pillar must be provided for AI/LLM model inventory".to_string(),
                );
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
}

fn parse_token_usage_inventory_pillars(pillar: Option<&String>) -> Result<Vec<Pillar>, String> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        format!(
                            "Unsupported pillar '{}' for AI/LLM token usage inventory; supported pillars are cost, resilience, and security",
                            part
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(
                    "At least one pillar must be provided for AI/LLM token usage inventory"
                        .to_string(),
                );
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
}

fn parse_model_cost_inventory_pillars(pillar: Option<&String>) -> Result<Vec<Pillar>, String> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        format!(
                            "Unsupported pillar '{}' for AI/LLM model cost inventory; supported pillars are cost, resilience, and security",
                            part
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(
                    "At least one pillar must be provided for AI/LLM model cost inventory"
                        .to_string(),
                );
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
}

fn parse_tool_call_trace_inventory_pillars(pillar: Option<&String>) -> Result<Vec<Pillar>, String> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        format!(
                            "Unsupported pillar '{}' for AI/LLM tool call trace inventory; supported pillars are cost, resilience, and security",
                            part
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(
                    "At least one pillar must be provided for AI/LLM tool call trace inventory"
                        .to_string(),
                );
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
}

fn parse_evaluation_dataset_inventory_pillars(
    pillar: Option<&String>,
) -> Result<Vec<Pillar>, String> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        format!(
                            "Unsupported pillar '{}' for AI/LLM evaluation dataset inventory; supported pillars are cost, resilience, and security",
                            part
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(
                    "At least one pillar must be provided for AI/LLM evaluation dataset inventory"
                        .to_string(),
                );
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
}

fn parse_response_quality_score_inventory_pillars(
    pillar: Option<&String>,
) -> Result<Vec<Pillar>, String> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        format!(
                            "Unsupported pillar '{}' for AI/LLM response quality score inventory; supported pillars are cost, resilience, and security",
                            part
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(
                    "At least one pillar must be provided for AI/LLM response quality score inventory"
                        .to_string(),
                );
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
}

fn parse_grounding_score_inventory_pillars(pillar: Option<&String>) -> Result<Vec<Pillar>, String> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        format!(
                            "Unsupported pillar '{}' for AI/LLM grounding score inventory; supported pillars are cost, resilience, and security",
                            part
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(
                    "At least one pillar must be provided for AI/LLM grounding score inventory"
                        .to_string(),
                );
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
}

fn parse_hallucination_feedback_inventory_pillars(
    pillar: Option<&String>,
) -> Result<Vec<Pillar>, String> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        format!(
                            "Unsupported pillar '{}' for AI/LLM hallucination feedback inventory; supported pillars are cost, resilience, and security",
                            part
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(
                    "At least one pillar must be provided for AI/LLM hallucination feedback inventory"
                        .to_string(),
                );
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
}

fn parse_unsafe_tool_call_inventory_pillars(
    pillar: Option<&String>,
) -> Result<Vec<Pillar>, String> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        format!(
                            "Unsupported pillar '{}' for AI/LLM unsafe tool-call inventory; supported pillars are cost, resilience, and security",
                            part
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(
                    "At least one pillar must be provided for AI/LLM unsafe tool-call inventory"
                        .to_string(),
                );
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
}

fn parse_prompt_injection_inventory_pillars(
    pillar: Option<&String>,
) -> Result<Vec<Pillar>, String> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        format!(
                            "Unsupported pillar '{}' for AI/LLM prompt injection inventory; supported pillars are cost, resilience, and security",
                            part
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(
                    "At least one pillar must be provided for AI/LLM prompt injection inventory"
                        .to_string(),
                );
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
}

fn parse_sensitive_data_leakage_inventory_pillars(
    pillar: Option<&String>,
) -> Result<Vec<Pillar>, String> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        format!(
                            "Unsupported pillar '{}' for AI/LLM sensitive-data leakage inventory; supported pillars are cost, resilience, and security",
                            part
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(
                    "At least one pillar must be provided for AI/LLM sensitive-data leakage inventory"
                        .to_string(),
                );
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
}

fn parse_error_rate_inventory_pillars(pillar: Option<&String>) -> Result<Vec<Pillar>, String> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        format!(
                            "Unsupported pillar '{}' for AI/LLM error rate inventory; supported pillars are cost, resilience, and security",
                            part
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(
                    "At least one pillar must be provided for AI/LLM error rate inventory"
                        .to_string(),
                );
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
}

fn parse_latency_inventory_pillars(pillar: Option<&String>) -> Result<Vec<Pillar>, String> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        format!(
                            "Unsupported pillar '{}' for AI/LLM latency inventory; supported pillars are cost, resilience, and security",
                            part
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(
                    "At least one pillar must be provided for AI/LLM latency inventory".to_string(),
                );
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
}

fn parse_agent_inventory_pillars(pillar: Option<&String>) -> Result<Vec<Pillar>, String> {
    match pillar {
        Some(value) => {
            let pillars = value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(|part| {
                    Pillar::parse(part).ok_or_else(|| {
                        format!(
                            "Unsupported pillar '{}' for AI/LLM agent inventory; supported pillars are cost, resilience, and security",
                            part
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            if pillars.is_empty() {
                return Err(
                    "At least one pillar must be provided for AI/LLM agent inventory".to_string(),
                );
            }

            Ok(pillars)
        }
        None => Ok(vec![Pillar::Cost, Pillar::Resilience, Pillar::Security]),
    }
}
