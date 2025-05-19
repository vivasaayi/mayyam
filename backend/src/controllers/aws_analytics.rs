use std::sync::Arc;
use actix_web::{web, HttpResponse, Responder};
use tracing::{info, error, debug};
use crate::services::aws_analytics::{
    AwsAnalyticsService,
    models::analytics::{AwsResourceAnalysisRequest, AwsResourceAnalysisResponse}
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
        debug!("Received request to analyze resource: {:?}", req);
        
        let result = self.aws_analytics_service
            .analyze_resource(&req.into_inner())
            .await?;
            
        Ok(HttpResponse::Ok().json(result))
    }

    // Configure and register the controller's routes
    pub fn configure(cfg: &mut web::ServiceConfig, controller: Arc<AwsAnalyticsController>) {
        cfg.service(
            web::scope("/api/aws/analytics")
                .route("/analyze", web::post().to(move |req: web::Json<AwsResourceAnalysisRequest>| {
                    let controller = controller.clone();
                    async move { controller.analyze_resource(req).await }
                }))
        );
    }
}
