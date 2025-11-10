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
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::models::llm_provider::{
    LlmPromptFormat, LlmProviderResponseDto, LlmProviderStatus, LlmProviderType,
};
use crate::services::llm_provider::{
    CreateLlmProviderInput, ListLlmProvidersFilter, LlmProviderService, UpdateLlmProviderInput,
};

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
    service: Arc<LlmProviderService>,
}

impl LlmProviderController {
    pub fn new(service: Arc<LlmProviderService>) -> Self {
        Self { service }
    }

    pub async fn create_llm_provider(
        controller: web::Data<LlmProviderController>,
        request: web::Json<CreateLlmProviderRequest>,
    ) -> ActixResult<HttpResponse> {
        let provider = controller
            .service
            .create_provider(CreateLlmProviderInput {
                name: request.name.clone(),
                provider_type: request.provider_type.clone(),
                model_name: request.model_name.clone(),
                api_endpoint: request.api_endpoint.clone(),
                api_key: request.api_key.clone(),
                model_config: request.model_config.clone(),
                prompt_format: request.prompt_format.clone(),
                description: request.description.clone(),
                enabled: request.enabled,
                is_default: request.is_default,
            })
            .await
            .map_err(actix_web::Error::from)?;

        Ok(HttpResponse::Ok().json(provider))
    }

    pub async fn get_llm_provider(
        controller: web::Data<LlmProviderController>,
        path: web::Path<Uuid>,
    ) -> ActixResult<HttpResponse> {
        let provider = controller
            .service
            .get_provider(*path)
            .await
            .map_err(actix_web::Error::from)?;

        Ok(HttpResponse::Ok().json(provider))
    }

    pub async fn list_llm_providers(
        controller: web::Data<LlmProviderController>,
        params: web::Query<LlmProviderQueryParams>,
    ) -> ActixResult<HttpResponse> {
        let providers = controller
            .service
            .list_providers(ListLlmProvidersFilter {
                provider_type: params.provider_type.clone(),
                status: params.status.clone(),
                active_only: params.active_only.unwrap_or(false),
            })
            .await
            .map_err(actix_web::Error::from)?;

        Ok(HttpResponse::Ok().json(LlmProviderListResponse {
            total: providers.len(),
            providers,
        }))
    }

    pub async fn update_llm_provider(
        controller: web::Data<LlmProviderController>,
        path: web::Path<Uuid>,
        request: web::Json<UpdateLlmProviderRequest>,
    ) -> ActixResult<HttpResponse> {
        let provider = controller
            .service
            .update_provider(
                *path,
                UpdateLlmProviderInput {
                    name: request.name.clone(),
                    model_name: request.model_name.clone(),
                    api_endpoint: request.api_endpoint.clone(),
                    api_key: request.api_key.clone(),
                    model_config: request.model_config.clone(),
                    prompt_format: request.prompt_format.clone(),
                    description: request.description.clone(),
                    status: request.status.clone(),
                    enabled: request.enabled,
                    is_default: request.is_default,
                },
            )
            .await
            .map_err(actix_web::Error::from)?;

        Ok(HttpResponse::Ok().json(provider))
    }

    pub async fn delete_llm_provider(
        controller: web::Data<LlmProviderController>,
        path: web::Path<Uuid>,
    ) -> ActixResult<HttpResponse> {
        controller
            .service
            .delete_provider(*path)
            .await
            .map_err(actix_web::Error::from)?;
        Ok(HttpResponse::NoContent().finish())
    }

    pub async fn test_llm_provider(
        controller: web::Data<LlmProviderController>,
        path: web::Path<Uuid>,
        request: web::Json<TestLlmProviderRequest>,
    ) -> ActixResult<HttpResponse> {
        let success = controller
            .service
            .test_provider(*path)
            .await
            .map_err(actix_web::Error::from)?;

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
        Ok(HttpResponse::Ok().json(LlmProviderService::list_provider_types()))
    }

    pub async fn get_prompt_formats(
        _controller: web::Data<LlmProviderController>,
    ) -> ActixResult<HttpResponse> {
        Ok(HttpResponse::Ok().json(LlmProviderService::list_prompt_formats()))
    }
}
