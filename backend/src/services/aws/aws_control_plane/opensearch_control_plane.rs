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
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, AwsResourceType, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use serde_json::json;
use std::sync::Arc;

pub struct OpenSearchControlPlane {
    aws_service: Arc<AwsService>,
}

impl OpenSearchControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_domains(
        &self,
        account_id: &str,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self
            .aws_service
            .create_opensearch_client(aws_account_dto)
            .await?;

        let repo = &self.aws_service.aws_resource_repo;

        // List all OpenSearch domains from AWS
        let list_response = client.list_domain_names().send().await.map_err(|e| {
            AppError::ExternalService(format!("Failed to list OpenSearch domains: {}", e))
        })?;

        let mut domains = Vec::new();

        for domain_name_entry in list_response.domain_names() {
            if let Some(domain_name) = domain_name_entry.domain_name() {
                // Get detailed information for the domain
                let describe_response = client
                    .describe_domain()
                    .domain_name(domain_name)
                    .send()
                    .await
                    .map_err(|e| {
                        AppError::ExternalService(format!(
                            "Failed to describe OpenSearch domain {}: {}",
                            domain_name, e
                        ))
                    })?;

                if let Some(domain_status) = describe_response.domain_status() {
                    // Get tags for the domain
                    let tags_response = client
                        .list_tags()
                        .arn(domain_status.arn())
                        .send()
                        .await
                        .map_err(|e| {
                            AppError::ExternalService(format!(
                                "Failed to get tags for OpenSearch domain {}: {}",
                                domain_name, e
                            ))
                        })?;

                    // Process tags
                    let mut tags_map = serde_json::Map::new();
                    let mut name = None;

                    // for tag in tags_response.tag_list() {
                    //     if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                    //         if key == "Name" {
                    //             name = Some(value.to_string());
                    //         }
                    //         tags_map.insert(key.to_string(), json!(value));
                    //     }
                    // }

                    // If no name tag was found, use the domain name
                    if name.is_none() {
                        name = Some(domain_name.to_string());
                    }

                    // Build domain data
                    let mut domain_data = serde_json::Map::new();

                    domain_data.insert("domain_name".to_string(), json!(domain_name));
                    domain_data.insert("domain_id".to_string(), json!(domain_status.domain_id()));
                    domain_data.insert(
                        "engine_version".to_string(),
                        json!(domain_status.engine_version()),
                    );

                    // Process cluster configuration
                    if let Some(cluster_config) = domain_status.cluster_config() {
                        let mut config = serde_json::Map::new();

                        if let Some(instance_type) =
                            cluster_config.instance_type().map(|it| it.as_str())
                        {
                            config.insert("instance_type".to_string(), json!(instance_type));
                        }

                        if let Some(instance_count) = cluster_config.instance_count() {
                            config.insert("instance_count".to_string(), json!(instance_count));
                        }

                        if let Some(dedicated_master_enabled) =
                            cluster_config.dedicated_master_enabled()
                        {
                            config.insert(
                                "dedicated_master_enabled".to_string(),
                                json!(dedicated_master_enabled),
                            );
                        }

                        if let Some(zone_awareness_enabled) =
                            cluster_config.zone_awareness_enabled()
                        {
                            config.insert(
                                "zone_awareness_enabled".to_string(),
                                json!(zone_awareness_enabled),
                            );
                        }

                        domain_data.insert("cluster_config".to_string(), json!(config));
                    }

                    // Process EBS options
                    if let Some(ebs_options) = domain_status.ebs_options() {
                        let mut ebs = serde_json::Map::new();

                        if let Some(ebs_enabled) = ebs_options.ebs_enabled() {
                            ebs.insert("ebs_enabled".to_string(), json!(ebs_enabled));
                        }

                        if let Some(volume_type) = ebs_options.volume_type().map(|vt| vt.as_str()) {
                            ebs.insert("volume_type".to_string(), json!(volume_type));
                        }

                        if let Some(volume_size) = ebs_options.volume_size() {
                            ebs.insert("volume_size".to_string(), json!(volume_size));
                        }

                        if let Some(iops) = ebs_options.iops() {
                            ebs.insert("iops".to_string(), json!(iops));
                        }

                        domain_data.insert("ebs_options".to_string(), json!(ebs));
                    }

                    // Get access policies
                    if let Some(access_policies) = domain_status.access_policies() {
                        domain_data.insert("access_policies".to_string(), json!(access_policies));
                    }

                    // Process snapshot options
                    if let Some(snapshot_options) = domain_status.snapshot_options() {
                        let mut snapshot = serde_json::Map::new();

                        if let Some(automated_snapshot_start_hour) =
                            snapshot_options.automated_snapshot_start_hour()
                        {
                            snapshot.insert(
                                "automated_snapshot_start_hour".to_string(),
                                json!(automated_snapshot_start_hour),
                            );
                        }

                        domain_data.insert("snapshot_options".to_string(), json!(snapshot));
                    }

                    // Process VPC options
                    if let Some(vpc_options) = domain_status.vpc_options() {
                        let mut vpc = serde_json::Map::new();

                        if let Some(vpc_id) = vpc_options.vpc_id() {
                            vpc.insert("vpc_id".to_string(), json!(vpc_id));
                        }

                        vpc.insert("subnet_ids".to_string(), json!(vpc_options.subnet_ids()));
                        vpc.insert(
                            "security_group_ids".to_string(),
                            json!(vpc_options.security_group_ids()),
                        );

                        domain_data.insert("vpc_options".to_string(), json!(vpc));
                    } else {
                        domain_data.insert("vpc_options".to_string(), json!(null));
                    }

                    // Process encryption options
                    if let Some(encryption) = domain_status.encryption_at_rest_options() {
                        let mut encryption_data = serde_json::Map::new();

                        if let Some(enabled) = encryption.enabled() {
                            encryption_data.insert("enabled".to_string(), json!(enabled));
                        }

                        domain_data.insert(
                            "encryption_at_rest_options".to_string(),
                            json!(encryption_data),
                        );
                    }

                    // Process node-to-node encryption
                    if let Some(node_to_node) = domain_status.node_to_node_encryption_options() {
                        let mut node_encryption = serde_json::Map::new();

                        if let Some(enabled) = node_to_node.enabled() {
                            node_encryption.insert("enabled".to_string(), json!(enabled));
                        }

                        domain_data.insert(
                            "node_to_node_encryption_options".to_string(),
                            json!(node_encryption),
                        );
                    }

                    // Process advanced options
                    if let Some(advanced_options) = domain_status.advanced_options() {
                        domain_data.insert("advanced_options".to_string(), json!(advanced_options));
                    }

                    // Process endpoints
                    if let Some(endpoints) = domain_status.endpoints() {
                        domain_data.insert("endpoints".to_string(), json!(endpoints));
                    }

                    // Create resource DTO
                    let domain = AwsResourceDto {
                        id: None,
                        account_id: aws_account_dto.account_id.clone(),
                        profile: aws_account_dto.profile.clone(),
                        region: aws_account_dto.default_region.clone(),
                        resource_type: AwsResourceType::OpenSearchDomain.to_string(),
                        resource_id: domain_name.to_string(),
                        arn: domain_name.clone().to_string(), // Clone to avoid move
                        name,
                        tags: serde_json::Value::Object(tags_map),
                        resource_data: serde_json::Value::Object(domain_data),
                        sync_id: None,
                    };

                    // Save to database
                    let saved_domain = match repo.find_by_arn(&domain_name.clone()).await? {
                        Some(existing) => repo.update(existing.id, &domain).await?,
                        None => repo.create(&domain).await?,
                    };

                    domains.push(saved_domain);
                }
            }
        }

        Ok(domains)
    }
}
