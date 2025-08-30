use actix_web::{HttpResponse, ResponseError};
use sea_orm::DbErr;
use thiserror::Error;
use serde::{Serialize, Deserialize};
use tracing::error;
use aws_smithy_http::operation::error::BuildError;
use aws_smithy_http::result::SdkError;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Database error: {0}")]
    Database(#[from] DbErr),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Conflict: {0}")]
    Conflict(String),
    
    #[error("Bad request: {0}")]
    BadRequest(String),
    
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Integration error: {0}")]
    Integration(String),

    #[error("External service error: {0}")]
    ExternalService(String),

    #[error("Cloud provider error: {0}")]
    CloudProvider(String),
    
    #[error("Kubernetes error: {0}")]
    Kubernetes(String),
    
    #[error("Kafka error: {0}")]
    Kafka(String),

    #[error("AI service error: {0}")]
    AI(String),
    
    #[error("Internal server error: {0}")]
    Internal(String),
    
    // Add aliases for compatibility
    #[error("External service error: {0}")]
    ExternalServiceError(String),
    
    #[error("Internal server error: {0}")]
    InternalServerError(String),
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::Auth(_) => {
                HttpResponse::Unauthorized().json(ErrorResponse::new(self))
            }
            AppError::Validation(_) | AppError::Config(_) | AppError::BadRequest(_) => {
                HttpResponse::BadRequest().json(ErrorResponse::new(self))
            }
            AppError::NotFound(_) => {
                HttpResponse::NotFound().json(ErrorResponse::new(self))
            }
            AppError::Conflict(_) => {
                HttpResponse::Conflict().json(ErrorResponse::new(self))
            }
            _ => HttpResponse::InternalServerError().json(ErrorResponse::new(self)),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ErrorResponse {
    error: String,
    message: String,
}

impl ErrorResponse {
    fn new(error: &AppError) -> Self {
        let error_type = match error {
            AppError::Auth(_) => "AUTH_ERROR",
            AppError::Database(_) => "DATABASE_ERROR",
            AppError::Validation(_) => "VALIDATION_ERROR",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Conflict(_) => "CONFLICT_ERROR",
            AppError::BadRequest(_) => "BAD_REQUEST",
            AppError::Config(_) => "CONFIG_ERROR",
            AppError::Integration(_) => "INTEGRATION_ERROR",
            AppError::ExternalService(_) => "EXTERNAL_SERVICE_ERROR",
            AppError::CloudProvider(_) => "CLOUD_PROVIDER_ERROR",
            AppError::Kubernetes(_) => "KUBERNETES_ERROR",
            AppError::Kafka(_) => "KAFKA_ERROR",
            AppError::AI(_) => "AI_ERROR",
            AppError::Internal(_) => "INTERNAL_SERVER_ERROR",
            AppError::ExternalServiceError(_) => "EXTERNAL_SERVICE_ERROR",
            AppError::InternalServerError(_) => "INTERNAL_SERVER_ERROR",
        };

        Self {
            error: error_type.to_string(),
            message: error.to_string(),
        }
    }
}

// Common conversion implementations
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<String> for AppError {
    fn from(err: String) -> Self {
        AppError::Internal(err)
    }
}

impl From<&str> for AppError {
    fn from(err: &str) -> Self {
        AppError::Internal(err.to_string())
    }
}

// Specific conversions for external libraries
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

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::ExternalService(err.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        AppError::Auth(err.to_string())
    }
}

// Generic AWS SDK error handling
impl<E> From<SdkError<E>> for AppError {
    fn from(err: SdkError<E>) -> Self {
        AppError::CloudProvider(err.to_string())
    }
}

impl From<BuildError> for AppError {
    fn from(err: BuildError) -> Self {
        AppError::CloudProvider(err.to_string())
    }
}
