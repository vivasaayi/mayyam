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

pub struct GlacierControlPlane {
    aws_service: Arc<AwsService>,
}

impl GlacierControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_vaults(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Glacier Vaults for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_glacier_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut marker = None;

        loop {
            let mut request = client.list_vaults().account_id("-");
            if let Some(m) = marker {
                request = request.marker(m);
            }

            let response = match request.send().await {
                Ok(res) => res,
                Err(e) => {
                    error!("Failed to list Glacier Vaults: {}", e);
                    break;
                }
            };

            for vault in response.vault_list() {
                let arn = vault.vault_arn().unwrap_or("");
                let name = vault.vault_name().unwrap_or("");
                
                let resource_data = json!({
                    "VaultName": name,
                    "VaultARN": arn,
                    "CreationDate": vault.creation_date().unwrap_or(""),
                    "LastInventoryDate": vault.last_inventory_date().unwrap_or(""),
                    "NumberOfArchives": vault.number_of_archives(),
                    "SizeInBytes": vault.size_in_bytes()
                });

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::GlacierArchive.to_string(),
                    resource_id: name.to_string(),
                    arn: arn.to_string(),
                    name: Some(name.to_string()),
                    tags: json!({}),
                    resource_data,
                };

                resources.push(dto.into());
            }

            marker = response.marker().map(String::from);
            if marker.is_none() {
                break;
            }
        }

        debug!(
            "Successfully synced {} Glacier Vaults for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
