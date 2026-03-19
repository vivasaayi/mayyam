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
use tracing::debug;
use uuid::Uuid;

pub struct CloudWatchControlPlane {
    aws_service: Arc<AwsService>,
}

impl CloudWatchControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_alarms(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing CloudWatch alarms for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_cloudwatch_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut next_token = None;
        loop {
            let mut request = client.describe_alarms();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = match request.send().await {
                Ok(res) => res,
                Err(e) => {
                    tracing::error!("Failed to list CloudWatch alarms: {}", e);
                    break;
                }
            };

            let alarms = response.metric_alarms();
            if true {
                for alarm in alarms {
                    let name = alarm.alarm_name().unwrap_or_default();
                    let arn = alarm.alarm_arn().unwrap_or_default();

                    let resource_data = serde_json::json!({
                        "AlarmName": name,
                        "AlarmArn": arn,
                        "AlarmDescription": alarm.alarm_description(),
                        "StateValue": alarm.state_value().map(|s| s.as_str()),
                        "MetricName": alarm.metric_name(),
                        "Namespace": alarm.namespace(),
                    });

                    let dto = AwsResourceDto {
                        id: None,
                        sync_id: Some(sync_id),
                        account_id: aws_account_dto.account_id.clone(),
                        profile: aws_account_dto.profile.clone(),
                        region: aws_account_dto.default_region.clone(),
                        resource_type: AwsResourceType::CloudWatchAlarm.to_string(),
                        resource_id: name.to_string(),
                        arn: arn.to_string(),
                        name: Some(name.to_string()),
                        tags: json!({}),
                        resource_data,
                    };

                    resources.push(dto.into());
                }
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        debug!(
            "Successfully synced {} CloudWatch alarms for account: {} with sync_id: {}",
            resources.len(), &aws_account_dto.account_id, sync_id
        );

        Ok(resources)
    }

    pub async fn sync_dashboards(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing CloudWatch dashboards for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_cloudwatch_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut next_token = None;
        loop {
            let mut request = client.list_dashboards();
            if let Some(token) = next_token {
                request = request.next_token(token);
            }

            let response = match request.send().await {
                Ok(res) => res,
                Err(e) => {
                    tracing::error!("Failed to list CloudWatch dashboards: {}", e);
                    break;
                }
            };

            let dashboards = response.dashboard_entries();
            if true {
                for dashboard in dashboards {
                    let name = dashboard.dashboard_name().unwrap_or_default();
                    let arn = dashboard.dashboard_arn().unwrap_or_default();

                    let resource_data = serde_json::json!({
                        "DashboardName": name,
                        "DashboardArn": arn,
                        "Size": dashboard.size(),
                        "LastModified": dashboard.last_modified().map(|d| d.to_string()),
                    });

                    let dto = AwsResourceDto {
                        id: None,
                        sync_id: Some(sync_id),
                        account_id: aws_account_dto.account_id.clone(),
                        profile: aws_account_dto.profile.clone(),
                        region: aws_account_dto.default_region.clone(),
                        resource_type: AwsResourceType::CloudWatchDashboard.to_string(),
                        resource_id: name.to_string(),
                        arn: arn.to_string(),
                        name: Some(name.to_string()),
                        tags: json!({}),
                        resource_data,
                    };

                    resources.push(dto.into());
                }
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        debug!(
            "Successfully synced {} CloudWatch dashboards for account: {} with sync_id: {}",
            resources.len(), &aws_account_dto.account_id, sync_id
        );

        Ok(resources)
    }
}
