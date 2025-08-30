use actix_web::{web, HttpResponse, Result as ActixResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::repositories::llm_provider::LlmProviderRepository;
use crate::models::llm_provider::{LlmProviderType, LlmProviderStatus, LlmPromptFormat, LlmProviderResponseDto};

#[derive(Debug, Deserialize)]
pub struct CreateLlmProviderRequest {
    pub name: String,
    pub provider_type: LlmProviderType,
    pub model_name: String,
    pub api_endpoint: Option<String>,
    pub api_key: Option<String>,
    pub model_config: Option<Value>,
    pub prompt_format: LlmPromptFormat,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLlmProviderRequest {
    pub name: Option<String>,
    pub model_name: Option<String>,
    pub api_endpoint: Option<Option<String>>,
    pub api_key: Option<Option<String>>,
    pub model_config: Option<Option<Value>>,
    pub prompt_format: Option<LlmPromptFormat>,
    pub description: Option<Option<String>>,
    pub status: Option<LlmProviderStatus>,
    pub enabled: Option<bool>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct LlmProviderQueryParams {
    pub provider_type: Option<LlmProviderType>,
    pub status: Option<LlmProviderStatus>,
    pub active_only: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct LlmProviderListResponse {
    pub providers: Vec<LlmProviderResponseDto>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct TestLlmProviderResponse {
    pub success: bool,
    pub message: String,
    pub model_info: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct TestLlmProviderRequest {
    pub test_prompt: Option<String>,
}

pub struct LlmProviderController {
    llm_provider_repo: Arc<LlmProviderRepository>,
}

impl LlmProviderController {
    pub fn new(llm_provider_repo: Arc<LlmProviderRepository>) -> Self {
        Self { llm_provider_repo }
    }

    pub async fn create_llm_provider(
        controller: web::Data<LlmProviderController>,
        request: web::Json<CreateLlmProviderRequest>,
    ) -> ActixResult<HttpResponse> {
        let provider = controller
            .llm_provider_repo
            .create(
                request.name.clone(),
                request.provider_type.clone(),
                request.model_name.clone(),
                request.api_endpoint.clone(),
                request.api_key.clone(),
                request.model_config.clone(),
                request.prompt_format.clone(),
                request.description.clone(),
                request.enabled,
                request.is_default,
            )
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

        Ok(HttpResponse::Ok().json(LlmProviderResponseDto::from(provider)))
    }

    pub async fn get_llm_provider(
        controller: web::Data<LlmProviderController>,
        path: web::Path<Uuid>,
    ) -> ActixResult<HttpResponse> {
        let provider = controller
            .llm_provider_repo
            .find_by_id(*path)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?
            .ok_or_else(|| actix_web::error::ErrorNotFound("LLM provider not found"))?;

        Ok(HttpResponse::Ok().json(LlmProviderResponseDto::from(provider)))
    }

    pub async fn list_llm_providers(
        controller: web::Data<LlmProviderController>,
        params: web::Query<LlmProviderQueryParams>,
    ) -> ActixResult<HttpResponse> {
        let providers = if params.active_only.unwrap_or(false) {
            controller.llm_provider_repo.find_active().await.map_err(|e| actix_web::error::ErrorInternalServerError(e))?
        } else if let Some(provider_type) = &params.provider_type {
            controller
                .llm_provider_repo
                .find_by_provider_type(provider_type.clone())
                .await.map_err(|e| actix_web::error::ErrorInternalServerError(e))?
        } else {
            controller.llm_provider_repo.find_all().await.map_err(|e| actix_web::error::ErrorInternalServerError(e))?
        };

        let response_dtos: Vec<LlmProviderResponseDto> = providers
            .into_iter()
            .map(LlmProviderResponseDto::from)
            .collect();

        Ok(HttpResponse::Ok().json(LlmProviderListResponse {
            total: response_dtos.len(),
            providers: response_dtos,
        }))
    }

    pub async fn update_llm_provider(
        controller: web::Data<LlmProviderController>,
        path: web::Path<Uuid>,
        request: web::Json<UpdateLlmProviderRequest>,
    ) -> ActixResult<HttpResponse> {
        let provider = controller
            .llm_provider_repo
            .update(
                *path,
                request.name.clone(),
                request.model_name.clone(),
                request.api_endpoint.clone(),
                request.api_key.clone(),
                request.model_config.clone(),
                request.prompt_format.clone(),
                request.description.clone(),
                request.status.clone(),
                request.enabled,
                request.is_default,
            )
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

        Ok(HttpResponse::Ok().json(LlmProviderResponseDto::from(provider)))
    }

    pub async fn delete_llm_provider(
        controller: web::Data<LlmProviderController>,
        path: web::Path<Uuid>,
    ) -> ActixResult<HttpResponse> {
        controller.llm_provider_repo.delete(*path).await.map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        Ok(HttpResponse::NoContent().finish())
    }

    pub async fn test_llm_provider(
        controller: web::Data<LlmProviderController>,
        path: web::Path<Uuid>,
        request: web::Json<TestLlmProviderRequest>,
    ) -> ActixResult<HttpResponse> {
        let success = controller
            .llm_provider_repo
            .test_connection(*path)
            .await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

        let message = if success {
            "LLM provider connection successful".to_string()
        } else {
            "LLM provider connection failed".to_string()
        };

        // In a real implementation, you might want to make an actual test call
        // to the LLM provider with the test prompt
        let model_info = if success {
            Some(serde_json::json!({
                "status": "available",
                "test_prompt": request.test_prompt.clone().unwrap_or("Hello, world!".to_string())
            }))
        } else {
            None
        };

        Ok(HttpResponse::Ok().json(TestLlmProviderResponse {
            success,
            message,
            model_info,
        }))
    }

    pub async fn get_provider_types(
        _controller: web::Data<LlmProviderController>,
    ) -> ActixResult<HttpResponse> {
        let provider_types = vec![
            "OpenAI".to_string(),
            "Ollama".to_string(),
            "Anthropic".to_string(),
            "Local".to_string(),
            "Gemini".to_string(),
            "Custom".to_string(),
        ];

        Ok(HttpResponse::Ok().json(provider_types))
    }

    pub async fn get_prompt_formats(
        _controller: web::Data<LlmProviderController>,
    ) -> ActixResult<HttpResponse> {
        let prompt_formats = vec![
            "OpenAI".to_string(),
            "Anthropic".to_string(),
            "Custom".to_string(),
        ];

        Ok(HttpResponse::Ok().json(prompt_formats))
    }
}
