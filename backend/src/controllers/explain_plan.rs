use actix_web::{web, HttpResponse, Responder};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::Config;
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::explain_plan::ExplainPlan;
use crate::repositories::explain_plan_repository::ExplainPlanRepository;
use crate::services::explain_plan_service::ExplainPlanService;
use crate::services::ai_analysis_service::AIAnalysisService;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ExplainPlanFilter {
    pub fingerprint_id: Option<String>,
    pub cluster_id: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateExplainPlanRequest {
    pub fingerprint_id: Uuid,
    pub cluster_id: Uuid,
    pub plan_data: serde_json::Value,
    pub plan_format: String,
    pub execution_time_ms: Option<f64>,
    pub total_cost: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ComparePlansRequest {
    pub plan_id_1: Uuid,
    pub plan_id_2: Uuid,
}

#[derive(Debug, Serialize)]
pub struct ExplainPlanResponse {
    pub plan: ExplainPlan,
}

#[derive(Debug, Serialize)]
pub struct ExplainPlansResponse {
    pub plans: Vec<ExplainPlan>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct PlanComparisonResponse {
    pub plan_1: ExplainPlan,
    pub plan_2: ExplainPlan,
    pub comparison: serde_json::Value,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PlanAnalysisResponse {
    pub plan_id: Uuid,
    pub analysis: serde_json::Value,
    pub recommendations: Vec<String>,
    pub optimization_flags: Vec<String>,
}

pub async fn create_explain_plan(
    req: web::Json<CreateExplainPlanRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let explain_repo = ExplainPlanRepository::new(db_pool.get_ref().clone());
    let explain_service = ExplainPlanService::new(explain_repo);

    let plan = explain_service.create_explain_plan(
        req.fingerprint_id,
        req.cluster_id,
        req.plan_data.clone(),
        req.plan_format.clone(),
        req.execution_time_ms,
        req.total_cost,
    ).await?;

    let response = ExplainPlanResponse { plan };

    Ok(HttpResponse::Created().json(response))
}

pub async fn get_explain_plans(
    query: web::Query<ExplainPlanFilter>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let explain_repo = ExplainPlanRepository::new(db_pool.get_ref().clone());

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

    let plans = if let Some(fingerprint_id) = fingerprint_id {
        explain_repo.find_by_fingerprint(fingerprint_id, limit).await?
    } else if let Some(cluster_id) = cluster_id {
        explain_repo.find_by_cluster(cluster_id, limit).await?
    } else {
        explain_repo.find_recent(limit).await?
    };

    let response = ExplainPlansResponse {
        plans,
        total: plans.len(),
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_explain_plan(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let plan_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let explain_repo = ExplainPlanRepository::new(db_pool.get_ref().clone());

    let plan = explain_repo.find_by_id(plan_id).await?
        .ok_or_else(|| AppError::NotFound(format!("Explain plan not found: {}", plan_id)))?;

    let response = ExplainPlanResponse { plan };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn analyze_explain_plan(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let plan_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let explain_repo = ExplainPlanRepository::new(db_pool.get_ref().clone());
    let explain_service = ExplainPlanService::new(explain_repo);

    let analysis = explain_service.analyze_explain_plan(plan_id).await?;

    let response = PlanAnalysisResponse {
        plan_id,
        analysis: analysis.analysis,
        recommendations: analysis.recommendations,
        optimization_flags: analysis.optimization_flags,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn compare_explain_plans(
    req: web::Json<ComparePlansRequest>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let explain_repo = ExplainPlanRepository::new(db_pool.get_ref().clone());
    let explain_service = ExplainPlanService::new(explain_repo);

    let comparison = explain_service.compare_explain_plans(req.plan_id_1, req.plan_id_2).await?;

    let response = PlanComparisonResponse {
        plan_1: comparison.plan_1,
        plan_2: comparison.plan_2,
        comparison: comparison.comparison,
        recommendations: comparison.recommendations,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_latest_explain_plan(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let fingerprint_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid fingerprint UUID: {}", e)))?;
    let explain_repo = ExplainPlanRepository::new(db_pool.get_ref().clone());

    let plan = explain_repo.find_latest_by_fingerprint(fingerprint_id).await?
        .ok_or_else(|| AppError::NotFound(format!("No explain plan found for fingerprint: {}", fingerprint_id)))?;

    let response = ExplainPlanResponse { plan };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn update_optimization_flags(
    path: web::Path<String>,
    req: web::Json<Vec<String>>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let plan_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let explain_repo = ExplainPlanRepository::new(db_pool.get_ref().clone());
    let explain_service = ExplainPlanService::new(explain_repo);

    explain_service.update_optimization_flags(plan_id, req.into_inner()).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Optimization flags updated successfully",
        "plan_id": plan_id
    })))
}

pub async fn delete_explain_plan(
    path: web::Path<String>,
    db_pool: web::Data<Arc<DatabaseConnection>>,
    _config: web::Data<Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let plan_id = Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?;
    let explain_repo = ExplainPlanRepository::new(db_pool.get_ref().clone());

    explain_repo.delete(plan_id).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Explain plan deleted successfully",
        "plan_id": plan_id
    })))
}