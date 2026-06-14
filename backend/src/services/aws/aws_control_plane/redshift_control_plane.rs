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

pub struct RedshiftControlPlane {
    aws_service: Arc<AwsService>,
}

impl RedshiftControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_clusters(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Redshift clusters for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_redshift_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut marker: Option<String> = None;

        loop {
            let mut request = client.describe_clusters();
            if let Some(m) = marker {
                request = request.marker(m);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to describe Redshift clusters: {}", e);
                AppError::ExternalService(format!("Failed to describe Redshift clusters: {}", e))
            })?;

            for cluster in response.clusters() {
                let cluster_identifier = match cluster.cluster_identifier() {
                    Some(id) => id.to_string(),
                    None => {
                        debug!("Skipping Redshift cluster entry without an identifier");
                        continue;
                    }
                };

                // DescribeClusters does not return the cluster ARN directly
                // (only the namespace ARN), so build the well-known cluster ARN.
                let arn = format!(
                    "arn:aws:redshift:{}:{}:cluster:{}",
                    aws_account_dto.default_region, aws_account_dto.account_id, cluster_identifier
                );

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("cluster_identifier".to_string(), json!(cluster_identifier));
                resource_data.insert("arn".to_string(), json!(arn));

                if let Some(namespace_arn) = cluster.cluster_namespace_arn() {
                    resource_data.insert("cluster_namespace_arn".to_string(), json!(namespace_arn));
                }

                if let Some(node_type) = cluster.node_type() {
                    resource_data.insert("node_type".to_string(), json!(node_type));
                }

                if let Some(number_of_nodes) = cluster.number_of_nodes() {
                    resource_data.insert("number_of_nodes".to_string(), json!(number_of_nodes));
                }

                if let Some(status) = cluster.cluster_status() {
                    resource_data.insert("cluster_status".to_string(), json!(status));
                }

                if let Some(availability_status) = cluster.cluster_availability_status() {
                    resource_data.insert(
                        "cluster_availability_status".to_string(),
                        json!(availability_status),
                    );
                }

                if let Some(az) = cluster.availability_zone() {
                    resource_data.insert("availability_zone".to_string(), json!(az));
                }

                // multi_az is a string in the SDK ("Enabled"/"Disabled"), not a bool.
                if let Some(multi_az) = cluster.multi_az() {
                    resource_data.insert("multi_az".to_string(), json!(multi_az));
                }

                if let Some(publicly_accessible) = cluster.publicly_accessible() {
                    resource_data.insert(
                        "publicly_accessible".to_string(),
                        json!(publicly_accessible),
                    );
                }

                if let Some(encrypted) = cluster.encrypted() {
                    resource_data.insert("encrypted".to_string(), json!(encrypted));
                }

                if let Some(kms_key_id) = cluster.kms_key_id() {
                    resource_data.insert("kms_key_id".to_string(), json!(kms_key_id));
                }

                if let Some(enhanced_vpc_routing) = cluster.enhanced_vpc_routing() {
                    resource_data.insert(
                        "enhanced_vpc_routing".to_string(),
                        json!(enhanced_vpc_routing),
                    );
                }

                if let Some(retention) = cluster.automated_snapshot_retention_period() {
                    resource_data.insert(
                        "automated_snapshot_retention_period".to_string(),
                        json!(retention),
                    );
                }

                if let Some(manual_retention) = cluster.manual_snapshot_retention_period() {
                    resource_data.insert(
                        "manual_snapshot_retention_period".to_string(),
                        json!(manual_retention),
                    );
                }

                if let Some(version) = cluster.cluster_version() {
                    resource_data.insert("cluster_version".to_string(), json!(version));
                }

                if let Some(allow_version_upgrade) = cluster.allow_version_upgrade() {
                    resource_data.insert(
                        "allow_version_upgrade".to_string(),
                        json!(allow_version_upgrade),
                    );
                }

                if let Some(track) = cluster.maintenance_track_name() {
                    resource_data.insert("maintenance_track_name".to_string(), json!(track));
                }

                resource_data.insert(
                    "has_pending_modified_values".to_string(),
                    json!(cluster.pending_modified_values().is_some()),
                );

                if let Some(create_time) = cluster.cluster_create_time() {
                    let formatted = create_time
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", create_time));
                    resource_data.insert("cluster_create_time".to_string(), json!(formatted));
                }

                if let Some(db_name) = cluster.db_name() {
                    resource_data.insert("db_name".to_string(), json!(db_name));
                }

                if let Some(master_username) = cluster.master_username() {
                    resource_data.insert("master_username".to_string(), json!(master_username));
                }

                if let Some(vpc_id) = cluster.vpc_id() {
                    resource_data.insert("vpc_id".to_string(), json!(vpc_id));
                }

                if let Some(subnet_group) = cluster.cluster_subnet_group_name() {
                    resource_data
                        .insert("cluster_subnet_group_name".to_string(), json!(subnet_group));
                }

                if let Some(az_relocation) = cluster.availability_zone_relocation_status() {
                    resource_data.insert(
                        "availability_zone_relocation_status".to_string(),
                        json!(az_relocation),
                    );
                }

                if let Some(aqua) = cluster.aqua_configuration() {
                    if let Some(aqua_status) = aqua.aqua_status() {
                        resource_data
                            .insert("aqua_status".to_string(), json!(aqua_status.as_str()));
                    }
                }

                if let Some(capacity) = cluster.total_storage_capacity_in_mega_bytes() {
                    resource_data.insert(
                        "total_storage_capacity_in_mega_bytes".to_string(),
                        json!(capacity),
                    );
                }

                // Tags come inline on the Cluster; persist as an object map.
                let mut tags_map = serde_json::Map::new();
                for tag in cluster.tags() {
                    if let Some(key) = tag.key() {
                        tags_map.insert(key.to_string(), json!(tag.value().unwrap_or("")));
                    }
                }

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::RedshiftCluster.to_string(),
                    resource_id: cluster_identifier.clone(),
                    arn,
                    name: Some(cluster_identifier),
                    tags: serde_json::Value::Object(tags_map),
                    resource_data: serde_json::Value::Object(resource_data),
                };

                resources.push(dto.into());
            }

            marker = response.marker().map(String::from);
            if marker.is_none() {
                break;
            }
        }

        debug!(
            "Successfully synced {} Redshift clusters for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
