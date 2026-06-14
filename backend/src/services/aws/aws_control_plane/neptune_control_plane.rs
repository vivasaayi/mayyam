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

pub struct NeptuneControlPlane {
    aws_service: Arc<AwsService>,
}

impl NeptuneControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_clusters(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Neptune clusters for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_rds_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();
        let mut marker: Option<String> = None;

        loop {
            let engine_filter = aws_sdk_rds::types::Filter::builder()
                .name("engine")
                .values("neptune")
                .build();

            let mut request = client.describe_db_clusters().filters(engine_filter);
            if let Some(m) = marker {
                request = request.marker(m);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to describe Neptune clusters: {}", e);
                AppError::ExternalService(format!("Failed to describe Neptune clusters: {}", e))
            })?;

            for cluster in response.db_clusters() {
                let cluster_id = cluster.db_cluster_identifier().unwrap_or("").to_string();
                if cluster_id.is_empty() {
                    continue;
                }

                let arn = cluster.db_cluster_arn().unwrap_or("").to_string();
                let mut resource_data = serde_json::Map::new();

                if let Some(engine) = cluster.engine() {
                    resource_data.insert("engine".to_string(), json!(engine));
                }
                if let Some(version) = cluster.engine_version() {
                    resource_data.insert("engine_version".to_string(), json!(version));
                }
                if let Some(status) = cluster.status() {
                    resource_data.insert("status".to_string(), json!(status));
                }
                resource_data.insert(
                    "deletion_protection".to_string(),
                    json!(cluster.deletion_protection().unwrap_or(false)),
                );
                resource_data.insert("multi_az".to_string(), json!(cluster.multi_az()));
                if let Some(retention) = cluster.backup_retention_period() {
                    resource_data.insert("backup_retention_period".to_string(), json!(retention));
                }
                resource_data.insert(
                    "storage_encrypted".to_string(),
                    json!(cluster.storage_encrypted()),
                );
                if let Some(kms) = cluster.kms_key_id() {
                    resource_data.insert("kms_key_id".to_string(), json!(kms));
                }
                if let Some(endpoint) = cluster.endpoint() {
                    resource_data.insert("endpoint".to_string(), json!(endpoint));
                }
                if let Some(reader) = cluster.reader_endpoint() {
                    resource_data.insert("reader_endpoint".to_string(), json!(reader));
                }
                if let Some(port) = cluster.port() {
                    resource_data.insert("port".to_string(), json!(port));
                }
                resource_data.insert(
                    "member_count".to_string(),
                    json!(cluster.db_cluster_members().len()),
                );
                if let Some(backup_window) = cluster.preferred_backup_window() {
                    resource_data
                        .insert("preferred_backup_window".to_string(), json!(backup_window));
                }
                if let Some(maint_window) = cluster.preferred_maintenance_window() {
                    resource_data.insert(
                        "preferred_maintenance_window".to_string(),
                        json!(maint_window),
                    );
                }
                if let Some(iam_auth) = cluster.iam_database_authentication_enabled() {
                    resource_data.insert(
                        "iam_database_authentication_enabled".to_string(),
                        json!(iam_auth),
                    );
                }
                // Enabled CloudWatch log types (audit for Neptune)
                let log_exports = cluster.enabled_cloudwatch_logs_exports();
                resource_data.insert(
                    "audit_logs_enabled".to_string(),
                    json!(log_exports.iter().any(|l| l == "audit")),
                );
                if !log_exports.is_empty() {
                    resource_data.insert(
                        "enabled_cloudwatch_logs_exports".to_string(),
                        json!(log_exports),
                    );
                }

                let mut tags_map = serde_json::Map::new();
                for tag in cluster.tag_list() {
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
                    resource_type: AwsResourceType::NeptuneCluster.to_string(),
                    resource_id: cluster_id.clone(),
                    arn,
                    name: Some(cluster_id),
                    tags: serde_json::Value::Object(tags_map),
                    resource_data: serde_json::Value::Object(resource_data),
                };
                resources.push(dto.into());
            }

            marker = response.marker().map(String::from);
            if marker.is_none() {
                break;
            }
        }

        debug!(
            "Successfully synced {} Neptune clusters for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
