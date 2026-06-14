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
use aws_smithy_types::date_time::Format;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct QuickSightControlPlane {
    aws_service: Arc<AwsService>,
}

impl QuickSightControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_assets(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing QuickSight assets for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_quicksight_client(aws_account_dto)
            .await?;
        let mut resources = Vec::new();

        resources.extend(sync_dashboards(&client, aws_account_dto, sync_id).await?);
        resources.extend(sync_analyses(&client, aws_account_dto, sync_id).await?);
        resources.extend(sync_data_sources(&client, aws_account_dto, sync_id).await?);

        debug!(
            "Successfully synced {} QuickSight assets for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}

async fn sync_dashboards(
    client: &aws_sdk_quicksight::Client,
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
) -> Result<Vec<AwsResourceModel>, AppError> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client
            .list_dashboards()
            .aws_account_id(&aws_account_dto.account_id);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = request.send().await.map_err(|e| {
            error!("Failed to list QuickSight dashboards: {}", e);
            AppError::ExternalService(format!("Failed to list QuickSight dashboards: {}", e))
        })?;

        for dashboard in response.dashboard_summary_list() {
            let Some(arn) = dashboard.arn() else {
                continue;
            };
            let Some(id) = dashboard.dashboard_id() else {
                continue;
            };
            let mut resource_data = serde_json::Map::new();
            resource_data.insert("asset_kind".to_string(), json!("dashboard"));
            resource_data.insert("dashboard_id".to_string(), json!(id));
            if let Some(name) = dashboard.name() {
                resource_data.insert("name".to_string(), json!(name));
            }
            resource_data.insert(
                "published_version_number".to_string(),
                json!(dashboard.published_version_number()),
            );
            if let Some(created) = fmt_date(dashboard.created_time()) {
                resource_data.insert("created_time".to_string(), json!(created));
            }
            if let Some(updated) = fmt_date(dashboard.last_updated_time()) {
                resource_data.insert("last_updated_time".to_string(), json!(updated));
            }
            if let Some(published) = fmt_date(dashboard.last_published_time()) {
                resource_data.insert("last_published_time".to_string(), json!(published));
            }

            let dto = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone(),
                resource_type: AwsResourceType::QuickSightAsset.to_string(),
                resource_id: format!("dashboard/{}", id),
                arn: arn.to_string(),
                name: dashboard.name().map(String::from),
                tags: list_tags(client, arn).await,
                resource_data: serde_json::Value::Object(resource_data),
            };
            resources.push(dto.into());
        }

        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn sync_analyses(
    client: &aws_sdk_quicksight::Client,
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
) -> Result<Vec<AwsResourceModel>, AppError> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client
            .list_analyses()
            .aws_account_id(&aws_account_dto.account_id);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = request.send().await.map_err(|e| {
            error!("Failed to list QuickSight analyses: {}", e);
            AppError::ExternalService(format!("Failed to list QuickSight analyses: {}", e))
        })?;

        for analysis in response.analysis_summary_list() {
            let Some(arn) = analysis.arn() else {
                continue;
            };
            let Some(id) = analysis.analysis_id() else {
                continue;
            };
            let mut resource_data = serde_json::Map::new();
            resource_data.insert("asset_kind".to_string(), json!("analysis"));
            resource_data.insert("analysis_id".to_string(), json!(id));
            if let Some(name) = analysis.name() {
                resource_data.insert("name".to_string(), json!(name));
            }
            if let Some(status) = analysis.status() {
                resource_data.insert("status".to_string(), json!(status.as_str()));
            }
            if let Some(created) = fmt_date(analysis.created_time()) {
                resource_data.insert("created_time".to_string(), json!(created));
            }
            if let Some(updated) = fmt_date(analysis.last_updated_time()) {
                resource_data.insert("last_updated_time".to_string(), json!(updated));
            }

            let dto = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone(),
                resource_type: AwsResourceType::QuickSightAsset.to_string(),
                resource_id: format!("analysis/{}", id),
                arn: arn.to_string(),
                name: analysis.name().map(String::from),
                tags: list_tags(client, arn).await,
                resource_data: serde_json::Value::Object(resource_data),
            };
            resources.push(dto.into());
        }

        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn sync_data_sources(
    client: &aws_sdk_quicksight::Client,
    aws_account_dto: &AwsAccountDto,
    sync_id: Uuid,
) -> Result<Vec<AwsResourceModel>, AppError> {
    let mut resources = Vec::new();
    let mut next_token: Option<String> = None;

    loop {
        let mut request = client
            .list_data_sources()
            .aws_account_id(&aws_account_dto.account_id);
        if let Some(token) = next_token {
            request = request.next_token(token);
        }

        let response = request.send().await.map_err(|e| {
            error!("Failed to list QuickSight data sources: {}", e);
            AppError::ExternalService(format!("Failed to list QuickSight data sources: {}", e))
        })?;

        for data_source in response.data_sources() {
            let Some(arn) = data_source.arn() else {
                continue;
            };
            let Some(id) = data_source.data_source_id() else {
                continue;
            };
            let mut resource_data = serde_json::Map::new();
            resource_data.insert("asset_kind".to_string(), json!("data_source"));
            resource_data.insert("data_source_id".to_string(), json!(id));
            if let Some(name) = data_source.name() {
                resource_data.insert("name".to_string(), json!(name));
            }
            if let Some(source_type) = data_source.r#type() {
                resource_data.insert("data_source_type".to_string(), json!(source_type.as_str()));
            }
            if let Some(status) = data_source.status() {
                resource_data.insert("status".to_string(), json!(status.as_str()));
            }
            if let Some(ssl) = data_source.ssl_properties() {
                resource_data.insert("disable_ssl".to_string(), json!(ssl.disable_ssl()));
            }
            if let Some(created) = fmt_date(data_source.created_time()) {
                resource_data.insert("created_time".to_string(), json!(created));
            }
            if let Some(updated) = fmt_date(data_source.last_updated_time()) {
                resource_data.insert("last_updated_time".to_string(), json!(updated));
            }

            let dto = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone(),
                resource_type: AwsResourceType::QuickSightAsset.to_string(),
                resource_id: format!("data-source/{}", id),
                arn: arn.to_string(),
                name: data_source.name().map(String::from),
                tags: list_tags(client, arn).await,
                resource_data: serde_json::Value::Object(resource_data),
            };
            resources.push(dto.into());
        }

        next_token = response.next_token().map(String::from);
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

async fn list_tags(client: &aws_sdk_quicksight::Client, arn: &str) -> serde_json::Value {
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
            debug!("Failed to list QuickSight tags for {}: {}", arn, e);
        }
    }

    serde_json::Value::Object(tags_map)
}

fn fmt_date(date: Option<&aws_smithy_types::DateTime>) -> Option<String> {
    date.map(|d| {
        d.fmt(Format::DateTime)
            .unwrap_or_else(|_| format!("{:?}", d))
    })
}
