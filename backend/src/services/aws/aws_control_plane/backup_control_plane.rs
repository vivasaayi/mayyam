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

use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_resource::{AwsResourceDto, AwsResourceType, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct BackupControlPlane {
    aws_service: Arc<AwsService>,
}

impl BackupControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_vaults(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Backup vaults for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_backup_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_backup_vaults();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list Backup vaults: {}", e);
                AppError::ExternalService(format!("Failed to list Backup vaults: {}", e))
            })?;

            for vault in response.backup_vault_list() {
                let name = match vault.backup_vault_name() {
                    Some(n) if !n.is_empty() => n.to_string(),
                    _ => {
                        debug!("Skipping Backup vault list entry without a vault name");
                        continue;
                    }
                };
                let arn = vault.backup_vault_arn().unwrap_or("").to_string();

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("backup_vault_name".to_string(), json!(name));
                resource_data.insert("backup_vault_arn".to_string(), json!(arn));

                if let Some(vault_type) = vault.vault_type() {
                    resource_data.insert("vault_type".to_string(), json!(vault_type.as_str()));
                }

                if let Some(vault_state) = vault.vault_state() {
                    resource_data.insert("vault_state".to_string(), json!(vault_state.as_str()));
                }

                if let Some(creation_date) = vault.creation_date() {
                    let formatted = creation_date
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", creation_date));
                    resource_data.insert("creation_date".to_string(), json!(formatted));
                }

                if let Some(encryption_key_arn) = vault.encryption_key_arn() {
                    resource_data
                        .insert("encryption_key_arn".to_string(), json!(encryption_key_arn));
                }

                if let Some(encryption_key_type) = vault.encryption_key_type() {
                    resource_data.insert(
                        "encryption_key_type".to_string(),
                        json!(encryption_key_type.as_str()),
                    );
                }

                resource_data.insert(
                    "number_of_recovery_points".to_string(),
                    json!(vault.number_of_recovery_points()),
                );

                if let Some(locked) = vault.locked() {
                    resource_data.insert("locked".to_string(), json!(locked));
                }

                if let Some(min_retention_days) = vault.min_retention_days() {
                    resource_data
                        .insert("min_retention_days".to_string(), json!(min_retention_days));
                }

                if let Some(max_retention_days) = vault.max_retention_days() {
                    resource_data
                        .insert("max_retention_days".to_string(), json!(max_retention_days));
                }

                if let Some(lock_date) = vault.lock_date() {
                    let formatted = lock_date
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", lock_date));
                    resource_data.insert("lock_date".to_string(), json!(formatted));
                }

                // Tags require the vault ARN; a per-vault tag failure must not
                // fail the sync — the tags column is simply left empty.
                let tags = if arn.is_empty() {
                    json!({})
                } else {
                    match client.list_tags().resource_arn(&arn).send().await {
                        Ok(tags_response) => {
                            let mut tags_map = serde_json::Map::new();
                            if let Some(tag_pairs) = tags_response.tags() {
                                for (key, value) in tag_pairs {
                                    tags_map.insert(key.to_string(), json!(value));
                                }
                            }
                            serde_json::Value::Object(tags_map)
                        }
                        Err(e) => {
                            debug!("Failed to list tags for Backup vault {}: {}", name, e);
                            json!({})
                        }
                    }
                };

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::BackupVault.to_string(),
                    resource_id: name.clone(),
                    arn,
                    name: Some(name),
                    tags,
                    resource_data: serde_json::Value::Object(resource_data),
                };

                resources.push(dto.into());
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        debug!(
            "Successfully synced {} Backup vaults for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
