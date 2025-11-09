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
use serde::{Deserialize, Serialize};

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

pub async fn get_slow_queries(
    query: web::Query<SlowQueryFilter>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let slow_query_repo = SlowQueryRepository::new(db_pool.get_ref().clone());

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
        slow_query_repo.find_by_cluster_and_time_range(cluster_id, start, end).await?
    } else {
        // Default to last 24 hours for all clusters
        let end_time = chrono::Utc::now().naive_utc();
        let start_time = end_time - Duration::hours(24);

        if let Some(cluster_id) = cluster_id {
            slow_query_repo.find_by_cluster_and_time_range(cluster_id, start_time, end_time).await?
        } else {
            // Get top slow queries across all clusters
            slow_query_repo.find_top_by_total_time(None, 24, limit as u64).await?
        }
    };

    let has_more = events.len() == limit as usize;
    let response = SlowQueryEventsResponse {
        events,
        total: events.len(),
        has_more,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_slow_query(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let event_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let slow_query_repo = SlowQueryRepository::new(db_pool.get_ref().clone());

    // For now, we'll need to implement a find_by_id method in the repository
    // Since we don't have it yet, let's return a not implemented error
    Err(AppError::NotImplemented("Individual slow query retrieval not yet implemented".to_string()))
}

pub async fn get_slow_query_stats(
    query: web::Query<SlowQueryFilter>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let slow_query_repo = SlowQueryRepository::new(db_pool.get_ref().clone());

    let cluster_id = if let Some(cluster_id_str) = &query.cluster_id {
        Some(Uuid::parse_str(cluster_id_str).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?)
    } else {
        None
    };

    // Get total events count
    let total_events = if let Some(cluster_id) = cluster_id {
        slow_query_repo.count_by_cluster(cluster_id).await?
    } else {
        // Count across all clusters - this would need a new repository method
        0 // Placeholder
    };

    // Get events in last hour
    let events_last_hour = slow_query_repo.find_top_by_total_time(cluster_id, 1, 1000).await?.len() as u64;

    // Get events in last 24 hours
    let events_last_24h = slow_query_repo.find_top_by_total_time(cluster_id, 24, 1000).await?.len() as u64;

    // Get top slow queries for analysis
    let top_slow_queries = slow_query_repo.find_top_by_total_time(cluster_id, 24, 10).await?;

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

pub async fn delete_old_slow_queries(
    query: web::Query<std::collections::HashMap<String, String>>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let days_to_keep = query.get("days")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(30); // Default 30 days

    let slow_query_repo = SlowQueryRepository::new(db_pool.get_ref().clone());
    let deleted_count = slow_query_repo.delete_old_events(days_to_keep).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Deleted {} old slow query events", deleted_count),
        "deleted_count": deleted_count
    })))
}