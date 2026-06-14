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

pub struct AppRunnerControlPlane {
    aws_service: Arc<AwsService>,
}

impl AppRunnerControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_services(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing App Runner services for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_apprunner_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_services();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list App Runner services: {}", e);
                AppError::ExternalService(format!("Failed to list App Runner services: {}", e))
            })?;

            for summary in response.service_summary_list() {
                let summary_arn = match summary.service_arn() {
                    Some(arn) => arn,
                    None => {
                        debug!("Skipping App Runner service list entry without an ARN");
                        continue;
                    }
                };

                let describe_response = match client
                    .describe_service()
                    .service_arn(summary_arn)
                    .send()
                    .await
                {
                    Ok(res) => res,
                    Err(e) => {
                        error!(
                            "Failed to describe App Runner service {}: {}",
                            summary_arn, e
                        );
                        continue;
                    }
                };

                let service = match describe_response.service() {
                    Some(s) => s,
                    None => {
                        debug!("DescribeService returned no service for {}", summary_arn);
                        continue;
                    }
                };

                let service_arn = service.service_arn().to_string();
                let service_id = service.service_id().to_string();
                let service_name = service.service_name().to_string();

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("service_name".to_string(), json!(service_name));
                resource_data.insert("service_arn".to_string(), json!(service_arn));
                resource_data.insert("service_id".to_string(), json!(service_id));
                resource_data.insert("status".to_string(), json!(service.status().as_str()));

                if let Some(url) = service.service_url() {
                    resource_data.insert("service_url".to_string(), json!(url));
                }

                let created = service
                    .created_at()
                    .fmt(aws_smithy_types::date_time::Format::DateTime)
                    .unwrap_or_else(|_| format!("{:?}", service.created_at()));
                resource_data.insert("created_at".to_string(), json!(created));

                let updated = service
                    .updated_at()
                    .fmt(aws_smithy_types::date_time::Format::DateTime)
                    .unwrap_or_else(|_| format!("{:?}", service.updated_at()));
                resource_data.insert("updated_at".to_string(), json!(updated));

                if let Some(source) = service.source_configuration() {
                    let source_type = if source.image_repository().is_some() {
                        "IMAGE"
                    } else if source.code_repository().is_some() {
                        "CODE_REPOSITORY"
                    } else {
                        "UNKNOWN"
                    };
                    resource_data.insert("source_type".to_string(), json!(source_type));
                    if let Some(auto_deploy) = source.auto_deployments_enabled() {
                        resource_data
                            .insert("auto_deployments_enabled".to_string(), json!(auto_deploy));
                    }
                }

                if let Some(instance) = service.instance_configuration() {
                    // Marker that instance configuration was collected, so the
                    // evaluator can distinguish "no role" from a data gap.
                    resource_data
                        .insert("instance_configuration_collected".to_string(), json!(true));
                    if let Some(cpu) = instance.cpu() {
                        resource_data.insert("instance_cpu".to_string(), json!(cpu));
                    }
                    if let Some(memory) = instance.memory() {
                        resource_data.insert("instance_memory".to_string(), json!(memory));
                    }
                    if let Some(role_arn) = instance.instance_role_arn() {
                        resource_data.insert("instance_role_arn".to_string(), json!(role_arn));
                    }
                }

                // App Runner only returns an encryption configuration when the
                // service uses a customer-managed KMS key; absence means the
                // default AWS-managed key. Persist that semantics explicitly.
                match service.encryption_configuration() {
                    Some(encryption) => {
                        resource_data.insert("customer_managed_kms".to_string(), json!(true));
                        resource_data.insert("kms_key".to_string(), json!(encryption.kms_key()));
                    }
                    None => {
                        resource_data.insert("customer_managed_kms".to_string(), json!(false));
                    }
                }

                if let Some(health) = service.health_check_configuration() {
                    if let Some(protocol) = health.protocol() {
                        resource_data.insert(
                            "health_check_protocol".to_string(),
                            json!(protocol.as_str()),
                        );
                    }
                    if let Some(path) = health.path() {
                        resource_data.insert("health_check_path".to_string(), json!(path));
                    }
                    if let Some(interval) = health.interval() {
                        resource_data.insert("health_check_interval".to_string(), json!(interval));
                    }
                    if let Some(timeout) = health.timeout() {
                        resource_data.insert("health_check_timeout".to_string(), json!(timeout));
                    }
                    if let Some(healthy) = health.healthy_threshold() {
                        resource_data
                            .insert("health_check_healthy_threshold".to_string(), json!(healthy));
                    }
                    if let Some(unhealthy) = health.unhealthy_threshold() {
                        resource_data.insert(
                            "health_check_unhealthy_threshold".to_string(),
                            json!(unhealthy),
                        );
                    }
                }

                if let Some(auto_scaling) = service.auto_scaling_configuration_summary() {
                    if let Some(arn) = auto_scaling.auto_scaling_configuration_arn() {
                        resource_data
                            .insert("auto_scaling_configuration_arn".to_string(), json!(arn));
                    }
                    if let Some(name) = auto_scaling.auto_scaling_configuration_name() {
                        resource_data
                            .insert("auto_scaling_configuration_name".to_string(), json!(name));
                    }
                }

                if let Some(network) = service.network_configuration() {
                    if let Some(egress) = network.egress_configuration() {
                        if let Some(egress_type) = egress.egress_type() {
                            resource_data
                                .insert("egress_type".to_string(), json!(egress_type.as_str()));
                        }
                        if let Some(vpc_connector_arn) = egress.vpc_connector_arn() {
                            resource_data
                                .insert("vpc_connector_arn".to_string(), json!(vpc_connector_arn));
                        }
                    }
                    if let Some(ingress) = network.ingress_configuration() {
                        resource_data.insert(
                            "is_publicly_accessible".to_string(),
                            json!(ingress.is_publicly_accessible()),
                        );
                    }
                    if let Some(ip_type) = network.ip_address_type() {
                        resource_data
                            .insert("ip_address_type".to_string(), json!(ip_type.as_str()));
                    }
                }

                if let Some(observability) = service.observability_configuration() {
                    resource_data.insert(
                        "observability_enabled".to_string(),
                        json!(observability.observability_enabled()),
                    );
                    if let Some(obs_arn) = observability.observability_configuration_arn() {
                        resource_data.insert(
                            "observability_configuration_arn".to_string(),
                            json!(obs_arn),
                        );
                    }
                }

                let tags = match client
                    .list_tags_for_resource()
                    .resource_arn(&service_arn)
                    .send()
                    .await
                {
                    Ok(tags_response) => {
                        let mut tags_map = serde_json::Map::new();
                        for tag in tags_response.tags() {
                            if let Some(key) = tag.key() {
                                tags_map.insert(key.to_string(), json!(tag.value().unwrap_or("")));
                            }
                        }
                        serde_json::Value::Object(tags_map)
                    }
                    Err(e) => {
                        debug!(
                            "Failed to list tags for App Runner service {}: {}",
                            service_arn, e
                        );
                        json!({})
                    }
                };

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::AppRunnerService.to_string(),
                    resource_id: service_id,
                    arn: service_arn,
                    name: Some(service_name),
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
            "Successfully synced {} App Runner services for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
