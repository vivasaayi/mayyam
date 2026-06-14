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
use aws_sdk_wafv2::types::Scope;
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

pub struct WafControlPlane {
    aws_service: Arc<AwsService>,
}

impl WafControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_web_acls(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing WAF Web ACLs for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_waf_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        // Regional web ACLs exist in every region; a failure here fails the sync.
        self.sync_scope(
            &client,
            aws_account_dto,
            sync_id,
            Scope::Regional,
            &mut resources,
        )
        .await?;

        // CLOUDFRONT-scoped web ACLs are global but the WAFv2 API only serves
        // them from us-east-1. Sync them only when the account's default
        // region is us-east-1, and degrade gracefully so a CloudFront-scope
        // failure does not discard the regional results.
        if aws_account_dto.default_region == "us-east-1" {
            if let Err(e) = self
                .sync_scope(
                    &client,
                    aws_account_dto,
                    sync_id,
                    Scope::Cloudfront,
                    &mut resources,
                )
                .await
            {
                error!(
                    "Failed to sync CLOUDFRONT-scoped WAF Web ACLs for account {}: {}",
                    &aws_account_dto.account_id, e
                );
            }
        }

        debug!(
            "Successfully synced {} WAF Web ACLs for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }

    async fn sync_scope(
        &self,
        client: &aws_sdk_wafv2::Client,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
        scope: Scope,
        resources: &mut Vec<AwsResourceModel>,
    ) -> Result<(), AppError> {
        let scope_str = scope.as_str().to_string();
        let mut next_marker: Option<String> = None;

        loop {
            let mut request = client.list_web_acls().scope(scope.clone());
            if let Some(m) = next_marker {
                request = request.next_marker(m);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list WAF Web ACLs ({} scope): {}", scope_str, e);
                AppError::ExternalService(format!(
                    "Failed to list WAF Web ACLs ({} scope): {}",
                    scope_str, e
                ))
            })?;

            for summary in response.web_acls() {
                let (name, id, arn) = match (summary.name(), summary.id(), summary.arn()) {
                    (Some(name), Some(id), Some(arn)) => {
                        (name.to_string(), id.to_string(), arn.to_string())
                    }
                    _ => {
                        debug!("Skipping WAF Web ACL list entry without name/id/arn");
                        continue;
                    }
                };

                let mut resource_data = serde_json::Map::new();
                resource_data.insert("name".to_string(), json!(name));
                resource_data.insert("web_acl_id".to_string(), json!(id));
                resource_data.insert("arn".to_string(), json!(arn));
                resource_data.insert("scope".to_string(), json!(scope_str));
                if let Some(description) = summary.description() {
                    if !description.is_empty() {
                        resource_data.insert("description".to_string(), json!(description));
                    }
                }

                // Per-ACL detail. A failure degrades to summary + collected:false.
                match client
                    .get_web_acl()
                    .name(&name)
                    .scope(scope.clone())
                    .id(&id)
                    .send()
                    .await
                {
                    Ok(detail_response) => match detail_response.web_acl() {
                        Some(acl) => {
                            resource_data.insert("collected".to_string(), json!(true));

                            if let Some(default_action) = acl.default_action() {
                                let action = if default_action.block().is_some() {
                                    "block"
                                } else {
                                    "allow"
                                };
                                resource_data.insert("default_action".to_string(), json!(action));
                            }

                            let rules = acl.rules();
                            resource_data.insert("rules_count".to_string(), json!(rules.len()));
                            let managed_rule_group_count = rules
                                .iter()
                                .filter(|rule| {
                                    rule.statement()
                                        .and_then(|s| s.managed_rule_group_statement())
                                        .is_some()
                                })
                                .count();
                            resource_data.insert(
                                "managed_rule_group_count".to_string(),
                                json!(managed_rule_group_count),
                            );

                            resource_data.insert("capacity".to_string(), json!(acl.capacity()));
                            resource_data.insert(
                                "managed_by_firewall_manager".to_string(),
                                json!(acl.managed_by_firewall_manager()),
                            );

                            if let Some(label_namespace) = acl.label_namespace() {
                                resource_data
                                    .insert("label_namespace".to_string(), json!(label_namespace));
                            }

                            if let Some(visibility) = acl.visibility_config() {
                                resource_data.insert(
                                    "cloud_watch_metrics_enabled".to_string(),
                                    json!(visibility.cloud_watch_metrics_enabled()),
                                );
                                resource_data.insert(
                                    "sampled_requests_enabled".to_string(),
                                    json!(visibility.sampled_requests_enabled()),
                                );
                                resource_data.insert(
                                    "metric_name".to_string(),
                                    json!(visibility.metric_name()),
                                );
                            }
                        }
                        None => {
                            debug!("GetWebACL returned no detail for WAF Web ACL {}", id);
                            resource_data.insert("collected".to_string(), json!(false));
                        }
                    },
                    Err(e) => {
                        error!("Failed to get WAF Web ACL detail for {}: {}", id, e);
                        resource_data.insert("collected".to_string(), json!(false));
                    }
                }

                // Logging configuration. WAFNonexistentItemException means no
                // logging configuration exists -> logging_enabled false. Any
                // other failure is a data gap and the field stays absent.
                match client
                    .get_logging_configuration()
                    .resource_arn(&arn)
                    .send()
                    .await
                {
                    Ok(logging_response) => match logging_response.logging_configuration() {
                        Some(config) => {
                            resource_data.insert("logging_enabled".to_string(), json!(true));
                            resource_data.insert(
                                "log_destination_configs".to_string(),
                                json!(config.log_destination_configs()),
                            );
                        }
                        None => {
                            resource_data.insert("logging_enabled".to_string(), json!(false));
                        }
                    },
                    Err(e) => {
                        let not_configured = e
                            .as_service_error()
                            .map(|se| se.is_waf_nonexistent_item_exception())
                            .unwrap_or(false);
                        if not_configured {
                            resource_data.insert("logging_enabled".to_string(), json!(false));
                        } else {
                            debug!(
                                "Failed to get logging configuration for WAF Web ACL {}: {}",
                                id, e
                            );
                        }
                    }
                }

                // Tags. A failure degrades to an empty tag map.
                let tags = match client
                    .list_tags_for_resource()
                    .resource_arn(&arn)
                    .send()
                    .await
                {
                    Ok(tags_response) => {
                        let mut tags_map = serde_json::Map::new();
                        if let Some(tag_info) = tags_response.tag_info_for_resource() {
                            for tag in tag_info.tag_list() {
                                tags_map.insert(tag.key().to_string(), json!(tag.value()));
                            }
                        }
                        serde_json::Value::Object(tags_map)
                    }
                    Err(e) => {
                        debug!("Failed to list tags for WAF Web ACL {}: {}", id, e);
                        json!({})
                    }
                };

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::WafWebAcl.to_string(),
                    resource_id: id,
                    arn,
                    name: Some(name),
                    tags,
                    resource_data: serde_json::Value::Object(resource_data),
                };

                resources.push(dto.into());
            }

            next_marker = response.next_marker().map(String::from);
            if next_marker.is_none() {
                break;
            }
        }

        Ok(())
    }
}
