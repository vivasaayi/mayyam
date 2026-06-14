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
use aws_sdk_config::types::{ConfigRule, ConfigRuleEvaluationStatus};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error};
use uuid::Uuid;

/// DescribeConfigRuleEvaluationStatus accepts at most 25 rule names per call.
const EVALUATION_STATUS_BATCH_SIZE: usize = 25;

pub struct ConfigControlPlane {
    aws_service: Arc<AwsService>,
}

impl ConfigControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_rules(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Config rules for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_config_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        // First pass: collect every rule definition with NextToken pagination.
        let mut rules: Vec<ConfigRule> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.describe_config_rules();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to list Config rules: {}", e);
                AppError::ExternalService(format!("Failed to list Config rules: {}", e))
            })?;

            rules.extend(response.config_rules().iter().cloned());

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        // Second pass: enrich with evaluation statuses in batches of 25 rule
        // names. Failures here must not fail the sync; affected rules are
        // simply persisted without evaluation status fields.
        let rule_names: Vec<String> = rules
            .iter()
            .filter_map(|r| r.config_rule_name().map(String::from))
            .collect();
        let statuses = self.fetch_evaluation_statuses(&client, &rule_names).await;

        for rule in &rules {
            let rule_name = match rule.config_rule_name() {
                Some(name) => name.to_string(),
                None => {
                    debug!("Skipping Config rule entry without a name");
                    continue;
                }
            };

            let arn = rule.config_rule_arn().unwrap_or("").to_string();

            let mut resource_data = serde_json::Map::new();
            resource_data.insert("rule_name".to_string(), json!(rule_name));
            resource_data.insert("arn".to_string(), json!(arn));

            if let Some(rule_id) = rule.config_rule_id() {
                resource_data.insert("rule_id".to_string(), json!(rule_id));
            }

            if let Some(state) = rule.config_rule_state() {
                resource_data.insert("config_rule_state".to_string(), json!(state.as_str()));
            }

            if let Some(description) = rule.description() {
                resource_data.insert("description".to_string(), json!(description));
            }

            if let Some(source) = rule.source() {
                resource_data.insert("source_owner".to_string(), json!(source.owner().as_str()));
                if let Some(identifier) = source.source_identifier() {
                    resource_data.insert("source_identifier".to_string(), json!(identifier));
                }
            }

            if let Some(scope) = rule.scope() {
                let mut scope_map = serde_json::Map::new();
                if !scope.compliance_resource_types().is_empty() {
                    scope_map.insert(
                        "compliance_resource_types".to_string(),
                        json!(scope.compliance_resource_types()),
                    );
                }
                if let Some(tag_key) = scope.tag_key() {
                    scope_map.insert("tag_key".to_string(), json!(tag_key));
                }
                if let Some(tag_value) = scope.tag_value() {
                    scope_map.insert("tag_value".to_string(), json!(tag_value));
                }
                if let Some(resource_id) = scope.compliance_resource_id() {
                    scope_map.insert("compliance_resource_id".to_string(), json!(resource_id));
                }
                resource_data.insert("scope".to_string(), serde_json::Value::Object(scope_map));
            }

            if let Some(frequency) = rule.maximum_execution_frequency() {
                resource_data.insert(
                    "maximum_execution_frequency".to_string(),
                    json!(frequency.as_str()),
                );
            }

            if let Some(created_by) = rule.created_by() {
                resource_data.insert("created_by".to_string(), json!(created_by));
            }

            if let Some(status) = statuses.get(&rule_name) {
                resource_data.insert("evaluation_status_collected".to_string(), json!(true));

                if let Some(t) = status.last_successful_evaluation_time() {
                    resource_data.insert(
                        "last_successful_evaluation_time".to_string(),
                        json!(format_time(t)),
                    );
                }
                if let Some(t) = status.last_failed_evaluation_time() {
                    resource_data.insert(
                        "last_failed_evaluation_time".to_string(),
                        json!(format_time(t)),
                    );
                }
                if let Some(t) = status.last_successful_invocation_time() {
                    resource_data.insert(
                        "last_successful_invocation_time".to_string(),
                        json!(format_time(t)),
                    );
                }
                if let Some(t) = status.last_failed_invocation_time() {
                    resource_data.insert(
                        "last_failed_invocation_time".to_string(),
                        json!(format_time(t)),
                    );
                }
                if let Some(t) = status.first_activated_time() {
                    resource_data.insert("first_activated_time".to_string(), json!(format_time(t)));
                }
                if let Some(code) = status.last_error_code() {
                    resource_data.insert("last_error_code".to_string(), json!(code));
                }
                if let Some(message) = status.last_error_message() {
                    resource_data.insert("last_error_message".to_string(), json!(message));
                }
                resource_data.insert(
                    "first_evaluation_started".to_string(),
                    json!(status.first_evaluation_started()),
                );
            }

            // Tags are best-effort enrichment; a failure must not fail the sync.
            let tags = if arn.is_empty() {
                json!({})
            } else {
                match client
                    .list_tags_for_resource()
                    .resource_arn(&arn)
                    .send()
                    .await
                {
                    Ok(tags_response) => {
                        let mut tags_map = serde_json::Map::new();
                        for tag in tags_response.tags() {
                            if let Some(key) = tag.key() {
                                tags_map.insert(key.to_string(), json!(tag.value().unwrap_or("")));
                            }
                        }
                        serde_json::Value::Object(tags_map)
                    }
                    Err(e) => {
                        debug!("Failed to list tags for Config rule {}: {}", rule_name, e);
                        json!({})
                    }
                }
            };

            let dto = AwsResourceDto {
                id: None,
                sync_id: Some(sync_id),
                account_id: aws_account_dto.account_id.clone(),
                profile: aws_account_dto.profile.clone(),
                region: aws_account_dto.default_region.clone(),
                resource_type: AwsResourceType::ConfigRule.to_string(),
                resource_id: rule_name.clone(),
                arn,
                name: Some(rule_name),
                tags,
                resource_data: serde_json::Value::Object(resource_data),
            };

            resources.push(dto.into());
        }

        debug!(
            "Successfully synced {} Config rules for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }

    /// Fetch evaluation statuses keyed by rule name, batching rule names by
    /// 25 and following NextToken inside each batch. Errors are logged and
    /// the affected batch is skipped so the sync never fails on enrichment.
    async fn fetch_evaluation_statuses(
        &self,
        client: &aws_sdk_config::Client,
        rule_names: &[String],
    ) -> HashMap<String, ConfigRuleEvaluationStatus> {
        let mut statuses: HashMap<String, ConfigRuleEvaluationStatus> = HashMap::new();

        for chunk in rule_names.chunks(EVALUATION_STATUS_BATCH_SIZE) {
            let mut next_token: Option<String> = None;

            loop {
                let mut request = client
                    .describe_config_rule_evaluation_status()
                    .set_config_rule_names(Some(chunk.to_vec()));
                if let Some(token) = next_token {
                    request = request.next_token(token);
                }

                match request.send().await {
                    Ok(response) => {
                        for status in response.config_rules_evaluation_status() {
                            if let Some(name) = status.config_rule_name() {
                                statuses.insert(name.to_string(), status.clone());
                            }
                        }
                        next_token = response.next_token().map(String::from);
                        if next_token.is_none() {
                            break;
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to describe Config rule evaluation statuses for batch of {} rules: {}",
                            chunk.len(),
                            e
                        );
                        break;
                    }
                }
            }
        }

        statuses
    }
}

/// Persist AWS timestamps as ISO-8601 strings (same convention as the KMS
/// collector) so evaluators can parse them with RFC 3339 parsers.
fn format_time(dt: &aws_smithy_types::DateTime) -> String {
    dt.fmt(aws_smithy_types::date_time::Format::DateTime)
        .unwrap_or_else(|_| format!("{:?}", dt))
}
