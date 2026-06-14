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
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct AutoScalingControlPlane {
    aws_service: Arc<AwsService>,
}

impl AutoScalingControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_auto_scaling_groups(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Auto Scaling groups for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_autoscaling_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.describe_auto_scaling_groups();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let collection_started_at = Utc::now();
            let response = request.send().await.map_err(|e| {
                error!("Failed to describe Auto Scaling groups: {}", e);
                AppError::ExternalService(format!("Failed to describe Auto Scaling groups: {}", e))
            })?;
            let collection_completed_at = Utc::now();
            let duration_ms = (collection_completed_at - collection_started_at).num_milliseconds();

            for group in response.auto_scaling_groups() {
                let name = match group.auto_scaling_group_name() {
                    Some(name) => name.to_string(),
                    None => {
                        debug!("Skipping Auto Scaling group entry without a name");
                        continue;
                    }
                };

                let arn = group.auto_scaling_group_arn().unwrap_or("").to_string();

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("auto_scaling_group_name".to_string(), json!(name));
                resource_data.insert("arn".to_string(), json!(arn));
                resource_data.insert(
                    "telemetry_source".to_string(),
                    json!("autoscaling:DescribeAutoScalingGroups"),
                );
                resource_data.insert(
                    "telemetry_collection_started_at".to_string(),
                    json!(collection_started_at.to_rfc3339()),
                );
                resource_data.insert(
                    "telemetry_collection_completed_at".to_string(),
                    json!(collection_completed_at.to_rfc3339()),
                );
                resource_data.insert(
                    "telemetry_collection_duration_ms".to_string(),
                    json!(duration_ms.max(0)),
                );
                resource_data.insert("telemetry_collection_success_count".to_string(), json!(1));
                resource_data.insert("telemetry_collection_failure_count".to_string(), json!(0));
                resource_data.insert("telemetry_collection_error_count".to_string(), json!(0));
                resource_data.insert("telemetry_collection_errors".to_string(), json!([]));

                if let Some(min_size) = group.min_size() {
                    resource_data.insert("min_size".to_string(), json!(min_size));
                }

                if let Some(max_size) = group.max_size() {
                    resource_data.insert("max_size".to_string(), json!(max_size));
                }

                if let Some(desired_capacity) = group.desired_capacity() {
                    resource_data.insert("desired_capacity".to_string(), json!(desired_capacity));
                }

                resource_data.insert(
                    "availability_zones".to_string(),
                    json!(group.availability_zones()),
                );

                if let Some(health_check_type) = group.health_check_type() {
                    resource_data.insert("health_check_type".to_string(), json!(health_check_type));
                }

                if let Some(grace_period) = group.health_check_grace_period() {
                    resource_data
                        .insert("health_check_grace_period".to_string(), json!(grace_period));
                }

                resource_data.insert(
                    "load_balancer_names".to_string(),
                    json!(group.load_balancer_names()),
                );
                resource_data.insert(
                    "target_group_arns".to_string(),
                    json!(group.target_group_arns()),
                );

                if let Some(launch_configuration_name) = group.launch_configuration_name() {
                    resource_data.insert(
                        "launch_configuration_name".to_string(),
                        json!(launch_configuration_name),
                    );
                }

                resource_data.insert(
                    "uses_launch_template".to_string(),
                    json!(group.launch_template().is_some()),
                );
                resource_data.insert(
                    "uses_mixed_instances_policy".to_string(),
                    json!(group.mixed_instances_policy().is_some()),
                );

                let enabled_metrics = group
                    .enabled_metrics()
                    .iter()
                    .filter_map(|metric| metric.metric().map(String::from))
                    .collect::<Vec<_>>();
                resource_data.insert(
                    "enabled_metric_count".to_string(),
                    json!(enabled_metrics.len()),
                );
                resource_data.insert("enabled_metrics".to_string(), json!(enabled_metrics));

                resource_data.insert("instance_count".to_string(), json!(group.instances().len()));
                let instance_health = group
                    .instances()
                    .iter()
                    .map(|instance| {
                        json!({
                            "instance_id": instance.instance_id(),
                            "health_status": instance.health_status(),
                            "lifecycle_state": instance.lifecycle_state().map(|state| state.as_str()),
                        })
                    })
                    .collect::<Vec<_>>();
                let healthy_instance_count = group
                    .instances()
                    .iter()
                    .filter(|instance| instance.health_status() == Some("Healthy"))
                    .count();
                let unhealthy_instance_count = group
                    .instances()
                    .iter()
                    .filter(|instance| instance.health_status() != Some("Healthy"))
                    .count();
                let in_service_instance_count = group
                    .instances()
                    .iter()
                    .filter(|instance| {
                        instance
                            .lifecycle_state()
                            .map(|state| state.as_str() == "InService")
                            .unwrap_or(false)
                    })
                    .count();
                resource_data.insert("instance_health".to_string(), json!(instance_health));
                resource_data.insert(
                    "healthy_instance_count".to_string(),
                    json!(healthy_instance_count),
                );
                resource_data.insert(
                    "unhealthy_instance_count".to_string(),
                    json!(unhealthy_instance_count),
                );
                resource_data.insert(
                    "in_service_instance_count".to_string(),
                    json!(in_service_instance_count),
                );

                resource_data.insert(
                    "suspended_process_count".to_string(),
                    json!(group.suspended_processes().len()),
                );

                if let Some(vpc_zone_identifier) = group.vpc_zone_identifier() {
                    resource_data.insert(
                        "vpc_zone_identifier".to_string(),
                        json!(vpc_zone_identifier),
                    );
                }

                if let Some(status) = group.status() {
                    resource_data.insert("status".to_string(), json!(status));
                }

                if let Some(capacity_rebalance) = group.capacity_rebalance() {
                    resource_data
                        .insert("capacity_rebalance".to_string(), json!(capacity_rebalance));
                }

                if let Some(max_instance_lifetime) = group.max_instance_lifetime() {
                    resource_data.insert(
                        "max_instance_lifetime".to_string(),
                        json!(max_instance_lifetime),
                    );
                }

                if let Some(protected) = group.new_instances_protected_from_scale_in() {
                    resource_data.insert(
                        "new_instances_protected_from_scale_in".to_string(),
                        json!(protected),
                    );
                }

                if let Some(created_time) = group.created_time() {
                    let formatted = created_time
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", created_time));
                    resource_data.insert("created_time".to_string(), json!(formatted));
                }

                let mut tags_map = serde_json::Map::new();
                for tag in group.tags() {
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
                    resource_type: AwsResourceType::AutoScalingGroup.to_string(),
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
            "Successfully synced {} Auto Scaling groups for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
