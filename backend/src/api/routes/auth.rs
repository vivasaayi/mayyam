use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::controllers::auth::AuthController;
use crate::models::user::{LoginRequest, RegisterRequest, UserInfo};
use crate::middleware::auth::Claims;

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenResponse {
    pub token: String,
    pub token_type: String,
    pub expires_in: u64,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    let scope = web::scope("/api/auth")
        .route("/login", web::post().to(login))
        .route("/register", web::post().to(register))
        .route("/profile", web::get().to(get_profile));
    
    cfg.service(scope);
}

async fn login(
    login_data: web::Json<LoginRequest>,
    auth_controller: web::Data<Arc<AuthController>>,
) -> HttpResponse {
    match auth_controller.login(login_data).await {
        Ok(response) => response,
        Err(e) => e.error_response(),
    }
}

async fn register(
    register_data: web::Json<RegisterRequest>,
    auth_controller: web::Data<Arc<AuthController>>,
) -> HttpResponse {
    match auth_controller.register(register_data).await {
        Ok(response) => response,
        Err(e) => e.error_response(),
    }
}

async fn get_profile(
    req: HttpRequest,
    auth_controller: web::Data<Arc<AuthController>>,
) -> HttpResponse {
    // Extract user claims from request extensions (set by auth middleware)
    let user_info = match req.extensions().get::<Claims>() {
        Some(claims) => UserInfo {
            id: claims.sub.clone(),
            username: claims.username.clone(),
            email: claims.email.clone().unwrap_or_default(),
            roles: claims.roles.clone(),
            first_name: None,
            last_name: None,
        },
        None => return HttpResponse::Unauthorized().json(serde_json::json!({
            "status": "401",
            "message": "Not authenticated"
        })),
    };
    
    match auth_controller.get_current_user(user_info).await {
        Ok(response) => response,
        Err(e) => e.error_response(),
    }
}
