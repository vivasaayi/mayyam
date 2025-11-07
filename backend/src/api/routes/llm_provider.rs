use actix_web::{web, HttpResponse, Result};
use std::sync::Arc;
use uuid::Uuid;

use crate::controllers::llm_model::LlmModelController;
use crate::controllers::llm_provider::LlmProviderController;
use crate::controllers::llm_provider::{
    CreateLlmProviderRequest, LlmProviderQueryParams, UpdateLlmProviderRequest,
};

pub fn configure(
    cfg: &mut web::ServiceConfig,
    controller: Arc<LlmProviderController>,
    model_controller: Arc<LlmModelController>,
) {
    cfg.service(
        web::scope("/api/v1/llm-providers")
            .app_data(web::Data::from(controller))
            .app_data(web::Data::from(model_controller))
            .route("", web::get().to(list_llm_providers))
            .route("", web::post().to(create_llm_provider))
            .route("/{id}", web::get().to(get_llm_provider))
            .route("/{id}", web::put().to(update_llm_provider))
            .route("/{id}", web::delete().to(delete_llm_provider))
            .route("/{id}/test", web::post().to(test_llm_provider))
            .service(
                web::scope("/{id}/models")
                    .route("", web::get().to(list_models))
                    .route("", web::post().to(create_model))
                    .route("/{model_id}", web::put().to(update_model))
                    .route("/{model_id}", web::delete().to(delete_model))
                    .route("/{model_id}/toggle", web::post().to(toggle_model)),
            )
            .route("/search", web::get().to(search_llm_providers)),
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

// LLM Provider Model routes delegations
async fn list_models(
    model_controller: web::Data<LlmModelController>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    LlmModelController::list(model_controller, path).await
}

async fn create_model(
    model_controller: web::Data<LlmModelController>,
    path: web::Path<Uuid>,
    req: web::Json<crate::controllers::llm_model::CreateModelRequest>,
) -> Result<HttpResponse> {
    LlmModelController::create(model_controller, path, req).await
}

async fn update_model(
    model_controller: web::Data<LlmModelController>,
    path: web::Path<(Uuid, Uuid)>,
    req: web::Json<crate::controllers::llm_model::UpdateModelRequest>,
) -> Result<HttpResponse> {
    LlmModelController::update(model_controller, path, req).await
}

async fn delete_model(
    model_controller: web::Data<LlmModelController>,
    path: web::Path<(Uuid, Uuid)>,
) -> Result<HttpResponse> {
    LlmModelController::delete(model_controller, path).await
}

async fn toggle_model(
    model_controller: web::Data<LlmModelController>,
    path: web::Path<(Uuid, Uuid)>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse> {
    LlmModelController::toggle(model_controller, path, query).await
}

async fn search_llm_providers(
    controller: web::Data<LlmProviderController>,
    query: web::Query<LlmProviderQueryParams>,
) -> Result<HttpResponse> {
    // Use the list function for now as search functionality
    LlmProviderController::list_llm_providers(controller, query).await
}
