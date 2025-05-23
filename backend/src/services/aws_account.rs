use std::sync::Arc;
use uuid::Uuid;
use tracing::{error, info};

use crate::models::aws_account::{AwsAccountCreateDto, AwsAccountDto, AwsAccountUpdateDto, DomainModel, SyncResponse};
use crate::repositories::aws_account::AwsAccountRepository;
use crate::services::aws::AwsControlPlane;
use crate::errors::AppError;
use crate::services::aws::aws_types::resource_sync::ResourceSyncRequest;

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
        
        // Create a sync request with all available authentication information
        let sync_request = ResourceSyncRequest {
            account_id: account.account_id.clone(),
            profile: account.profile.clone(),
            region: account.default_region.clone(),
            resource_types: None, // Sync all resource types
            // Add authentication information directly to the request
            // These will be available as fallbacks if the profile doesn't exist
            use_role: account.use_role,
            role_arn: account.role_arn.clone(),
            external_id: account.external_id.clone(),
            access_key_id: account.access_key_id.clone(),
            secret_access_key: account.secret_access_key.clone(),
        };

        // Log the sync attempt with account details for better debugging
        info!("Attempting to sync resources for AWS account {} (id: {}) with profile: {:?}, region: {}, auth_method: {}", 
               account.account_id, id, account.profile, account.default_region, 
               if account.use_role { "IAM Role" } else { "Access Key" });

        // Call the AWS control plane to sync resources
        match self.aws_control_plane.sync_resources(&sync_request).await {
            Ok(response) => {
                // Update the last synced timestamp
                self.repo.update_last_synced(id).await?;
                
                info!("Successfully synced resources for AWS account {}: {} resources", 
                       account.account_id, response.total_resources);
                
                Ok(SyncResponse {
                    success: true,
                    count: response.total_resources,
                    message: format!("Successfully synced {} resources", response.total_resources),
                })
            },
            Err(err) => {
                error!("Failed to sync resources for AWS account {} (id: {}): {:?}", 
                       account.account_id, id, err);
                Err(err)
            }
        }
    }
    
    /// Sync resources for all AWS accounts
    pub async fn sync_all_accounts_resources(&self) -> Result<SyncResponse, AppError> {
        // Get all accounts
        let accounts = self.repo.get_all().await?;
        
        if accounts.is_empty() {
            return Ok(SyncResponse {
                success: true,
                count: 0,
                message: "No accounts found to sync".to_string(),
            });
        }
        
        info!("Starting sync for all {} AWS accounts", accounts.len());
        
        let mut total_resources = 0;
        let mut failed_accounts = Vec::new();
        
        // Sync each account sequentially
        for account in accounts {
            match self.sync_account_resources(account.id).await {
                Ok(response) => {
                    total_resources += response.count;
                },
                Err(err) => {
                    // Log error but continue with next account
                    error!("Failed to sync AWS account {}: {:?}", account.account_id, err);
                    failed_accounts.push(account.account_id.clone());
                }
            }
        }
        
        // Create response message
        let message = if failed_accounts.is_empty() {
            format!("Successfully synced {} resources from all accounts", total_resources)
        } else {
            format!(
                "Synced {} resources. Failed to sync accounts: {}",
                total_resources,
                failed_accounts.join(", ")
            )
        };
        
        info!("{}", message);
        
        Ok(SyncResponse {
            success: failed_accounts.is_empty(),
            count: total_resources,
            message,
        })
    }
}
