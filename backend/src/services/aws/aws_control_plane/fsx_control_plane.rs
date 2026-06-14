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

pub struct FsxControlPlane {
    aws_service: Arc<AwsService>,
}

impl FsxControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_file_systems(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing FSx file systems for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_fsx_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.describe_file_systems().max_results(100);
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to describe FSx file systems: {}", e);
                AppError::ExternalService(format!("Failed to describe FSx file systems: {}", e))
            })?;

            for file_system in response.file_systems() {
                let file_system_id = file_system.file_system_id().unwrap_or("").to_string();
                if file_system_id.is_empty() {
                    continue;
                }

                let arn = file_system.resource_arn().unwrap_or("").to_string();
                let mut resource_data = serde_json::Map::new();

                if let Some(owner_id) = file_system.owner_id() {
                    resource_data.insert("owner_id".to_string(), json!(owner_id));
                }
                if let Some(file_system_type) = file_system.file_system_type() {
                    resource_data.insert(
                        "file_system_type".to_string(),
                        json!(file_system_type.as_str()),
                    );
                }
                if let Some(version) = file_system.file_system_type_version() {
                    resource_data.insert("file_system_type_version".to_string(), json!(version));
                }
                if let Some(lifecycle) = file_system.lifecycle() {
                    resource_data.insert("lifecycle".to_string(), json!(lifecycle.as_str()));
                }
                if let Some(storage_capacity) = file_system.storage_capacity() {
                    resource_data
                        .insert("storage_capacity_gib".to_string(), json!(storage_capacity));
                }
                if let Some(storage_type) = file_system.storage_type() {
                    resource_data.insert("storage_type".to_string(), json!(storage_type.as_str()));
                }
                if let Some(vpc_id) = file_system.vpc_id() {
                    resource_data.insert("vpc_id".to_string(), json!(vpc_id));
                }
                resource_data.insert(
                    "subnet_count".to_string(),
                    json!(file_system.subnet_ids().len()),
                );
                resource_data.insert(
                    "network_interface_count".to_string(),
                    json!(file_system.network_interface_ids().len()),
                );
                if let Some(dns_name) = file_system.dns_name() {
                    resource_data.insert("dns_name".to_string(), json!(dns_name));
                }
                if let Some(kms_key_id) = file_system.kms_key_id() {
                    resource_data.insert("kms_key_id".to_string(), json!(kms_key_id));
                }
                resource_data.insert(
                    "customer_managed_kms_key".to_string(),
                    json!(file_system.kms_key_id().is_some()),
                );
                if let Some(network_type) = file_system.network_type() {
                    resource_data.insert("network_type".to_string(), json!(network_type.as_str()));
                }

                enrich_windows(file_system.windows_configuration(), &mut resource_data);
                enrich_lustre(file_system.lustre_configuration(), &mut resource_data);
                enrich_ontap(file_system.ontap_configuration(), &mut resource_data);
                enrich_open_zfs(file_system.open_zfs_configuration(), &mut resource_data);

                let deployment_type = resource_data
                    .get("deployment_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                resource_data.insert(
                    "multi_az".to_string(),
                    json!(
                        deployment_type.contains("MULTI_AZ") || file_system.subnet_ids().len() > 1
                    ),
                );

                let mut tags_map = serde_json::Map::new();
                for tag in file_system.tags() {
                    if let Some(key) = tag.key() {
                        tags_map.insert(key.to_string(), json!(tag.value().unwrap_or("")));
                    }
                }
                if tags_map.is_empty() && !arn.is_empty() {
                    let mut tags_next_token: Option<String> = None;
                    loop {
                        let mut tags_request = client.list_tags_for_resource().resource_arn(&arn);
                        if let Some(token) = tags_next_token {
                            tags_request = tags_request.next_token(token);
                        }
                        match tags_request.send().await {
                            Ok(tags_response) => {
                                for tag in tags_response.tags() {
                                    if let Some(key) = tag.key() {
                                        tags_map.insert(
                                            key.to_string(),
                                            json!(tag.value().unwrap_or("")),
                                        );
                                    }
                                }
                                tags_next_token = tags_response.next_token().map(String::from);
                                if tags_next_token.is_none() {
                                    break;
                                }
                            }
                            Err(e) => {
                                debug!(
                                    "Failed to list tags for FSx file system {}: {}",
                                    file_system_id, e
                                );
                                break;
                            }
                        }
                    }
                }

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::FsxFileSystem.to_string(),
                    resource_id: file_system_id.clone(),
                    arn,
                    name: Some(file_system_id),
                    tags: serde_json::Value::Object(tags_map),
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
            "Successfully synced {} FSx file systems for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

fn enrich_windows(
    config: Option<&aws_sdk_fsx::types::WindowsFileSystemConfiguration>,
    resource_data: &mut serde_json::Map<String, serde_json::Value>,
) {
    if let Some(config) = config {
        if let Some(deployment_type) = config.deployment_type() {
            resource_data.insert(
                "deployment_type".to_string(),
                json!(deployment_type.as_str()),
            );
        }
        if let Some(capacity) = config.throughput_capacity() {
            resource_data.insert("throughput_capacity_mbps".to_string(), json!(capacity));
        }
        if let Some(retention) = config.automatic_backup_retention_days() {
            resource_data.insert(
                "automatic_backup_retention_days".to_string(),
                json!(retention),
            );
        }
        if let Some(copy_tags) = config.copy_tags_to_backups() {
            resource_data.insert("copy_tags_to_backups".to_string(), json!(copy_tags));
        }
    }
}

fn enrich_lustre(
    config: Option<&aws_sdk_fsx::types::LustreFileSystemConfiguration>,
    resource_data: &mut serde_json::Map<String, serde_json::Value>,
) {
    if let Some(config) = config {
        if let Some(deployment_type) = config.deployment_type() {
            resource_data.insert(
                "deployment_type".to_string(),
                json!(deployment_type.as_str()),
            );
        }
        if let Some(throughput) = config.per_unit_storage_throughput() {
            resource_data.insert("per_unit_storage_throughput".to_string(), json!(throughput));
        }
        if let Some(retention) = config.automatic_backup_retention_days() {
            resource_data.insert(
                "automatic_backup_retention_days".to_string(),
                json!(retention),
            );
        }
        if let Some(copy_tags) = config.copy_tags_to_backups() {
            resource_data.insert("copy_tags_to_backups".to_string(), json!(copy_tags));
        }
        if let Some(compression) = config.data_compression_type() {
            resource_data.insert(
                "data_compression_type".to_string(),
                json!(compression.as_str()),
            );
        }
    }
}

fn enrich_ontap(
    config: Option<&aws_sdk_fsx::types::OntapFileSystemConfiguration>,
    resource_data: &mut serde_json::Map<String, serde_json::Value>,
) {
    if let Some(config) = config {
        if let Some(deployment_type) = config.deployment_type() {
            resource_data.insert(
                "deployment_type".to_string(),
                json!(deployment_type.as_str()),
            );
        }
        if let Some(retention) = config.automatic_backup_retention_days() {
            resource_data.insert(
                "automatic_backup_retention_days".to_string(),
                json!(retention),
            );
        }
        if let Some(capacity) = config.throughput_capacity() {
            resource_data.insert("throughput_capacity_mbps".to_string(), json!(capacity));
        }
        resource_data.insert(
            "route_table_count".to_string(),
            json!(config.route_table_ids().len()),
        );
    }
}

fn enrich_open_zfs(
    config: Option<&aws_sdk_fsx::types::OpenZfsFileSystemConfiguration>,
    resource_data: &mut serde_json::Map<String, serde_json::Value>,
) {
    if let Some(config) = config {
        if let Some(deployment_type) = config.deployment_type() {
            resource_data.insert(
                "deployment_type".to_string(),
                json!(deployment_type.as_str()),
            );
        }
        if let Some(retention) = config.automatic_backup_retention_days() {
            resource_data.insert(
                "automatic_backup_retention_days".to_string(),
                json!(retention),
            );
        }
        if let Some(copy_tags) = config.copy_tags_to_backups() {
            resource_data.insert("copy_tags_to_backups".to_string(), json!(copy_tags));
        }
        if let Some(copy_tags) = config.copy_tags_to_volumes() {
            resource_data.insert("copy_tags_to_volumes".to_string(), json!(copy_tags));
        }
        if let Some(capacity) = config.throughput_capacity() {
            resource_data.insert("throughput_capacity_mbps".to_string(), json!(capacity));
        }
        resource_data.insert(
            "route_table_count".to_string(),
            json!(config.route_table_ids().len()),
        );
    }
}
