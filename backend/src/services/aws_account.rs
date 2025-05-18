use std::sync::Arc;
use uuid::Uuid;
use tracing::{info, error};

use crate::models::aws_account::{DomainModel, AwsAccountCreateDto, AwsAccountUpdateDto, AwsAccountDto, SyncResponse};
use crate::repositories::aws_account::AwsAccountRepository;
use crate::services::aws::{AwsControlPlane, ResourceSyncRequest};
use crate::errors::AppError;

/// Service for AWS account management
///
/// Provides business logic for account operations and
/// integrates with AWS control plane for resource syncing
pub struct AwsAccountService {
    repo: Arc<AwsAccountRepository>,
    aws_control_plane: Arc<AwsControlPlane>,
}

impl AwsAccountService {
    pub fn new(repo: Arc<AwsAccountRepository>, aws_control_plane: Arc<AwsControlPlane>) -> Self {
        Self { repo, aws_control_plane }
    }

    /// List all AWS accounts
    pub async fn list_accounts(&self) -> Result<Vec<AwsAccountDto>, AppError> {
        let accounts = self.repo.get_all().await?;
        Ok(accounts.into_iter().map(AwsAccountDto::from).collect())
    }

    /// Get a single AWS account by ID
    pub async fn get_account(&self, id: Uuid) -> Result<AwsAccountDto, AppError> {
        let account = self.repo.get_by_id(id).await?
            .ok_or_else(|| AppError::NotFound(format!("AWS account with ID {} not found", id)))?;
        
        // Create a DTO with access key ID for editing purposes
        let mut dto = AwsAccountDto::from(account.clone());
        
        // Include access key ID when fetching a specific account for editing
        if !account.use_role {
            dto.access_key_id = account.access_key_id.clone();
        }
        
        Ok(dto)
    }

    /// Create a new AWS account
    pub async fn create_account(&self, dto: AwsAccountCreateDto) -> Result<AwsAccountDto, AppError> {
        // Validate that credentials are provided appropriately
        if !dto.use_role && (dto.access_key_id.is_none() || dto.secret_access_key.is_none()) {
            return Err(AppError::Validation(
                "Access key ID and secret access key are required when not using a role".to_string()
            ));
        }

        if dto.use_role && dto.role_arn.is_none() {
            return Err(AppError::Validation(
                "Role ARN is required when using a role".to_string()
            ));
        }

        // Create the account
        let account = self.repo.create(dto).await?;
        info!("Created new AWS account: {}", account.account_id);
        
        Ok(AwsAccountDto::from(account))
    }

    /// Update an existing AWS account
    pub async fn update_account(&self, id: Uuid, dto: AwsAccountUpdateDto) -> Result<AwsAccountDto, AppError> {
        // Get the current account to have access to existing values
        let current_account = self.repo.get_by_id(id).await?
            .ok_or_else(|| AppError::NotFound(format!("AWS account with ID {} not found", id)))?;
            
        // Validate based on authentication method
        if !dto.use_role {
            // When using access key authentication, access key ID is required
            if dto.access_key_id.is_none() || dto.access_key_id.as_ref().map_or(true, |k| k.is_empty()) {
                return Err(AppError::Validation(
                    "Access key ID is required when not using a role".to_string()
                ));
            }
        } else if dto.role_arn.is_none() || dto.role_arn.as_ref().map_or(true, |r| r.is_empty()) {
            // When using role authentication, role ARN is required
            return Err(AppError::Validation(
                "Role ARN is required when using a role".to_string()
            ));
        }

        // Update the account
        let account = self.repo.update(id, dto).await?;
        info!("Updated AWS account: {}", account.account_id);
        
        Ok(AwsAccountDto::from(account))
    }

    /// Delete an AWS account
    pub async fn delete_account(&self, id: Uuid) -> Result<(), AppError> {
        self.repo.delete(id).await?;
        info!("Deleted AWS account: {}", id);
        
        Ok(())
    }

    /// Sync resources for an AWS account
    pub async fn sync_account_resources(&self, id: Uuid) -> Result<SyncResponse, AppError> {
        // Get the account
        let account = self.repo.get_by_id(id).await?
            .ok_or_else(|| AppError::NotFound(format!("AWS account with ID {} not found", id)))?;
        
        // Create a sync request
        let sync_request = ResourceSyncRequest {
            account_id: account.account_id.clone(),
            profile: account.profile.clone(),
            region: account.default_region.clone(),
            resource_types: None, // Sync all resource types
        };

        // Call the AWS control plane to sync resources
        let response = self.aws_control_plane.sync_resources(&sync_request).await?;
        
        // Update the last synced timestamp
        self.repo.update_last_synced(id).await?;
        
        info!("Synced resources for AWS account {}: {} resources", account.account_id, response.total_resources);
        
        Ok(SyncResponse {
            success: true,
            count: response.total_resources,
            message: format!("Successfully synced {} resources", response.total_resources),
        })
    }
}
