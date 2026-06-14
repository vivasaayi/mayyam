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

pub struct StepFunctionsControlPlane {
    aws_service: Arc<AwsService>,
}

impl StepFunctionsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_state_machines(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Step Functions state machines for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_sfn_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_state_machines();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list Step Functions state machines: {}", e);
                AppError::ExternalService(format!(
                    "Failed to list Step Functions state machines: {}",
                    e
                ))
            })?;

            for machine in response.state_machines() {
                let name = machine.name().to_string();
                let arn = machine.state_machine_arn().to_string();

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("name".to_string(), json!(name));
                resource_data.insert("arn".to_string(), json!(arn));
                resource_data.insert(
                    "state_machine_type".to_string(),
                    json!(machine.r#type().as_str()),
                );

                let created = machine
                    .creation_date()
                    .fmt(aws_smithy_types::date_time::Format::DateTime)
                    .unwrap_or_else(|_| format!("{:?}", machine.creation_date()));
                resource_data.insert("creation_date".to_string(), json!(created));

                // Per-machine enrichment must never fail the whole sync. On a
                // describe error (e.g. missing kms:Decrypt on an encrypted
                // definition) the list-level fields above are still persisted
                // and the evaluators report the gap.
                match client
                    .describe_state_machine()
                    .state_machine_arn(&arn)
                    .send()
                    .await
                {
                    Ok(detail) => {
                        if let Some(status) = detail.status() {
                            resource_data.insert("status".to_string(), json!(status.as_str()));
                        }

                        resource_data.insert("role_arn".to_string(), json!(detail.role_arn()));
                        resource_data.insert(
                            "state_machine_type".to_string(),
                            json!(detail.r#type().as_str()),
                        );

                        if let Some(description) = detail.description() {
                            resource_data.insert("description".to_string(), json!(description));
                        }

                        if let Some(logging) = detail.logging_configuration() {
                            if let Some(level) = logging.level() {
                                resource_data
                                    .insert("logging_level".to_string(), json!(level.as_str()));
                            }
                            resource_data.insert(
                                "logging_include_execution_data".to_string(),
                                json!(logging.include_execution_data()),
                            );
                            resource_data.insert(
                                "logging_destination_count".to_string(),
                                json!(logging.destinations().len()),
                            );
                        }

                        if let Some(tracing_config) = detail.tracing_configuration() {
                            resource_data.insert(
                                "tracing_enabled".to_string(),
                                json!(tracing_config.enabled()),
                            );
                        }

                        if let Some(encryption) = detail.encryption_configuration() {
                            resource_data.insert(
                                "encryption_type".to_string(),
                                json!(encryption.r#type().as_str()),
                            );
                            if let Some(kms_key_id) = encryption.kms_key_id() {
                                resource_data.insert("kms_key_id".to_string(), json!(kms_key_id));
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to describe state machine {}: {}", arn, e);
                    }
                }

                let tags = match client
                    .list_tags_for_resource()
                    .resource_arn(&arn)
                    .send()
                    .await
                {
                    Ok(tags_response) => {
                        let mut tags_map = serde_json::Map::new();
                        for tag in tags_response.tags() {
                            if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                                tags_map.insert(key.to_string(), json!(value));
                            }
                        }
                        serde_json::Value::Object(tags_map)
                    }
                    Err(e) => {
                        debug!("Failed to list tags for state machine {}: {}", arn, e);
                        json!({})
                    }
                };

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::StepFunction.to_string(),
                    resource_id: name.clone(),
                    arn,
                    name: Some(name),
                    tags,
                    resource_data: serde_json::Value::Object(resource_data),
                };

                resources.push(dto.into());
            }

            match response.next_token() {
                Some(token) => next_token = Some(token.to_string()),
                None => break,
            }
        }

        debug!(
            "Successfully synced {} Step Functions state machines for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
