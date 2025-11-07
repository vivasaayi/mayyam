use actix_web::{web, HttpResponse, Responder};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::config::Config;
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::query_template::{CreateQueryTemplateRequest, UpdateQueryTemplateRequest};
use crate::repositories::query_template::QueryTemplateRepository;

pub async fn list_templates(
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let template_repo =
        QueryTemplateRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    let templates = template_repo.find_all().await?;

    Ok(HttpResponse::Ok().json(templates))
}

pub async fn list_common_templates(
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let template_repo =
        QueryTemplateRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());
    let templates = template_repo.find_common_templates().await?;

    Ok(HttpResponse::Ok().json(templates))
}

pub async fn list_templates_by_type(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let connection_type = path.into_inner();
    let template_repo =
        QueryTemplateRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Get the templates for this connection type including common templates
    let templates = template_repo
        .find_by_connection_type_with_common(&connection_type)
        .await?;

    Ok(HttpResponse::Ok().json(templates))
}

pub async fn get_template(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let template_repo =
        QueryTemplateRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Improved error handling for UUID parsing
    let id_str = path.into_inner();
    let template_id = match uuid::Uuid::parse_str(&id_str) {
        Ok(id) => id,
        Err(e) => {
            // More detailed error message for debugging
            return Err(AppError::BadRequest(format!(
                "Invalid UUID: {}. Provided value was: '{}'",
                e, id_str
            )));
        }
    };

    let template = template_repo
        .find_by_id(template_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Query template not found".to_string()))?;

    Ok(HttpResponse::Ok().json(template))
}

pub async fn create_template(
    template: web::Json<CreateQueryTemplateRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let template_repo =
        QueryTemplateRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

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
    let template_repo =
        QueryTemplateRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Improved error handling for UUID parsing
    let id_str = path.into_inner();
    let template_id = match uuid::Uuid::parse_str(&id_str) {
        Ok(id) => id,
        Err(e) => {
            // More detailed error message for debugging
            return Err(AppError::BadRequest(format!(
                "Invalid UUID: {}. Provided value was: '{}'",
                e, id_str
            )));
        }
    };

    let updated_template = template_repo.update(template_id, &template).await?;

    Ok(HttpResponse::Ok().json(updated_template))
}

pub async fn delete_template(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let template_repo =
        QueryTemplateRepository::new(db_pool.get_ref().clone(), config.get_ref().clone());

    // Improved error handling for UUID parsing
    let id_str = path.into_inner();
    let template_id = match uuid::Uuid::parse_str(&id_str) {
        Ok(id) => id,
        Err(e) => {
            // More detailed error message for debugging
            return Err(AppError::BadRequest(format!(
                "Invalid UUID: {}. Provided value was: '{}'",
                e, id_str
            )));
        }
    };

    template_repo.delete(template_id).await?;

    Ok(HttpResponse::NoContent().finish())
}
