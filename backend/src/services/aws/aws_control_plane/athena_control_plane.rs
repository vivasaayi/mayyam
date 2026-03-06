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

pub struct AthenaControlPlane {
    aws_service: Arc<AwsService>,
}

impl AthenaControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_workgroups(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        debug!(
            "Syncing Athena workgroups for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        let client = self.aws_service.create_athena_client(aws_account_dto).await?;
        let repo = &self.aws_service.aws_resource_repo;
        let mut resources = Vec::new();

        // List Athena workgroups from AWS
        let response = client.list_work_groups()
            .send()
            .await
            .map_err(|e| {
                AppError::ExternalService(format!("Failed to list Athena workgroups: {}", e))
            })?;

        // Process results
        debug!(
            "Successfully synced Athena workgroups for account: {} with sync_id: {}",
            &aws_account_dto.account_id, sync_id
        );

        Ok(resources)
    }
}
