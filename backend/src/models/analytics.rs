use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Base analysis request type for all AWS resources
#[derive(Debug, Serialize, Deserialize)]
pub struct AwsResourceAnalysisRequest {

































}    pub context: Option<String>,    pub question: String,    pub workflow: Option<String>,    pub resource_id: String,pub struct ResourceRelatedQuestionRequest {#[derive(Debug, Serialize, Deserialize)]/// Request type for follow-up questions}    pub data_sources: Vec<String>,    pub time_range: Option<String>,    pub workflow_type: String,    pub resource_type: String,    pub timestamp: DateTime<Utc>,pub struct AnalysisMetadata {#[derive(Debug, Serialize, Deserialize)]/// Metadata for analysis results}    pub metadata: AnalysisMetadata,    pub related_questions: Vec<String>,    pub content: String,    pub format: String,  // "markdown", "json", "html", "yaml"pub struct AwsResourceAnalysisResponse {#[derive(Debug, Serialize, Deserialize)]/// Base analysis response type for all AWS resources}    pub additional_context: Option<String>,    pub time_range: Option<String>,    pub workflow: String,    pub resource_id: String,    pub workflow: String,
    pub time_range: Option<String>,
    pub additional_context: Option<String>,
}

/// Base response type for all analysis results
#[derive(Debug, Serialize, Deserialize)]
pub struct AwsResourceAnalysisResponse {
    pub format: String,
    pub content: String, 
    pub related_questions: Vec<String>,
}

/// Base related question request type
#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceRelatedQuestionRequest {
    pub resource_id: String,
    pub resource_type: String,
    pub question: String,
    pub workflow: Option<String>,
}
