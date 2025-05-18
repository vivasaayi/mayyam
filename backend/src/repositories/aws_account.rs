use uuid::Uuid;
use sea_orm::{
    DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, Set, ActiveModelTrait,
    IntoActiveModel, QueryOrder, Order
};
use chrono::Utc;
use tracing::{info, error};

use crate::models::aws_account::{
    Entity as AwsAccountEntity, 
    Model, 
    DomainModel, 
    ActiveModel, 
    AwsAccountCreateDto, 
    AwsAccountUpdateDto,
    Column as AwsAccountColumn
};
use crate::errors::AppError;

/// Repository for AWS account operations
/// 
/// Uses SeaORM for database interactions, providing type-safe
/// query operations and entity management
pub struct AwsAccountRepository {
    db: DatabaseConnection,
}

impl AwsAccountRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Get all AWS accounts
    pub async fn get_all(&self) -> Result<Vec<DomainModel>, AppError> {
        let entities = AwsAccountEntity::find()
            .order_by(AwsAccountColumn::AccountName, Order::Asc)
            .all(&self.db)
            .await
            .map_err(|e| {
                error!("Error fetching AWS accounts: {:?}", e);
                AppError::Database(e)
            })?;
        
        Ok(entities.into_iter().map(DomainModel::from).collect())
    }

    /// Get a single AWS account by ID
    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<DomainModel>, AppError> {
        let entity = AwsAccountEntity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| {
                error!("Error fetching AWS account by ID: {:?}", e);
                AppError::Database(e)
            })?;
        
        Ok(entity.map(DomainModel::from))
    }

    /// Get a single AWS account by account ID
    pub async fn get_by_account_id(&self, account_id: &str) -> Result<Option<DomainModel>, AppError> {
        let entity = AwsAccountEntity::find()
            .filter(AwsAccountColumn::AccountId.eq(account_id))
            .one(&self.db)
            .await
            .map_err(|e| {
                error!("Error fetching AWS account by account ID: {:?}", e);
                AppError::Database(e)
            })?;
        
        Ok(entity.map(DomainModel::from))
    }

    /// Create a new AWS account
    pub async fn create(&self, dto: AwsAccountCreateDto) -> Result<DomainModel, AppError> {
        // Check if account ID already exists
        let existing = self.get_by_account_id(&dto.account_id).await?;
        if existing.is_some() {
            return Err(AppError::Validation(format!(
                "AWS account with ID {} already exists",
                dto.account_id
            )));
        }
        
        // Convert DTO to ActiveModel and insert
        let active_model = ActiveModel::from(dto);
        let entity = active_model.insert(&self.db).await.map_err(|e| {
            error!("Error creating AWS account: {:?}", e);
            AppError::Database(e)
        })?;
        
        Ok(DomainModel::from(entity))
    }

    /// Update an existing AWS account
    pub async fn update(&self, id: Uuid, dto: AwsAccountUpdateDto) -> Result<DomainModel, AppError> {
        // Find the existing entity to get the secret key
        let existing = AwsAccountEntity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| {
                error!("Error finding AWS account for update: {:?}", e);
                AppError::Database(e)
            })?
            .ok_or_else(|| AppError::NotFound(format!("AWS account with ID {} not found", id)))?;
        
        // Convert DTO to ActiveModel and update
        let active_model = ActiveModel::from((dto, existing.secret_access_key, id));
        let entity = active_model.update(&self.db).await.map_err(|e| {
            error!("Error updating AWS account: {:?}", e);
            AppError::Database(e)
        })?;
        
        Ok(DomainModel::from(entity))
    }

    /// Delete an AWS account
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        // Check if account exists first
        if AwsAccountEntity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| {
                error!("Error finding AWS account for deletion: {:?}", e);
                AppError::Database(e)
            })?
            .is_none()
        {
            return Err(AppError::NotFound(format!(
                "AWS account with ID {} not found",
                id
            )));
        }

        // Delete the account
        AwsAccountEntity::delete_by_id(id)
            .exec(&self.db)
            .await
            .map_err(|e| {
                error!("Error deleting AWS account: {:?}", e);
                AppError::Database(e)
            })?;
        
        Ok(())
    }

    /// Update the last synced timestamp for an account
    pub async fn update_last_synced(&self, id: Uuid) -> Result<(), AppError> {
        let now = Utc::now();
        
        // Find the existing entity
        let account = AwsAccountEntity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| {
                error!("Error finding AWS account for updating last synced: {:?}", e);
                AppError::Database(e)
            })?
            .ok_or_else(|| AppError::NotFound(format!("AWS account with ID {} not found", id)))?;

        // Update only the last_synced_at and updated_at fields
        let mut active_model = account.into_active_model();
        active_model.last_synced_at = Set(Some(now));
        active_model.updated_at = Set(now);

        active_model.update(&self.db).await.map_err(|e| {
            error!("Error updating last synced timestamp: {:?}", e);
            AppError::Database(e)
        })?;
        
        Ok(())
    }
}
