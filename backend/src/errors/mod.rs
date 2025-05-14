use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;
use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Authentication error: {0}")]
    Auth(String),
    
    #[error("Authorization error: {0}")]
    Authorization(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Kafka error: {0}")]
    Kafka(String),
    
    #[error("Cloud provider error: {0}")]
    Cloud(String),
    
    #[error("Kubernetes error: {0}")]
    Kubernetes(String),
    
    #[error("Internal server error: {0}")]
    Internal(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("API error: {0}")]
    Api(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    message: String,
    error_type: String,
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        let (status_code, error_type) = match self {
            AppError::Auth(_) => (actix_web::http::StatusCode::UNAUTHORIZED, "AUTH_ERROR"),
            AppError::Authorization(_) => (actix_web::http::StatusCode::FORBIDDEN, "AUTHORIZATION_ERROR"),
            AppError::Validation(_) => (actix_web::http::StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
            AppError::NotFound(_) => (actix_web::http::StatusCode::NOT_FOUND, "NOT_FOUND_ERROR"),
            AppError::Database(_) => (actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR"),
            AppError::Kafka(_) => (actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "KAFKA_ERROR"),
            AppError::Cloud(_) => (actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "CLOUD_ERROR"),
            AppError::Kubernetes(_) => (actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "KUBERNETES_ERROR"),
            AppError::Internal(_) => (actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
            AppError::Api(_) => (actix_web::http::StatusCode::BAD_REQUEST, "API_ERROR"),
        };
        
        let error_response = ErrorResponse {
            code: status_code.as_u16(),
            message: self.to_string(),
            error_type: error_type.to_string(),
        };
        
        HttpResponse::build(status_code).json(error_response)
    }
}

// Conversion from various error types to AppError
impl From<sea_orm::DbErr> for AppError {
    fn from(err: sea_orm::DbErr) -> Self {
        AppError::Database(err.to_string())
    }
}

impl From<rdkafka::error::KafkaError> for AppError {
    fn from(err: rdkafka::error::KafkaError) -> Self {
        AppError::Kafka(err.to_string())
    }
}

impl From<kube::Error> for AppError {
    fn from(err: kube::Error) -> Self {
        AppError::Kubernetes(err.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        AppError::Auth(err.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Api(err.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}
