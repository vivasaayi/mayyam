use actix_web::{web, HttpResponse, Responder};
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use std::sync::Arc;
use tracing::info;

use crate::models::analytics::{
    AwsResourceAnalysisRequest, 
    AwsResourceAnalysisResponse,
    ResourceRelatedQuestionRequest
};
use crate::services::aws_analytics::AwsAnalyticsService;

pub async fn analyze_resource(
    req: web::Json<AwsResourceAnalysisRequest>,
    analytics_service: web::Data<Arc<AwsAnalyticsService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    info!(
        "Analyzing resource {} with workflow {}", 
        req.resource_id, 
        req.workflow
    );

    let result = analytics_service.analyze_resource(&req).await?;
    Ok(HttpResponse::Ok().json(result))
}

pub async fn get_resource_workflows(
    path: web::Path<String>,
    analytics_service: web::Data<Arc<AwsAnalyticsService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let resource_type = path.into_inner();
    
    info!("Getting analysis workflows for resource type {}", resource_type);
    
    let workflows = analytics_service.get_workflows_for_resource(&resource_type).await?;
    Ok(HttpResponse::Ok().json(workflows))
}

pub async fn answer_resource_question(
    req: web::Json<ResourceRelatedQuestionRequest>,
    analytics_service: web::Data<Arc<AwsAnalyticsService>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    info!(
        "Answering question about resource {}: {}", 
        req.resource_id,
        req.question
    );

    let answer = analytics_service.answer_resource_question(&req).await?;
    Ok(HttpResponse::Ok().json(answer))
}
