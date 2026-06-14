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

pub struct LakeFormationControlPlane {
    aws_service: Arc<AwsService>,
}

impl LakeFormationControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_data_lake_resources(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Lake Formation resources for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_lakeformation_client(aws_account_dto)
            .await?;
        let settings = load_lakeformation_settings(&client).await;
        let lf_tag_count = count_lf_tags(&client).await;
        let mut resources = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_resources().max_results(100);
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list Lake Formation registered resources: {}", e);
                AppError::ExternalService(format!(
                    "Failed to list Lake Formation registered resources: {}",
                    e
                ))
            })?;

            for resource in response.resource_info_list() {
                let arn = resource.resource_arn().unwrap_or("").to_string();
                if arn.is_empty() {
                    continue;
                }

                let mut resource_data = settings.clone();
                resource_data.insert("resource_kind".to_string(), json!("registered_resource"));
                resource_data.insert("resource_arn".to_string(), json!(arn));
                resource_data.insert("lf_tag_count".to_string(), json!(lf_tag_count));
                if let Some(role_arn) = resource.role_arn() {
                    resource_data.insert("role_arn".to_string(), json!(role_arn));
                }
                resource_data.insert(
                    "with_federation".to_string(),
                    json!(resource.with_federation()),
                );
                resource_data.insert(
                    "hybrid_access_enabled".to_string(),
                    json!(resource.hybrid_access_enabled()),
                );
                if let Some(last_modified) = resource.last_modified() {
                    resource_data.insert(
                        "last_modified".to_string(),
                        json!(last_modified.to_string()),
                    );
                }

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::LakeFormationDataLake.to_string(),
                    resource_id: lakeformation_resource_id(&arn),
                    arn: arn.clone(),
                    name: Some(lakeformation_resource_id(&arn)),
                    tags: json!({}),
                    resource_data: serde_json::Value::Object(resource_data),
                };
                resources.push(dto.into());
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        if resources.is_empty() {
            let arn = format!(
                "arn:aws:lakeformation:{}:{}:data-lake/settings",
                aws_account_dto.default_region, aws_account_dto.account_id
            );
            let mut resource_data = settings;
            resource_data.insert("resource_kind".to_string(), json!("settings"));
            resource_data.insert("lf_tag_count".to_string(), json!(lf_tag_count));
            resources.push(
                AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::LakeFormationDataLake.to_string(),
                    resource_id: "data-lake-settings".to_string(),
                    arn,
                    name: Some("Lake Formation Settings".to_string()),
                    tags: json!({}),
                    resource_data: serde_json::Value::Object(resource_data),
                }
                .into(),
            );
        }

        debug!(
            "Successfully synced {} Lake Formation resources for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

async fn load_lakeformation_settings(
    client: &aws_sdk_lakeformation::Client,
) -> serde_json::Map<String, serde_json::Value> {
    let mut data = serde_json::Map::new();

    match client.get_data_lake_settings().send().await {
        Ok(response) => {
            if let Some(settings) = response.data_lake_settings() {
                data.insert(
                    "data_lake_admin_count".to_string(),
                    json!(settings.data_lake_admins().len()),
                );
                data.insert(
                    "create_database_default_permissions_count".to_string(),
                    json!(settings.create_database_default_permissions().len()),
                );
                data.insert(
                    "create_table_default_permissions_count".to_string(),
                    json!(settings.create_table_default_permissions().len()),
                );
                data.insert(
                    "trusted_resource_owners_count".to_string(),
                    json!(settings.trusted_resource_owners().len()),
                );
                data.insert(
                    "allow_external_data_filtering".to_string(),
                    json!(settings.allow_external_data_filtering()),
                );
            }
        }
        Err(e) => {
            debug!("Failed to get Lake Formation data lake settings: {}", e);
        }
    }

    data
}

async fn count_lf_tags(client: &aws_sdk_lakeformation::Client) -> usize {
    let mut count = 0usize;
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client.list_lf_tags().max_results(100);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        match request.send().await {
            Ok(response) => {
                count += response.lf_tags().len();
                next_token = response.next_token().map(String::from);
                if next_token.is_none() {
                    break;
                }
            }
            Err(e) => {
                debug!("Failed to list Lake Formation LF-Tags: {}", e);
                break;
            }
        }
    }

    count
}

fn lakeformation_resource_id(arn: &str) -> String {
    arn.rsplit(':')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(arn)
        .to_string()
}
