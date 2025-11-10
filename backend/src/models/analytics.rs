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


use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Base analysis request type for all AWS resources
#[derive(Debug, Serialize, Deserialize)]
pub struct AwsResourceAnalysisRequest {
    pub context: Option<String>,
    pub question: String,
    pub workflow: Option<String>,
    pub resource_id: String,
    pub data_sources: Vec<String>,
    pub time_range: Option<String>,
    pub workflow_type: String,
    pub resource_type: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisMetadata {
    // Add fields as needed for metadata
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AwsResourceAnalysisResponse {
    pub metadata: AnalysisMetadata,
    pub related_questions: Vec<String>,
    pub content: String,
    pub format: String, // "markdown", "json", "html", "yaml"
    pub additional_context: Option<String>,
    pub time_range: Option<String>,
    pub workflow: String,
    pub resource_id: String,
}

/// Base response type for all analysis results
#[derive(Debug, Serialize, Deserialize)]
pub struct AwsResourceAnalysisResponseSimple {
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Insight {
    pub title: String,
    pub description: String,
    pub severity: InsightSeverity,
    pub category: String,
    pub metrics_involved: Vec<String>,
    pub supporting_data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum InsightSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Recommendation {
    pub title: String,
    pub description: String,
    pub priority: RecommendationPriority,
    pub impact: String,
    pub action_items: Vec<String>,
    pub estimated_effort: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RecommendationPriority {
    High,
    Medium,
    Low,
}
