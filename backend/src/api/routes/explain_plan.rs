use crate::controllers::explain_plan;
use actix_web::{web, HttpResponse};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub fn configure(cfg: &mut web::ServiceConfig, db: Arc<DatabaseConnection>) {
    let explain_controller = explain_plan::ExplainPlanController::new(db.clone());

    cfg.service(
        web::scope("/api/explain-plans")
            .app_data(web::Data::new(explain_controller))
            .service(
                web::resource("")
                    .route(web::post().to(explain_plan::create_explain_plan)),
            )
            .service(
                web::resource("/{id}")
                    .route(web::get().to(explain_plan::get_explain_plan)),
            )
            .service(
                web::resource("/{id}/analysis")
                    .route(web::get().to(explain_plan::analyze_explain_plan)),
            )
            .service(
                web::resource("/compare")
                    .route(web::post().to(explain_plan::compare_explain_plans)),
            ),
    );
}