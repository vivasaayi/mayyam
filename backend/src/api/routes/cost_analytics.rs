use actix_web::{web, HttpResponse};
use std::sync::Arc;

use crate::controllers::cost_analytics;
use crate::repositories::aws_account::AwsAccountRepository;
use crate::repositories::aws_resource::AwsResourceRepository;
use crate::repositories::cost_analytics::CostAnalyticsRepository;
use crate::services::aws_cost_analytics::AwsCostAnalyticsService;

pub fn configure_routes(
    cfg: &mut web::ServiceConfig,
    aws_account_repo: Arc<AwsAccountRepository>,
    aws_resource_repo: Arc<AwsResourceRepository>,
) {
    let aws_account_repo_data = web::Data::new(aws_account_repo);
    let aws_resource_repo_data = web::Data::new(aws_resource_repo);

    cfg.service(
        web::scope("/api/cost-analytics")
            .app_data(aws_account_repo_data)
            .app_data(aws_resource_repo_data)
            .route("/fetch", web::get().to(cost_analytics::fetch_cost_data))
            .route(
                "/aggregates",
                web::get().to(cost_analytics::get_monthly_aggregates),
            )
            .route(
                "/top-services",
                web::get().to(cost_analytics::get_top_services),
            )
            .route(
                "/anomalies",
                web::get().to(cost_analytics::get_cost_anomalies),
            )
            .route(
                "/insights",
                web::get().to(cost_analytics::get_cost_insights),
            )
            .route(
                "/compute-aggregates",
                web::post().to(cost_analytics::compute_monthly_aggregates),
            )
            .route("/summary", web::get().to(cost_analytics::get_cost_summary))
            .route("/resources", web::get().to(cost_analytics::get_resource_costs))
            .route("/new-resources", web::get().to(cost_analytics::get_new_resources))
            .route("/resource-trends", web::get().to(cost_analytics::get_resource_count_trends))
            .route("/cost-increases", web::get().to(cost_analytics::detect_cost_increases))
            .route("/resource-cost-history", web::get().to(cost_analytics::get_resource_cost_history))
            .route(
                "/analyze/{anomaly_id}",
                web::post().to(cost_analytics::analyze_cost_with_llm),
            )
            .route("/health", web::get().to(health_check)),
    );
}

async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "cost-analytics",
        "message": "AWS Cost Analytics API is running"
    }))
}
