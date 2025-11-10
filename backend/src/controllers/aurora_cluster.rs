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
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::Config;
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::aurora_cluster::{AuroraCluster, AuroraClusterDto};
use crate::repositories::aurora_cluster_repository::AuroraClusterRepository;
use crate::services::slow_query_ingestion_service::SlowQueryIngestionService;
use crate::services::mysql_performance_service::MySQLPerformanceService;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreateAuroraClusterRequest {
    pub name: String,
    pub engine: String,
    pub region: String,
    pub log_group: Option<String>,
    pub log_stream: Option<String>,
    pub read_only_dsn: String,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAuroraClusterRequest {
    pub name: Option<String>,
    pub engine: Option<String>,
    pub region: Option<String>,
    pub log_group: Option<String>,
    pub log_stream: Option<String>,
    pub read_only_dsn: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct IngestSlowQueryRequest {
    pub log_content: String,
}

#[derive(Debug, Serialize)]
pub struct AuroraClusterResponse {
    pub cluster: AuroraClusterDto,
}

#[derive(Debug, Serialize)]
pub struct AuroraClustersResponse {
    pub clusters: Vec<AuroraClusterDto>,
    pub total: usize,
}

pub async fn create_cluster(
    req: web::Json<CreateAuroraClusterRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_repo = AuroraClusterRepository::new(db_pool.get_ref().clone());

    // Check if cluster with same name already exists
    let existing = cluster_repo.find_all_active().await?;
    if existing.iter().any(|c| c.name == req.name) {
        return Err(AppError::BadRequest(format!(
            "Cluster with name '{}' already exists",
            req.name
        )));
    }

    let cluster = AuroraCluster {
        id: Uuid::new_v4(),
        name: req.name.clone(),
        engine: req.engine.clone(),
        region: req.region.clone(),
        log_group: req.log_group.clone(),
        log_stream: req.log_stream.clone(),
        read_only_dsn: req.read_only_dsn.clone(),
        is_active: req.is_active.unwrap_or(true),
        created_at: chrono::Utc::now().naive_utc(),
        updated_at: chrono::Utc::now().naive_utc(),
    };

    let created_cluster = cluster_repo.create(cluster).await?;
    let response = AuroraClusterResponse {
        cluster: AuroraClusterDto::from(created_cluster),
    };

    Ok(HttpResponse::Created().json(response))
}

pub async fn get_clusters(
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_repo = AuroraClusterRepository::new(db_pool.get_ref().clone());
    let clusters = cluster_repo.find_all_active().await?;
    let total = clusters.len();

    let response = AuroraClustersResponse {
        clusters: clusters.into_iter().map(AuroraClusterDto::from).collect(),
        total,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_cluster(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let cluster_repo = AuroraClusterRepository::new(db_pool.get_ref().clone());

    let cluster = cluster_repo.find_by_id(cluster_id).await?
        .ok_or_else(|| AppError::NotFound(format!("Cluster not found: {}", cluster_id)))?;

    let response = AuroraClusterResponse {
        cluster: AuroraClusterDto::from(cluster),
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn update_cluster(
    path: web::Path<String>,
    req: web::Json<UpdateAuroraClusterRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let cluster_repo = AuroraClusterRepository::new(db_pool.get_ref().clone());

    let mut existing_cluster = cluster_repo.find_by_id(cluster_id).await?
        .ok_or_else(|| AppError::NotFound(format!("Cluster not found: {}", cluster_id)))?;

    // Update fields if provided
    if let Some(name) = &req.name {
        existing_cluster.name = name.clone();
    }
    if let Some(engine) = &req.engine {
        existing_cluster.engine = engine.clone();
    }
    if let Some(region) = &req.region {
        existing_cluster.region = region.clone();
    }
    if let Some(log_group) = &req.log_group {
        existing_cluster.log_group = Some(log_group.clone());
    }
    if let Some(log_stream) = &req.log_stream {
        existing_cluster.log_stream = Some(log_stream.clone());
    }
    if let Some(read_only_dsn) = &req.read_only_dsn {
        existing_cluster.read_only_dsn = read_only_dsn.clone();
    }
    if let Some(is_active) = req.is_active {
        existing_cluster.is_active = is_active;
    }
    existing_cluster.updated_at = chrono::Utc::now().naive_utc();

    let updated_cluster = cluster_repo.update(existing_cluster).await?;
    let response = AuroraClusterResponse {
        cluster: AuroraClusterDto::from(updated_cluster),
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn delete_cluster(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let cluster_repo = AuroraClusterRepository::new(db_pool.get_ref().clone());

    cluster_repo.delete(cluster_id).await?;

    Ok(HttpResponse::NoContent().finish())
}

pub async fn ingest_slow_queries(
    path: web::Path<String>,
    req: web::Json<IngestSlowQueryRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;

    let cluster_repo = AuroraClusterRepository::new(db_pool.get_ref().clone());
    let slow_query_repo = crate::repositories::slow_query_repository::SlowQueryRepository::new(db_pool.get_ref().clone());
    let fingerprint_repo = crate::repositories::query_fingerprint_repository::QueryFingerprintRepository::new(db_pool.get_ref().clone());

    let ingestion_service = SlowQueryIngestionService::new(
        slow_query_repo,
        fingerprint_repo,
        cluster_repo,
    );

    ingestion_service.ingest_slow_query_log(cluster_id, &req.log_content).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Slow query logs ingested successfully",
        "cluster_id": cluster_id
    })))
}

pub async fn get_cluster_stats(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;

    let slow_query_repo = crate::repositories::slow_query_repository::SlowQueryRepository::new(db_pool.get_ref().clone());
    let fingerprint_repo = crate::repositories::query_fingerprint_repository::QueryFingerprintRepository::new(db_pool.get_ref().clone());
    let performance_repo = crate::repositories::mysql_performance_repository::MySQLPerformanceRepository::new(db_pool.get_ref().clone());

    let slow_query_count = slow_query_repo.count_by_cluster(cluster_id).await?;
    let fingerprint_count = fingerprint_repo.count_by_cluster(cluster_id).await?;
    let snapshot_count = performance_repo.count_by_cluster(cluster_id).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "cluster_id": cluster_id,
        "slow_query_count": slow_query_count,
        "fingerprint_count": fingerprint_count,
        "performance_snapshot_count": snapshot_count
    })))
}