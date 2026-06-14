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

pub struct KmsControlPlane {
    aws_service: Arc<AwsService>,
}

impl KmsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_keys(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing KMS keys for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_kms_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut marker: Option<String> = None;

        loop {
            let mut request = client.list_keys();
            if let Some(m) = marker {
                request = request.marker(m);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list KMS keys: {}", e);
                AppError::ExternalService(format!("Failed to list KMS keys: {}", e))
            })?;

            for key_entry in response.keys() {
                let entry_key_id = match key_entry.key_id() {
                    Some(id) => id,
                    None => {
                        debug!("Skipping KMS key list entry without a key id");
                        continue;
                    }
                };

                let describe_response =
                    match client.describe_key().key_id(entry_key_id).send().await {
                        Ok(res) => res,
                        Err(e) => {
                            error!("Failed to describe KMS key {}: {}", entry_key_id, e);
                            continue;
                        }
                    };

                let metadata = match describe_response.key_metadata() {
                    Some(m) => m,
                    None => {
                        debug!("DescribeKey returned no metadata for {}", entry_key_id);
                        continue;
                    }
                };

                let key_id = metadata.key_id().to_string();
                let arn = metadata
                    .arn()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| key_entry.key_arn().unwrap_or("").to_string());

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("key_id".to_string(), json!(key_id));
                resource_data.insert("arn".to_string(), json!(arn));

                if let Some(state) = metadata.key_state() {
                    resource_data.insert("key_state".to_string(), json!(state.as_str()));
                }

                if let Some(usage) = metadata.key_usage() {
                    resource_data.insert("key_usage".to_string(), json!(usage.as_str()));
                }

                if let Some(spec) = metadata.key_spec() {
                    resource_data.insert("key_spec".to_string(), json!(spec.as_str()));
                }

                if let Some(manager) = metadata.key_manager() {
                    resource_data.insert("key_manager".to_string(), json!(manager.as_str()));
                }

                if let Some(origin) = metadata.origin() {
                    resource_data.insert("origin".to_string(), json!(origin.as_str()));
                }

                resource_data.insert("enabled".to_string(), json!(metadata.enabled()));

                if let Some(multi_region) = metadata.multi_region() {
                    resource_data.insert("multi_region".to_string(), json!(multi_region));
                }

                if let Some(creation_date) = metadata.creation_date() {
                    let formatted = creation_date
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", creation_date));
                    resource_data.insert("creation_date".to_string(), json!(formatted));
                }

                if let Some(description) = metadata.description() {
                    resource_data.insert("description".to_string(), json!(description));
                }

                // Rotation status applies only to symmetric customer-managed keys.
                // A failure here must not fail the sync; the field is simply absent.
                let is_customer_managed = metadata
                    .key_manager()
                    .map(|m| m.as_str() == "CUSTOMER")
                    .unwrap_or(false);
                let is_symmetric = metadata
                    .key_spec()
                    .map(|s| s.as_str() == "SYMMETRIC_DEFAULT")
                    .unwrap_or(false);

                if is_customer_managed && is_symmetric {
                    match client
                        .get_key_rotation_status()
                        .key_id(&key_id)
                        .send()
                        .await
                    {
                        Ok(rotation) => {
                            resource_data.insert(
                                "rotation_enabled".to_string(),
                                json!(rotation.key_rotation_enabled()),
                            );
                        }
                        Err(e) => {
                            debug!(
                                "Failed to get rotation status for KMS key {}: {}",
                                key_id, e
                            );
                        }
                    }
                }

                let tags = match client.list_resource_tags().key_id(&key_id).send().await {
                    Ok(tags_response) => {
                        let mut tags_map = serde_json::Map::new();
                        for tag in tags_response.tags() {
                            tags_map.insert(tag.tag_key().to_string(), json!(tag.tag_value()));
                        }
                        serde_json::Value::Object(tags_map)
                    }
                    Err(e) => {
                        debug!("Failed to list tags for KMS key {}: {}", key_id, e);
                        json!({})
                    }
                };

                let name = metadata
                    .description()
                    .filter(|d| !d.is_empty())
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| key_id.clone());

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::KmsKey.to_string(),
                    resource_id: key_id,
                    arn,
                    name: Some(name),
                    tags,
                    resource_data: serde_json::Value::Object(resource_data),
                };

                resources.push(dto.into());
            }

            if response.truncated() {
                marker = response.next_marker().map(String::from);
                if marker.is_none() {
                    break;
                }
            } else {
                break;
            }
        }

        debug!(
            "Successfully synced {} KMS keys for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
