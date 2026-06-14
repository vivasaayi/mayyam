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
use aws_sdk_emr::types::ClusterState;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct EmrControlPlane {
    aws_service: Arc<AwsService>,
}

impl EmrControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_clusters(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing EMR clusters for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_emr_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut marker: Option<String> = None;

        loop {
            // Like the other live-resource collectors, only active clusters are
            // synced; terminated EMR clusters are dead resources that no longer
            // bill or serve traffic.
            let mut request = client
                .list_clusters()
                .cluster_states(ClusterState::Starting)
                .cluster_states(ClusterState::Bootstrapping)
                .cluster_states(ClusterState::Running)
                .cluster_states(ClusterState::Waiting);
            if let Some(m) = marker {
                request = request.marker(m);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list EMR clusters: {}", e);
                AppError::ExternalService(format!("Failed to list EMR clusters: {}", e))
            })?;

            for summary in response.clusters() {
                let cluster_id = match summary.id() {
                    Some(id) => id.to_string(),
                    None => {
                        debug!("Skipping EMR cluster list entry without a cluster id");
                        continue;
                    }
                };

                let summary_name = summary.name().unwrap_or(&cluster_id).to_string();
                let summary_arn = summary.cluster_arn().unwrap_or("").to_string();

                // Summary fields are always persisted so a describe failure
                // still leaves a usable inventory row.
                let mut resource_data = serde_json::Map::new();
                resource_data.insert("id".to_string(), json!(cluster_id));
                resource_data.insert("name".to_string(), json!(summary_name));
                resource_data.insert("cluster_arn".to_string(), json!(summary_arn));
                if let Some(outpost_arn) = summary.outpost_arn() {
                    resource_data.insert("outpost_arn".to_string(), json!(outpost_arn));
                }
                if let Some(hours) = summary.normalized_instance_hours() {
                    resource_data.insert("normalized_instance_hours".to_string(), json!(hours));
                }
                if let Some(status) = summary.status() {
                    if let Some(state) = status.state() {
                        resource_data.insert("state".to_string(), json!(state.as_str()));
                    }
                }

                let mut arn = summary_arn.clone();
                let mut name = summary_name.clone();
                let mut tags = json!({});

                let described = match client
                    .describe_cluster()
                    .cluster_id(&cluster_id)
                    .send()
                    .await
                {
                    Ok(res) => res.cluster().cloned(),
                    Err(e) => {
                        error!("Failed to describe EMR cluster {}: {}", cluster_id, e);
                        None
                    }
                };

                match described {
                    Some(cluster) => {
                        resource_data.insert("collected".to_string(), json!(true));

                        if let Some(n) = cluster.name() {
                            name = n.to_string();
                            resource_data.insert("name".to_string(), json!(n));
                        }
                        if let Some(a) = cluster.cluster_arn() {
                            arn = a.to_string();
                            resource_data.insert("cluster_arn".to_string(), json!(a));
                        }
                        if let Some(outpost_arn) = cluster.outpost_arn() {
                            resource_data.insert("outpost_arn".to_string(), json!(outpost_arn));
                        }

                        if let Some(status) = cluster.status() {
                            if let Some(state) = status.state() {
                                resource_data.insert("state".to_string(), json!(state.as_str()));
                            }
                            if let Some(reason) = status.state_change_reason() {
                                if let Some(code) = reason.code() {
                                    resource_data.insert(
                                        "state_change_reason_code".to_string(),
                                        json!(code.as_str()),
                                    );
                                }
                                if let Some(message) = reason.message() {
                                    resource_data.insert(
                                        "state_change_reason_message".to_string(),
                                        json!(message),
                                    );
                                }
                            }
                            if let Some(timeline) = status.timeline() {
                                if let Some(created) = timeline.creation_date_time() {
                                    let formatted = created
                                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                                        .unwrap_or_else(|_| format!("{:?}", created));
                                    resource_data
                                        .insert("creation_date_time".to_string(), json!(formatted));
                                }
                            }
                        }

                        if let Some(hours) = cluster.normalized_instance_hours() {
                            resource_data
                                .insert("normalized_instance_hours".to_string(), json!(hours));
                        }
                        if let Some(release_label) = cluster.release_label() {
                            resource_data.insert("release_label".to_string(), json!(release_label));
                        }
                        if let Some(auto_terminate) = cluster.auto_terminate() {
                            resource_data
                                .insert("auto_terminate".to_string(), json!(auto_terminate));
                        }
                        if let Some(protected) = cluster.termination_protected() {
                            resource_data
                                .insert("termination_protected".to_string(), json!(protected));
                        }
                        if let Some(visible) = cluster.visible_to_all_users() {
                            resource_data
                                .insert("visible_to_all_users".to_string(), json!(visible));
                        }
                        if let Some(unhealthy_replacement) = cluster.unhealthy_node_replacement() {
                            resource_data.insert(
                                "unhealthy_node_replacement".to_string(),
                                json!(unhealthy_replacement),
                            );
                        }

                        let applications: Vec<serde_json::Value> = cluster
                            .applications()
                            .iter()
                            .map(|app| {
                                json!({
                                    "name": app.name(),
                                    "version": app.version(),
                                })
                            })
                            .collect();
                        resource_data.insert("applications".to_string(), json!(applications));

                        if let Some(security_configuration) = cluster.security_configuration() {
                            resource_data.insert(
                                "security_configuration".to_string(),
                                json!(security_configuration),
                            );
                        }
                        if let Some(log_uri) = cluster.log_uri() {
                            resource_data.insert("log_uri".to_string(), json!(log_uri));
                        }
                        if let Some(log_kms_key) = cluster.log_encryption_kms_key_id() {
                            resource_data.insert(
                                "log_encryption_kms_key_id".to_string(),
                                json!(log_kms_key),
                            );
                        }
                        if let Some(service_role) = cluster.service_role() {
                            resource_data.insert("service_role".to_string(), json!(service_role));
                        }
                        if let Some(auto_scaling_role) = cluster.auto_scaling_role() {
                            resource_data
                                .insert("auto_scaling_role".to_string(), json!(auto_scaling_role));
                        }
                        if let Some(scale_down) = cluster.scale_down_behavior() {
                            resource_data.insert(
                                "scale_down_behavior".to_string(),
                                json!(scale_down.as_str()),
                            );
                        }
                        if let Some(ebs_root) = cluster.ebs_root_volume_size() {
                            resource_data
                                .insert("ebs_root_volume_size".to_string(), json!(ebs_root));
                        }
                        if let Some(collection_type) = cluster.instance_collection_type() {
                            resource_data.insert(
                                "instance_collection_type".to_string(),
                                json!(collection_type.as_str()),
                            );
                        }

                        // Tags come inline on the describe_cluster Cluster struct;
                        // persist as an object map like the other collectors.
                        let mut tags_map = serde_json::Map::new();
                        for tag in cluster.tags() {
                            if let Some(key) = tag.key() {
                                tags_map.insert(key.to_string(), json!(tag.value().unwrap_or("")));
                            }
                        }
                        tags = serde_json::Value::Object(tags_map);
                    }
                    None => {
                        // Describe failed or returned no cluster: degrade
                        // gracefully and let evaluators report the data gap.
                        resource_data.insert("collected".to_string(), json!(false));
                    }
                }

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::EmrCluster.to_string(),
                    resource_id: cluster_id,
                    arn,
                    name: Some(name),
                    tags,
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
            "Successfully synced {} EMR clusters for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
