use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/chaos")
            .route("/experiments", web::get().to(|| async { "List chaos experiments" }))
            .route("/experiments", web::post().to(|| async { "Create chaos experiment" }))
            .route("/experiments/{id}", web::get().to(|| async { "Get experiment details" }))
            .route("/experiments/{id}/run", web::post().to(|| async { "Run chaos experiment" }))
            .route("/experiments/{id}/stop", web::post().to(|| async { "Stop chaos experiment" }))
            .route("/history", web::get().to(|| async { "Get experiment history" }))
            .route("/templates", web::get().to(|| async { "List experiment templates" }))
            .route("/templates", web::post().to(|| async { "Create experiment template" }))
            .route("/templates/{id}", web::get().to(|| async { "Get template details" }))
    );
}
