use actix_web::{web, HttpResponse, Responder};
use jsonwebtoken::{encode, Header, EncodingKey};
use chrono::{Utc, Duration};
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::api::routes::auth::{LoginRequest, RegisterRequest, TokenResponse};
use uuid::Uuid;

pub async fn login(
    login: web::Json<LoginRequest>,
    config: web::Data<crate::config::Config>,
) -> Result<impl Responder, AppError> {
    // In a real application, verify the credentials against a database
    // For now, we'll just create a token if the username is "admin" and password is "password"
    
    if login.username != "admin" || login.password != "password" {
        return Err(AppError::Auth("Invalid username or password".to_string()));
    }
    
    let now = Utc::now();
    let expiration = now + Duration::hours(24);
    
    let claims = Claims {
        sub: login.username.clone(),
        exp: expiration.timestamp() as usize,
        iat: now.timestamp() as usize,
        user_id: Uuid::new_v4().to_string(),
        role: "admin".to_string(),
    };
    
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.auth.jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Auth(format!("Failed to generate token: {}", e)))?;
    
    Ok(HttpResponse::Ok().json(TokenResponse {
        token,
        token_type: "Bearer".to_string(),
        expires_in: config.auth.jwt_expiration,
    }))
}

pub async fn register(
    _register: web::Json<RegisterRequest>,
    _config: web::Data<crate::config::Config>,
) -> Result<impl Responder, AppError> {
    // In a real application, register the user in a database
    // For now, we'll just return an error
    
    Err(AppError::Api("User registration not implemented yet".to_string()))
}

pub async fn refresh_token(
    _config: web::Data<crate::config::Config>,
    claims: Option<web::ReqData<Claims>>,
) -> Result<impl Responder, AppError> {
    if claims.is_none() {
        return Err(AppError::Auth("No valid token provided".to_string()));
    }
    
    // In a real application, we'd generate a new token based on the current user
    // For now, we'll just return an error
    
    Err(AppError::Api("Token refresh not implemented yet".to_string()))
}

pub async fn get_current_user(
    claims: Option<web::ReqData<Claims>>,
) -> Result<impl Responder, AppError> {
    match claims {
        Some(user_claims) => {
            Ok(HttpResponse::Ok().json(user_claims.into_inner()))
        },
        None => {
            Err(AppError::Auth("No valid token provided".to_string()))
        }
    }
}
