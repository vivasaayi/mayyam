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

pub struct ElasticBeanstalkControlPlane {
    aws_service: Arc<AwsService>,
}

impl ElasticBeanstalkControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_environments(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Elastic Beanstalk environments for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self
            .aws_service
            .create_elasticbeanstalk_client(aws_account_dto)
            .await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.describe_environments().include_deleted(false);
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = request.send().await.map_err(|e| {
                error!("Failed to describe Elastic Beanstalk environments: {}", e);
                AppError::ExternalService(format!(
                    "Failed to describe Elastic Beanstalk environments: {}",
                    e
                ))
            })?;

            for environment in response.environments() {
                let environment_id = environment
                    .environment_id()
                    .or_else(|| environment.environment_name())
                    .unwrap_or("")
                    .to_string();
                if environment_id.is_empty() {
                    continue;
                }

                let arn = environment.environment_arn().unwrap_or("").to_string();
                let name = environment.environment_name().map(String::from);
                let mut resource_data = serde_json::Map::new();

                if let Some(application) = environment.application_name() {
                    resource_data.insert("application_name".to_string(), json!(application));
                }
                if let Some(version) = environment.version_label() {
                    resource_data.insert("version_label".to_string(), json!(version));
                }
                if let Some(solution_stack) = environment.solution_stack_name() {
                    resource_data.insert("solution_stack_name".to_string(), json!(solution_stack));
                }
                if let Some(platform_arn) = environment.platform_arn() {
                    resource_data.insert("platform_arn".to_string(), json!(platform_arn));
                }
                if let Some(status) = environment.status() {
                    resource_data.insert("status".to_string(), json!(status.as_str()));
                }
                if let Some(health) = environment.health() {
                    resource_data.insert("health".to_string(), json!(health.as_str()));
                }
                if let Some(health_status) = environment.health_status() {
                    resource_data
                        .insert("health_status".to_string(), json!(health_status.as_str()));
                }
                if let Some(tier) = environment.tier() {
                    if let Some(name) = tier.name() {
                        resource_data.insert("tier_name".to_string(), json!(name));
                    }
                    if let Some(tier_type) = tier.r#type() {
                        resource_data.insert("tier_type".to_string(), json!(tier_type));
                    }
                }
                if let Some(endpoint) = environment.endpoint_url() {
                    resource_data.insert("endpoint_url".to_string(), json!(endpoint));
                }
                if let Some(cname) = environment.cname() {
                    resource_data.insert("cname".to_string(), json!(cname));
                }
                resource_data.insert(
                    "abortable_operation_in_progress".to_string(),
                    json!(environment
                        .abortable_operation_in_progress()
                        .unwrap_or(false)),
                );
                if let Some(operations_role) = environment.operations_role() {
                    resource_data.insert("operations_role".to_string(), json!(operations_role));
                }

                if let (Some(application), Some(env_name)) = (
                    environment.application_name(),
                    environment.environment_name(),
                ) {
                    self.enrich_configuration(&client, application, env_name, &mut resource_data)
                        .await;
                }

                let mut tags_map = serde_json::Map::new();
                if !arn.is_empty() {
                    match client
                        .list_tags_for_resource()
                        .resource_arn(&arn)
                        .send()
                        .await
                    {
                        Ok(tags_response) => {
                            for tag in tags_response.resource_tags() {
                                if let Some(key) = tag.key() {
                                    tags_map
                                        .insert(key.to_string(), json!(tag.value().unwrap_or("")));
                                }
                            }
                        }
                        Err(e) => {
                            debug!(
                                "Failed to list tags for Elastic Beanstalk environment {}: {}",
                                environment_id, e
                            );
                        }
                    }
                }

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::ElasticBeanstalkEnvironment.to_string(),
                    resource_id: environment_id.clone(),
                    arn,
                    name,
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
            "Successfully synced {} Elastic Beanstalk environments for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }

    async fn enrich_configuration(
        &self,
        client: &aws_sdk_elasticbeanstalk::Client,
        application: &str,
        environment_name: &str,
        resource_data: &mut serde_json::Map<String, serde_json::Value>,
    ) {
        match client
            .describe_configuration_settings()
            .application_name(application)
            .environment_name(environment_name)
            .send()
            .await
        {
            Ok(response) => {
                if let Some(settings) = response.configuration_settings().first() {
                    let mut options = serde_json::Map::new();
                    for option in settings.option_settings() {
                        let namespace = option.namespace().unwrap_or("");
                        let option_name = option.option_name().unwrap_or("");
                        let value = option.value().unwrap_or("");
                        if namespace.is_empty() || option_name.is_empty() {
                            continue;
                        }

                        options.insert(format!("{}::{}", namespace, option_name), json!(value));

                        match (namespace, option_name) {
                            ("aws:autoscaling:asg", "MinSize") => {
                                if let Ok(min_size) = value.parse::<i64>() {
                                    resource_data.insert("min_size".to_string(), json!(min_size));
                                }
                            }
                            ("aws:autoscaling:asg", "MaxSize") => {
                                if let Ok(max_size) = value.parse::<i64>() {
                                    resource_data.insert("max_size".to_string(), json!(max_size));
                                }
                            }
                            ("aws:autoscaling:launchconfiguration", "InstanceType") => {
                                resource_data.insert("instance_type".to_string(), json!(value));
                            }
                            ("aws:autoscaling:launchconfiguration", "IamInstanceProfile") => {
                                resource_data
                                    .insert("iam_instance_profile".to_string(), json!(value));
                            }
                            ("aws:elasticbeanstalk:environment", "EnvironmentType") => {
                                resource_data.insert("environment_type".to_string(), json!(value));
                            }
                            ("aws:elasticbeanstalk:environment", "LoadBalancerType") => {
                                resource_data
                                    .insert("load_balancer_type".to_string(), json!(value));
                            }
                            ("aws:elasticbeanstalk:environment", "ServiceRole") => {
                                resource_data.insert("service_role".to_string(), json!(value));
                            }
                            ("aws:elasticbeanstalk:healthreporting:system", "SystemType") => {
                                resource_data
                                    .insert("health_reporting_system".to_string(), json!(value));
                                resource_data.insert(
                                    "enhanced_health_reporting_enabled".to_string(),
                                    json!(value.eq_ignore_ascii_case("enhanced")),
                                );
                            }
                            ("aws:elasticbeanstalk:managedactions", "ManagedActionsEnabled") => {
                                resource_data.insert(
                                    "managed_actions_enabled".to_string(),
                                    json!(value.eq_ignore_ascii_case("true")),
                                );
                            }
                            (
                                "aws:autoscaling:updatepolicy:rollingupdate",
                                "RollingUpdateEnabled",
                            ) => {
                                resource_data.insert(
                                    "rolling_updates_enabled".to_string(),
                                    json!(value.eq_ignore_ascii_case("true")),
                                );
                            }
                            ("aws:elasticbeanstalk:cloudwatch:logs", "StreamLogs") => {
                                resource_data.insert(
                                    "stream_logs_enabled".to_string(),
                                    json!(value.eq_ignore_ascii_case("true")),
                                );
                            }
                            ("aws:elasticbeanstalk:cloudwatch:logs", "RetentionInDays") => {
                                if let Ok(retention) = value.parse::<i64>() {
                                    resource_data
                                        .insert("log_retention_days".to_string(), json!(retention));
                                }
                            }
                            _ => {}
                        }
                    }
                    resource_data.insert(
                        "configuration_options".to_string(),
                        serde_json::Value::Object(options),
                    );
                }
            }
            Err(e) => {
                debug!(
                    "Failed to describe Elastic Beanstalk configuration for {}: {}",
                    environment_name, e
                );
            }
        }
    }
}
