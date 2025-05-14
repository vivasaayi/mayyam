use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use futures::future::{ready, LocalBoxFuture, Ready};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::config::Config;
use crate::errors::AppError;

pub struct AuthMiddleware {
    pub config: Arc<Config>,
}

impl AuthMiddleware {
    pub fn new(config: &Config) -> Self {
        Self {
            config: Arc::new(config.clone()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub user_id: String,
    pub role: String,
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService {
            service: Rc::new(service),
            config: self.config.clone(),
        }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: Rc<S>,
    config: Arc<Config>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        let config = self.config.clone();
        
        // Skip authentication for certain paths
        if should_skip_auth(&req) {
            return Box::pin(async move {
                let res = service.call(req).await?;
                Ok(res)
            });
        }

        // Extract token from Authorization header
        let token = match extract_token(&req) {
            Some(token) => token,
            None => {
                return Box::pin(async move {
                    Err(AppError::Auth("No authorization token provided".to_string()).into())
                });
            }
        };

        // Validate token
        let claims = match validate_token(token, &config.auth.jwt_secret) {
            Ok(claims) => claims,
            Err(e) => {
                return Box::pin(async move {
                    Err(AppError::Auth(format!("Invalid token: {}", e)).into())
                });
            }
        };

        // Set user claims in request extensions
        req.extensions_mut().insert(claims);

        Box::pin(async move {
            let res = service.call(req).await?;
            Ok(res)
        })
    }
}

fn should_skip_auth(req: &ServiceRequest) -> bool {
    let path = req.path();
    path.starts_with("/api/auth/login") || 
    path.starts_with("/api/auth/register") || 
    path.starts_with("/api/health") ||
    path.starts_with("/api/auth/saml")
}

fn extract_token(req: &ServiceRequest) -> Option<&str> {
    req.headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|auth_header| {
            if auth_header.starts_with("Bearer ") {
                Some(&auth_header[7..])
            } else {
                None
            }
        })
}

fn validate_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|e| AppError::Auth(format!("Token validation failed: {}", e)))?;

    Ok(token_data.claims)
}
