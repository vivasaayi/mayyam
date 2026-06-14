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

pub struct SecretsManagerControlPlane {
    aws_service: Arc<AwsService>,
}

impl SecretsManagerControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_secrets(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Secrets Manager secrets for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_secretsmanager_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_secrets().max_results(100);
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list Secrets Manager secrets: {}", e);
                AppError::ExternalService(format!("Failed to list Secrets Manager secrets: {}", e))
            })?;

            for entry in response.secret_list() {
                let name = match entry.name() {
                    Some(name) => name.to_string(),
                    None => {
                        debug!("Skipping Secrets Manager secret entry without a name");
                        continue;
                    }
                };

                let arn = entry.arn().unwrap_or("").to_string();

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("name".to_string(), json!(name));
                resource_data.insert("arn".to_string(), json!(arn));

                if let Some(description) = entry.description() {
                    resource_data.insert("description".to_string(), json!(description));
                }

                if let Some(kms_key_id) = entry.kms_key_id() {
                    resource_data.insert("kms_key_id".to_string(), json!(kms_key_id));
                }

                if let Some(rotation_enabled) = entry.rotation_enabled() {
                    resource_data.insert("rotation_enabled".to_string(), json!(rotation_enabled));
                }

                if let Some(rotation_lambda_arn) = entry.rotation_lambda_arn() {
                    resource_data.insert(
                        "rotation_lambda_arn".to_string(),
                        json!(rotation_lambda_arn),
                    );
                }

                if let Some(rotation_rules) = entry.rotation_rules() {
                    if let Some(automatically_after_days) =
                        rotation_rules.automatically_after_days()
                    {
                        resource_data.insert(
                            "automatically_after_days".to_string(),
                            json!(automatically_after_days),
                        );
                    }
                }

                if let Some(last_rotated_date) = entry.last_rotated_date() {
                    let formatted = last_rotated_date
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", last_rotated_date));
                    resource_data.insert("last_rotated_date".to_string(), json!(formatted));
                }

                if let Some(last_accessed_date) = entry.last_accessed_date() {
                    let formatted = last_accessed_date
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", last_accessed_date));
                    resource_data.insert("last_accessed_date".to_string(), json!(formatted));
                }

                if let Some(last_changed_date) = entry.last_changed_date() {
                    let formatted = last_changed_date
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", last_changed_date));
                    resource_data.insert("last_changed_date".to_string(), json!(formatted));
                }

                if let Some(created_date) = entry.created_date() {
                    let formatted = created_date
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", created_date));
                    resource_data.insert("created_date".to_string(), json!(formatted));
                }

                if let Some(owning_service) = entry.owning_service() {
                    resource_data.insert("owning_service".to_string(), json!(owning_service));
                }

                if let Some(primary_region) = entry.primary_region() {
                    resource_data.insert("primary_region".to_string(), json!(primary_region));
                }

                let mut tags_map = serde_json::Map::new();
                for tag in entry.tags() {
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
                    resource_type: AwsResourceType::SecretsManagerSecret.to_string(),
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
            "Successfully synced {} Secrets Manager secrets for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
