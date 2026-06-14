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
use aws_sdk_ssm::types::{DocumentKeyValuesFilter, DocumentPermissionType};
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct SsmControlPlane {
    aws_service: Arc<AwsService>,
}

impl SsmControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_documents(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing SSM documents for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_ssm_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut next_token: Option<String> = None;

        loop {
            // Only ingest account-owned documents. Without this filter,
            // ListDocuments returns thousands of AWS-managed documents.
            let owner_filter = DocumentKeyValuesFilter::builder()
                .key("Owner")
                .values("Self")
                .build();

            let mut request = client
                .list_documents()
                .filters(owner_filter)
                .max_results(50);
            if let Some(t) = next_token {
                request = request.next_token(t);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list SSM documents: {}", e);
                AppError::ExternalService(format!("Failed to list SSM documents: {}", e))
            })?;

            for doc in response.document_identifiers() {
                let name = match doc.name() {
                    Some(n) if !n.is_empty() => n.to_string(),
                    _ => {
                        debug!("Skipping SSM document list entry without a name");
                        continue;
                    }
                };

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("name".to_string(), json!(name));

                if let Some(display_name) = doc.display_name() {
                    resource_data.insert("display_name".to_string(), json!(display_name));
                }

                if let Some(owner) = doc.owner() {
                    resource_data.insert("owner".to_string(), json!(owner));
                }

                if let Some(author) = doc.author() {
                    resource_data.insert("author".to_string(), json!(author));
                }

                if let Some(version_name) = doc.version_name() {
                    resource_data.insert("version_name".to_string(), json!(version_name));
                }

                if let Some(document_version) = doc.document_version() {
                    resource_data.insert("document_version".to_string(), json!(document_version));
                }

                if let Some(document_type) = doc.document_type() {
                    resource_data
                        .insert("document_type".to_string(), json!(document_type.as_str()));
                }

                if let Some(document_format) = doc.document_format() {
                    resource_data.insert(
                        "document_format".to_string(),
                        json!(document_format.as_str()),
                    );
                }

                if let Some(schema_version) = doc.schema_version() {
                    resource_data.insert("schema_version".to_string(), json!(schema_version));
                }

                if let Some(target_type) = doc.target_type() {
                    resource_data.insert("target_type".to_string(), json!(target_type));
                }

                let platform_types: Vec<&str> =
                    doc.platform_types().iter().map(|p| p.as_str()).collect();
                resource_data.insert("platform_types".to_string(), json!(platform_types));

                if let Some(created_date) = doc.created_date() {
                    let formatted = created_date
                        .fmt(aws_smithy_types::date_time::Format::DateTime)
                        .unwrap_or_else(|_| format!("{:?}", created_date));
                    resource_data.insert("created_date".to_string(), json!(formatted));
                }

                if let Some(review_status) = doc.review_status() {
                    resource_data
                        .insert("review_status".to_string(), json!(review_status.as_str()));
                }

                // Sharing posture. A failure here must not fail the sync; the
                // fields are simply absent and the evaluator reports a data gap.
                match client
                    .describe_document_permission()
                    .name(&name)
                    .permission_type(DocumentPermissionType::Share)
                    .send()
                    .await
                {
                    Ok(permission) => {
                        let account_ids: Vec<&str> = permission
                            .account_ids()
                            .iter()
                            .map(|s| s.as_str())
                            .collect();
                        let shared_publicly =
                            account_ids.iter().any(|id| id.eq_ignore_ascii_case("all"));
                        resource_data.insert("shared_account_ids".to_string(), json!(account_ids));
                        resource_data.insert("shared_publicly".to_string(), json!(shared_publicly));
                    }
                    Err(e) => {
                        debug!(
                            "Failed to describe document permission for SSM document {}: {}",
                            name, e
                        );
                    }
                }

                // DocumentIdentifier carries tags inline; persist them as a map.
                let mut tags_map = serde_json::Map::new();
                for tag in doc.tags() {
                    tags_map.insert(tag.key().to_string(), json!(tag.value()));
                }

                let arn = format!(
                    "arn:aws:ssm:{}:{}:document/{}",
                    aws_account_dto.default_region, aws_account_dto.account_id, name
                );

                let display = doc
                    .display_name()
                    .filter(|d| !d.is_empty())
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| name.clone());

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::SsmDocument.to_string(),
                    resource_id: name,
                    arn,
                    name: Some(display),
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
            "Successfully synced {} SSM documents for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
