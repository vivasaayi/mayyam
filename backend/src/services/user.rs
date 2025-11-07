use std::sync::Arc;
use tracing;
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
        tracing::debug!("Fetching user by ID: {}", id);
        self.user_repository.find_by_id(id).await
    }

    pub async fn get_user_by_username(
        &self,
        username: &str,
    ) -> Result<Option<UserModel>, AppError> {
        tracing::debug!("Fetching user by username: {}", username);
        self.user_repository.find_by_username(username).await
    }

    pub async fn create_user(&self, user_data: &CreateUserDto) -> Result<UserModel, AppError> {
        if user_data.password.len() < 8 {
            tracing::warn!(
                "Unable to create user: {}",
                "Password must be at least 8 characters long"
            );
            return Err(AppError::Validation(
                "Password must be at least 8 characters long".to_string(),
            ));
        }

        tracing::info!("Creating user: {:?}", &user_data.username);
        self.user_repository.create(user_data).await
    }

    pub async fn authenticate_user(
        &self,
        login_data: &LoginUserDto,
    ) -> Result<Option<UserModel>, AppError> {
        tracing::info!("Authenticating user: {:?}", &login_data.username);
        self.user_repository.verify_credentials(login_data).await
    }

    pub async fn update_user(
        &self,
        id: Uuid,
        user_data: &UpdateUserDto,
    ) -> Result<UserModel, AppError> {
        tracing::debug!("Updating user: {:?}", &user_data.first_name);

        if let Some(password) = &user_data.password {
            if password.len() < 8 {
                tracing::warn!(
                    "Unable to update user: {}",
                    "Password must be at least 8 characters long"
                );
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
        let offset = page * page_size;
        let users = self.user_repository.list_users(page_size, offset).await?;
        let total = self.user_repository.count_users().await?;

        Ok((users, total))
    }

    pub async fn delete_user(&self, id: Uuid) -> Result<(), AppError> {
        tracing::warn!("Deleting user: {}", id);
        self.user_repository.delete(id).await
    }
}
