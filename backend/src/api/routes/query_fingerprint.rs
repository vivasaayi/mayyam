use crate::controllers::query_fingerprint;
use actix_web::{web, HttpResponse};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub fn configure(cfg: &mut web::ServiceConfig, db: Arc<DatabaseConnection>) {
    let fingerprint_controller = query_fingerprint::QueryFingerprintController::new(db.clone());

    cfg.service(
        web::scope("/api/query-fingerprints")
            .app_data(web::Data::new(fingerprint_controller))
            .service(
                web::resource("")
                    .route(web::get().to(query_fingerprint::get_fingerprints)),
            )
            .service(
                web::resource("/{id}")
                    .route(web::get().to(query_fingerprint::get_fingerprint)),
            )
            .service(
                web::resource("/{id}/analysis")
                    .route(web::get().to(query_fingerprint::analyze_fingerprint)),
            )
            .service(
                web::resource("/patterns/{cluster_id}")
                    .route(web::get().to(query_fingerprint::get_fingerprint_patterns)),
            ),
    );
}