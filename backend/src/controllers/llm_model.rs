use actix_web::{web, HttpResponse, Result as ActixResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::repositories::llm_model::LlmProviderModelRepository;

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
}

impl LlmModelController {
    pub fn new(repo: Arc<LlmProviderModelRepository>) -> Self { Self { repo } }

    pub async fn list(
        controller: web::Data<LlmModelController>,
        path: web::Path<Uuid>,
    ) -> ActixResult<HttpResponse> {
        let models = controller.repo.list_by_provider(*path).await.map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        let dtos: Vec<crate::models::llm_model::LlmProviderModelDto> = models.into_iter().map(Into::into).collect();
        Ok(HttpResponse::Ok().json(ModelListResponse { total: dtos.len(), models: dtos }))
    }

    pub async fn create(
        controller: web::Data<LlmModelController>,
        path: web::Path<Uuid>,
        req: web::Json<CreateModelRequest>,
    ) -> ActixResult<HttpResponse> {
        let model = controller.repo.create(*path, req.model_name.clone(), req.model_config.clone(), req.enabled.unwrap_or(true)).await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        Ok(HttpResponse::Ok().json(crate::models::llm_model::LlmProviderModelDto::from(model)))
    }

    pub async fn update(
        controller: web::Data<LlmModelController>,
        path: web::Path<(Uuid, Uuid)>,
        req: web::Json<UpdateModelRequest>,
    ) -> ActixResult<HttpResponse> {
        let (_provider_id, model_id) = path.into_inner();
        let model = controller.repo.update(model_id, req.model_name.clone(), req.model_config.clone(), req.enabled).await
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        Ok(HttpResponse::Ok().json(crate::models::llm_model::LlmProviderModelDto::from(model)))
    }

    pub async fn delete(
        controller: web::Data<LlmModelController>,
        path: web::Path<(Uuid, Uuid)>,
    ) -> ActixResult<HttpResponse> {
        let (_provider_id, model_id) = path.into_inner();
        controller.repo.delete(model_id).await.map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        Ok(HttpResponse::NoContent().finish())
    }

    pub async fn toggle(
        controller: web::Data<LlmModelController>,
        path: web::Path<(Uuid, Uuid)>,
        enabled: web::Query<std::collections::HashMap<String, String>>,
    ) -> ActixResult<HttpResponse> {
        let (_provider_id, model_id) = path.into_inner();
        let en = enabled.get("enabled").map(|v| v == "true").unwrap_or(true);
        let model = controller.repo.set_enabled(model_id, en).await.map_err(|e| actix_web::error::ErrorInternalServerError(e))?;
        Ok(HttpResponse::Ok().json(crate::models::llm_model::LlmProviderModelDto::from(model)))
    }
}
