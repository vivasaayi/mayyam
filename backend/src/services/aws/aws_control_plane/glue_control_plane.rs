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

/// Upper bound for the per-database table sample. One bounded GetTables call
/// per database; the persisted count is exact only when the response is not
/// truncated, and `table_count_truncated` records that.
const TABLE_SAMPLE_MAX_RESULTS: i32 = 10;

pub struct GlueControlPlane {
    aws_service: Arc<AwsService>,
}

impl GlueControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_databases(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Glue databases for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_glue_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.get_databases();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list Glue databases: {}", e);
                AppError::ExternalService(format!("Failed to list Glue databases: {}", e))
            })?;

            for database in response.database_list() {
                let name = database.name().to_string();
                let arn = format!(
                    "arn:aws:glue:{}:{}:database/{}",
                    aws_account_dto.default_region, aws_account_dto.account_id, name
                );

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("name".to_string(), json!(name));
                resource_data.insert("arn".to_string(), json!(arn));

                if let Some(description) = database.description() {
                    resource_data.insert("description".to_string(), json!(description));
                }

                if let Some(location_uri) = database.location_uri() {
                    resource_data.insert("location_uri".to_string(), json!(location_uri));
                }

                if let Some(catalog_id) = database.catalog_id() {
                    resource_data.insert("catalog_id".to_string(), json!(catalog_id));
                }

                if let Some(create_time) = database.create_time() {
                    let formatted = create_time
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", create_time));
                    resource_data.insert("create_time".to_string(), json!(formatted));
                }

                let parameters_count = database
                    .parameters()
                    .map(|params| params.len())
                    .unwrap_or(0);
                resource_data.insert("parameters_count".to_string(), json!(parameters_count));

                // Lake Formation legacy-open-catalog signal: the default table
                // permissions grant ALL to the IAM_ALLOWED_PRINCIPALS group.
                let default_permissions = database.create_table_default_permissions();
                let grants_all_to_iam_allowed_principals =
                    default_permissions.iter().any(|grant| {
                        let is_iam_allowed_principals = grant
                            .principal()
                            .and_then(|p| p.data_lake_principal_identifier())
                            .map(|id| id == "IAM_ALLOWED_PRINCIPALS")
                            .unwrap_or(false);
                        let grants_all = grant
                            .permissions()
                            .iter()
                            .any(|perm| perm.as_str() == "ALL");
                        is_iam_allowed_principals && grants_all
                    });
                resource_data.insert(
                    "create_table_default_permissions_count".to_string(),
                    json!(default_permissions.len()),
                );
                resource_data.insert(
                    "default_permissions_grant_all_to_iam_allowed_principals".to_string(),
                    json!(grants_all_to_iam_allowed_principals),
                );

                resource_data.insert(
                    "is_federated".to_string(),
                    json!(database.federated_database().is_some()),
                );
                resource_data.insert(
                    "is_resource_link".to_string(),
                    json!(database.target_database().is_some()),
                );
                if let Some(target) = database.target_database() {
                    if let Some(target_name) = target.database_name() {
                        resource_data
                            .insert("target_database_name".to_string(), json!(target_name));
                    }
                }

                // Bounded table sample: a single GetTables call per database
                // with a small page size. Failure must not fail the sync; the
                // fields are simply absent (evaluators report a data gap).
                match client
                    .get_tables()
                    .database_name(&name)
                    .max_results(TABLE_SAMPLE_MAX_RESULTS)
                    .send()
                    .await
                {
                    Ok(tables_response) => {
                        resource_data.insert(
                            "table_count".to_string(),
                            json!(tables_response.table_list().len()),
                        );
                        resource_data.insert(
                            "table_count_truncated".to_string(),
                            json!(tables_response.next_token().is_some()),
                        );
                    }
                    Err(e) => {
                        debug!("Failed to sample tables for Glue database {}: {}", name, e);
                    }
                }

                let tags = match client.get_tags().resource_arn(&arn).send().await {
                    Ok(tags_response) => {
                        let mut tags_map = serde_json::Map::new();
                        if let Some(tag_pairs) = tags_response.tags() {
                            for (key, value) in tag_pairs {
                                tags_map.insert(key.to_string(), json!(value));
                            }
                        }
                        serde_json::Value::Object(tags_map)
                    }
                    Err(e) => {
                        debug!("Failed to get tags for Glue database {}: {}", name, e);
                        json!({})
                    }
                };

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::GlueDatabase.to_string(),
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
            "Successfully synced {} Glue databases for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
