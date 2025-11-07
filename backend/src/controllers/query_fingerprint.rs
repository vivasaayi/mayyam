use actix_web::{web, HttpResponse, Responder};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::Config;
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::query_fingerprint::QueryFingerprint;
use crate::repositories::query_fingerprint_repository::QueryFingerprintRepository;
use crate::services::query_fingerprinting_service::QueryFingerprintingService;
use crate::services::ai_analysis_service::AIAnalysisService;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct FingerprintFilter {
    pub cluster_id: Option<String>,
    pub hours: Option<i64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AnalyzeFingerprintRequest {
    pub analysis_type: String,
    pub context_data: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Serialize)]
pub struct QueryFingerprintResponse {
    pub fingerprint: QueryFingerprint,
}

#[derive(Debug, Serialize)]
pub struct QueryFingerprintsResponse {
    pub fingerprints: Vec<QueryFingerprint>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct FingerprintAnalysisResponse {
    pub fingerprint_id: Uuid,
    pub recommendations: Vec<String>,
    pub root_causes: Vec<String>,
    pub confidence_score: f64,
    pub suggestions: Vec<String>,
}

pub async fn get_fingerprints(
    query: web::Query<FingerprintFilter>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let fingerprint_repo = QueryFingerprintRepository::new(db_pool.get_ref().clone());

    let cluster_id = if let Some(cluster_id_str) = &query.cluster_id {
        Some(Uuid::parse_str(cluster_id_str).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?)
    } else {
        None
    };

    let hours = query.hours.unwrap_or(24);
    let limit = query.limit.unwrap_or(50).min(200); // Max 200 records

    let fingerprints = fingerprint_repo.find_top_by_execution_time(cluster_id, hours, limit).await?;

    let response = QueryFingerprintsResponse {
        fingerprints,
        total: fingerprints.len(),
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_fingerprint(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let fingerprint_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let fingerprint_repo = QueryFingerprintRepository::new(db_pool.get_ref().clone());

    let fingerprint = fingerprint_repo.find_by_id(fingerprint_id).await?
        .ok_or_else(|| AppError::NotFound(format!("Fingerprint not found: {}", fingerprint_id)))?;

    let response = QueryFingerprintResponse { fingerprint };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn analyze_fingerprint(
    path: web::Path<String>,
    req: web::Json<AnalyzeFingerprintRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let fingerprint_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;

    let ai_repo = crate::repositories::ai_analysis_repository::AIAnalysisRepository::new(db_pool.get_ref().clone());
    let fingerprint_repo = QueryFingerprintRepository::new(db_pool.get_ref().clone());
    let slow_query_repo = crate::repositories::slow_query_repository::SlowQueryRepository::new(db_pool.get_ref().clone());
    let explain_repo = crate::repositories::explain_plan_repository::ExplainPlanRepository::new(db_pool.get_ref().clone());

    let ai_service = AIAnalysisService::new(
        ai_repo,
        fingerprint_repo,
        slow_query_repo,
        explain_repo,
    );

    let analysis_request = crate::services::ai_analysis_service::AnalysisRequest {
        fingerprint_id,
        analysis_type: req.analysis_type.clone(),
        context_data: req.context_data.clone().unwrap_or_default(),
    };

    let result = ai_service.generate_analysis(analysis_request).await?;

    let response = FingerprintAnalysisResponse {
        fingerprint_id,
        recommendations: result.recommendations,
        root_causes: result.root_causes,
        confidence_score: result.confidence_score,
        suggestions: result.suggestions,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_fingerprint_analysis_history(
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let fingerprint_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let analysis_type = query.get("analysis_type");

    let ai_repo = crate::repositories::ai_analysis_repository::AIAnalysisRepository::new(db_pool.get_ref().clone());
    let fingerprint_repo = QueryFingerprintRepository::new(db_pool.get_ref().clone());
    let slow_query_repo = crate::repositories::slow_query_repository::SlowQueryRepository::new(db_pool.get_ref().clone());
    let explain_repo = crate::repositories::explain_plan_repository::ExplainPlanRepository::new(db_pool.get_ref().clone());

    let ai_service = AIAnalysisService::new(
        ai_repo,
        fingerprint_repo,
        slow_query_repo,
        explain_repo,
    );

    let analyses = if let Some(analysis_type) = analysis_type {
        ai_service.get_analysis_history(fingerprint_id, Some(analysis_type)).await?
    } else {
        ai_service.get_analysis_history(fingerprint_id, None).await?
    };

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "fingerprint_id": fingerprint_id,
        "analyses": analyses,
        "total": analyses.len()
    })))
}

pub async fn update_fingerprint_catalog(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let fingerprint_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;

    let fingerprint_repo = QueryFingerprintRepository::new(db_pool.get_ref().clone());
    let fingerprinting_service = QueryFingerprintingService::new(fingerprint_repo);

    let fingerprint = fingerprint_repo.find_by_id(fingerprint_id).await?
        .ok_or_else(|| AppError::NotFound(format!("Fingerprint not found: {}", fingerprint_id)))?;

    // Update catalog data based on the normalized SQL
    fingerprinting_service.fingerprint_and_update_catalog(fingerprint_id, &fingerprint.normalized_sql).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Fingerprint catalog updated successfully",
        "fingerprint_id": fingerprint_id
    })))
}

pub async fn get_fingerprint_stats(
    query: web::Query<FingerprintFilter>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let fingerprint_repo = QueryFingerprintRepository::new(db_pool.get_ref().clone());

    let cluster_id = if let Some(cluster_id_str) = &query.cluster_id {
        Some(Uuid::parse_str(cluster_id_str).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?)
    } else {
        None
    };

    let total_fingerprints = if let Some(cluster_id) = cluster_id {
        fingerprint_repo.count_by_cluster(cluster_id).await?
    } else {
        // This would need a method to count across all clusters
        0 // Placeholder
    };

    let hours = query.hours.unwrap_or(24);
    let active_fingerprints = fingerprint_repo.find_top_by_execution_time(cluster_id, hours, 1000).await?.len() as u64;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "total_fingerprints": total_fingerprints,
        "active_fingerprints_last_hours": active_fingerprints,
        "hours": hours
    })))
}