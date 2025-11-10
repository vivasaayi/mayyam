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
use crate::repositories::slow_query_repository::SlowQueryRepository;
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

#[derive(Clone)]
pub struct QueryFingerprintController {
    fingerprint_repo: QueryFingerprintRepository,
    slow_query_repo: SlowQueryRepository,
    fingerprint_service: QueryFingerprintingService,
    ai_service: AIAnalysisService,
}

impl QueryFingerprintController {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        let fingerprint_repo = QueryFingerprintRepository::new(db.clone());
        let slow_query_repo = SlowQueryRepository::new(db.clone());
        let fingerprint_service = QueryFingerprintingService::new(fingerprint_repo.clone());
        let ai_repo = crate::repositories::ai_analysis_repository::AIAnalysisRepository::new(db.clone());
        let explain_repo = crate::repositories::explain_plan_repository::ExplainPlanRepository::new(db.clone());
        let ai_service = AIAnalysisService::new(ai_repo, fingerprint_repo.clone(), slow_query_repo.clone(), explain_repo);

        Self {
            fingerprint_repo,
            slow_query_repo,
            fingerprint_service,
            ai_service,
        }
    }
}

pub async fn get_fingerprints(
    controller: web::Data<QueryFingerprintController>,
    query: web::Query<FingerprintFilter>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = if let Some(cluster_id_str) = &query.cluster_id {
        Some(Uuid::parse_str(cluster_id_str).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?)
    } else {
        None
    };

    let hours = query.hours.unwrap_or(24);
    let limit = query.limit.unwrap_or(50).min(200); // Max 200 records

    let fingerprints = controller.fingerprint_repo.find_top_by_execution_time(cluster_id, hours, limit).await?;
    let total = fingerprints.len();

    let response = QueryFingerprintsResponse {
        fingerprints,
        total,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_fingerprint(
    controller: web::Data<QueryFingerprintController>,
    path: web::Path<String>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let fingerprint_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;

    let fingerprint = controller.fingerprint_repo.find_by_id(fingerprint_id).await?
        .ok_or_else(|| AppError::NotFound(format!("Fingerprint not found: {}", fingerprint_id)))?;

    let response = QueryFingerprintResponse { fingerprint };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn analyze_fingerprint(
    controller: web::Data<QueryFingerprintController>,
    path: web::Path<String>,
    req: web::Json<AnalyzeFingerprintRequest>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let fingerprint_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;

    let fingerprint = controller.fingerprint_repo.find_by_id(fingerprint_id).await?
        .ok_or_else(|| AppError::NotFound(format!("Fingerprint not found: {}", fingerprint_id)))?;

    // Get slow query events for this fingerprint
    let events = controller.slow_query_repo.find_by_fingerprint(fingerprint_id, 100).await?;

    // Perform AI analysis
    let analysis_request = crate::services::ai_analysis_service::AnalysisRequest {
        fingerprint_id,
        analysis_type: req.analysis_type.clone(),
        context_data: std::collections::HashMap::new(),
    };
    let analysis = controller.ai_service.generate_analysis(analysis_request).await?;

    let response = FingerprintAnalysisResponse {
        fingerprint_id,
        recommendations: analysis.recommendations,
        root_causes: analysis.root_causes,
        confidence_score: analysis.confidence_score,
        suggestions: analysis.suggestions,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_fingerprint_patterns(
    controller: web::Data<QueryFingerprintController>,
    path: web::Path<String>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?;

    // Get fingerprint patterns for the cluster
    let patterns = controller.fingerprint_repo.find_patterns_by_cluster(cluster_id, 24).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "cluster_id": cluster_id,
        "patterns": patterns,
        "total_patterns": patterns.len()
    })))
}