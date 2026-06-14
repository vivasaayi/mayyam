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

pub struct DataSyncControlPlane {
    aws_service: Arc<AwsService>,
}

impl DataSyncControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_tasks(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing DataSync tasks for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_datasync_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_tasks();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list DataSync tasks: {}", e);
                AppError::ExternalService(format!("Failed to list DataSync tasks: {}", e))
            })?;

            for task in response.tasks() {
                let arn = task.task_arn().unwrap_or("").to_string();
                if arn.is_empty() {
                    continue;
                }

                let mut resource_data = serde_json::Map::new();
                if let Some(status) = task.status() {
                    resource_data.insert("status".to_string(), json!(status.as_str()));
                }
                if let Some(name) = task.name() {
                    resource_data.insert("name".to_string(), json!(name));
                }
                if let Some(task_mode) = task.task_mode() {
                    resource_data.insert("task_mode".to_string(), json!(task_mode.as_str()));
                }

                let mut name = task.name().map(String::from);
                match client.describe_task().task_arn(&arn).send().await {
                    Ok(details) => {
                        if name.is_none() {
                            name = details.name().map(String::from);
                        }
                        if let Some(status) = details.status() {
                            resource_data.insert("status".to_string(), json!(status.as_str()));
                        }
                        if let Some(task_mode) = details.task_mode() {
                            resource_data
                                .insert("task_mode".to_string(), json!(task_mode.as_str()));
                        }
                        if let Some(current) = details.current_task_execution_arn() {
                            resource_data
                                .insert("current_task_execution_arn".to_string(), json!(current));
                        }
                        if let Some(source) = details.source_location_arn() {
                            resource_data.insert("source_location_arn".to_string(), json!(source));
                        }
                        if let Some(destination) = details.destination_location_arn() {
                            resource_data
                                .insert("destination_location_arn".to_string(), json!(destination));
                        }
                        if let Some(log_group) = details.cloud_watch_log_group_arn() {
                            resource_data
                                .insert("cloud_watch_log_group_arn".to_string(), json!(log_group));
                        }
                        resource_data.insert(
                            "has_cloudwatch_logs".to_string(),
                            json!(details.cloud_watch_log_group_arn().is_some()),
                        );
                        resource_data.insert(
                            "source_network_interface_count".to_string(),
                            json!(details.source_network_interface_arns().len()),
                        );
                        resource_data.insert(
                            "destination_network_interface_count".to_string(),
                            json!(details.destination_network_interface_arns().len()),
                        );
                        if let Some(schedule) = details.schedule() {
                            resource_data.insert(
                                "schedule_expression".to_string(),
                                json!(schedule.schedule_expression()),
                            );
                            if let Some(status) = schedule.status() {
                                resource_data
                                    .insert("schedule_status".to_string(), json!(status.as_str()));
                            }
                        }
                        resource_data.insert(
                            "has_schedule".to_string(),
                            json!(details.schedule().is_some()),
                        );
                        if let Some(error_code) = details.error_code() {
                            resource_data.insert("error_code".to_string(), json!(error_code));
                        }
                        if let Some(error_detail) = details.error_detail() {
                            resource_data.insert("error_detail".to_string(), json!(error_detail));
                        }
                        if let Some(options) = details.options() {
                            if let Some(verify_mode) = options.verify_mode() {
                                resource_data
                                    .insert("verify_mode".to_string(), json!(verify_mode.as_str()));
                            }
                            if let Some(transfer_mode) = options.transfer_mode() {
                                resource_data.insert(
                                    "transfer_mode".to_string(),
                                    json!(transfer_mode.as_str()),
                                );
                            }
                            if let Some(bytes) = options.bytes_per_second() {
                                resource_data.insert("bytes_per_second".to_string(), json!(bytes));
                            }
                            if let Some(log_level) = options.log_level() {
                                resource_data
                                    .insert("log_level".to_string(), json!(log_level.as_str()));
                            }
                            if let Some(task_queueing) = options.task_queueing() {
                                resource_data.insert(
                                    "task_queueing".to_string(),
                                    json!(task_queueing.as_str()),
                                );
                            }
                            if let Some(preserve_deleted) = options.preserve_deleted_files() {
                                resource_data.insert(
                                    "preserve_deleted_files".to_string(),
                                    json!(preserve_deleted.as_str()),
                                );
                            }
                            if let Some(object_tags) = options.object_tags() {
                                resource_data
                                    .insert("object_tags".to_string(), json!(object_tags.as_str()));
                            }
                            if let Some(security_copy) = options.security_descriptor_copy_flags() {
                                resource_data.insert(
                                    "security_descriptor_copy_flags".to_string(),
                                    json!(security_copy.as_str()),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Failed to describe DataSync task {}: {}", arn, e);
                    }
                }

                let mut tags_map = serde_json::Map::new();
                let mut tags_next_token: Option<String> = None;
                loop {
                    let mut tags_request = client.list_tags_for_resource().resource_arn(&arn);
                    if let Some(token) = tags_next_token {
                        tags_request = tags_request.next_token(token);
                    }

                    match tags_request.send().await {
                        Ok(tags_response) => {
                            for tag in tags_response.tags() {
                                tags_map.insert(
                                    tag.key().to_string(),
                                    json!(tag.value().unwrap_or("")),
                                );
                            }
                            tags_next_token = tags_response.next_token().map(String::from);
                            if tags_next_token.is_none() {
                                break;
                            }
                        }
                        Err(e) => {
                            debug!("Failed to list tags for DataSync task {}: {}", arn, e);
                            break;
                        }
                    }
                }

                let resource_id = data_sync_arn_resource_id(&arn);
                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::DataSyncTask.to_string(),
                    resource_id,
                    arn,
                    name,
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
            "Successfully synced {} DataSync tasks for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

fn data_sync_arn_resource_id(arn: &str) -> String {
    arn.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(arn)
        .to_string()
}
