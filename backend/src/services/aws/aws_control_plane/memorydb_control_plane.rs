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

pub struct MemoryDbControlPlane {
    aws_service: Arc<AwsService>,
}

impl MemoryDbControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_clusters(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing MemoryDB clusters for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_memorydb_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.describe_clusters().show_shard_details(false);
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to describe MemoryDB clusters: {}", e);
                AppError::ExternalService(format!("Failed to describe MemoryDB clusters: {}", e))
            })?;

            for cluster in response.clusters() {
                let name = cluster.name().unwrap_or("").to_string();
                if name.is_empty() {
                    continue;
                }

                let arn = cluster.arn().unwrap_or("").to_string();
                let mut resource_data = serde_json::Map::new();

                if let Some(status) = cluster.status() {
                    resource_data.insert("status".to_string(), json!(status));
                }
                if let Some(engine_version) = cluster.engine_version() {
                    resource_data.insert("engine_version".to_string(), json!(engine_version));
                }
                if let Some(node_type) = cluster.node_type() {
                    resource_data.insert("node_type".to_string(), json!(node_type));
                }
                if let Some(num_shards) = cluster.number_of_shards() {
                    resource_data.insert("num_shards".to_string(), json!(num_shards));
                }
                // Compute replicas per shard from the first shard's node count (nodes = 1 primary + N replicas).
                let shards = cluster.shards();
                if !shards.is_empty() {
                    let nodes_in_shard = shards[0].number_of_nodes().unwrap_or(1);
                    let replicas = (nodes_in_shard - 1).max(0);
                    resource_data.insert("num_replicas_per_shard".to_string(), json!(replicas));
                }
                if let Some(availability_mode) = cluster.availability_mode() {
                    resource_data.insert(
                        "availability_mode".to_string(),
                        json!(availability_mode.as_str()),
                    );
                }
                resource_data.insert(
                    "tls_enabled".to_string(),
                    json!(cluster.tls_enabled().unwrap_or(false)),
                );
                if let Some(kms) = cluster.kms_key_id() {
                    resource_data.insert("kms_key_id".to_string(), json!(kms));
                }
                if let Some(acl) = cluster.acl_name() {
                    resource_data.insert("acl_name".to_string(), json!(acl));
                }
                if let Some(param_group) = cluster.parameter_group_name() {
                    resource_data.insert("parameter_group_name".to_string(), json!(param_group));
                }
                if let Some(subnet_group) = cluster.subnet_group_name() {
                    resource_data.insert("subnet_group_name".to_string(), json!(subnet_group));
                }
                if let Some(sns_topic) = cluster.sns_topic_arn() {
                    resource_data.insert("sns_topic_arn".to_string(), json!(sns_topic));
                }
                if let Some(snapshot_retention) = cluster.snapshot_retention_limit() {
                    resource_data.insert(
                        "snapshot_retention_limit".to_string(),
                        json!(snapshot_retention),
                    );
                }
                if let Some(snapshot_window) = cluster.snapshot_window() {
                    resource_data.insert("snapshot_window".to_string(), json!(snapshot_window));
                }
                if let Some(desc) = cluster.description() {
                    resource_data.insert("description".to_string(), json!(desc));
                }

                // Engine patch version for maintenance tracking
                if let Some(patch) = cluster.engine_patch_version() {
                    resource_data.insert("engine_patch_version".to_string(), json!(patch));
                }

                // Endpoint for connection info
                if let Some(endpoint) = cluster.cluster_endpoint() {
                    if let Some(addr) = endpoint.address() {
                        resource_data.insert("endpoint_address".to_string(), json!(addr));
                    }
                    resource_data.insert("endpoint_port".to_string(), json!(endpoint.port()));
                }

                // Collect tags via a separate API call
                let mut tags_map = serde_json::Map::new();
                if !arn.is_empty() {
                    match client.list_tags().resource_arn(&arn).send().await {
                        Ok(tags_response) => {
                            for tag in tags_response.tag_list() {
                                if let (Some(key), Some(val)) = (tag.key(), tag.value()) {
                                    tags_map.insert(key.to_string(), json!(val));
                                }
                            }
                        }
                        Err(e) => {
                            debug!("Failed to list tags for MemoryDB cluster {}: {}", name, e);
                        }
                    }
                }

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::MemoryDbCluster.to_string(),
                    resource_id: name.clone(),
                    arn,
                    name: Some(name),
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
            "Successfully synced {} MemoryDB clusters for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
