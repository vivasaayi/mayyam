use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use aws_sdk_sqs::Client as SqsClient;
use crate::errors::AppError;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::models::aws_auth::AccountAuthInfo;
use super::{AwsService, CloudWatchMetricsRequest, CloudWatchMetricsResult};
use super::client_factory::AwsClientFactory;

pub struct SqsControlPlane {
    aws_service: Arc<AwsService>,
}

impl SqsControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn sync_queues(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        self.sync_queues_with_auth(account_id, profile, region, None).await
    }

    pub async fn sync_queues_with_auth(&self, account_id: &str, profile: Option<&str>, region: &str, account_auth: Option<&AccountAuthInfo>) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self.aws_service.create_sqs_client_with_auth(profile, region, account_auth).await?;
        self.sync_queues_with_client(account_id, profile, region).await
    }

    async fn sync_queues_with_client(&self, account_id: &str, profile: Option<&str>, region: &str) -> Result<Vec<AwsResourceModel>, AppError> {
        let mut queues = Vec::new();
        let queue = AwsResourceDto {
            id: None,
            account_id: account_id.to_string(),
            profile: profile.map(|p| p.to_string()),
            region: region.to_string(),
            resource_type: "SqsQueue".to_string(),
            resource_id: "sample-queue".to_string(),
            arn: format!("arn:aws:sqs:{}:{}:sample-queue", region, account_id),
            name: Some("Sample Queue".to_string()),
            tags: json!({"Name": "Sample Queue", "Environment": "Development"}),
            resource_data: json!({
                "queue_url": format!("https://sqs.{}.amazonaws.com/{}/sample-queue", region, account_id),
                "message_retention_period": 345600,
                "visibility_timeout": 30,
                "delay_seconds": 0,
                "receive_message_wait_time_seconds": 0,
                "max_message_size": 262144,
                "fifo_queue": false,
                "content_based_deduplication": false
            }),
        };
        queues.push(queue);

        Ok(queues.into_iter().map(|q| q.into()).collect())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqsSendMessageRequest {
    pub queue_url: String,
    pub message_body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqsReceiveMessageRequest {
    pub queue_url: String,
    pub max_number_of_messages: Option<i32>,
    pub visibility_timeout: Option<i32>,
    pub wait_time_seconds: Option<i32>,
}

// Data plane implementation for SQS
pub struct SqsDataPlane {
    aws_service: Arc<AwsService>,
}

impl SqsDataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn send_message(&self, profile: Option<&str>, region: &str, request: &SqsSendMessageRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_sqs_client(profile, region).await?;
        
        // In a real implementation, this would call send_message
        let response = json!({
            "MessageId": format!("msg-{}", chrono::Utc::now().timestamp()),
            "MD5OfMessageBody": "d41d8cd98f00b204e9800998ecf8427e"
        });
        
        Ok(response)
    }

    pub async fn receive_messages(&self, profile: Option<&str>, region: &str, request: &SqsReceiveMessageRequest) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_sqs_client(profile, region).await?;
        
        // In a real implementation, this would call receive_message
        let response = json!({
            "Messages": [
                {
                    "MessageId": format!("msg-{}", chrono::Utc::now().timestamp()),
                    "ReceiptHandle": "receipt-handle-1",
                    "MD5OfBody": "d41d8cd98f00b204e9800998ecf8427e",
                    "Body": "Sample message body"
                }
            ]
        });
        
        Ok(response)
    }

    pub async fn get_queue_metrics(&self, request: &CloudWatchMetricsRequest) -> Result<CloudWatchMetricsResult, AppError> {
        let client = self.aws_service.create_cloudwatch_client(None, &request.region).await?;
        
        // SQS-specific metric collection logic would go here
        // For now returning empty result
        Ok(CloudWatchMetricsResult {
            resource_id: request.resource_id.clone(),
            resource_type: request.resource_type.clone(),
            metrics: vec![],
        })
    }
}
