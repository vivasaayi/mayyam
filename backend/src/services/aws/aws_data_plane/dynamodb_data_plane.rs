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
use crate::services::aws::aws_types::dynamodb::{
    DynamoDBGetItemRequest, DynamoDBPutItemRequest, DynamoDBQueryRequest,
};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use serde_json::json;
use std::sync::Arc;
use uuid;

// Data plane implementation for DynamoDB
pub struct DynamoDBDataPlane {
    aws_service: Arc<AwsService>,
}

impl DynamoDBDataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn get_item(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &DynamoDBGetItemRequest,
    ) -> Result<serde_json::Value, AppError> {
        let client = self
            .aws_service
            .create_dynamodb_client(aws_account_dto)
            .await?;

        // In a real implementation, this would call get_item
        let response = json!({
            "Item": {
                "id": {"S": "sample-id"},
                "name": {"S": "Sample Item"},
                "count": {"N": "42"}
            }
        });

        Ok(response)
    }

    pub async fn put_item(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &DynamoDBPutItemRequest,
    ) -> Result<serde_json::Value, AppError> {
        let client = self
            .aws_service
            .create_dynamodb_client(aws_account_dto)
            .await?;

        // In a real implementation, this would call put_item
        let response = json!({
            "ConsumedCapacity": {
                "TableName": request.table_name,
                "CapacityUnits": 1.0
            }
        });

        Ok(response)
    }

    pub async fn query(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &DynamoDBQueryRequest,
    ) -> Result<serde_json::Value, AppError> {
        let client = self
            .aws_service
            .create_dynamodb_client(aws_account_dto)
            .await?;

        // In a real implementation, this would call query
        let response = json!({
            "Items": [
                {
                    "id": {"S": "sample-id-1"},
                    "name": {"S": "Sample Item 1"},
                    "count": {"N": "42"}
                },
                {
                    "id": {"S": "sample-id-2"},
                    "name": {"S": "Sample Item 2"},
                    "count": {"N": "43"}
                }
            ],
            "Count": 2,
            "ScannedCount": 2,
            "ConsumedCapacity": {
                "TableName": request.table_name,
                "CapacityUnits": 0.5
            }
        });

        Ok(response)
    }
}
