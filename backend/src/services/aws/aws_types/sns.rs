use serde::{Deserialize, Serialize};

// SNS Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnsPublishRequest {
    pub topic_arn: String,
    pub message: String,
    pub subject: Option<String>,
    pub message_attributes: Option<serde_json::Value>,
}
