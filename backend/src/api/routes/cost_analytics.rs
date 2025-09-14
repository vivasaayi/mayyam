use actix_web::{web, HttpResponse};
use std::sync::Arc;

use crate::controllers::cost_analytics;
use crate::services::aws_cost_analytics::AwsCostAnalyticsService;
use crate::repositories::cost_analytics::CostAnalyticsRepository;

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/cost-analytics")
            .route("/fetch", web::get().to(cost_analytics::fetch_cost_data))
            .route("/aggregates", web::get().to(cost_analytics::get_monthly_aggregates))
            .route("/top-services", web::get().to(cost_analytics::get_top_services))
            .route("/anomalies", web::get().to(cost_analytics::get_cost_anomalies))
            .route("/insights", web::get().to(cost_analytics::get_cost_insights))
            .route("/compute-aggregates", web::post().to(cost_analytics::compute_monthly_aggregates))
            .route("/summary", web::get().to(cost_analytics::get_cost_summary))
            .route("/analyze/{anomaly_id}", web::post().to(cost_analytics::analyze_cost_with_llm))
            .route("/health", web::get().to(health_check))
    );
}

async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "cost-analytics",
        "message": "AWS Cost Analytics API is running"
    }))
}
