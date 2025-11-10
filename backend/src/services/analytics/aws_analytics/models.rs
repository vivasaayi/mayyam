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


use chrono::Utc;
use serde::{Deserialize, Serialize};

pub mod analytics {
    use chrono::Utc;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct AwsResourceAnalysisRequest {
        pub resource_id: String,
        pub workflow: String,
        pub time_range: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct AwsResourceAnalysisResponse {
        pub format: String,
        pub content: String,
        pub related_questions: Vec<String>,
        pub metadata: AnalysisMetadata,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct AnalysisMetadata {
        pub timestamp: chrono::DateTime<Utc>,
        pub resource_type: String,
        pub workflow_type: String,
        pub time_range: Option<String>,
        pub data_sources: Vec<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ResourceRelatedQuestionRequest {
        pub resource_id: String,
        pub question: String,
        pub workflow: Option<String>,
    }
}

pub mod resource_workflows {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub enum ResourceAnalysisWorkflow {
        Performance,
        Cost,
        Storage,
        Memory,
        FiveWhy,
    }

    impl ResourceAnalysisWorkflow {
        pub fn from_str(s: &str) -> Result<Self, String> {
            match s.to_lowercase().as_str() {
                "performance" => Ok(Self::Performance),
                "cost" => Ok(Self::Cost),
                "storage" => Ok(Self::Storage),
                "memory" => Ok(Self::Memory),
                "five-why" | "five_why" | "fivewhy" | "5why" | "5-why" => Ok(Self::FiveWhy),
                _ => Err(format!("Unknown workflow type: {}", s)),
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ResourceAnalysisMetadata {
        pub workflow_id: String,
        pub name: String,
        pub description: String,
        pub resource_type: String,
        pub required_permissions: Vec<String>,
        pub supported_formats: Vec<String>,
        pub estimated_duration: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct AnalysisWorkflowInfo {
        pub resource_type: String,
        pub workflows: Vec<ResourceAnalysisMetadata>,
        pub common_questions: Vec<String>,
        pub best_practices_url: Option<String>,
    }
}
