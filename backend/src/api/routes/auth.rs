use actix_web::{web, HttpMessage, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::controllers::auth::AuthController;
use crate::models::user::{LoginUserDto, CreateUserDto};
use crate::models::user::Claims;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub email: String,
    pub roles: Vec<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
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
    // Map the web request to the expected DTO
    let login_dto = LoginUserDto {
        username: login_data.username.clone(),
        password: login_data.password.clone(),
    };

    match auth_controller.login(login_dto).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => HttpResponse::InternalServerError().json(e.to_string()),
    }
}

async fn register(
    register_data: web::Json<RegisterRequest>,
    auth_controller: web::Data<Arc<AuthController>>,
) -> HttpResponse {
    // Map the web request to the expected DTO
    let create_dto = CreateUserDto {
        username: register_data.username.clone(),
        email: register_data.email.clone(),
        password: register_data.password.clone(),
        first_name: None,
        last_name: None,
        is_admin: None,
        permissions: None,
    };

    match auth_controller.register(create_dto).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => HttpResponse::InternalServerError().json(e.to_string()),
    }
}

async fn get_profile(
    req: HttpRequest,
    _auth_controller: web::Data<Arc<AuthController>>, // Prefix with underscore to ignore unused variable
) -> HttpResponse {
    // Extract user claims from request extensions (set by auth middleware)
    if let Some(claims) = req.extensions().get::<Claims>().cloned() {
        // Create a user info response directly from claims
        let user_info = UserInfo {
            id: claims.sub.to_string(),
            username: claims.username,
            email: String::new(), // Using empty string as we don't have email in these claims
            roles: claims.permissions, // Using permissions as roles
            first_name: None,
            last_name: None,
        };
        
        return HttpResponse::Ok().json(user_info);
    }
    
    HttpResponse::Unauthorized().json(serde_json::json!({
        "status": "401",
        "message": "Not authenticated"
    }))
    

}
