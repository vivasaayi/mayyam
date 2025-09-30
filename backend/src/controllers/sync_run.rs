use std::sync::Arc;
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use uuid::Uuid;
use tracing::{debug};

use crate::errors::AppError;
use crate::models::sync_run::{SyncRunCreateDto, SyncRunQueryParams};
use crate::repositories::sync_run::SyncRunRepository;

#[derive(Clone)]
pub struct SyncRunController {
    repo: Arc<SyncRunRepository>,
}

impl SyncRunController {
    pub fn new(repo: Arc<SyncRunRepository>) -> Self { Self { repo } }
}

pub async fn create_sync_run(
    controller: web::Data<Arc<SyncRunController>>,
    payload: web::Json<SyncRunCreateDto>,
) -> Result<HttpResponse, AppError> {
    debug!("Creating sync_run: {:?}", payload);
    let created = controller.repo.create(payload.into_inner()).await?;
    Ok(HttpResponse::Created().json(created))
}

pub async fn list_sync_runs(
    controller: web::Data<Arc<SyncRunController>>,
    query: web::Query<SyncRunQueryParams>,
) -> Result<HttpResponse, AppError> {
    let runs = controller.repo.list(query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(runs))
}

pub async fn get_sync_run(
    controller: web::Data<Arc<SyncRunController>>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    match controller.repo.get(id.into_inner()).await? {
        Some(run) => Ok(HttpResponse::Ok().json(run)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}
