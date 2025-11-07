use actix_web::{web, HttpResponse};
use std::sync::Arc;

use crate::controllers::budget;
use crate::repositories::cost_budget_repository::CostBudgetRepository;
use crate::services::budget_service::BudgetService;

pub fn configure_routes(
    cfg: &mut web::ServiceConfig,
    cost_budget_repo: Arc<CostBudgetRepository>,
) {
    let budget_service = Arc::new(BudgetService::new(cost_budget_repo.get_db().clone()));
    let budget_service_data = web::Data::new(budget_service);

    cfg.service(
        web::scope("/api/budgets")
            .app_data(budget_service_data)
            .route("/{account_id}", web::post().to(budget::create_budget))
            .route("/{account_id}", web::get().to(budget::get_budgets))
            .route("/{account_id}/{budget_id}", web::get().to(budget::get_budget))
            .route("/{account_id}/{budget_id}", web::put().to(budget::update_budget))
            .route("/{account_id}/{budget_id}", web::delete().to(budget::delete_budget))
            .route("/{account_id}/{budget_id}/status", web::get().to(budget::get_budget_status))
            .route("/{account_id}/alerts", web::get().to(budget::get_budget_alerts))
            .route("/health", web::get().to(health_check)),
    );
}

async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "budget",
        "message": "Budget Management API is running"
    }))
}