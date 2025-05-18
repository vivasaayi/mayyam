use std::sync::Arc;
use actix_web::{web, HttpResponse, Responder};
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::aws_account::{AwsAccountCreateDto, AwsAccountUpdateDto};
use crate::services::aws_account::AwsAccountService;

/// List all AWS accounts
pub async fn list_accounts(
    service: web::Data<Arc<AwsAccountService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let accounts = service.list_accounts().await?;
    Ok(HttpResponse::Ok().json(accounts))
}

/// Get a specific AWS account by ID
pub async fn get_account(
    id: web::Path<Uuid>,
    service: web::Data<Arc<AwsAccountService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let account = service.get_account(id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(account))
}

/// Create a new AWS account
pub async fn create_account(
    dto: web::Json<AwsAccountCreateDto>,
    service: web::Data<Arc<AwsAccountService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let account = service.create_account(dto.into_inner()).await?;
    Ok(HttpResponse::Created().json(account))
}

/// Update an existing AWS account
pub async fn update_account(
    id: web::Path<Uuid>,
    dto: web::Json<AwsAccountUpdateDto>,
    service: web::Data<Arc<AwsAccountService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let account = service.update_account(id.into_inner(), dto.into_inner()).await?;
    Ok(HttpResponse::Ok().json(account))
}

/// Delete an AWS account
pub async fn delete_account(
    id: web::Path<Uuid>,
    service: web::Data<Arc<AwsAccountService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    service.delete_account(id.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Sync resources for an AWS account
pub async fn sync_account_resources(
    id: web::Path<Uuid>,
    service: web::Data<Arc<AwsAccountService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let response = service.sync_account_resources(id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(response))
}
