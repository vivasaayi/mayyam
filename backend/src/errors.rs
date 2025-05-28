use actix_web::{error::ResponseError, HttpResponse};
use derive_more::Display;
use serde::Serialize;

#[derive(Debug, Display, Serialize)]
pub enum AppError {
    #[display(fmt = "Internal Server Error")]
    InternalError(String),

    #[display(fmt = "Bad Request: {}", _0)]
    BadRequest(String),

    #[display(fmt = "Not Found: {}", _0)]
    NotFound(String),

    #[display(fmt = "Unauthorized: {}", _0)]
    Unauthorized(String),

    #[display(fmt = "External Service Error: {}", _0)]
    ExternalService(String),

    #[display(fmt = "Database Error: {}", _0)]
    DatabaseError(String),

    #[display(fmt = "Validation Error: {}", _0)]
    ValidationError(String),
    // Add other error types as needed
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    message: Option<String>,
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        let (status, error_type, message) = match self {
            AppError::InternalError(ref msg) => (HttpResponse::InternalServerError(), "InternalError", Some(msg.clone())),
            AppError::BadRequest(ref msg) => (HttpResponse::BadRequest(), "BadRequest", Some(msg.clone())),
            AppError::NotFound(ref msg) => (HttpResponse::NotFound(), "NotFound", Some(msg.clone())),
            AppError::Unauthorized(ref msg) => (HttpResponse::Unauthorized(), "Unauthorized", Some(msg.clone())),
            AppError::ExternalService(ref msg) => (HttpResponse::InternalServerError(), "ExternalServiceError", Some(msg.clone())),
            AppError::DatabaseError(ref msg) => (HttpResponse::InternalServerError(), "DatabaseError", Some(msg.clone())),
            AppError::ValidationError(ref msg) => (HttpResponse::BadRequest(), "ValidationError", Some(msg.clone())),
        };
        
        status.json(ErrorResponse {
            error: error_type.to_string(),
            message,
        })
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        match *self {
            AppError::InternalError(_) => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            AppError::BadRequest(_) => actix_web::http::StatusCode::BAD_REQUEST,
            AppError::NotFound(_) => actix_web::http::StatusCode::NOT_FOUND,
            AppError::Unauthorized(_) => actix_web::http::StatusCode::UNAUTHORIZED,
            AppError::ExternalService(_) => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, // Or a more specific 5xx error
            AppError::DatabaseError(_) => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            AppError::ValidationError(_) => actix_web::http::StatusCode::BAD_REQUEST,
        }
    }
}

// Helper for converting other errors to AppError
impl From<kube::Error> for AppError {
    fn from(err: kube::Error) -> AppError {
        AppError::ExternalService(format!("Kubernetes API error: {}", err))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> AppError {
        AppError::InternalError(format!("JSON serialization/deserialization error: {}", err))
    }
}

impl From<sea_orm::DbErr> for AppError {
    fn from(err: sea_orm::DbErr) -> AppError {
        AppError::DatabaseError(format!("Database error: {}", err))
    }
}
