use actix_web::{web, HttpResponse, Responder};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{NaiveDateTime, Duration};

use crate::config::Config;
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::slow_query_event::SlowQueryEvent;
use crate::repositories::slow_query_repository::SlowQueryRepository;
use crate::repositories::aurora_cluster_repository::AuroraClusterRepository;
use crate::services::slow_query_ingestion_service::SlowQueryIngestionService;
use crate::services::query_fingerprinting_service::QueryFingerprintingService;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct SlowQueryController {
    slow_query_repo: SlowQueryRepository,
    cluster_repo: AuroraClusterRepository,
    ingestion_service: SlowQueryIngestionService,
    fingerprint_service: QueryFingerprintingService,
}

impl SlowQueryController {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        let slow_query_repo = SlowQueryRepository::new(db.clone());
        let cluster_repo = AuroraClusterRepository::new(db.clone());
        let fingerprint_repo = crate::repositories::query_fingerprint_repository::QueryFingerprintRepository::new(db.clone());

        let ingestion_service = SlowQueryIngestionService::new(
            slow_query_repo.clone(),
            crate::repositories::query_fingerprint_repository::QueryFingerprintRepository::new(db.clone()),
            cluster_repo.clone(),
        );

        let fingerprint_service = QueryFingerprintingService::new(fingerprint_repo);

        Self {
            slow_query_repo,
            cluster_repo,
            ingestion_service,
            fingerprint_service,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SlowQueryFilter {
    pub cluster_id: Option<String>,
    pub start_time: Option<String>, // ISO 8601 format
    pub end_time: Option<String>,   // ISO 8601 format
    pub min_query_time: Option<f64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct SlowQueryEventResponse {
    pub event: SlowQueryEvent,
}

#[derive(Debug, Serialize)]
pub struct SlowQueryEventsResponse {
    pub events: Vec<SlowQueryEvent>,
    pub total: usize,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct SlowQueryStatsResponse {
    pub total_events: u64,
    pub events_last_hour: u64,
    pub events_last_24h: u64,
    pub avg_query_time: f64,
    pub max_query_time: f64,
    pub top_slow_queries: Vec<SlowQueryEvent>,
}

#[derive(Debug, Deserialize)]
pub struct IngestSlowQueriesRequest {
    pub cluster_id: String,
    pub log_content: String,
}

pub async fn get_slow_queries(
    controller: web::Data<SlowQueryController>,
    query: web::Query<SlowQueryFilter>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = if let Some(cluster_id_str) = &query.cluster_id {
        Some(Uuid::parse_str(cluster_id_str).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?)
    } else {
        None
    };

    let start_time = if let Some(start_str) = &query.start_time {
        Some(NaiveDateTime::parse_from_str(start_str, "%Y-%m-%dT%H:%M:%S%.fZ")
            .map_err(|e| AppError::BadRequest(format!("Invalid start_time format: {}", e)))?)
    } else {
        None
    };

    let end_time = if let Some(end_str) = &query.end_time {
        Some(NaiveDateTime::parse_from_str(end_str, "%Y-%m-%dT%H:%M:%S%.fZ")
            .map_err(|e| AppError::BadRequest(format!("Invalid end_time format: {}", e)))?)
    } else {
        None
    };

    let limit = query.limit.unwrap_or(100).min(1000); // Max 1000 records

    let events = if let (Some(cluster_id), Some(start), Some(end)) = (cluster_id, start_time, end_time) {
        controller.slow_query_repo.find_by_cluster_and_time_range(cluster_id, start, end).await?
    } else {
        // Default to last 24 hours for all clusters
        let end_time = chrono::Utc::now().naive_utc();
        let start_time = end_time - Duration::hours(24);

        if let Some(cluster_id) = cluster_id {
            controller.slow_query_repo.find_by_cluster_and_time_range(cluster_id, start_time, end_time).await?
        } else {
            // Get top slow queries across all clusters
            controller.slow_query_repo.find_top_by_total_time(None, 24, limit as u64).await?
        }
    };

    let total = events.len();
    let has_more = total == limit as usize;

    let response = SlowQueryEventsResponse {
        events,
        total,
        has_more,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_slow_query(
    _controller: web::Data<SlowQueryController>,
    path: web::Path<String>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let _event_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;

    // For now, we'll need to implement a find_by_id method in the repository
    // Since we don't have it yet, let's return a not implemented error
    Ok(HttpResponse::NotImplemented().json(serde_json::json!({
        "error": "Individual slow query retrieval not yet implemented"
    })))
}

pub async fn get_slow_query_stats(
    controller: web::Data<SlowQueryController>,
    query: web::Query<SlowQueryFilter>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = if let Some(cluster_id_str) = &query.cluster_id {
        Some(Uuid::parse_str(cluster_id_str).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?)
    } else {
        None
    };

    // Get total events count
    let total_events = if let Some(cluster_id) = cluster_id {
        controller.slow_query_repo.count_by_cluster(cluster_id).await?
    } else {
        // Count across all clusters - this would need a new repository method
        0 // Placeholder
    };

    // Get events in last hour
    let events_last_hour = controller.slow_query_repo.find_top_by_total_time(cluster_id, 1, 1000).await?.len() as u64;

    // Get events in last 24 hours
    let events_last_24h = controller.slow_query_repo.find_top_by_total_time(cluster_id, 24, 1000).await?.len() as u64;

    // Get top slow queries for analysis
    let top_slow_queries = controller.slow_query_repo.find_top_by_total_time(cluster_id, 24, 10).await?;

    // Calculate stats from top queries
    let avg_query_time = if !top_slow_queries.is_empty() {
        top_slow_queries.iter().map(|e| e.query_time).sum::<f64>() / top_slow_queries.len() as f64
    } else {
        0.0
    };

    let max_query_time = top_slow_queries.iter()
        .map(|e| e.query_time)
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);

    let response = SlowQueryStatsResponse {
        total_events,
        events_last_hour,
        events_last_24h,
        avg_query_time,
        max_query_time,
        top_slow_queries,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn ingest_slow_queries(
    controller: web::Data<SlowQueryController>,
    payload: web::Json<IngestSlowQueriesRequest>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = Uuid::parse_str(&payload.cluster_id).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?;

    // Use the ingestion service to process the log content
    controller.ingestion_service.ingest_slow_query_log(cluster_id, &payload.log_content).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Slow query logs ingested successfully",
        "cluster_id": payload.cluster_id,
    })))
}

pub async fn delete_slow_query(
    _controller: web::Data<SlowQueryController>,
    path: web::Path<String>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let _event_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;

    // For now, we'll need to implement a delete_by_id method in the repository
    // Since we don't have it yet, let's return a not implemented error
    Ok(HttpResponse::NotImplemented().json(serde_json::json!({
        "error": "Slow query deletion not yet implemented"
    })))
}