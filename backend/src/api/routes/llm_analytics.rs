use actix_web::{web, HttpResponse, Result};
use std::sync::Arc;
use uuid::Uuid;

use crate::controllers::llm_analytics::{
    AnalyticsRequest, BatchAnalyticsRequest, LlmAnalyticsController, ResourceAnalyticsRequest,
};

pub fn configure(cfg: &mut web::ServiceConfig, controller: Arc<LlmAnalyticsController>) {
    cfg.service(
        web::scope("/api/v1/llm-analytics")
            .app_data(web::Data::new(controller))
            .route("", web::post().to(execute_analytics))
            .route("/resource", web::post().to(analyze_resource))
            .route("/batch", web::post().to(execute_batch_analytics))
            .route("/history", web::get().to(get_analytics_history))
            .route(
                "/analysis-types/{resource_type}",
                web::get().to(get_analysis_types),
            )
            .route("/metrics", web::get().to(get_analytics_metrics))
            .route("/{analysis_id}/cancel", web::post().to(cancel_analytics)),
    );
}

async fn execute_analytics(
    controller: web::Data<LlmAnalyticsController>,
    request: web::Json<AnalyticsRequest>,
) -> Result<HttpResponse> {
    controller.execute_analytics(request).await
}

async fn analyze_resource(
    controller: web::Data<LlmAnalyticsController>,
    request: web::Json<ResourceAnalyticsRequest>,
) -> Result<HttpResponse> {
    controller.analyze_resource(request).await
}

async fn execute_batch_analytics(
    controller: web::Data<LlmAnalyticsController>,
    request: web::Json<BatchAnalyticsRequest>,
) -> Result<HttpResponse> {
    controller.execute_batch_analytics(request).await
}

async fn get_analytics_history(
    controller: web::Data<LlmAnalyticsController>,
    query: web::Query<serde_json::Value>,
) -> Result<HttpResponse> {
    controller.get_analytics_history(query).await
}

async fn get_analysis_types(
    controller: web::Data<LlmAnalyticsController>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    controller.get_analysis_types(path).await
}

async fn get_analytics_metrics(
    controller: web::Data<LlmAnalyticsController>,
) -> Result<HttpResponse> {
    controller.get_analytics_metrics().await
}

async fn cancel_analytics(
    controller: web::Data<LlmAnalyticsController>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    controller.cancel_analytics(path).await
}
