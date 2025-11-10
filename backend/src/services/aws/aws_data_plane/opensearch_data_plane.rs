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
use crate::services::aws::aws_types::opensearch::OpenSearchClusterHealthRequest;
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use serde_json::json;
use std::sync::Arc;
use tracing::info;
use uuid;

pub struct OpenSearchDataPlane {
    aws_service: Arc<AwsService>,
}

impl OpenSearchDataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn get_cluster_health(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &OpenSearchClusterHealthRequest,
    ) -> Result<serde_json::Value, AppError> {
        let client = self
            .aws_service
            .create_opensearch_client(aws_account_dto)
            .await?;

        info!("Getting cluster health for domain {}", request.domain_name);

        // Mock implementation
        let response = json!({
            "cluster_name": request.domain_name,
            "status": "green",
            "timed_out": false,
            "number_of_nodes": 1,
            "number_of_data_nodes": 1,
            "active_primary_shards": 5,
            "active_shards": 5,
            "relocating_shards": 0,
            "initializing_shards": 0,
            "unassigned_shards": 0,
            "delayed_unassigned_shards": 0,
            "number_of_pending_tasks": 0,
            "number_of_in_flight_fetch": 0,
            "task_max_waiting_in_queue_millis": 0,
            "active_shards_percent_as_number": 100.0
        });

        Ok(response)
    }
}
