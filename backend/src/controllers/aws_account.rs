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


use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::aws_account::{AwsAccountCreateDto, AwsAccountUpdateDto};
use crate::services::aws_account::AwsAccountService;

#[derive(Deserialize)]
pub struct SyncResourcesQuery {
    pub sync_id: Option<Uuid>,
}

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
    let account = service
        .update_account(id.into_inner(), dto.into_inner())
        .await?;
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
    query: web::Query<SyncResourcesQuery>,
    service: web::Data<Arc<AwsAccountService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let sync_id = query.sync_id.unwrap_or_else(Uuid::new_v4);
    debug!(
        "Syncing resources for AWS account: {} with sync_id: {}",
        id, sync_id
    );
    let response = service
        .sync_account_resources(id.into_inner(), sync_id)
        .await?;
    Ok(HttpResponse::Ok().json(response))
}

/// Sync resources for all AWS accounts
pub async fn sync_all_accounts_resources(
    service: web::Data<Arc<AwsAccountService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let response = service.sync_all_accounts_resources().await?;
    Ok(HttpResponse::Ok().json(response))
}
