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

pub struct ConnectControlPlane {
    aws_service: Arc<AwsService>,
}

impl ConnectControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_instances(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Connect Instances for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_connect_client(aws_account_dto).await?;
        let mut resources: Vec<AwsResourceModel> = Vec::new();

        let mut next_token = None;

        loop {
            let mut request = client.list_instances();
            if let Some(t) = next_token {
                request = request.next_token(t);
            }

            let response = match request.send().await {
                Ok(res) => res,
                Err(e) => {
                    error!("Failed to list Connect Instances: {}", e);
                    break;
                }
            };

            for instance in response.instance_summary_list() {
                let arn = instance.arn().unwrap_or("");
                let id = instance.id().unwrap_or("");
                let name = instance.instance_alias().unwrap_or("");
                
                let resource_data = json!({
                    "Id": id,
                    "Arn": arn,
                    "InstanceAlias": name,
                    "IdentityManagementType": instance.identity_management_type().map(|t| t.as_str()).unwrap_or(""),
                    "InboundCallsEnabled": instance.inbound_calls_enabled(),
                    "OutboundCallsEnabled": instance.outbound_calls_enabled()
                });

                let dto = AwsResourceDto {
                    id: None,
                    sync_id: Some(sync_id),
                    account_id: aws_account_dto.account_id.clone(),
                    profile: aws_account_dto.profile.clone(),
                    region: aws_account_dto.default_region.clone(),
                    resource_type: AwsResourceType::ConnectInstance.to_string(),
                    resource_id: id.to_string(),
                    arn: arn.to_string(),
                    name: Some(name.to_string()),
                    tags: json!({}),
                    resource_data,
                };

                resources.push(dto.into());
            }

            next_token = response.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        debug!(
            "Successfully synced {} Connect Instances for account: {} with sync_id: {}",
            resources.len(),
            &aws_account_dto.account_id,
            sync_id
        );

        Ok(resources)
    }
}
