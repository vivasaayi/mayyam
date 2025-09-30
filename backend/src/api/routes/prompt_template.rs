use actix_web::{web, HttpResponse, Result};
use std::sync::Arc;
use uuid::Uuid;

use crate::controllers::prompt_template::{
    CreatePromptTemplateRequest, PromptTemplateController, PromptTemplateQueryParams,
    UpdatePromptTemplateRequest,
};

pub fn configure(cfg: &mut web::ServiceConfig, controller: Arc<PromptTemplateController>) {
    cfg.service(
        web::scope("/api/v1/prompt-templates")
            .app_data(web::Data::new(controller))
            .route("", web::get().to(list_prompt_templates))
            .route("", web::post().to(create_prompt_template))
            .route("/{id}", web::get().to(get_prompt_template))
            .route("/{id}", web::put().to(update_prompt_template))
            .route("/{id}", web::delete().to(delete_prompt_template))
            .route("/categories", web::get().to(get_categories))
            .route("/types", web::get().to(get_prompt_types)),
    );
}

async fn list_prompt_templates(
    controller: web::Data<PromptTemplateController>,
    query: web::Query<PromptTemplateQueryParams>,
) -> Result<HttpResponse> {
    PromptTemplateController::list_prompt_templates(controller, query).await
}

async fn create_prompt_template(
    controller: web::Data<PromptTemplateController>,
    request: web::Json<CreatePromptTemplateRequest>,
) -> Result<HttpResponse> {
    PromptTemplateController::create_prompt_template(controller, request).await
}

async fn get_prompt_template(
    controller: web::Data<PromptTemplateController>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    PromptTemplateController::get_prompt_template(controller, path).await
}

async fn update_prompt_template(
    controller: web::Data<PromptTemplateController>,
    path: web::Path<Uuid>,
    request: web::Json<UpdatePromptTemplateRequest>,
) -> Result<HttpResponse> {
    PromptTemplateController::update_prompt_template(controller, path, request).await
}

async fn delete_prompt_template(
    controller: web::Data<PromptTemplateController>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    PromptTemplateController::delete_prompt_template(controller, path).await
}

async fn get_categories(controller: web::Data<PromptTemplateController>) -> Result<HttpResponse> {
    PromptTemplateController::get_categories(controller).await
}

async fn get_prompt_types(controller: web::Data<PromptTemplateController>) -> Result<HttpResponse> {
    PromptTemplateController::get_prompt_types(controller).await
}
