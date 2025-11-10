use crate::controllers::slow_query;
use actix_web::{web, HttpResponse};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub fn configure(cfg: &mut web::ServiceConfig, db: Arc<DatabaseConnection>) {
    let slow_query_controller = slow_query::SlowQueryController::new(db.clone());

    cfg.service(
        web::scope("/api/slow-queries")
            .app_data(web::Data::new(slow_query_controller))
            .service(
                web::resource("")
                    .route(web::get().to(slow_query::get_slow_queries))
                    .route(web::post().to(slow_query::ingest_slow_queries)),
            )
            .service(
                web::resource("/stats")
                    .route(web::get().to(slow_query::get_slow_query_stats)),
            )
            .service(
                web::resource("/{id}")
                    .route(web::get().to(slow_query::get_slow_query))
                    .route(web::delete().to(slow_query::delete_slow_query)),
            ),
    );
}