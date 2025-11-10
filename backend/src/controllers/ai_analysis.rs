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
use crate::models::ai_analysis::AIAnalysis;
use crate::repositories::ai_analysis_repository::AIAnalysisRepository;
use crate::services::ai_analysis_service::AIAnalysisService;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct AIAnalysisFilter {
    pub fingerprint_id: Option<String>,
    pub analysis_type: Option<String>,
    pub cluster_id: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAIAnalysisRequest {
    pub fingerprint_id: Uuid,
    pub analysis_type: String,
    pub context_data: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Serialize)]
pub struct AIAnalysisResponse {
    pub analysis: AIAnalysis,
}

#[derive(Debug, Serialize)]
pub struct AIAnalysesResponse {
    pub analyses: Vec<AIAnalysis>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct AnalysisResultResponse {
    pub analysis_id: Uuid,
    pub fingerprint_id: Uuid,
    pub analysis_type: String,
    pub recommendations: Vec<String>,
    pub root_causes: Vec<String>,
    pub confidence_score: f64,
    pub suggestions: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create_ai_analysis(
    req: web::Json<CreateAIAnalysisRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let ai_repo = AIAnalysisRepository::new(db_pool.get_ref().clone());
    let fingerprint_repo = crate::repositories::query_fingerprint_repository::QueryFingerprintRepository::new(db_pool.get_ref().clone());
    let slow_query_repo = crate::repositories::slow_query_repository::SlowQueryRepository::new(db_pool.get_ref().clone());
    let explain_repo = crate::repositories::explain_plan_repository::ExplainPlanRepository::new(db_pool.get_ref().clone());

    let ai_service = AIAnalysisService::new(
        ai_repo,
        fingerprint_repo,
        slow_query_repo,
        explain_repo,
    );

    let analysis_request = crate::services::ai_analysis_service::AnalysisRequest {
        fingerprint_id: req.fingerprint_id,
        analysis_type: req.analysis_type.clone(),
        context_data: req.context_data.clone().unwrap_or_default(),
    };

    let result = ai_service.generate_analysis(analysis_request).await?;

    let response = AnalysisResultResponse {
        analysis_id: result.analysis_id,
        fingerprint_id: req.fingerprint_id,
        analysis_type: req.analysis_type.clone(),
        recommendations: result.recommendations,
        root_causes: result.root_causes,
        confidence_score: result.confidence_score,
        suggestions: result.suggestions,
        created_at: chrono::Utc::now(),
    };

    Ok(HttpResponse::Created().json(response))
}

pub async fn get_ai_analyses(
    query: web::Query<AIAnalysisFilter>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let ai_repo = AIAnalysisRepository::new(db_pool.get_ref().clone());

    let fingerprint_id = if let Some(fingerprint_id_str) = &query.fingerprint_id {
        Some(Uuid::parse_str(fingerprint_id_str).map_err(|e| AppError::BadRequest(format!("Invalid fingerprint UUID: {}", e)))?)
    } else {
        None
    };

    let cluster_id = if let Some(cluster_id_str) = &query.cluster_id {
        Some(Uuid::parse_str(cluster_id_str).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?)
    } else {
        None
    };

    let limit = query.limit.unwrap_or(50).min(200); // Max 200 records

    let analyses = if let Some(fingerprint_id) = fingerprint_id {
        ai_repo.find_by_fingerprint(fingerprint_id, Some(limit)).await?
    } else if let Some(cluster_id) = cluster_id {
        ai_repo.find_by_cluster(cluster_id, limit).await?
    } else if let Some(analysis_type) = &query.analysis_type {
        ai_repo.find_by_analysis_type_with_limit(analysis_type.clone(), limit).await?
    } else {
        ai_repo.find_recent(limit).await?
    };

    let total = analyses.len();

    let response = AIAnalysesResponse {
        analyses,
        total,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_ai_analysis(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let analysis_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let ai_repo = AIAnalysisRepository::new(db_pool.get_ref().clone());

    let analysis = ai_repo.find_by_id(analysis_id).await?
        .ok_or_else(|| AppError::NotFound(format!("AI analysis not found: {}", analysis_id)))?;

    let response = AIAnalysisResponse { analysis };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_analysis_history(
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let fingerprint_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid fingerprint UUID: {}", e)))?;
    let analysis_type = query.get("analysis_type");

    let ai_repo = AIAnalysisRepository::new(db_pool.get_ref().clone());
    let fingerprint_repo = crate::repositories::query_fingerprint_repository::QueryFingerprintRepository::new(db_pool.get_ref().clone());
    let slow_query_repo = crate::repositories::slow_query_repository::SlowQueryRepository::new(db_pool.get_ref().clone());
    let explain_repo = crate::repositories::explain_plan_repository::ExplainPlanRepository::new(db_pool.get_ref().clone());

    let ai_service = AIAnalysisService::new(
        ai_repo,
        fingerprint_repo,
        slow_query_repo,
        explain_repo,
    );

    let analyses = if let Some(analysis_type) = analysis_type {
        ai_service.get_analysis_history(fingerprint_id, Some(analysis_type.clone())).await?
    } else {
        ai_service.get_analysis_history(fingerprint_id, None).await?
    };

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "fingerprint_id": fingerprint_id,
        "analyses": analyses,
        "total": analyses.len()
    })))
}

pub async fn update_analysis_confidence(
    path: web::Path<String>,
    req: web::Json<f64>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let analysis_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let ai_repo = AIAnalysisRepository::new(db_pool.get_ref().clone());

    ai_repo.update_confidence_score(analysis_id, *req).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Analysis confidence score updated successfully",
        "analysis_id": analysis_id,
        "confidence_score": req
    })))
}

pub async fn get_analysis_stats(
    query: web::Query<AIAnalysisFilter>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let ai_repo = AIAnalysisRepository::new(db_pool.get_ref().clone());

    let cluster_id = if let Some(cluster_id_str) = &query.cluster_id {
        Some(Uuid::parse_str(cluster_id_str).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?)
    } else {
        None
    };

    let total_analyses = if let Some(cluster_id) = cluster_id {
        ai_repo.count_by_cluster(cluster_id).await?
    } else {
        // This would need a method to count across all clusters
        0 // Placeholder
    };

    let analysis_types = ai_repo.get_analysis_types().await?;
    let avg_confidence = ai_repo.get_average_confidence().await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "total_analyses": total_analyses,
        "analysis_types": analysis_types,
        "average_confidence_score": avg_confidence
    })))
}

pub async fn delete_ai_analysis(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let analysis_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let ai_repo = AIAnalysisRepository::new(db_pool.get_ref().clone());

    ai_repo.delete(analysis_id).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "AI analysis deleted successfully",
        "analysis_id": analysis_id
    })))
}

pub async fn get_analysis_types(
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let ai_repo = AIAnalysisRepository::new(db_pool.get_ref().clone());

    let analysis_types = ai_repo.get_analysis_types().await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "analysis_types": analysis_types
    })))
}