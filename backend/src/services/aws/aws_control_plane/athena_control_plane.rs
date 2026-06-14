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

pub struct AthenaControlPlane {
    aws_service: Arc<AwsService>,
}

impl AthenaControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_workgroups(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Athena workgroups for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_athena_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_work_groups();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list Athena workgroups: {}", e);
                AppError::ExternalService(format!("Failed to list Athena workgroups: {}", e))
            })?;

            for summary in response.work_groups() {
                let wg_name = match summary.name() {
                    Some(n) => n.to_string(),
                    None => {
                        debug!("Skipping Athena workgroup list entry without a name");
                        continue;
                    }
                };

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("name".to_string(), json!(wg_name));
                // Until GetWorkGroup succeeds and returns a configuration this
                // stays false so pillar evaluators can emit data-gap findings
                // instead of guessing.
                resource_data.insert("configuration_collected".to_string(), json!(false));

                // Summary-level fields act as fallbacks; the detail call below
                // overwrites them when it succeeds.
                if let Some(state) = summary.state() {
                    resource_data.insert("state".to_string(), json!(state.as_str()));
                }
                if let Some(description) = summary.description() {
                    resource_data.insert("description".to_string(), json!(description));
                }
                if let Some(creation_time) = summary.creation_time() {
                    let formatted = creation_time
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", creation_time));
                    resource_data.insert("creation_time".to_string(), json!(formatted));
                }
                if let Some(engine_version) = summary.engine_version() {
                    if let Some(selected) = engine_version.selected_engine_version() {
                        resource_data
                            .insert("engine_version_selected".to_string(), json!(selected));
                    }
                    if let Some(effective) = engine_version.effective_engine_version() {
                        resource_data
                            .insert("engine_version_effective".to_string(), json!(effective));
                    }
                }

                // Per-workgroup configuration detail. A failure here must not
                // fail the sync; the workgroup is persisted with summary data
                // only and configuration_collected stays false.
                match client.get_work_group().work_group(&wg_name).send().await {
                    Ok(detail) => {
                        if let Some(work_group) = detail.work_group() {
                            if let Some(state) = work_group.state() {
                                resource_data.insert("state".to_string(), json!(state.as_str()));
                            }
                            if let Some(description) = work_group.description() {
                                resource_data.insert("description".to_string(), json!(description));
                            }
                            if let Some(creation_time) = work_group.creation_time() {
                                let formatted = creation_time
                                    .fmt(aws_smithy_types::date_time::Format::DateTime)
                                    .unwrap_or_else(|_| format!("{:?}", creation_time));
                                resource_data.insert("creation_time".to_string(), json!(formatted));
                            }

                            if let Some(config) = work_group.configuration() {
                                resource_data
                                    .insert("configuration_collected".to_string(), json!(true));

                                if let Some(enforce) = config.enforce_work_group_configuration() {
                                    resource_data.insert(
                                        "enforce_work_group_configuration".to_string(),
                                        json!(enforce),
                                    );
                                }
                                if let Some(metrics) = config.publish_cloud_watch_metrics_enabled()
                                {
                                    resource_data.insert(
                                        "publish_cloud_watch_metrics_enabled".to_string(),
                                        json!(metrics),
                                    );
                                }
                                if let Some(cutoff) = config.bytes_scanned_cutoff_per_query() {
                                    resource_data.insert(
                                        "bytes_scanned_cutoff_per_query".to_string(),
                                        json!(cutoff),
                                    );
                                }
                                if let Some(requester_pays) = config.requester_pays_enabled() {
                                    resource_data.insert(
                                        "requester_pays_enabled".to_string(),
                                        json!(requester_pays),
                                    );
                                }
                                if let Some(min_enc) =
                                    config.enable_minimum_encryption_configuration()
                                {
                                    resource_data.insert(
                                        "enable_minimum_encryption_configuration".to_string(),
                                        json!(min_enc),
                                    );
                                }
                                if let Some(role) = config.execution_role() {
                                    resource_data.insert("execution_role".to_string(), json!(role));
                                }
                                if let Some(engine_version) = config.engine_version() {
                                    if let Some(selected) = engine_version.selected_engine_version()
                                    {
                                        resource_data.insert(
                                            "engine_version_selected".to_string(),
                                            json!(selected),
                                        );
                                    }
                                    if let Some(effective) =
                                        engine_version.effective_engine_version()
                                    {
                                        resource_data.insert(
                                            "engine_version_effective".to_string(),
                                            json!(effective),
                                        );
                                    }
                                }
                                if let Some(result_config) = config.result_configuration() {
                                    if let Some(output_location) = result_config.output_location() {
                                        resource_data.insert(
                                            "output_location".to_string(),
                                            json!(output_location),
                                        );
                                    }
                                    if let Some(encryption) =
                                        result_config.encryption_configuration()
                                    {
                                        resource_data.insert(
                                            "result_encryption_option".to_string(),
                                            json!(encryption.encryption_option().as_str()),
                                        );
                                        if let Some(kms_key) = encryption.kms_key() {
                                            resource_data.insert(
                                                "result_encryption_kms_key".to_string(),
                                                json!(kms_key),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to get Athena workgroup {}: {}", wg_name, e);
                    }
                }

                // Athena APIs do not return the workgroup ARN; it is
                // constructed from the account and region being synced.
                let arn = format!(
                    "arn:aws:athena:{}:{}:workgroup/{}",
                    aws_account_dto.default_region, aws_account_dto.account_id, wg_name
                );

                // Tags are best-effort, same as the KMS collector.
                let tags = match client
                    .list_tags_for_resource()
                    .resource_arn(&arn)
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
                            "Failed to list tags for Athena workgroup {}: {}",
                            wg_name, e
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
                    resource_type: AwsResourceType::AthenaWorkgroup.to_string(),
                    resource_id: wg_name.clone(),
                    arn,
                    name: Some(wg_name),
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
            "Successfully synced {} Athena workgroups for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
