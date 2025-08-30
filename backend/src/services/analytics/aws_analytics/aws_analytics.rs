use std::sync::Arc;
use chrono::Utc;
use tracing::info;
use crate::errors::AppError;
use crate::config::Config;
use crate::services::aws::{self, AwsDataPlane, AwsService};
use crate::models::aws_resource;
use crate::repositories::aws_resource::AwsResourceRepository;
use crate::services::aws::aws_types::cloud_watch;

// Import the new modules
use crate::services::analytics::aws_analytics::models::analytics::*;
use crate::services::analytics::aws_analytics::models::resource_workflows::*;
use crate::services::analytics::aws_analytics::metrics::MetricsAnalyzer;
use crate::services::analytics::aws_analytics::questions::QuestionGenerator;
use crate::services::analytics::aws_analytics::resources::*;

pub struct AwsAnalyticsService {
    config: Config,
    aws_service: Arc<AwsService>,
    aws_data_plane: Arc<AwsDataPlane>,
    aws_resource_repo: Arc<AwsResourceRepository>,
}

impl AwsAnalyticsService {
    pub fn new(
        config: Config,
        aws_service: Arc<AwsService>,
        aws_data_plane: Arc<AwsDataPlane>,
        aws_resource_repo: Arc<AwsResourceRepository>,
    ) -> Self {
        Self {
            config,
            aws_service,
            aws_data_plane,
            aws_resource_repo,
        }
    }

    pub async fn analyze_resource(
        &self,
        request: &AwsResourceAnalysisRequest,
    ) -> Result<AwsResourceAnalysisResponse, AppError> {
        // Get resource details from repository
        let resource = self.aws_resource_repo
            .find_by_arn(&request.resource_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!(
                "Resource {} not found",
                request.resource_id
            )))?;

        // Get cloudwatch metrics for the resource
        let metrics_request = self.get_metrics_request(
            request.resource_id.clone(),
            resource.resource_type.clone(),
            resource.region.clone(),
            request.time_range.clone(),
        ).await;

        let metrics = self.aws_data_plane
            .get_cloudwatch_metrics(&metrics_request)
            .await?;

        // Parse the workflow type
        let workflow = ResourceAnalysisWorkflow::from_str(&request.workflow)
            .map_err(|e| AppError::BadRequest(format!(
                "Invalid workflow type: {}", e
            )))?;

        // Generate analysis based on resource type and workflow
        let analysis = match resource.resource_type.as_str() {
            "EC2Instance" => Ec2Analyzer::analyze_ec2_instance(
                &resource,
                &workflow,
                &metrics
            ).await?,
            "S3Bucket" => S3Analyzer::analyze_s3_bucket(
                &resource,
                &workflow,
                &metrics
            ).await?,
            "RdsInstance" => RdsAnalyzer::analyze_rds_instance(
                &resource,
                &workflow,
                &metrics
            ).await?,
            "DynamoDbTable" => DynamoDbAnalyzer::analyze_dynamodb_table(
                &resource,
                &workflow,
                &metrics
            ).await?,
            "ElastiCache" => ElastiCacheAnalyzer::analyze_elasticache_cluster(
                &resource,
                &workflow,
                &metrics
            ).await?,
            "KinesisStream" => KinesisAnalyzer::analyze_kinesis_stream(
                &resource,
                &workflow,
                &metrics
            ).await?,
            // Add other resource types...
            _ => return Err(AppError::BadRequest(format!(
                "Unsupported resource type: {}",
                resource.resource_type
            ))),
        };

        Ok(AwsResourceAnalysisResponse {
            format: "markdown".to_string(),
            content: analysis,
            related_questions: QuestionGenerator::generate_related_questions(
                &resource.resource_type,
                &request.workflow
            ),
            metadata: AnalysisMetadata {
                timestamp: Utc::now(),
                resource_type: resource.resource_type,
                workflow_type: request.workflow.clone(),
                time_range: request.time_range.clone(),
                data_sources: vec![
                    "CloudWatch Metrics".to_string(),
                    "Resource Configuration".to_string(),
                    "Historical Data".to_string(),
                ],
            },
        })
    }

    pub async fn get_workflows_for_resource(
        &self,
        resource_type: &str
    ) -> Result<AnalysisWorkflowInfo, AppError> {
        info!("Fetching workflows for resource type: '{}'", resource_type);
        info!("Resource type length: {}", resource_type.len());
        info!("Resource type bytes: {:?}", resource_type.as_bytes());

        // Add extra debug info to help diagnose resource type matching issues
        info!("Attempting exact string comparison for resource type");

        // Normalize input resource type for comparison
        let normalized_resource_type = resource_type.trim();
        info!("Normalized resource type: '{}', length: {}", normalized_resource_type, normalized_resource_type.len());

        let workflows = match normalized_resource_type {
            "KinesisStream" | "kinesisstream" | "kinesis_stream" | "kinesis-stream" | "Kinesis" | "kinesis" => {
                info!("Matched KinesisStream resource type successfully");
                vec![
                    ResourceAnalysisMetadata {
                        workflow_id: "performance".to_string(),
                        name: "Stream Performance Analysis".to_string(),
                        description: "Analyze stream throughput, latency, and records processing".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string()],
                        supported_formats: vec!["markdown".to_string(), "json".to_string()],
                        estimated_duration: "1-2 minutes".to_string(),
                    },
                    ResourceAnalysisMetadata {
                        workflow_id: "cost".to_string(),
                        name: "Cost Analysis".to_string(),
                        description: "Analyze stream costs and shard usage efficiency".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string(), "ce:GetCostAndUsage".to_string()],
                        supported_formats: vec!["markdown".to_string(), "json".to_string()],
                        estimated_duration: "2-3 minutes".to_string(),
                    },
                    ResourceAnalysisMetadata {
                        workflow_id: "five-why".to_string(),
                        name: "5 Why Analysis".to_string(),
                        description: "Perform a 5 Why root cause analysis on this resource".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string()],
                        supported_formats: vec!["markdown".to_string()],
                        estimated_duration: "5-10 minutes".to_string(),
                    },
                ]
            },
            "EC2Instance" => {
                info!("Matched resource type: EC2Instance");
                vec![
                    ResourceAnalysisMetadata {
                        workflow_id: "performance".to_string(),
                        name: "Performance Analysis".to_string(),
                        description: "Analyze CPU, memory, and network performance".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string()],
                        supported_formats: vec!["markdown".to_string(), "json".to_string()],
                        estimated_duration: "1-2 minutes".to_string(),
                    },
                    ResourceAnalysisMetadata {
                        workflow_id: "cost".to_string(),
                        name: "Cost Analysis".to_string(),
                        description: "Analyze instance costs and potential savings".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string(), "ce:GetCostAndUsage".to_string()],
                        supported_formats: vec!["markdown".to_string(), "json".to_string()],
                        estimated_duration: "2-3 minutes".to_string(),
                    },
                    ResourceAnalysisMetadata {
                        workflow_id: "five-why".to_string(),
                        name: "5 Why Analysis".to_string(),
                        description: "Perform a 5 Why root cause analysis on this EC2 instance".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string()],
                        supported_formats: vec!["markdown".to_string()],
                        estimated_duration: "5-10 minutes".to_string(),
                    },
                ]
            },
            "RdsInstance" | "rdsinstance" | "rds_instance" | "rds-instance" | "Rds" | "rds" => {
                info!("Matched resource type: RdsInstance");
                vec![
                    ResourceAnalysisMetadata {
                        workflow_id: "performance".to_string(),
                        name: "Database Performance Analysis".to_string(),
                        description: "Analyze database CPU, memory, and I/O performance".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string()],
                        supported_formats: vec!["markdown".to_string(), "json".to_string()],
                        estimated_duration: "1-2 minutes".to_string(),
                    },
                    ResourceAnalysisMetadata {
                        workflow_id: "cost".to_string(),
                        name: "Database Cost Analysis".to_string(),
                        description: "Analyze instance costs and potential savings".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string(), "ce:GetCostAndUsage".to_string()],
                        supported_formats: vec!["markdown".to_string(), "json".to_string()],
                        estimated_duration: "2-3 minutes".to_string(),
                    },
                    ResourceAnalysisMetadata {
                        workflow_id: "slow-queries".to_string(),
                        name: "Slow Query Analysis".to_string(),
                        description: "Analyze slow queries and database performance bottlenecks".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string(), "rds:DescribeDBLogFiles".to_string()],
                        supported_formats: vec!["markdown".to_string(), "json".to_string()],
                        estimated_duration: "3-5 minutes".to_string(),
                    },
                    ResourceAnalysisMetadata {
                        workflow_id: "five-why".to_string(),
                        name: "5 Why Analysis".to_string(),
                        description: "Perform a 5 Why root cause analysis on this database instance".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string()],
                        supported_formats: vec!["markdown".to_string()],
                        estimated_duration: "5-10 minutes".to_string(),
                    },
                ]
            },
            "S3Bucket" | "s3bucket" | "s3_bucket" | "s3-bucket" | "S3" | "s3" => {
                info!("Matched resource type: S3Bucket");
                vec![
                    ResourceAnalysisMetadata {
                        workflow_id: "storage-usage".to_string(),
                        name: "Storage Usage Analysis".to_string(),
                        description: "Analyze bucket size, object count, and storage class distribution".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string(), "s3:ListBucket".to_string()],
                        supported_formats: vec!["markdown".to_string(), "json".to_string()],
                        estimated_duration: "1-2 minutes".to_string(),
                    },
                    ResourceAnalysisMetadata {
                        workflow_id: "cost-optimization".to_string(),
                        name: "Cost Optimization Analysis".to_string(),
                        description: "Analyze storage costs and identify optimization opportunities".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string(), "ce:GetCostAndUsage".to_string()],
                        supported_formats: vec!["markdown".to_string(), "json".to_string()],
                        estimated_duration: "2-3 minutes".to_string(),
                    },
                    ResourceAnalysisMetadata {
                        workflow_id: "five-why".to_string(),
                        name: "5 Why Analysis".to_string(),
                        description: "Perform a 5 Why root cause analysis on this S3 bucket".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string(), "s3:ListBucket".to_string()],
                        supported_formats: vec!["markdown".to_string()],
                        estimated_duration: "5-10 minutes".to_string(),
                    },
                ]
            },
            "DynamoDbTable" | "dynamodbtable" | "dynamodb_table" | "dynamodb-table" | "DynamoDB" | "dynamodb" => {
                info!("Matched resource type: DynamoDbTable");
                vec![
                    ResourceAnalysisMetadata {
                        workflow_id: "performance".to_string(),
                        name: "Table Performance Analysis".to_string(),
                        description: "Analyze read/write capacity, throttling events, and latency".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string(), "dynamodb:DescribeTable".to_string()],
                        supported_formats: vec!["markdown".to_string(), "json".to_string()],
                        estimated_duration: "1-2 minutes".to_string(),
                    },
                    ResourceAnalysisMetadata {
                        workflow_id: "cost".to_string(),
                        name: "Table Cost Analysis".to_string(),
                        description: "Analyze table costs and capacity utilization".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string(), "ce:GetCostAndUsage".to_string()],
                        supported_formats: vec!["markdown".to_string(), "json".to_string()],
                        estimated_duration: "2-3 minutes".to_string(),
                    },
                    ResourceAnalysisMetadata {
                        workflow_id: "five-why".to_string(),
                        name: "5 Why Analysis".to_string(),
                        description: "Perform a 5 Why root cause analysis on this DynamoDB table".to_string(),
                        resource_type: resource_type.to_string(),
                        required_permissions: vec!["cloudwatch:GetMetricData".to_string(), "dynamodb:DescribeTable".to_string()],
                        supported_formats: vec!["markdown".to_string()],
                        estimated_duration: "5-10 minutes".to_string(),
                    },
                ]
            },
            _ => {
                info!("No matching resource type found for: '{}'", resource_type);
                info!("Supported resource types include: KinesisStream, EC2Instance, S3Bucket, RdsInstance, DynamoDbTable, ElastiCache");
                return Err(AppError::BadRequest(format!(
                    "Unsupported resource type: {}. Please use one of the supported types: KinesisStream, EC2Instance, S3Bucket, RdsInstance, DynamoDbTable, ElastiCache", 
                    resource_type
                )))
            },
        };

        info!("Successfully retrieved workflows for resource type: {}", resource_type);

        let result = AnalysisWorkflowInfo {
            resource_type: resource_type.to_string(),
            workflows,
            common_questions: QuestionGenerator::get_common_questions(resource_type),
            best_practices_url: Some(format!(
                "https://aws.amazon.com/blogs/architecture/category/{}/",
                resource_type.to_lowercase()
            )),
        };

        info!("Returning analysis workflow info with {} workflows", result.workflows.len());
        Ok(result)
    }

    pub async fn answer_resource_question(
        &self,
        request: &ResourceRelatedQuestionRequest,
    ) -> Result<AwsResourceAnalysisResponse, AppError> {
        // Get resource details
        let resource = self.aws_resource_repo
            .find_by_arn(&request.resource_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!(
                "Resource {} not found",
                request.resource_id
            )))?;

        // Get recent metrics
        let metrics_request = self.get_metrics_request(
            request.resource_id.clone(),
            resource.resource_type.clone(),
            resource.region.clone(),
            None,
        ).await;

        let metrics = self.aws_data_plane
            .get_cloudwatch_metrics(&metrics_request)
            .await?;

        // Generate answer based on question and context
        let answer = match resource.resource_type.as_str() {
            "EC2Instance" => Ec2Analyzer::answer_ec2_question(
                &resource,
                &request.question,
                &metrics
            ).await?,
            "RdsInstance" => RdsAnalyzer::answer_rds_question(
                &resource,
                &request.question,
                &metrics
            ).await?,
            "DynamoDbTable" => DynamoDbAnalyzer::answer_dynamodb_question(
                &resource,
                &request.question,
                &metrics
            ).await?,
            // Add other resource types...
            _ => return Err(AppError::BadRequest(format!(
                "Unsupported resource type: {}",
                resource.resource_type
            ))),
        };

        Ok(AwsResourceAnalysisResponse {
            format: "markdown".to_string(),
            content: answer,
            related_questions: QuestionGenerator::generate_followup_questions(
                &resource.resource_type,
                &request.question
            ),
            metadata: AnalysisMetadata {
                timestamp: Utc::now(),
                resource_type: resource.resource_type,
                workflow_type: request.workflow.clone().unwrap_or_else(|| "question".to_string()),
                time_range: None,
                data_sources: vec![
                    "CloudWatch Metrics".to_string(),
                    "Resource Configuration".to_string(),
                ],
            },
        })
    }

    // Helper method to get metrics request
    async fn get_metrics_request(
        &self,
        resource_id: String,
        resource_type: String,
        region: String,
        time_range: Option<String>,
    ) -> cloud_watch::CloudWatchMetricsRequest {
        cloud_watch::CloudWatchMetricsRequest {
            metrics: vec!["CPUUtilization", "MemoryUtilization", "NetworkIn", "NetworkOut"]
                .into_iter()
                .map(String::from)
                .collect(),
            resource_id,
            resource_type,
            region,
            start_time: time_range
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok())
                .map(|t| t.with_timezone(&Utc))
                .unwrap_or_else(|| Utc::now() - chrono::Duration::hours(24)),
            end_time: Utc::now(),
            period: 300, // 5-minute intervals
        }
    }
}
