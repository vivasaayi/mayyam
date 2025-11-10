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
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use serde_json::Value;
use std::sync::Arc;

/// Base service for AWS Cost Explorer functionality
pub struct AwsCostService {
    aws_service: Arc<AwsService>,
}

impl AwsCostService {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    /// Create a Cost Explorer client with the given AWS account
    pub(crate) async fn create_client(
        &self,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<aws_sdk_costexplorer::Client, AppError> {
        self.aws_service
            .create_cost_explorer_client(aws_account_dto)
            .await
    }
}
