use std::sync::Arc;
use actix_web::{web, HttpResponse, Responder};
use sea_orm::DatabaseConnection;

use crate::config::Config;
use crate::errors::AppError;
use crate::models::query_template::{
    CreateQueryTemplateRequest, UpdateQueryTemplateRequest
};
use crate::repositories::query_template::QueryTemplateRepository;
use crate::middleware::auth::Claims;

pub async fn list_templates(
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let template_repo = QueryTemplateRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    let templates = template_repo.find_all().await?;
    
    Ok(HttpResponse::Ok().json(templates))
}

pub async fn list_templates_by_type(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let connection_type = path.into_inner();
    let template_repo = QueryTemplateRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    
    // Get the templates for this connection type
    let templates = template_repo.find_by_connection_type(&connection_type).await?;
    
    Ok(HttpResponse::Ok().json(templates))
}

pub async fn get_template(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let template_repo = QueryTemplateRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    
    let template_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    
    let template = template_repo.find_by_id(template_id).await?
        .ok_or_else(|| AppError::NotFound("Query template not found".to_string()))?;
    
    Ok(HttpResponse::Ok().json(template))
}

pub async fn create_template(
    template: web::Json<CreateQueryTemplateRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let template_repo = QueryTemplateRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    
    // Create the template
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID in token: {}", e)))?;
    
    let new_template = template_repo.create(&template, user_id).await?;
    
    Ok(HttpResponse::Created().json(new_template))
}

pub async fn update_template(
    path: web::Path<String>,
    template: web::Json<UpdateQueryTemplateRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let template_repo = QueryTemplateRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    
    // Update the template
    let template_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    
    let updated_template = template_repo.update(template_id, &template).await?;
    
    Ok(HttpResponse::Ok().json(updated_template))
}

pub async fn delete_template(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let template_repo = QueryTemplateRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    
    // Delete the template
    let template_id = uuid::Uuid::parse_str(&path.into_inner())
        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    
    template_repo.delete(template_id).await?;
    
    Ok(HttpResponse::NoContent().finish())
}
