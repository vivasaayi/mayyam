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

pub struct TimestreamControlPlane {
    aws_service: Arc<AwsService>,
}

impl TimestreamControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_tables(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Timestream tables for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_timestreamwrite_client(aws_account_dto)
            .await?;
        let mut resources = Vec::new();
        let mut database_token: Option<String> = None;

        loop {
            let mut database_request = client.list_databases();
            if let Some(token) = database_token {
                database_request = database_request.next_token(token);
            }

            let database_response = database_request.send().await.map_err(|e| {
                error!("Failed to list Timestream databases: {}", e);
                AppError::ExternalService(format!("Failed to list Timestream databases: {}", e))
            })?;

            for database in database_response.databases() {
                let Some(database_name) = database.database_name() else {
                    continue;
                };
                let database_arn = database.arn().unwrap_or("").to_string();
                let kms_key_id = database.kms_key_id().map(String::from);
                let mut table_token: Option<String> = None;

                loop {
                    let mut table_request = client.list_tables().database_name(database_name);
                    if let Some(token) = table_token {
                        table_request = table_request.next_token(token);
                    }

                    let table_response = table_request.send().await.map_err(|e| {
                        error!(
                            "Failed to list Timestream tables for database {}: {}",
                            database_name, e
                        );
                        AppError::ExternalService(format!(
                            "Failed to list Timestream tables for database {}: {}",
                            database_name, e
                        ))
                    })?;

                    for table in table_response.tables() {
                        let Some(table_name) = table.table_name() else {
                            continue;
                        };
                        let arn = table.arn().unwrap_or("").to_string();
                        if arn.is_empty() {
                            continue;
                        }

                        let mut resource_data = serde_json::Map::new();
                        resource_data.insert("database_name".to_string(), json!(database_name));
                        resource_data.insert("database_arn".to_string(), json!(database_arn));
                        resource_data.insert("table_name".to_string(), json!(table_name));
                        resource_data.insert("kms_key_id".to_string(), json!(kms_key_id));
                        if let Some(status) = table.table_status() {
                            resource_data
                                .insert("table_status".to_string(), json!(status.as_str()));
                        }
                        if let Some(retention) = table.retention_properties() {
                            resource_data.insert(
                                "memory_retention_hours".to_string(),
                                json!(retention.memory_store_retention_period_in_hours()),
                            );
                            resource_data.insert(
                                "magnetic_retention_days".to_string(),
                                json!(retention.magnetic_store_retention_period_in_days()),
                            );
                        }
                        if let Some(magnetic) = table.magnetic_store_write_properties() {
                            resource_data.insert(
                                "magnetic_store_writes_enabled".to_string(),
                                json!(magnetic.enable_magnetic_store_writes()),
                            );
                            resource_data.insert(
                                "has_magnetic_store_rejected_data_location".to_string(),
                                json!(magnetic.magnetic_store_rejected_data_location().is_some()),
                            );
                        }

                        let tags = list_timestream_tags(&client, &arn).await;
                        let dto = AwsResourceDto {
                            id: None,
                            sync_id: Some(sync_id),
                            account_id: aws_account_dto.account_id.clone(),
                            profile: aws_account_dto.profile.clone(),
                            region: aws_account_dto.default_region.clone(),
                            resource_type: AwsResourceType::TimestreamTable.to_string(),
                            resource_id: format!("{}/{}", database_name, table_name),
                            arn,
                            name: Some(table_name.to_string()),
                            tags,
                            resource_data: serde_json::Value::Object(resource_data),
                        };
                        resources.push(dto.into());
                    }

                    table_token = table_response.next_token().map(String::from);
                    if table_token.is_none() {
                        break;
                    }
                }
            }

            database_token = database_response.next_token().map(String::from);
            if database_token.is_none() {
                break;
            }
        }

        debug!(
            "Successfully synced {} Timestream tables for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

async fn list_timestream_tags(
    client: &aws_sdk_timestreamwrite::Client,
    arn: &str,
) -> serde_json::Value {
    let mut tags_map = serde_json::Map::new();

    match client
        .list_tags_for_resource()
        .resource_arn(arn)
        .send()
        .await
    {
        Ok(response) => {
            for tag in response.tags() {
                tags_map.insert(tag.key().to_string(), json!(tag.value()));
            }
        }
        Err(e) => {
            debug!("Failed to list Timestream tags for {}: {}", arn, e);
        }
    }

    serde_json::Value::Object(tags_map)
}
