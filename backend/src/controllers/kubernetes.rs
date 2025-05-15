use actix_web::{web, HttpResponse, Responder};
use crate::errors::AppError;
use crate::middleware::auth::Claims;

// Placeholder for kubernetes controller functionality
pub async fn list_clusters(_claims: web::ReqData<Claims>) -> Result<impl Responder, AppError> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Kubernetes clusters API - Not yet implemented"
    })))
}
