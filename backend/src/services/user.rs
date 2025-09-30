use chrono::Utc;
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::user::{CreateUserDto, LoginUserDto, Model as UserModel, UpdateUserDto};
use crate::repositories::user::UserRepository;

pub struct UserService {
    user_repository: Arc<UserRepository>,
}

impl UserService {
    pub fn new(user_repository: Arc<UserRepository>) -> Self {
        Self { user_repository }
    }

    pub async fn get_user_by_id(&self, id: Uuid) -> Result<Option<UserModel>, AppError> {
        self.user_repository.find_by_id(id).await
    }

    pub async fn get_user_by_username(
        &self,
        username: &str,
    ) -> Result<Option<UserModel>, AppError> {
        self.user_repository.find_by_username(username).await
    }

    pub async fn create_user(&self, user_data: &CreateUserDto) -> Result<UserModel, AppError> {
        // Business logic can be added here
        // For example, validate password strength, enforce organization policies, etc.
        if user_data.password.len() < 8 {
            return Err(AppError::Validation(
                "Password must be at least 8 characters long".to_string(),
            ));
        }

        // Additional validation logic could be added here

        // Delegate to repository for data persistence
        self.user_repository.create(user_data).await
    }

    pub async fn authenticate_user(
        &self,
        login_data: &LoginUserDto,
    ) -> Result<Option<UserModel>, AppError> {
        // Business logic for authentication
        // For example, handle rate limiting, account lockouts, etc.

        // Here we could implement additional security measures:
        // - Rate limiting failed attempts
        // - Recording login attempts
        // - Handling MFA if implemented

        self.user_repository.verify_credentials(login_data).await
    }

    pub async fn update_user(
        &self,
        id: Uuid,
        user_data: &UpdateUserDto,
    ) -> Result<UserModel, AppError> {
        // Business logic for updates
        // For example, enforce certain fields that can't be changed,
        // or require additional verification for sensitive changes

        if let Some(password) = &user_data.password {
            if password.len() < 8 {
                return Err(AppError::Validation(
                    "Password must be at least 8 characters long".to_string(),
                ));
            }
        }

        self.user_repository.update(id, user_data).await
    }

    pub async fn list_users(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<UserModel>, u64), AppError> {
        // Business logic for listing users
        // For example, filtering, sorting, etc.

        let offset = page * page_size;
        let users = self.user_repository.list_users(page_size, offset).await?;
        let total = self.user_repository.count_users().await?;

        Ok((users, total))
    }

    pub async fn delete_user(&self, id: Uuid) -> Result<(), AppError> {
        // Business logic for deletion
        // For example, archive instead of delete, handle dependencies, etc.

        // We could implement soft deletion here or handle any cleanup
        // before delegating to the repository

        self.user_repository.delete(id).await
    }
}
