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

pub struct EventBridgeControlPlane {
    aws_service: Arc<AwsService>,
}

impl EventBridgeControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_rules(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing EventBridge rules for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_eventbridge_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        // Enumerate event buses so custom-bus rules are inventoried too. A
        // failure here must not abort the sync; fall back to the default bus.
        let event_bus_names = match self.list_event_bus_names(&client).await {
            Ok(names) if !names.is_empty() => names,
            Ok(_) => vec!["default".to_string()],
            Err(e) => {
                error!(
                    "Failed to list EventBridge event buses for account {}: {}; falling back to default bus",
                    &aws_account_dto.account_id, e
                );
                vec!["default".to_string()]
            }
        };

        for event_bus_name in &event_bus_names {
            let mut next_token: Option<String> = None;

            loop {
                let mut request = client.list_rules().event_bus_name(event_bus_name);
                if let Some(token) = next_token {
                    request = request.next_token(token);
                }

                let response = request.send().await.map_err(|e| {
                    error!(
                        "Failed to list EventBridge rules on bus {}: {}",
                        event_bus_name, e
                    );
                    AppError::ExternalService(format!(
                        "Failed to list EventBridge rules on bus {}: {}",
                        event_bus_name, e
                    ))
                })?;

                for rule in response.rules() {
                    let rule_name = match rule.name() {
                        Some(name) => name.to_string(),
                        None => {
                            debug!("Skipping EventBridge rule list entry without a name");
                            continue;
                        }
                    };
                    let arn = rule.arn().unwrap_or("").to_string();

                    let mut resource_data = serde_json::Map::new();
                    resource_data.insert("name".to_string(), json!(rule_name));
                    resource_data.insert("arn".to_string(), json!(arn));
                    resource_data.insert("event_bus_name".to_string(), json!(event_bus_name));

                    if let Some(state) = rule.state() {
                        resource_data.insert("state".to_string(), json!(state.as_str()));
                    }
                    if let Some(description) = rule.description() {
                        resource_data.insert("description".to_string(), json!(description));
                    }
                    if let Some(schedule) = rule.schedule_expression() {
                        resource_data.insert("schedule_expression".to_string(), json!(schedule));
                    }
                    if let Some(pattern) = rule.event_pattern() {
                        resource_data.insert("event_pattern".to_string(), json!(pattern));
                    }
                    if let Some(managed_by) = rule.managed_by() {
                        resource_data.insert("managed_by".to_string(), json!(managed_by));
                    }
                    if let Some(role_arn) = rule.role_arn() {
                        resource_data.insert("role_arn".to_string(), json!(role_arn));
                    }

                    // Target enrichment failures must not fail the sync; the
                    // target_count/targets fields are simply absent, which the
                    // pillar evaluator reports as a data gap.
                    match self
                        .list_rule_targets(&client, &rule_name, event_bus_name)
                        .await
                    {
                        Ok(targets) => {
                            resource_data.insert("target_count".to_string(), json!(targets.len()));
                            resource_data.insert("targets".to_string(), json!(targets));
                        }
                        Err(e) => {
                            error!(
                                "Failed to list targets for EventBridge rule {} on bus {}: {}",
                                rule_name, event_bus_name, e
                            );
                        }
                    }

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
                                    tags_map.insert(tag.key().to_string(), json!(tag.value()));
                                }
                                serde_json::Value::Object(tags_map)
                            }
                            Err(e) => {
                                debug!(
                                    "Failed to list tags for EventBridge rule {}: {}",
                                    rule_name, e
                                );
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
                        resource_type: AwsResourceType::EventBridgeRule.to_string(),
                        resource_id: rule_name.clone(),
                        arn,
                        name: Some(rule_name),
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
        }

        debug!(
            "Successfully synced {} EventBridge rules across {} event buses for account: {} with sync_id: {}",
            resources.len(),
            event_bus_names.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }

    async fn list_event_bus_names(
        &self,
        client: &aws_sdk_eventbridge::Client,
    ) -> Result<Vec<String>, AppError> {
        let mut names: Vec<String> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_event_buses();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                AppError::ExternalService(format!("Failed to list EventBridge event buses: {}", e))
            })?;

            for bus in response.event_buses() {
                if let Some(name) = bus.name() {
                    names.push(name.to_string());
                }
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        Ok(names)
    }

    /// Compact per-target summary persisted into `resource_data.targets`:
    /// id, arn, whether a dead-letter queue is configured, and the explicit
    /// retry policy maximum attempts when one is set.
    async fn list_rule_targets(
        &self,
        client: &aws_sdk_eventbridge::Client,
        rule_name: &str,
        event_bus_name: &str,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let mut targets: Vec<serde_json::Value> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client
                .list_targets_by_rule()
                .rule(rule_name)
                .event_bus_name(event_bus_name);
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                AppError::ExternalService(format!(
                    "Failed to list targets for EventBridge rule {}: {}",
                    rule_name, e
                ))
            })?;

            for target in response.targets() {
                targets.push(json!({
                    "id": target.id(),
                    "arn": target.arn(),
                    "has_dead_letter_config": target.dead_letter_config().is_some(),
                    "has_retry_policy": target.retry_policy().is_some(),
                    "retry_max_attempts": target
                        .retry_policy()
                        .and_then(|p| p.maximum_retry_attempts()),
                }));
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        Ok(targets)
    }
}
