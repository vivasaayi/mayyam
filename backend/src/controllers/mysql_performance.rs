use actix_web::{web, HttpResponse, Responder};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::Config;
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::mysql_performance_snapshot::MySQLPerformanceSnapshot;
use crate::repositories::mysql_performance_repository::MySQLPerformanceRepository;
use crate::services::mysql_performance_service::MySQLPerformanceService;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct PerformanceFilter {
    pub cluster_id: Option<String>,
    pub hours: Option<i64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePerformanceSnapshotRequest {
    pub cluster_id: Uuid,
    pub metrics: serde_json::Value,
    pub issues: Option<Vec<String>>,
    pub recommendations: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct PerformanceSnapshotResponse {
    pub snapshot: MySQLPerformanceSnapshot,
}

#[derive(Debug, Serialize)]
pub struct PerformanceSnapshotsResponse {
    pub snapshots: Vec<MySQLPerformanceSnapshot>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct HealthCheckResponse {
    pub cluster_id: Uuid,
    pub health_score: f64,
    pub status: String,
    pub issues: Vec<String>,
    pub recommendations: Vec<String>,
    pub metrics: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct PerformanceTrendsResponse {
    pub cluster_id: Uuid,
    pub trends: serde_json::Value,
    pub period_hours: i64,
}

#[derive(Debug, Serialize)]
pub struct AnomaliesResponse {
    pub cluster_id: Uuid,
    pub anomalies: Vec<serde_json::Value>,
    pub detected_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create_performance_snapshot(
    req: web::Json<CreatePerformanceSnapshotRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let performance_repo = MySQLPerformanceRepository::new(db_pool.get_ref().clone());
    let performance_service = MySQLPerformanceService::new(performance_repo);

    let snapshot = performance_service.create_performance_snapshot(
        req.cluster_id,
        req.metrics.clone(),
        req.issues.clone().unwrap_or_default(),
        req.recommendations.clone().unwrap_or_default(),
    ).await?;

    let response = PerformanceSnapshotResponse { snapshot };

    Ok(HttpResponse::Created().json(response))
}

pub async fn get_performance_snapshots(
    query: web::Query<PerformanceFilter>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let performance_repo = MySQLPerformanceRepository::new(db_pool.get_ref().clone());

    let cluster_id = if let Some(cluster_id_str) = &query.cluster_id {
        Some(Uuid::parse_str(cluster_id_str).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?)
    } else {
        None
    };

    let hours = query.hours.unwrap_or(24);
    let limit = query.limit.unwrap_or(50).min(200); // Max 200 records

    let snapshots = if let Some(cluster_id) = cluster_id {
        performance_repo.find_by_cluster_and_time(cluster_id, hours, limit).await?
    } else {
        performance_repo.find_recent(limit).await?
    };

    let response = PerformanceSnapshotsResponse {
        snapshots,
        total: snapshots.len(),
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_performance_snapshot(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let snapshot_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let performance_repo = MySQLPerformanceRepository::new(db_pool.get_ref().clone());

    let snapshot = performance_repo.find_by_id(snapshot_id).await?
        .ok_or_else(|| AppError::NotFound(format!("Performance snapshot not found: {}", snapshot_id)))?;

    let response = PerformanceSnapshotResponse { snapshot };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn perform_health_check(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?;
    let performance_repo = MySQLPerformanceRepository::new(db_pool.get_ref().clone());
    let performance_service = MySQLPerformanceService::new(performance_repo);

    let health_check = performance_service.perform_health_check(cluster_id).await?;

    let response = HealthCheckResponse {
        cluster_id,
        health_score: health_check.health_score,
        status: health_check.status,
        issues: health_check.issues,
        recommendations: health_check.recommendations,
        metrics: health_check.metrics,
        timestamp: health_check.timestamp,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_performance_trends(
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?;
    let hours = query.get("hours")
        .and_then(|h| h.parse::<i64>().ok())
        .unwrap_or(24);

    let performance_repo = MySQLPerformanceRepository::new(db_pool.get_ref().clone());
    let performance_service = MySQLPerformanceService::new(performance_repo);

    let trends = performance_service.get_performance_trends(cluster_id, hours).await?;

    let response = PerformanceTrendsResponse {
        cluster_id,
        trends,
        period_hours: hours,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn detect_performance_anomalies(
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?;
    let hours = query.get("hours")
        .and_then(|h| h.parse::<i64>().ok())
        .unwrap_or(24);

    let performance_repo = MySQLPerformanceRepository::new(db_pool.get_ref().clone());
    let performance_service = MySQLPerformanceService::new(performance_repo);

    let anomalies = performance_service.detect_performance_anomalies(cluster_id, hours).await?;

    let response = AnomaliesResponse {
        cluster_id,
        anomalies,
        detected_at: chrono::Utc::now(),
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_performance_stats(
    query: web::Query<PerformanceFilter>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let performance_repo = MySQLPerformanceRepository::new(db_pool.get_ref().clone());

    let cluster_id = if let Some(cluster_id_str) = &query.cluster_id {
        Some(Uuid::parse_str(cluster_id_str).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?)
    } else {
        None
    };

    let total_snapshots = if let Some(cluster_id) = cluster_id {
        performance_repo.count_by_cluster(cluster_id).await?
    } else {
        // This would need a method to count across all clusters
        0 // Placeholder
    };

    let hours = query.hours.unwrap_or(24);
    let avg_health_score = if let Some(cluster_id) = cluster_id {
        performance_repo.get_average_health_score(cluster_id, hours).await?
    } else {
        None
    };

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "total_snapshots": total_snapshots,
        "average_health_score": avg_health_score,
        "period_hours": hours
    })))
}

pub async fn delete_performance_snapshot(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let snapshot_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let performance_repo = MySQLPerformanceRepository::new(db_pool.get_ref().clone());

    performance_repo.delete(snapshot_id).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Performance snapshot deleted successfully",
        "snapshot_id": snapshot_id
    })))
}

pub async fn cleanup_old_snapshots(
    query: web::Query<std::collections::HashMap<String, String>>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let days = query.get("days")
        .and_then(|d| d.parse::<i64>().ok())
        .unwrap_or(30); // Default 30 days

    let performance_repo = MySQLPerformanceRepository::new(db_pool.get_ref().clone());

    let deleted_count = performance_repo.cleanup_old_snapshots(days).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Cleaned up {} old performance snapshots", deleted_count),
        "deleted_count": deleted_count,
        "older_than_days": days
    })))
}