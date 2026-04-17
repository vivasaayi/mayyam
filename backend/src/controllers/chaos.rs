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

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::chaos_experiment::{
    BatchRunRequest, ChaosExperimentCreateDto, ChaosExperimentQuery, ChaosExperimentUpdateDto,
    RunExperimentRequest,
};
use crate::models::chaos_template::{ChaosTemplateCreateDto, ChaosTemplateQuery, ChaosTemplateUpdateDto};
use crate::services::chaos_service::ChaosService;

// ============================================================================
// Template Endpoints
// ============================================================================

/// List all chaos experiment templates
pub async fn list_templates(
    service: web::Data<Arc<ChaosService>>,
    query: web::Query<ChaosTemplateQuery>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let templates = service.list_templates(&query).await?;
    Ok(HttpResponse::Ok().json(templates))
}

/// Get a specific template by ID
pub async fn get_template(
    service: web::Data<Arc<ChaosService>>,
    id: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let template = service.get_template(id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(template))
}

/// Create a new experiment template
pub async fn create_template(
    service: web::Data<Arc<ChaosService>>,
    dto: web::Json<ChaosTemplateCreateDto>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let template = service.create_template(dto.into_inner()).await?;
    Ok(HttpResponse::Created().json(template))
}

/// Update an existing template
pub async fn update_template(
    service: web::Data<Arc<ChaosService>>,
    id: web::Path<Uuid>,
    dto: web::Json<ChaosTemplateUpdateDto>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let template = service.update_template(id.into_inner(), dto.into_inner()).await?;
    Ok(HttpResponse::Ok().json(template))
}

/// Delete a template
pub async fn delete_template(
    service: web::Data<Arc<ChaosService>>,
    id: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    service.delete_template(id.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

// ============================================================================
// Experiment Endpoints
// ============================================================================

/// List experiments with optional filtering
pub async fn list_experiments(
    service: web::Data<Arc<ChaosService>>,
    query: web::Query<ChaosExperimentQuery>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let page = service.list_experiments(&query).await?;
    Ok(HttpResponse::Ok().json(page))
}

/// List experiments enriched with latest run info
pub async fn list_experiments_with_runs(
    service: web::Data<Arc<ChaosService>>,
    query: web::Query<ChaosExperimentQuery>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let experiments = service.list_experiments_with_runs(&query).await?;
    Ok(HttpResponse::Ok().json(experiments))
}

/// Get a specific experiment
pub async fn get_experiment(
    service: web::Data<Arc<ChaosService>>,
    id: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let experiment = service.get_experiment(id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(experiment))
}

/// Create a new experiment
pub async fn create_experiment(
    service: web::Data<Arc<ChaosService>>,
    dto: web::Json<ChaosExperimentCreateDto>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let experiment = service.create_experiment(dto.into_inner()).await?;
    Ok(HttpResponse::Created().json(experiment))
}

/// Create experiment from template
#[derive(Deserialize)]
pub struct CreateFromTemplateDto {
    pub account_id: String,
    pub region: String,
    pub target_resource_id: String,
    pub target_resource_name: Option<String>,
    pub parameter_overrides: Option<serde_json::Value>,
}

pub async fn create_experiment_from_template(
    service: web::Data<Arc<ChaosService>>,
    template_id: web::Path<Uuid>,
    dto: web::Json<CreateFromTemplateDto>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let dto = dto.into_inner();
    let experiment = service
        .create_experiment_from_template(
            template_id.into_inner(),
            dto.account_id,
            dto.region,
            dto.target_resource_id,
            dto.target_resource_name,
            dto.parameter_overrides,
        )
        .await?;
    Ok(HttpResponse::Created().json(experiment))
}

/// Update an experiment
pub async fn update_experiment(
    service: web::Data<Arc<ChaosService>>,
    id: web::Path<Uuid>,
    dto: web::Json<ChaosExperimentUpdateDto>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let experiment = service.update_experiment(id.into_inner(), dto.into_inner()).await?;
    Ok(HttpResponse::Ok().json(experiment))
}

/// Delete an experiment
pub async fn delete_experiment(
    service: web::Data<Arc<ChaosService>>,
    id: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    service.delete_experiment(id.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

// ============================================================================
// Execution Endpoints
// ============================================================================

/// Run an experiment
pub async fn run_experiment(
    service: web::Data<Arc<ChaosService>>,
    id: web::Path<Uuid>,
    mut body: web::Json<RunExperimentRequest>,
    claims: web::ReqData<Claims>,
    req: HttpRequest,
) -> Result<impl Responder, AppError> {
    info!("Running chaos experiment: {}", id);

    // Extract user context from claims and request
    body.user_id = Some(claims.sub.clone());
    body.ip_address = req
        .connection_info()
        .peer_addr()
        .map(|s| s.to_string());
    body.user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    let run = service.run_experiment(id.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(run))
}

/// Stop a running experiment
pub async fn stop_experiment(
    service: web::Data<Arc<ChaosService>>,
    id: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    info!("Stopping chaos experiment: {}", id);
    let experiment = service.stop_experiment(id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(experiment))
}

/// Batch run multiple experiments
pub async fn batch_run_experiments(
    service: web::Data<Arc<ChaosService>>,
    body: web::Json<BatchRunRequest>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    info!("Batch running {} experiments", body.experiment_ids.len());
    let runs = service.batch_run_experiments(body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(runs))
}

// ============================================================================
// Run & Results Endpoints
// ============================================================================

/// Get runs for an experiment
pub async fn list_experiment_runs(
    service: web::Data<Arc<ChaosService>>,
    id: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let runs = service.list_runs_for_experiment(id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(runs))
}

/// Get a specific run with results
pub async fn get_run(
    service: web::Data<Arc<ChaosService>>,
    id: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let run = service.get_run_with_results(id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(run))
}

/// Get results for an experiment
pub async fn get_experiment_results(
    service: web::Data<Arc<ChaosService>>,
    id: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let results = service.get_results_for_experiment(id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(results))
}

// ============================================================================
// Resource-centric Endpoints
// ============================================================================

/// Get all experiments for a specific AWS resource
pub async fn get_experiments_for_resource(
    service: web::Data<Arc<ChaosService>>,
    resource_id: web::Path<String>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let experiments = service
        .get_experiments_for_resource(&resource_id.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(experiments))
}

/// Get chaos experiment history for a specific resource
pub async fn get_resource_experiment_history(
    service: web::Data<Arc<ChaosService>>,
    resource_id: web::Path<String>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let history = service
        .get_resource_experiment_history(&resource_id.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(history))
}

// ============================================================================
// Audit Logging Endpoints
// ============================================================================

/// Get audit logs with optional filtering
pub async fn list_audit_logs(
    audit_service: web::Data<Arc<crate::services::chaos_audit_service::ChaosAuditService>>,
    query: web::Query<crate::models::chaos_audit_log::AuditLogQuery>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let page = audit_service.list_audit_logs(&query).await?;
    Ok(HttpResponse::Ok().json(page))
}

/// Get audit trail for a specific experiment
pub async fn get_experiment_audit_trail(
    audit_service: web::Data<Arc<crate::services::chaos_audit_service::ChaosAuditService>>,
    experiment_id: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let logs = audit_service
        .get_experiment_audit_trail(experiment_id.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(logs))
}

/// Get audit trail for a specific run
pub async fn get_run_audit_trail(
    audit_service: web::Data<Arc<crate::services::chaos_audit_service::ChaosAuditService>>,
    run_id: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let logs = audit_service.get_run_audit_trail(run_id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(logs))
}

/// Get user activity
pub async fn get_user_activity(
    audit_service: web::Data<Arc<crate::services::chaos_audit_service::ChaosAuditService>>,
    user_id: web::Path<String>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let logs = audit_service.get_user_activity(&user_id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(logs))
}

// ============================================================================
// Metrics Endpoints
// ============================================================================

/// Get metrics statistics with optional filtering
pub async fn get_metrics_stats(
    metrics_service: web::Data<Arc<crate::services::chaos_metrics_service::ChaosMetricsService>>,
    query: web::Query<crate::models::chaos_metrics::MetricsQuery>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let stats = metrics_service.get_metrics_stats(&query).await?;
    Ok(HttpResponse::Ok().json(stats))
}

/// Get metrics for a specific experiment
pub async fn get_experiment_metrics(
    metrics_service: web::Data<Arc<crate::services::chaos_metrics_service::ChaosMetricsService>>,
    experiment_id: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let metrics = metrics_service
        .get_experiment_summary(experiment_id.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(metrics))
}

/// Get metrics for a specific resource type
pub async fn get_resource_type_metrics(
    metrics_service: web::Data<Arc<crate::services::chaos_metrics_service::ChaosMetricsService>>,
    resource_type: web::Path<String>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let metrics = metrics_service
        .get_resource_type_summary(&resource_type.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(metrics))
}
