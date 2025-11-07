use serde::{Deserialize, Serialize};

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
