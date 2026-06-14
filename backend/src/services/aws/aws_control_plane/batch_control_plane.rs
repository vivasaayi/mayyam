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

pub struct BatchControlPlane {
    aws_service: Arc<AwsService>,
}

impl BatchControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_compute_envs(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Batch compute environments for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_batch_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        // DescribeComputeEnvironments returns full ComputeEnvironmentDetail
        // objects directly (no separate describe call needed) and paginates
        // via next_token.
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.describe_compute_environments();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to describe Batch compute environments: {}", e);
                AppError::ExternalService(format!(
                    "Failed to describe Batch compute environments: {}",
                    e
                ))
            })?;

            for env in response.compute_environments() {
                let name = match env.compute_environment_name() {
                    Some(n) => n.to_string(),
                    None => {
                        debug!("Skipping Batch compute environment without a name");
                        continue;
                    }
                };
                let arn = env.compute_environment_arn().unwrap_or("").to_string();

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("compute_environment_name".to_string(), json!(name));
                resource_data.insert("compute_environment_arn".to_string(), json!(arn));

                // MANAGED or UNMANAGED.
                if let Some(ce_type) = env.r#type() {
                    resource_data.insert("type".to_string(), json!(ce_type.as_str()));
                }

                // ENABLED or DISABLED.
                if let Some(state) = env.state() {
                    resource_data.insert("state".to_string(), json!(state.as_str()));
                }

                // CREATING | UPDATING | DELETING | DELETED | VALID | INVALID.
                if let Some(status) = env.status() {
                    resource_data.insert("status".to_string(), json!(status.as_str()));
                }

                if let Some(reason) = env.status_reason() {
                    resource_data.insert("status_reason".to_string(), json!(reason));
                }

                if let Some(role) = env.service_role().filter(|r| !r.is_empty()) {
                    resource_data.insert("service_role".to_string(), json!(role));
                }

                // ECS (default) or EKS.
                if let Some(orchestration) = env.container_orchestration_type() {
                    resource_data.insert(
                        "container_orchestration_type".to_string(),
                        json!(orchestration.as_str()),
                    );
                }

                if let Some(uuid_value) = env.uuid() {
                    resource_data.insert("uuid".to_string(), json!(uuid_value));
                }

                if let Some(cluster_arn) = env.ecs_cluster_arn() {
                    resource_data.insert("ecs_cluster_arn".to_string(), json!(cluster_arn));
                }

                if let Some(unmanaged_vcpus) = env.unmanagedv_cpus() {
                    resource_data.insert("unmanagedv_cpus".to_string(), json!(unmanaged_vcpus));
                }

                // compute_resources only exists for MANAGED environments; the
                // pillar evaluators treat absent keys as a data gap.
                if let Some(cr) = env.compute_resources() {
                    // EC2 | SPOT | FARGATE | FARGATE_SPOT.
                    if let Some(cr_type) = cr.r#type() {
                        resource_data
                            .insert("compute_resource_type".to_string(), json!(cr_type.as_str()));
                    }

                    // BEST_FIT (default) | BEST_FIT_PROGRESSIVE |
                    // SPOT_CAPACITY_OPTIMIZED | SPOT_PRICE_CAPACITY_OPTIMIZED.
                    if let Some(strategy) = cr.allocation_strategy() {
                        resource_data
                            .insert("allocation_strategy".to_string(), json!(strategy.as_str()));
                    }

                    if let Some(min_vcpus) = cr.minv_cpus() {
                        resource_data.insert("minv_cpus".to_string(), json!(min_vcpus));
                    }

                    if let Some(max_vcpus) = cr.maxv_cpus() {
                        resource_data.insert("maxv_cpus".to_string(), json!(max_vcpus));
                    }

                    if let Some(desired_vcpus) = cr.desiredv_cpus() {
                        resource_data.insert("desiredv_cpus".to_string(), json!(desired_vcpus));
                    }

                    resource_data.insert("instance_types".to_string(), json!(cr.instance_types()));

                    if let Some(fleet_role) = cr.spot_iam_fleet_role() {
                        resource_data.insert("spot_iam_fleet_role".to_string(), json!(fleet_role));
                    }

                    resource_data.insert("subnet_count".to_string(), json!(cr.subnets().len()));
                    resource_data.insert(
                        "security_group_count".to_string(),
                        json!(cr.security_group_ids().len()),
                    );
                }

                // Tags are carried inline on ComputeEnvironmentDetail.
                let tags = match env.tags() {
                    Some(tag_map) => {
                        let mut tags_obj = serde_json::Map::new();
                        for (key, value) in tag_map {
                            tags_obj.insert(key.clone(), json!(value));
                        }
                        serde_json::Value::Object(tags_obj)
                    }
                    None => json!({}),
                };

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::BatchComputeEnv.to_string(),
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
            "Successfully synced {} Batch compute environments for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
