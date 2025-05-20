use std::sync::Arc;
use serde_json::json;
use crate::errors::AppError;
use crate::models::aws_auth::AccountAuthInfo;
use crate::models::aws_resource::{AwsResourceDto, Model as AwsResourceModel};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;

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