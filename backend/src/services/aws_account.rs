// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::aws_account::{
    AwsAccountCreateDto, AwsAccountDto, AwsAccountUpdateDto, SyncResponse,
};
use crate::repositories::aws_account::AwsAccountRepository;
use crate::repositories::sync_run::SyncRunRepository;
use crate::services::aws::aws_types::resource_sync::ResourceSyncRequest;
use crate::services::aws::AwsControlPlaneTrait;
use futures::stream::{self, StreamExt};

/// Service for AWS account management
///
/// Provides business logic for account operations and
/// integrates with AWS control plane for resource syncing
pub struct AwsAccountService {
    repo: Arc<AwsAccountRepository>,
    aws_control_plane: Arc<dyn AwsControlPlaneTrait>,
    sync_run_repo: Arc<SyncRunRepository>,
}

impl AwsAccountService {
    pub fn new(
        repo: Arc<AwsAccountRepository>,
        aws_control_plane: Arc<dyn AwsControlPlaneTrait>,
        sync_run_repo: Arc<SyncRunRepository>,
    ) -> Self {
        Self {
            repo,
            aws_control_plane,
            sync_run_repo,
        }
    }

    /// Validate authentication fields according to auth_type for create
    fn validate_create_auth(dto: &AwsAccountCreateDto) -> Result<(), AppError> {
        let auth = dto.auth_type.as_deref().unwrap_or("auto").to_lowercase();

        match auth.as_str() {
            "access_keys" => {
                if dto.access_key_id.as_deref().unwrap_or("").is_empty()
                    || dto.secret_access_key.as_deref().unwrap_or("").is_empty()
                {
                    return Err(AppError::Validation(
                        "access_keys auth requires access_key_id and secret_access_key".to_string(),
                    ));
                }
            }
            "assume_role" => {
                if dto.role_arn.as_deref().unwrap_or("").is_empty() {
                    return Err(AppError::Validation(
                        "assume_role auth requires role_arn".to_string(),
                    ));
                }
                // source_profile/profile optional; external_id/session_name optional
            }
            "profile" => {
                if dto.profile.as_deref().unwrap_or("").is_empty() {
                    return Err(AppError::Validation(
                        "profile auth requires profile name".to_string(),
                    ));
                }
            }
            "sso" => {
                // Accept sso_profile or profile as the profile name configured for SSO
                if dto.sso_profile.as_deref().unwrap_or("").is_empty()
                    && dto.profile.as_deref().unwrap_or("").is_empty()
                {
                    return Err(AppError::Validation(
                        "sso auth requires sso_profile or profile".to_string(),
                    ));
                }
            }
            "web_identity" => {
                if dto.role_arn.as_deref().unwrap_or("").is_empty() {
                    return Err(AppError::Validation(
                        "web_identity auth requires role_arn".to_string(),
                    ));
                }
                // token file optional (may come from env); session_name optional
            }
            "instance_role" => {
                // No specific fields required
            }
            "auto" | _ => {
                // Back-compat with legacy flags
                if dto.use_role {
                    if dto.role_arn.as_deref().unwrap_or("").is_empty() {
                        return Err(AppError::Validation(
                            "When use_role is true, role_arn is required".to_string(),
                        ));
                    }
                } else if dto.access_key_id.as_deref().unwrap_or("").is_empty()
                    || dto.secret_access_key.as_deref().unwrap_or("").is_empty()
                {
                    return Err(AppError::Validation(
                        "When not using a role, access_key_id and secret_access_key are required"
                            .to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Validate authentication fields according to auth_type for update
    fn validate_update_auth(dto: &AwsAccountUpdateDto) -> Result<(), AppError> {
        let auth = dto.auth_type.as_deref().unwrap_or("auto").to_lowercase();

        match auth.as_str() {
            "access_keys" => {
                if dto.access_key_id.as_deref().unwrap_or("").is_empty() {
                    return Err(AppError::Validation(
                        "access_keys auth requires access_key_id on update".to_string(),
                    ));
                }
                // secret_access_key may be omitted to keep existing; allow empty here
            }
            "assume_role" => {
                if dto.role_arn.as_deref().unwrap_or("").is_empty() {
                    return Err(AppError::Validation(
                        "assume_role auth requires role_arn".to_string(),
                    ));
                }
            }
            "profile" => {
                if dto.profile.as_deref().unwrap_or("").is_empty() {
                    return Err(AppError::Validation(
                        "profile auth requires profile name".to_string(),
                    ));
                }
            }
            "sso" => {
                if dto.sso_profile.as_deref().unwrap_or("").is_empty()
                    && dto.profile.as_deref().unwrap_or("").is_empty()
                {
                    return Err(AppError::Validation(
                        "sso auth requires sso_profile or profile".to_string(),
                    ));
                }
            }
            "web_identity" => {
                if dto.role_arn.as_deref().unwrap_or("").is_empty() {
                    return Err(AppError::Validation(
                        "web_identity auth requires role_arn".to_string(),
                    ));
                }
            }
            "instance_role" => {
                // No specific fields required
            }
            "auto" | _ => {
                // Back-compat with legacy flags
                if dto.use_role {
                    if dto.role_arn.as_deref().unwrap_or("").is_empty() {
                        return Err(AppError::Validation(
                            "When use_role is true, role_arn is required".to_string(),
                        ));
                    }
                } else if dto.access_key_id.as_deref().unwrap_or("").is_empty() {
                    return Err(AppError::Validation(
                        "When not using a role, access_key_id is required".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// List all AWS accounts
    pub async fn list_accounts(&self) -> Result<Vec<AwsAccountDto>, AppError> {
        let accounts = self.repo.get_all().await?;
        Ok(accounts.into_iter().map(AwsAccountDto::from).collect())
    }

    /// Get a single AWS account by ID
    pub async fn get_account(&self, id: Uuid) -> Result<AwsAccountDto, AppError> {
        let account =
            self.repo.get_by_id(id).await?.ok_or_else(|| {
                AppError::NotFound(format!("AWS account with ID {} not found", id))
            })?;

        // Create a DTO with access key ID for editing purposes
        let mut dto = AwsAccountDto::from(account.clone());

        // Include access key ID when fetching a specific account for editing if keys exist
        if account.access_key_id.is_some() {
            dto.access_key_id = account.access_key_id.clone();
            dto.secret_access_key = account.secret_access_key.clone();
        }

        Ok(dto)
    }

    /// Create a new AWS account
    pub async fn create_account(
        &self,
        dto: AwsAccountCreateDto,
    ) -> Result<AwsAccountDto, AppError> {
        // Validate based on auth_type (supports legacy flags via "auto")
        Self::validate_create_auth(&dto)?;

        // Create the account
        let account = self.repo.create(dto).await?;
        info!("Created new AWS account: {}", account.account_id);

        Ok(AwsAccountDto::from(account))
    }

    /// Update an existing AWS account
    pub async fn update_account(
        &self,
        id: Uuid,
        dto: AwsAccountUpdateDto,
    ) -> Result<AwsAccountDto, AppError> {
        // Get the current account to have access to existing values
        let _current_account =
            self.repo.get_by_id(id).await?.ok_or_else(|| {
                AppError::NotFound(format!("AWS account with ID {} not found", id))
            })?;

        // Validate based on authentication method
        Self::validate_update_auth(&dto)?;

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
    /// Multi-region behavior:
    /// - If SyncRun exists and metadata includes regions (array of strings) or all_regions=true, honor that
    /// - Else if account.regions set, use those
    /// - Else enumerate all regions via EC2 DescribeRegions and scan all
    pub async fn sync_account_resources(
        &self,
        id: Uuid,
        sync_id: Uuid,
    ) -> Result<SyncResponse, AppError> {
        // Get the account
        let account =
            self.repo.get_by_id(id).await?.ok_or_else(|| {
                AppError::NotFound(format!("AWS account with ID {} not found", id))
            })?;

        debug!(
            "Syncing resources for AWS account: {:?} with sync_id: {}",
            &account, sync_id
        );

        // Determine regions to scan
        let mut regions_to_scan: Option<Vec<String>> = None;
        if let Ok(Some(run)) = self.sync_run_repo.get(sync_id).await {
            // Look into metadata for either { all_regions: true } or { regions: [..] }
            if let Some(meta) = run.metadata.get("all_regions").and_then(|v| v.as_bool()) {
                if meta {
                    regions_to_scan = None;
                } // explicit all regions
            }
            if let Some(arr) = run.metadata.get("regions").and_then(|v| v.as_array()) {
                let list = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>();
                if !list.is_empty() {
                    regions_to_scan = Some(list);
                }
            }
        }
        if regions_to_scan.is_none() {
            if let Some(r) = account.regions.clone() {
                regions_to_scan = Some(r);
            }
        }

        // If still none, enumerate all regions using the account's auth context
        let resolved_regions: Vec<String> = if let Some(r) = regions_to_scan {
            r
        } else {
            // Bootstrap dto with default_region (any valid region is fine for DescribeRegions)
            let bootstrap = AwsAccountDto {
                id: account.id,
                account_id: account.account_id.clone(),
                account_name: account.account_name.clone(),
                profile: account.profile.clone(),
                default_region: account.default_region.clone(),
                regions: account.regions.clone(),
                use_role: account.use_role,
                role_arn: account.role_arn.clone(),
                external_id: account.external_id.clone(),
                has_access_key: account.access_key_id.is_some(),
                access_key_id: account.access_key_id.clone(),
                secret_access_key: account.secret_access_key.clone(),
                auth_type: account.auth_type.clone(),
                source_profile: account.source_profile.clone(),
                sso_profile: account.sso_profile.clone(),
                web_identity_token_file: account.web_identity_token_file.clone(),
                session_name: account.session_name.clone(),
                last_synced_at: account.last_synced_at,
                created_at: account.created_at,
                updated_at: account.updated_at,
            };
            self.aws_control_plane
                .list_all_regions(&bootstrap)
                .await
                .unwrap_or_else(|_| vec![account.default_region.clone()])
        };

        // Log the sync attempt with account details for better debugging
        info!("Attempting to sync resources for AWS account {} (id: {}) with sync_id: {} profile: {:?}, region: {}, auth_type: {}", 
         account.account_id, id, sync_id, account.profile, account.default_region, account.auth_type);

        // Mark the sync run as running if it exists (best-effort)
        let _ = self.sync_run_repo.mark_running(sync_id).await;

        // Persist resolved region scope into sync_run metadata (best-effort) so UI can show scope
        let _ = self
            .sync_run_repo
            .update_metadata(
                sync_id,
                serde_json::json!({
                    "regions": resolved_regions,
                }),
            )
            .await;

        // Iterate regions concurrently with bounded concurrency and aggregate results
        let concurrency_limit = std::cmp::max(
            1,
            crate::config::load_config()
                .ok()
                .map(|c| c.sync.region_concurrency)
                .unwrap_or(4),
        );
        let account_clone = account.clone();
        let tasks = stream::iter(resolved_regions.clone().into_iter().map(move |region| {
            let cp = self.aws_control_plane.clone();
            let acc = account_clone.clone();
            async move {
                let sync_request = ResourceSyncRequest {
                    sync_id,
                    account_id: acc.account_id.clone(),
                    profile: acc.profile.clone(),
                    region: region.clone(),
                    resource_types: None,
                    use_role: acc.use_role,
                    role_arn: acc.role_arn.clone(),
                    external_id: acc.external_id.clone(),
                    access_key_id: acc.access_key_id.clone(),
                    secret_access_key: acc.secret_access_key.clone(),
                };

                match cp.sync_resources(&sync_request).await {
                    Ok(resp) => {
                        info!(
                            "Region {}: {} resources synced (sync_id: {})",
                            region, resp.total_resources, sync_id
                        );
                        Ok::<usize, (String, AppError)>(resp.total_resources)
                    }
                    Err(err) => {
                        error!(
                            "Region {} failed to sync for account {} (sync_id: {}): {:?}",
                            region, acc.account_id, sync_id, err
                        );
                        Err::<usize, (String, AppError)>((region, err))
                    }
                }
            }
        }))
        .buffer_unordered(concurrency_limit)
        .collect::<Vec<_>>()
        .await;

        let mut grand_total = 0usize;
        let mut failures = 0i32;
        for res in tasks {
            match res {
                Ok(count) => grand_total += count,
                Err((_region, _err)) => failures += 1,
            }
        }

        // Update the last synced timestamp
        self.repo.update_last_synced(id).await?;
        // Mark completed (partial failures reflected in failure_count)
        let _ = self
            .sync_run_repo
            .complete(sync_id, grand_total as i32, grand_total as i32, failures)
            .await;

        info!(
            "Completed multi-region sync for account {} (sync_id: {}): total {} resources, {} region failure(s)",
            account.account_id,
            sync_id,
            grand_total,
            failures
        );

        Ok(SyncResponse {
            success: failures == 0,
            count: grand_total,
            message: format!(
                "Synced {} resources across {} region(s) (sync_id: {}, failures: {})",
                grand_total,
                resolved_regions.len(),
                sync_id,
                failures
            ),
        })
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
            let sync_id = Uuid::new_v4(); // Generate a unique sync_id for each account
            info!(
                "Starting sync for AWS account {} with sync_id: {}",
                account.account_id, sync_id
            );
            match self.sync_account_resources(account.id, sync_id).await {
                Ok(response) => {
                    total_resources += response.count;
                }
                Err(err) => {
                    // Log error but continue with next account
                    error!(
                        "Failed to sync AWS account {} (sync_id: {}): {:?}",
                        account.account_id, sync_id, err
                    );
                    failed_accounts.push(account.account_id.clone());
                }
            }
        }

        // Create response message
        let message = if failed_accounts.is_empty() {
            format!(
                "Successfully synced {} resources from all accounts",
                total_resources
            )
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
