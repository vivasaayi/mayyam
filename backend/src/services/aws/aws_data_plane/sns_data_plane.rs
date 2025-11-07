use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use crate::services::aws::aws_types::sns::SnsPublishRequest;
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use serde_json::json;
use std::sync::Arc;
use tracing::info;
use uuid;

pub struct SnsDataPlane {
    aws_service: Arc<AwsService>,
}

impl SnsDataPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    pub async fn publish_message(
        &self,
        aws_account_dto: &AwsAccountDto,
        request: &SnsPublishRequest,
    ) -> Result<serde_json::Value, AppError> {
        let client = self.aws_service.create_sns_client(aws_account_dto).await?;

        // Mock implementation for now
        info!("Publishing message to topic {}", request.topic_arn);

        let message_id = format!("message-{}", uuid::Uuid::new_v4().to_string());
        let response = json!({
            "message_id": message_id,
            "sequence_number": None::<String>,
        });

        Ok(response)
    }
}
