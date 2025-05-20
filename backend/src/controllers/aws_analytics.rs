use std::sync::Arc;
use actix_web::{web, HttpResponse, Responder};
use tracing::{info, error, debug};
use crate::services::aws_analytics::{
    AwsAnalyticsService,
    models::analytics::{AwsResourceAnalysisRequest, AwsResourceAnalysisResponse, ResourceRelatedQuestionRequest},
    models::resource_workflows::AnalysisWorkflowInfo
};
use crate::errors::AppError;

pub struct AwsAnalyticsController {
    aws_analytics_service: Arc<AwsAnalyticsService>,
}

impl AwsAnalyticsController {
    pub fn new(aws_analytics_service: Arc<AwsAnalyticsService>) -> Self {
        Self { aws_analytics_service }
    }

    pub async fn analyze_resource(&self, req: web::Json<AwsResourceAnalysisRequest>) -> Result<impl Responder, AppError> {
        info!("Received request to analyze resource: {:?}", req);
        info!("Request body JSON: {}", serde_json::to_string(&req.0).unwrap_or_default());
        
        // Validate request fields
        if req.resource_id.trim().is_empty() {
            return Err(AppError::BadRequest("Resource ID cannot be empty".to_string()));
        }
        
        if req.workflow.trim().is_empty() {
            return Err(AppError::BadRequest("Workflow cannot be empty".to_string()));
        }
        
        let result = self.aws_analytics_service
            .analyze_resource(&req.into_inner())
            .await?;
            
        info!("Analysis completed successfully");
        Ok(HttpResponse::Ok().json(result))
    }
    
    pub async fn get_resource_workflows(&self, path: web::Path<String>) -> Result<impl Responder, AppError> {
        let resource_type = path.into_inner();
        info!("Controller: Getting workflows for resource type: '{}'", resource_type);
        
        // Handle potential malformed resource types coming from the URL
        if resource_type.is_empty() {
            return Err(AppError::BadRequest("Resource type cannot be empty".to_string()));
        }
        
        let workflows = self.aws_analytics_service
            .get_workflows_for_resource(&resource_type)
            .await?;
            
        info!("Successfully retrieved workflows for resource type: '{}', found {} workflows", 
            resource_type, workflows.workflows.len());
        Ok(HttpResponse::Ok().json(workflows))
    }
    
    pub async fn answer_resource_question(&self, req: web::Json<ResourceRelatedQuestionRequest>) -> Result<impl Responder, AppError> {
        debug!("Answering question about resource: {:?}", req);
        
        let result = self.aws_analytics_service
            .answer_resource_question(&req.into_inner())
            .await?;
            
        Ok(HttpResponse::Ok().json(result))
    }

    // Configure and register the controller's routes
    pub fn configure(cfg: &mut web::ServiceConfig, controller: Arc<AwsAnalyticsController>) {
        let controller1 = controller.clone();
        let controller2 = controller.clone();
        let controller3 = controller;

        info!("Configuring AWS Analytics routes");
        
        cfg.service(
            web::scope("/api/aws/analytics")
                .route("/analyze", web::post().to(move |req: web::Json<AwsResourceAnalysisRequest>| {
                    let controller = controller1.clone();
                    info!("Route registered: POST /api/aws/analytics/analyze");
                    async move { controller.analyze_resource(req).await }
                }))
                .route("/workflows/{resource_type}", web::get().to(move |path: web::Path<String>| {
                    let controller = controller2.clone();
                    let resource_type = path.clone();
                    info!("Route registered: GET /api/aws/analytics/workflows/{{resource_type}}");
                    info!("Resource type from URL path: '{}'", &*resource_type);
                    async move { controller.get_resource_workflows(path).await }
                }))
                .route("/question", web::post().to(move |req: web::Json<ResourceRelatedQuestionRequest>| {
                    let controller = controller3.clone();
                    info!("Route registered: POST /api/aws/analytics/question");
                    async move { controller.answer_resource_question(req).await }
                }))
        );
        
        info!("AWS Analytics routes configured successfully");
    }
}
