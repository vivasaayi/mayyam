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


use serde::{Deserialize, Serialize};

/// Define specific analysis workflows for each AWS resource type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceAnalysisWorkflow {
    // EC2 Instance workflows
    EC2Performance {
        include_cloudwatch: bool,
        monitoring_period: String,
    },
    EC2Scaling {
        include_history: bool,
        recommendation_type: String,
    },
    EC2Cost {
        include_forecast: bool,
        optimization_target: String,
    },

    // S3 Bucket workflows
    S3Access {
        analyze_permissions: bool,
        check_encryption: bool,
    },
    S3Storage {
        include_lifecycle: bool,
        analyze_patterns: bool,
    },
    S3Cost {
        include_forecast: bool,
        storage_class_analysis: bool,
    },

    // DynamoDB workflows
    DynamoDBCapacity {
        analyze_throttling: bool,
        include_autoscaling: bool,
    },
    DynamoDBPerformance {
        analyze_hotspots: bool,
        check_indexes: bool,
    },
    DynamoDBCost {
        analyze_capacity_mode: bool,
        reserved_capacity_analysis: bool,
    },

    // RDS Instance workflows
    RDSMemory {
        include_cloudwatch: bool,
        analyze_settings: bool,
    },
    RDSPerformance {
        analyze_queries: bool,
        check_indexes: bool,
    },
    RDSStorage {
        analyze_growth: bool,
        check_iops: bool,
    },

    // ElastiCache workflows 
    ElastiCachePerformance {
        analyze_hits: bool,
        check_evictions: bool,
    },
    ElastiCacheCapacity {
        analyze_memory: bool,
        check_scaling: bool,
    },
    ElastiCacheConnections {
        analyze_patterns: bool,
        check_limits: bool,
    },

    // Lambda workflows
    LambdaPerformance {
        analyze_duration: bool,
        check_memory: bool,
    },
    LambdaColdStart {
        analyze_patterns: bool,
        include_solutions: bool,
    },
    LambdaCost {
        analyze_usage: bool,
        check_configuration: bool,
    },

    // SQS workflows
    SQSPerformance {
        analyze_latency: bool,
        check_throughput: bool,
    },
    SQSDLQ {
        analyze_failures: bool,
        include_samples: bool,
    },
    SQSLatency {
        analyze_patterns: bool,
        check_bottlenecks: bool,
    },

    // SNS workflows
    SNSDelivery {
        analyze_failures: bool,
        check_throttling: bool,
    },
    SNSSubscriptions {
        analyze_patterns: bool,
        check_filters: bool,
    },

    // Kinesis workflows
    KinesisPerformance {
        analyze_throughput: bool,
        check_throttling: bool,
    },
    KinesisSharding {
        analyze_distribution: bool,
        check_hotspots: bool,
    },
    KinesisLatency {
        analyze_patterns: bool,
        check_consumers: bool,
    }
}

/// Metadata about a resource analysis workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAnalysisMetadata {
    pub workflow_id: String,
    pub name: String,
    pub description: String,
    pub resource_type: String,
    pub required_permissions: Vec<String>,
    pub supported_formats: Vec<String>,
    pub estimated_duration: String,
}

/// Info about available workflows for a resource type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisWorkflowInfo {
    pub resource_type: String,
    pub workflows: Vec<ResourceAnalysisMetadata>,
    pub common_questions: Vec<String>,
    pub best_practices_url: Option<String>,
}
