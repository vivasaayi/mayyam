use std::sync::Arc;

use crate::errors::AppError;
use crate::services::aws::AwsService;
use crate::services::aws::s3::{
    S3DataPlane,
    S3GetObjectRequest,
    S3PutObjectRequest,
};
use crate::services::aws::dynamodb::{
    DynamoDBDataPlane,
    DynamoDBGetItemRequest,
    DynamoDBPutItemRequest,
    DynamoDBQueryRequest,
};
use crate::services::aws::sqs::{
    SqsDataPlane,
    SqsSendMessageRequest,
    SqsReceiveMessageRequest,
};
use crate::services::aws::kinesis::{
    KinesisDataPlane,
    KinesisPutRecordRequest,
};

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
    pub async fn s3_get_object(&self, profile: Option<&str>, region: &str, request: &S3GetObjectRequest) -> Result<serde_json::Value, AppError> {
        self.s3.get_object(profile, region, request).await
    }

    pub async fn s3_put_object(&self, profile: Option<&str>, region: &str, request: &S3PutObjectRequest) -> Result<serde_json::Value, AppError> {
        self.s3.put_object(profile, region, request).await
    }

    // DynamoDB operations
    pub async fn dynamodb_get_item(&self, profile: Option<&str>, region: &str, request: &DynamoDBGetItemRequest) -> Result<serde_json::Value, AppError> {
        self.dynamodb.get_item(profile, region, request).await
    }

    pub async fn dynamodb_put_item(&self, profile: Option<&str>, region: &str, request: &DynamoDBPutItemRequest) -> Result<serde_json::Value, AppError> {
        self.dynamodb.put_item(profile, region, request).await
    }

    pub async fn dynamodb_query(&self, profile: Option<&str>, region: &str, request: &DynamoDBQueryRequest) -> Result<serde_json::Value, AppError> {
        self.dynamodb.query(profile, region, request).await
    }

    // SQS operations
    pub async fn sqs_send_message(&self, profile: Option<&str>, region: &str, request: &SqsSendMessageRequest) -> Result<serde_json::Value, AppError> {
        self.sqs.send_message(profile, region, request).await
    }

    pub async fn sqs_receive_messages(&self, profile: Option<&str>, region: &str, request: &SqsReceiveMessageRequest) -> Result<serde_json::Value, AppError> {
        self.sqs.receive_messages(profile, region, request).await
    }

    // Kinesis operations
    pub async fn kinesis_put_record(&self, profile: Option<&str>, region: &str, request: &KinesisPutRecordRequest) -> Result<serde_json::Value, AppError> {
        self.kinesis.put_record(profile, region, request).await
    }
}
