use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    error::Error,
    HttpMessage,
};
use futures_util::future::{ready, Ready, LocalBoxFuture};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm, EncodingKey, encode, Header};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use tracing::error;

use crate::config::Config;
use crate::errors::AppError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub email: Option<String>,
    pub roles: Vec<String>,
    pub exp: i64,
    pub iat: i64,
}

pub struct AuthMiddleware {
    jwt_secret: String,
    public_paths: Vec<String>,
}

impl AuthMiddleware {
    pub fn new(config: &Config) -> Self {
        Self {
            jwt_secret: config.auth.jwt_secret.clone(),
            public_paths: vec![
                "/health".to_string(),
                "/api/auth/login".to_string(),
                "/api/auth/register".to_string(),
            ],
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService {
            service,
            jwt_secret: self.jwt_secret.clone(),
            public_paths: self.public_paths.clone(),
        }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: S,
    jwt_secret: String,
    public_paths: Vec<String>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let path = req.path().to_string();
        let method = req.method().clone();
        
        // Skip auth for public paths
        if self.public_paths.iter().any(|p| path.starts_with(p)) {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            });
        }
        
        // Skip auth for OPTIONS requests (CORS preflight)
        if method == actix_web::http::Method::OPTIONS {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            });
        }

        let auth_header = req.headers().get("Authorization");
        
        if let Some(auth_value) = auth_header {
            if let Ok(auth_str) = auth_value.to_str() {
                if auth_str.starts_with("Bearer ") {
                    let token = auth_str[7..].to_string(); // Remove "Bearer " prefix
                    
                    // Validate JWT token
                    let token_data = match decode::<Claims>(
                        &token,
                        &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
                        &Validation::default(),
                    ) {
                        Ok(data) => data,
                        Err(err) => {
                            error!("JWT validation error for path {}: {:?}", path, err);
                            return Box::pin(async move {
                                Err(AppError::Auth(format!("Invalid token: {}", err)).into())
                            });
                        }
                    };

                    // Check token expiration
                    let now = Utc::now().timestamp();
                    if token_data.claims.exp < now {
                        error!("Token expired for path {}: exp={}, now={}", path, token_data.claims.exp, now);
                        return Box::pin(async move {
                            Err(AppError::Auth("Token expired".to_string()).into())
                        });
                    }

                    // Store user info in request extensions
                    req.extensions_mut().insert(token_data.claims);

                    let fut = self.service.call(req);
                    return Box::pin(async move {
                        let res = fut.await?;
                        Ok(res)
                    });
                }
            }
        }

        // No valid token found
        Box::pin(async move {
            Err(AppError::Auth("Authorization required".to_string()).into())
        })
    }
}

// Helper functions for generating and validating JWTs
pub fn generate_token(
    user_id: &str, 
    username: &str, 
    email: Option<&str>, 
    roles: Vec<String>,
    config: &Config
) -> Result<String, AppError> {
    let jwt_secret = &config.auth.jwt_secret;
    let expiration = config.auth.jwt_expiration;
    
    let now = Utc::now();
    let exp = now + Duration::seconds(expiration as i64);
    
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        email: email.map(|e| e.to_string()),
        roles,
        exp: exp.timestamp(),
        iat: now.timestamp(),
    };
    
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|e| {
        error!("Failed to generate JWT token: {}", e);
        AppError::Internal("Failed to generate authentication token".to_string())
    })
}

pub fn validate_token(token: &str, config: &Config) -> Result<Claims, AppError> {
    let jwt_secret = &config.auth.jwt_secret;
    let validation = Validation::new(Algorithm::HS256);
    
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|e| {
        error!("JWT validation error: {}", e);
        AppError::Auth("Invalid token".to_string())
    })?;
    
    Ok(token_data.claims)
}
