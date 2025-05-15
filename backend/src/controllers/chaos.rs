use actix_web::{web, HttpResponse, Responder};
use crate::errors::AppError;
use crate::middleware::auth::Claims;

// Placeholder for chaos controller functionality
pub async fn list_experiments(_claims: web::ReqData<Claims>) -> Result<impl Responder, AppError> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Chaos experiments API - Not yet implemented"
    })))
}
