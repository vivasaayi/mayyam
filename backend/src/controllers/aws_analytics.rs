use crate::errors::AppError;
use crate::services::analytics::aws_analytics::aws_analytics::AwsAnalyticsService;
use crate::services::analytics::aws_analytics::models::analytics::{
    AwsResourceAnalysisRequest, AwsResourceAnalysisResponse, ResourceRelatedQuestionRequest,
};
use crate::services::analytics::aws_analytics::models::resource_workflows::AnalysisWorkflowInfo;
use actix_web::{web, HttpResponse, Responder};
use serde_json;
use std::sync::Arc;
use tracing::{debug, error, info};

pub struct AwsAnalyticsController {
    aws_analytics_service: Arc<AwsAnalyticsService>,
}

impl AwsAnalyticsController {
    pub fn new(aws_analytics_service: Arc<AwsAnalyticsService>) -> Self {
        Self {
            aws_analytics_service,
        }
    }

    pub async fn analyze_resource(
        &self,
        req: web::Json<AwsResourceAnalysisRequest>,
    ) -> Result<impl Responder, AppError> {
        info!("Received request to analyze resource: {:?}", req);
        info!(
            "Request body JSON: {}",
            serde_json::to_string(&req.0).unwrap_or_default()
        );

        // Extract the inner value
        let request = req.into_inner();

        // Validate request fields
        if request.resource_id.trim().is_empty() {
            return Err(AppError::BadRequest(
                "Resource ID cannot be empty".to_string(),
            ));
        }

        if request.workflow.trim().is_empty() {
            return Err(AppError::BadRequest("Workflow cannot be empty".to_string()));
        }

        let result = self
            .aws_analytics_service
            .analyze_resource(&request)
            .await?;

        info!("Analysis completed successfully");
        Ok(HttpResponse::Ok().json(result))
    }

    pub async fn analyze_resource_html(
        &self,
        req: web::Json<AwsResourceAnalysisRequest>,
    ) -> Result<impl Responder, AppError> {
        info!("Received HTML analysis request for resource: {:?}", req);

        let request = req.into_inner();

        // Validate request fields
        if request.resource_id.trim().is_empty() {
            return Err(AppError::BadRequest(
                "Resource ID cannot be empty".to_string(),
            ));
        }

        if request.workflow.trim().is_empty() {
            return Err(AppError::BadRequest("Workflow cannot be empty".to_string()));
        }

        let result = self
            .aws_analytics_service
            .analyze_resource(&request)
            .await?;

        // Convert analysis result to structured data for HTML generation
        let analysis_data = serde_json::json!({
            "resource_id": request.resource_id,
            "resource_type": request.workflow,
            "analysis_type": request.workflow,
            "time_range": request.time_range,
            "insights": self.extract_insights_from_content(&result.content),
            "recommendations": self.extract_recommendations_from_content(&result.content),
            "raw_response": result.content
        });

        // Generate HTML
        let html_content = crate::utils::html_generator::HtmlGenerator::generate_analysis_html(
            &format!("AWS {} Analysis", request.workflow),
            &analysis_data,
            chrono::Utc::now(),
        );

        info!("HTML analysis completed successfully");
        Ok(HttpResponse::Ok()
            .content_type("text/html")
            .body(html_content))
    }

    pub async fn get_resource_workflows(
        &self,
        path: web::Path<String>,
    ) -> Result<impl Responder, AppError> {
        let resource_type = path.into_inner();
        info!(
            "Controller: Getting workflows for resource type: '{}'",
            resource_type
        );

        // Handle potential malformed resource types coming from the URL
        if resource_type.is_empty() {
            return Err(AppError::BadRequest(
                "Resource type cannot be empty".to_string(),
            ));
        }

        let workflows = self
            .aws_analytics_service
            .get_workflows_for_resource(&resource_type)
            .await?;

        info!(
            "Successfully retrieved workflows for resource type: '{}', found {} workflows",
            resource_type,
            workflows.workflows.len()
        );
        Ok(HttpResponse::Ok().json(workflows))
    }

    pub async fn answer_resource_question(
        &self,
        req: web::Json<ResourceRelatedQuestionRequest>,
    ) -> Result<impl Responder, AppError> {
        debug!("Answering question about resource: {:?}", req);

        // Extract the inner value
        let request = req.into_inner();

        let result = self
            .aws_analytics_service
            .answer_resource_question(&request)
            .await?;

        Ok(HttpResponse::Ok().json(result))
    }

    fn extract_insights_from_content(&self, content: &str) -> Vec<String> {
        content
            .lines()
            .filter(|line| line.contains("**") || line.starts_with("- "))
            .map(|line| line.trim_start_matches("- ").to_string())
            .collect()
    }

    fn extract_recommendations_from_content(&self, content: &str) -> Vec<String> {
        content
            .lines()
            .filter(|line| {
                line.to_lowercase().contains("recommend") || line.to_lowercase().contains("action")
            })
            .map(|line| line.to_string())
            .collect()
    }

    // Configure and register the controller's routes
    pub fn configure(cfg: &mut web::ServiceConfig, controller: Arc<AwsAnalyticsController>) {
        let controller1 = controller.clone();
        let controller2 = controller.clone();
        let controller3 = controller.clone();
        let controller4 = controller;

        info!("Configuring AWS Analytics routes");

        cfg.service(
            web::scope("/api/aws/analytics")
                .route(
                    "/analyze",
                    web::post().to(move |req: web::Json<AwsResourceAnalysisRequest>| {
                        let controller = controller1.clone();
                        info!("Route registered: POST /api/aws/analytics/analyze");
                        async move { controller.analyze_resource(req).await }
                    }),
                )
                .route(
                    "/analyze/html",
                    web::post().to(move |req: web::Json<AwsResourceAnalysisRequest>| {
                        let controller = controller2.clone();
                        info!("Route registered: POST /api/aws/analytics/analyze/html");
                        async move { controller.analyze_resource_html(req).await }
                    }),
                )
                .route(
                    "/workflows/{resource_type}",
                    web::get().to(move |path: web::Path<String>| {
                        let controller = controller3.clone();
                        let resource_type = path.clone();
                        info!(
                            "Route registered: GET /api/aws/analytics/workflows/{{resource_type}}"
                        );
                        info!("Resource type from URL path: '{}'", &*resource_type);
                        async move { controller.get_resource_workflows(path).await }
                    }),
                )
                .route(
                    "/question",
                    web::post().to(move |req: web::Json<ResourceRelatedQuestionRequest>| {
                        let controller = controller4.clone();
                        info!("Route registered: POST /api/aws/analytics/question");
                        async move { controller.answer_resource_question(req).await }
                    }),
                ),
        );

        info!("AWS Analytics routes configured successfully");
    }
}
