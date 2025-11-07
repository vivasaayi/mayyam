use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

use crate::errors::AppError;
use crate::services::llm::{
    LlmGenerationRequest, LlmRequestBuilder, UnifiedLlmManager, UnifiedLlmRequest,
};

#[derive(Debug, Deserialize)]
pub struct SimpleGenerationRequest {
    pub provider: String,
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub enable_thinking: Option<bool>,
    pub format_response: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SmartGenerationRequest {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub enable_thinking: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct GenerationResponse {
    pub content: String,
    pub thinking: Option<String>,
    pub provider: String,
    pub model: String,
    pub usage: crate::services::llm::interface::TokenUsage,
    pub metadata: crate::services::llm::interface::ResponseMetadata,
    pub formatted: Option<crate::services::llm::formatting::FormattedResponse>,
}

pub struct UnifiedLlmController {
    llm_manager: Arc<UnifiedLlmManager>,
}

impl UnifiedLlmController {
    pub fn new(llm_manager: Arc<UnifiedLlmManager>) -> Self {
        Self { llm_manager }
    }

    /// Generate text with specified provider
    pub async fn generate(
        &self,
        request: web::Json<SimpleGenerationRequest>,
    ) -> Result<HttpResponse> {
        info!("Received LLM generation request: {:?}", request);

        let mut llm_request = LlmRequestBuilder::new()
            .prompt(&request.prompt)
            .temperature(request.temperature.unwrap_or(0.7))
            .max_tokens(request.max_tokens.unwrap_or(1000))
            .enable_thinking(request.enable_thinking.unwrap_or(false))
            .build();

        if let Some(system_prompt) = &request.system_prompt {
            llm_request.system_prompt = Some(system_prompt.clone());
        }

        let generation_request = LlmGenerationRequest {
            provider: request.provider.clone(),
            model: None,
            request: llm_request,
            format_response: request.format_response,
            formatting_options: None,
        };

        match self.llm_manager.generate(generation_request).await {
            Ok(response) => {
                info!("LLM generation completed successfully");

                let api_response = GenerationResponse {
                    content: response.response.content,
                    thinking: response.response.thinking,
                    provider: response.provider_info.name,
                    model: response.provider_info.model,
                    usage: response.response.usage,
                    metadata: response.response.metadata,
                    formatted: response.formatted,
                };

                Ok(HttpResponse::Ok().json(api_response))
            }
            Err(e) => {
                error!("Failed to generate LLM response: {:?}", e);
                Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Failed to generate response",
                    "details": e.to_string()
                })))
            }
        }
    }

    /// Smart generation - automatically selects best provider
    pub async fn generate_smart(
        &self,
        request: web::Json<SmartGenerationRequest>,
    ) -> Result<HttpResponse> {
        info!("Received smart LLM generation request: {:?}", request);

        let mut llm_request = LlmRequestBuilder::new()
            .prompt(&request.prompt)
            .temperature(request.temperature.unwrap_or(0.7))
            .max_tokens(request.max_tokens.unwrap_or(1000))
            .enable_thinking(request.enable_thinking.unwrap_or(false))
            .build();

        if let Some(system_prompt) = &request.system_prompt {
            llm_request.system_prompt = Some(system_prompt.clone());
        }

        match self.llm_manager.generate_smart(llm_request).await {
            Ok(response) => {
                info!("Smart LLM generation completed successfully");

                let api_response = GenerationResponse {
                    content: response.response.content,
                    thinking: response.response.thinking,
                    provider: response.provider_info.name,
                    model: response.provider_info.model,
                    usage: response.response.usage,
                    metadata: response.response.metadata,
                    formatted: response.formatted,
                };

                Ok(HttpResponse::Ok().json(api_response))
            }
            Err(e) => {
                error!("Failed to generate smart LLM response: {:?}", e);
                Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Failed to generate response",
                    "details": e.to_string()
                })))
            }
        }
    }

    /// List available providers
    pub async fn list_providers(&self) -> Result<HttpResponse> {
        let providers = self.llm_manager.list_providers();
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "providers": providers
        })))
    }

    /// Get provider capabilities
    pub async fn get_provider_capabilities(&self, path: web::Path<String>) -> Result<HttpResponse> {
        let provider_name = path.into_inner();

        match self.llm_manager.get_provider_capabilities(&provider_name) {
            Ok(capabilities) => Ok(HttpResponse::Ok().json(capabilities)),
            Err(e) => Ok(HttpResponse::NotFound().json(serde_json::json!({
                "error": "Provider not found",
                "details": e.to_string()
            }))),
        }
    }

    /// Estimate costs for a request across all providers
    pub async fn estimate_costs(
        &self,
        request: web::Json<SmartGenerationRequest>,
    ) -> Result<HttpResponse> {
        let llm_request = LlmRequestBuilder::new()
            .prompt(&request.prompt)
            .temperature(request.temperature.unwrap_or(0.7))
            .max_tokens(request.max_tokens.unwrap_or(1000))
            .build();

        match self.llm_manager.estimate_costs(&llm_request).await {
            Ok(costs) => Ok(HttpResponse::Ok().json(serde_json::json!({
                "estimated_costs": costs
            }))),
            Err(e) => {
                error!("Failed to estimate costs: {:?}", e);
                Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Failed to estimate costs",
                    "details": e.to_string()
                })))
            }
        }
    }

    /// Quick generation for simple use cases
    pub async fn quick_generate(
        &self,
        request: web::Json<serde_json::Value>,
    ) -> Result<HttpResponse> {
        let provider = request
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("openai");

        let prompt = request
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::BadRequest("Prompt is required".to_string()))
            .map_err(|e| actix_web::error::ErrorBadRequest(e.to_string()))?;

        match self.llm_manager.quick_generate(provider, prompt).await {
            Ok(content) => Ok(HttpResponse::Ok().json(serde_json::json!({
                "content": content
            }))),
            Err(e) => {
                error!("Failed to generate quick response: {:?}", e);
                Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Failed to generate response",
                    "details": e.to_string()
                })))
            }
        }
    }
}
