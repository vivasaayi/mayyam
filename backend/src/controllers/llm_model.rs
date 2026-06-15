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
    agent_inventory_items_from_models, evaluate_agent_inventory,
    RESOURCE_TYPE as AGENT_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::error_rate_inventory::{
    error_rate_inventory_item_from_model, evaluate_error_rate_inventory,
    RESOURCE_TYPE as ERROR_RATE_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::evaluation_dataset_inventory::{
    evaluate_evaluation_dataset_inventory, evaluation_dataset_inventory_item_from_model,
    RESOURCE_TYPE as EVALUATION_DATASET_RESOURCE_TYPE,
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
use crate::services::analytics::ai_llm_analytics::token_usage_inventory::{
    evaluate_token_usage_inventory, token_usage_inventory_item_from_model,
    RESOURCE_TYPE as TOKEN_USAGE_RESOURCE_TYPE,
};
use crate::services::analytics::ai_llm_analytics::tool_call_trace_inventory::{
    evaluate_tool_call_trace_inventory, tool_call_trace_inventory_item_from_model,
    RESOURCE_TYPE as TOOL_CALL_TRACE_RESOURCE_TYPE,
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

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": AGENT_RESOURCE_TYPE,
            "evaluated_at": now,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
            "resources_evaluated": items.len(),
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

        Ok(HttpResponse::Ok().json(serde_json::json!({
            "resource_type": TOOL_CALL_TRACE_RESOURCE_TYPE,
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
