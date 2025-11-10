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


use std::sync::Arc;

use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::services::aws::aws_data_plane::dynamodb_data_plane::DynamoDBDataPlane;
use crate::services::aws::aws_data_plane::kinesis_data_plane::KinesisDataPlane;
use crate::services::aws::aws_data_plane::s3_data_plane::S3DataPlane;
use crate::services::aws::aws_data_plane::sqs_data_plane::SqsDataPlane;
use crate::services::aws::aws_types::dynamodb::{
    DynamoDBGetItemRequest, DynamoDBPutItemRequest, DynamoDBQueryRequest,
};
use crate::services::aws::aws_types::kinesis::KinesisPutRecordRequest;
use crate::services::aws::aws_types::s3::{S3GetObjectRequest, S3PutObjectRequest};
use crate::services::aws::aws_types::sqs::{SqsReceiveMessageRequest, SqsSendMessageRequest};
use crate::services::aws::AwsService;

// Helper struct for AWS data plane operations
pub struct AwsDataPlane {
    s3: S3DataPlane,
    dynamodb: DynamoDBDataPlane,
    sqs: SqsDataPlane,
    kinesis: KinesisDataPlane,
}

impl AwsDataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self {
            s3: S3DataPlane::new(aws_service.clone()),
            dynamodb: DynamoDBDataPlane::new(aws_service.clone()),
            sqs: SqsDataPlane::new(aws_service.clone()),
            kinesis: KinesisDataPlane::new(aws_service.clone()),
        }
    }

    // S3 operations
    pub async fn s3_get_object(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &S3GetObjectRequest,
    ) -> Result<serde_json::Value, AppError> {
        self.s3.get_object(aws_account_dto, request).await
    }

    pub async fn s3_put_object(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &S3PutObjectRequest,
    ) -> Result<serde_json::Value, AppError> {
        self.s3.put_object(aws_account_dto, request).await
    }

    // DynamoDB operations
    pub async fn dynamodb_get_item(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &DynamoDBGetItemRequest,
    ) -> Result<serde_json::Value, AppError> {
        self.dynamodb.get_item(aws_account_dto, request).await
    }

    pub async fn dynamodb_put_item(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &DynamoDBPutItemRequest,
    ) -> Result<serde_json::Value, AppError> {
        self.dynamodb.put_item(aws_account_dto, request).await
    }

    pub async fn dynamodb_query(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &DynamoDBQueryRequest,
    ) -> Result<serde_json::Value, AppError> {
        self.dynamodb.query(aws_account_dto, request).await
    }

    // SQS operations
    pub async fn sqs_send_message(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &SqsSendMessageRequest,
    ) -> Result<serde_json::Value, AppError> {
        self.sqs.send_message(aws_account_dto, request).await
    }

    pub async fn sqs_receive_messages(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &SqsReceiveMessageRequest,
    ) -> Result<serde_json::Value, AppError> {
        self.sqs.receive_messages(aws_account_dto, request).await
    }

    // Kinesis operations
    pub async fn kinesis_put_record(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &KinesisPutRecordRequest,
    ) -> Result<serde_json::Value, AppError> {
        self.kinesis.put_record(aws_account_dto, request).await
    }
}
