use actix_web::{web, HttpResponse, Responder};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use std::sync::Arc;
use tracing::{error, info};

use crate::config::Config;
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::user::{AuthTokenResponse, CreateUserDto, LoginUserDto, UserResponse};
use crate::services::user::UserService;

pub struct AuthController {
    user_service: Arc<UserService>,
    config: Config,
}

impl AuthController {
    pub fn new(user_service: Arc<UserService>, config: Config) -> Self {
        Self {
            user_service,
            config,
        }
    }

    pub async fn login(&self, login_data: LoginUserDto) -> Result<AuthTokenResponse, AppError> {
        // Verify credentials using the service layer
        let user = match self.user_service.authenticate_user(&login_data).await? {
            Some(user) => user,
            None => return Err(AppError::Auth("Invalid username or password".to_string())),
        };

        // Generate JWT token
        let now = Utc::now();
        let expiration = now + Duration::seconds(self.config.auth.jwt_expiration as i64);

        let claims = Claims {
            sub: user.id.to_string(),
            username: user.username.clone(),
            email: Some(user.email.clone()),
            roles: user.permissions.clone(),
            exp: expiration.timestamp(),
            iat: now.timestamp(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.auth.jwt_secret.as_bytes()),
        )
        .map_err(|e| {
            error!("Failed to generate JWT token: {}", e);
            AppError::Internal("Failed to generate authentication token".to_string())
        })?;

        // Return token response
        let user_response = UserResponse::from(user);

        Ok(AuthTokenResponse {
            token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.auth.jwt_expiration as i64,
            user: user_response,
        })
    }

    pub async fn register(&self, user_data: CreateUserDto) -> Result<UserResponse, AppError> {
        // Create new user using the service layer
        let user = self.user_service.create_user(&user_data).await?;

        // Convert to response
        let user_response = UserResponse::from(user);

        Ok(user_response)
    }
}
