use actix_web::{web, HttpResponse, Responder};
use prometheus::{Encoder, TextEncoder};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/metrics", web::get().to(metrics_handler));
}

async fn metrics_handler() -> impl Responder {
    let encoder = TextEncoder::new();
    let mut buffer = vec![];
    let metric_families = prometheus::gather();
    
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        tracing::error!("Failed to encode prometheus metrics: {}", e);
        return HttpResponse::InternalServerError().body(format!("Error encoding metrics: {}", e));
    }
    
    let res = String::from_utf8(buffer).unwrap_or_else(|_| String::new());
    HttpResponse::Ok()
        .content_type(encoder.format_type())
        .body(res)
}
