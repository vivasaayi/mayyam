use actix_web::{web, HttpResponse, Result};
use std::sync::Arc;
use uuid::Uuid;

use crate::controllers::llm_provider::LlmProviderController;
use crate::controllers::llm_provider::{CreateLlmProviderRequest, UpdateLlmProviderRequest, LlmProviderQueryParams};

pub fn configure(cfg: &mut web::ServiceConfig, controller: Arc<LlmProviderController>) {
    cfg.service(
        web::scope("/api/v1/llm-providers")
            .app_data(web::Data::from(controller))
            .route("", web::get().to(list_llm_providers))
            .route("", web::post().to(create_llm_provider))
            .route("/{id}", web::get().to(get_llm_provider))
            .route("/{id}", web::put().to(update_llm_provider))
            .route("/{id}", web::delete().to(delete_llm_provider))
            .route("/{id}/test", web::post().to(test_llm_provider))
            .route("/{id}/models", web::get().to(list_available_models))
            .route("/search", web::get().to(search_llm_providers))
    );
}

async fn list_llm_providers(
    controller: web::Data<LlmProviderController>,
    query: web::Query<LlmProviderQueryParams>,
) -> Result<HttpResponse> {
    LlmProviderController::list_llm_providers(controller, query).await
}

async fn create_llm_provider(
    controller: web::Data<LlmProviderController>,
    request: web::Json<CreateLlmProviderRequest>,
) -> Result<HttpResponse> {
    LlmProviderController::create_llm_provider(controller, request).await
}

async fn get_llm_provider(
    controller: web::Data<LlmProviderController>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    LlmProviderController::get_llm_provider(controller, path).await
}

async fn update_llm_provider(
    controller: web::Data<LlmProviderController>,
    path: web::Path<Uuid>,
    request: web::Json<UpdateLlmProviderRequest>,
) -> Result<HttpResponse> {
    LlmProviderController::update_llm_provider(controller, path, request).await
}

async fn delete_llm_provider(
    controller: web::Data<LlmProviderController>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    LlmProviderController::delete_llm_provider(controller, path).await
}

async fn test_llm_provider(
    controller: web::Data<LlmProviderController>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let test_request = web::Json(crate::controllers::llm_provider::TestLlmProviderRequest {
        test_prompt: Some("Hello, this is a test.".to_string()),
    });
    LlmProviderController::test_llm_provider(controller, path, test_request).await
}

// Simplified functions for endpoints that don't exist in the controller
async fn list_available_models(
    _controller: web::Data<LlmProviderController>,
    _path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    // Placeholder implementation
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "models": [],
        "message": "Model listing not implemented yet"
    })))
}

async fn search_llm_providers(
    controller: web::Data<LlmProviderController>,
    query: web::Query<LlmProviderQueryParams>,
) -> Result<HttpResponse> {
    // Use the list function for now as search functionality
    LlmProviderController::list_llm_providers(controller, query).await
}
