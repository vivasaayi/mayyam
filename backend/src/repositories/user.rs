use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, Set, ActiveModelTrait, PaginatorTrait, QuerySelect};
use uuid::Uuid;
use chrono::Utc;
use bcrypt::{hash, verify, DEFAULT_COST};

use crate::models::user::{self, Entity as User, Model as UserModel, ActiveModel as UserActiveModel};
use crate::models::user::{CreateUserDto, LoginUserDto, UpdateUserDto};
use crate::errors::AppError;

pub struct UserRepository {
    db: DatabaseConnection,
}

impl UserRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<UserModel>, AppError> {
        let user = User::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(AppError::Database)?;
        
        Ok(user)
    }
    
    pub async fn find_by_username(&self, username: &str) -> Result<Option<UserModel>, AppError> {
        let user = User::find()
            .filter(user::Column::Username.eq(username))
            .one(&self.db)
            .await
            .map_err(AppError::Database)?;
        
        Ok(user)
    }
    
    pub async fn find_by_email(&self, email: &str) -> Result<Option<UserModel>, AppError> {
        let user = User::find()
            .filter(user::Column::Email.eq(email))
            .one(&self.db)
            .await
            .map_err(AppError::Database)?;
        
        Ok(user)
    }
    
    pub async fn create(&self, user_data: &CreateUserDto) -> Result<UserModel, AppError> {
        // Check if username or email already exists
        if let Some(_) = self.find_by_username(&user_data.username).await? {
            return Err(AppError::Validation(format!("Username '{}' already exists", user_data.username)));
        }
        
        if let Some(_) = self.find_by_email(&user_data.email).await? {
            return Err(AppError::Validation(format!("Email '{}' already exists", user_data.email)));
        }
        
        // Hash the password
        let password_hash = hash(&user_data.password, DEFAULT_COST)
            .map_err(|e| AppError::Internal(format!("Password hashing error: {}", e)))?;
        
        // Create new user
        let user = UserActiveModel {
            id: Set(Uuid::new_v4()),
            username: Set(user_data.username.clone()),
            email: Set(user_data.email.clone()),
            password_hash: Set(password_hash),
            first_name: Set(user_data.first_name.clone()),
            last_name: Set(user_data.last_name.clone()),
            active: Set(true),
            roles: Set("".to_string()),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            last_login: Set(None),
        };
        
        let user = user.insert(&self.db).await.map_err(AppError::Database)?;
        
        Ok(user)
    }
    
    pub async fn verify_credentials(&self, login_data: &LoginUserDto) -> Result<Option<UserModel>, AppError> {
        // Find user by username
        let user = match self.find_by_username(&login_data.username).await? {
            Some(user) => user,
            None => return Ok(None),
        };
        
        // Verify password
        let is_valid = verify(&login_data.password, &user.password_hash)
            .map_err(|e| AppError::Internal(format!("Password verification error: {}", e)))?;
        
        if is_valid {
            // Update last login time
            let mut user_active: UserActiveModel = user.clone().into();
            user_active.last_login = Set(Some(Utc::now()));
            user_active.updated_at = Set(Utc::now());
            
            let updated_user = user_active.update(&self.db).await.map_err(AppError::Database)?;
            
            // Convert roles string to permissions array if needed
            if updated_user.permissions.is_empty() && !updated_user.roles.is_empty() {
                let roles: Vec<String> = updated_user.roles.split(',').map(|s| s.trim().to_string()).collect();
                // We can't update permissions here directly, so we'll return the user with converted roles
                return Ok(Some(UserModel {
                    permissions: roles,
                    ..updated_user
                }));
            }
            
            Ok(Some(updated_user))
        } else {
            Ok(None)
        }
    }
    
    pub async fn update(&self, id: Uuid, user_data: &UpdateUserDto) -> Result<UserModel, AppError> {
        let user = self.find_by_id(id).await?
            .ok_or_else(|| AppError::NotFound(format!("User not found with ID: {}", id)))?;
        
        let mut user_active: UserActiveModel = user.into();
        
        // Update fields conditionally
        if let Some(email) = &user_data.email {
            // Check if email is already used by another user
            if let Some(existing) = self.find_by_email(email).await? {
                if existing.id != id {
                    return Err(AppError::Validation(format!("Email '{}' already exists", email)));
                }
            }
            user_active.email = Set(email.clone());
        }
        
        if let Some(password) = &user_data.password {
            let password_hash = hash(password, DEFAULT_COST)
                .map_err(|e| AppError::Internal(format!("Password hashing error: {}", e)))?;
            user_active.password_hash = Set(password_hash);
        }
        
        if let Some(first_name) = &user_data.first_name {
            user_active.first_name = Set(Some(first_name.clone()));
        }
        
        if let Some(last_name) = &user_data.last_name {
            user_active.last_name = Set(Some(last_name.clone()));
        }
        
        if let Some(is_active) = user_data.is_active {
            user_active.active = Set(is_active);
        }
        
        user_active.updated_at = Set(Utc::now());
        
        let updated_user = user_active.update(&self.db).await.map_err(AppError::Database)?;
        
        Ok(updated_user)
    }
    
    pub async fn list_users(&self, limit: u64, offset: u64) -> Result<Vec<UserModel>, AppError> {
        let users = User::find()
            .limit(Some(limit))
            .offset(Some(offset))
            .all(&self.db)
            .await
            .map_err(AppError::Database)?;
        
        Ok(users)
    }
    
    pub async fn count_users(&self) -> Result<u64, AppError> {
        let count = User::find()
            .count(&self.db)
            .await
            .map_err(AppError::Database)?;
        
        Ok(count)
    }
    
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let user = self.find_by_id(id).await?
            .ok_or_else(|| AppError::NotFound(format!("User not found with ID: {}", id)))?;
            
        let user_active: UserActiveModel = user.into();
        
        user_active.delete(&self.db)
            .await
            .map_err(AppError::Database)?;
            
        Ok(())
    }
}