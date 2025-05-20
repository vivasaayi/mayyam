use std::sync::Arc;
use chrono::Utc;
use tracing::info;
use crate::errors::AppError;
use crate::config::Config;
use crate::services::aws::{self, AwsService, AwsDataPlane};
use crate::models::aws_resource;
use crate::repositories::aws_resource::AwsResourceRepository;

pub mod models {
    pub mod analytics {
        use chrono::Utc;
        use serde::{Serialize, Deserialize};
        
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
        use serde::{Serialize, Deserialize};

        #[derive(Debug, Serialize, Deserialize)]
        pub enum ResourceAnalysisWorkflow {
            Performance,
            Cost,
            Storage,
            Memory,
        }

        impl ResourceAnalysisWorkflow {
            pub fn from_str(s: &str) -> Result<Self, String> {
                match s.to_lowercase().as_str() {
                    "performance" => Ok(Self::Performance),
                    "cost" => Ok(Self::Cost),
                    "storage" => Ok(Self::Storage),
                    "memory" => Ok(Self::Memory),
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
}

use self::models::{analytics::*, resource_workflows::*};

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
            "EC2Instance" => self.analyze_ec2_instance(
                &resource,
                &workflow,
                &metrics
            ).await?,
            "S3Bucket" => self.analyze_s3_bucket(
                &resource,
                &workflow,
                &metrics
            ).await?,
            "RdsInstance" => self.analyze_rds_instance(
                &resource,
                &workflow,
                &metrics
            ).await?,
            "DynamoDbTable" => self.analyze_dynamodb_table(
                &resource,
                &workflow,
                &metrics
            ).await?,
            "ElastiCache" => self.analyze_elasticache_cluster(
                &resource,
                &workflow,
                &metrics
            ).await?,
            "KinesisStream" => self.analyze_kinesis_stream(
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
            related_questions: self.generate_related_questions(
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
            common_questions: self.get_common_questions(resource_type),
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
            "EC2Instance" => self.answer_ec2_question(
                &resource,
                &request.question,
                &metrics
            ).await?,
            "RdsInstance" => self.answer_rds_question(
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
            related_questions: self.generate_followup_questions(
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

    // Helper methods for specific resource types...
    async fn analyze_ec2_instance(
        &self,
        _resource: &aws_resource::Model,  // Using _ prefix to indicate intentionally unused
        workflow: &ResourceAnalysisWorkflow,
        metrics: &aws::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut analysis = String::new();

        match workflow {
            ResourceAnalysisWorkflow::Performance => {
                analysis.push_str("# EC2 Instance Performance Analysis\n\n");

                // Analyze CPU metrics
                if let Some(cpu_metric) = self.find_metric(metrics, "CPUUtilization") {
                    let (avg, max) = self.calculate_statistics(&cpu_metric.datapoints);
                    analysis.push_str(&format!(
                        "## CPU Utilization\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                        avg, max
                    ));

                    // Add recommendations based on CPU usage
                    if max > 80.0 {
                        analysis.push_str("⚠️ **High CPU Usage Detected**\n");
                        analysis.push_str("Consider:\n");
                        analysis.push_str("1. Scaling up the instance type\n");
                        analysis.push_str("2. Using auto-scaling groups\n");
                        analysis.push_str("3. Analyzing resource-intensive processes\n\n");
                    }
                }

                // Analyze memory metrics if available
                if let Some(mem_metric) = self.find_metric(metrics, "MemoryUtilization") {
                    let (avg, max) = self.calculate_statistics(&mem_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Memory Utilization\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                        avg, max
                    ));
                }

                // Network analysis
                self.analyze_network_metrics(&mut analysis, metrics);
            },
            ResourceAnalysisWorkflow::Cost => {
                analysis.push_str("# EC2 Instance Cost Analysis\n\n");
                // Implement cost analysis...
                analysis.push_str("Cost analysis not yet implemented\n");
            },
            _ => {
                return Err(AppError::BadRequest(
                    "Unsupported workflow type for EC2 instance".to_string()
                ));
            }
        }

        Ok(analysis)
    }

    async fn analyze_rds_instance(
        &self,
        _resource: &aws_resource::Model,  // Using _ prefix to indicate intentionally unused
        workflow: &ResourceAnalysisWorkflow,
        metrics: &aws::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut analysis = String::new();

        match workflow {
            ResourceAnalysisWorkflow::Performance => {
                analysis.push_str("# RDS Instance Performance Analysis\n\n");

                // CPU analysis
                if let Some(cpu_metric) = self.find_metric(metrics, "CPUUtilization") {
                    let (avg, max) = self.calculate_statistics(&cpu_metric.datapoints);
                    analysis.push_str(&format!(
                        "## CPU Utilization\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                        avg, max
                    ));
                }

                // Memory analysis
                if let Some(mem_metric) = self.find_metric(metrics, "FreeableMemory") {
                    let (avg, _) = self.calculate_statistics(&mem_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Memory Usage\n- Average Free Memory: {:.2} MB\n\n",
                        avg / (1024.0 * 1024.0)
                    ));
                }
            },
            ResourceAnalysisWorkflow::Storage => {
                analysis.push_str("# RDS Storage Analysis\n\n");

                // Storage usage analysis
                if let Some(storage_metric) = self.find_metric(metrics, "FreeStorageSpace") {
                    let (avg, _) = self.calculate_statistics(&storage_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Storage Usage\n- Average Free Space: {:.2} GB\n\n",
                        avg / (1024.0 * 1024.0 * 1024.0)
                    ));
                }
            },
            _ => return Err(AppError::BadRequest(
                "Unsupported workflow type for RDS instance".to_string()
            )),
        }

        Ok(analysis)
    }

    async fn analyze_dynamodb_table(
        &self,
        _resource: &aws_resource::Model,  // Using _ prefix to indicate intentionally unused
        workflow: &ResourceAnalysisWorkflow,
        metrics: &aws::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut analysis = String::new();

        match workflow {
            ResourceAnalysisWorkflow::Performance => {
                analysis.push_str("# DynamoDB Table Performance Analysis\n\n");

                // Analyze read/write capacity
                if let Some(read_metric) = self.find_metric(metrics, "ConsumedReadCapacityUnits") {
                    let (avg, max) = self.calculate_statistics(&read_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Read Capacity Usage\n- Average: {:.2} RCUs\n- Peak: {:.2} RCUs\n\n",
                        avg, max
                    ));
                }
            },
            ResourceAnalysisWorkflow::Cost => {
                analysis.push_str("# DynamoDB Cost Analysis\n\n");
                // Add cost analysis implementation
            },
            _ => return Err(AppError::BadRequest(
                "Unsupported workflow type for DynamoDB table".to_string()
            )),
        }

        Ok(analysis)
    }

    async fn analyze_elasticache_cluster(
        &self,
        _resource: &aws_resource::Model,  // Using _ prefix to indicate intentionally unused
        workflow: &ResourceAnalysisWorkflow,
        metrics: &aws::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut analysis = String::new();

        match workflow {
            ResourceAnalysisWorkflow::Performance => {
                analysis.push_str("# ElastiCache Cluster Performance Analysis\n\n");

                // Cache hit rate analysis
                if let Some(hits_metric) = self.find_metric(metrics, "CacheHits") {
                    if let Some(misses_metric) = self.find_metric(metrics, "CacheMisses") {
                        let (hits_avg, _) = self.calculate_statistics(&hits_metric.datapoints);
                        let (misses_avg, _) = self.calculate_statistics(&misses_metric.datapoints);
                        let hit_rate = hits_avg / (hits_avg + misses_avg) * 100.0;
                        analysis.push_str(&format!(
                            "## Cache Hit Rate\n- Average: {:.2}%\n\n",
                            hit_rate
                        ));
                    }
                }
            },
            ResourceAnalysisWorkflow::Memory => {
                analysis.push_str("# ElastiCache Memory Analysis\n\n");
                // Add memory analysis implementation
            },
            _ => return Err(AppError::BadRequest(
                "Unsupported workflow type for ElastiCache cluster".to_string()
            )),
        }

        Ok(analysis)
    }

    async fn answer_ec2_question(
        &self,
        _resource: &aws_resource::Model,  // Using _ prefix to indicate intentionally unused
        question: &str,
        metrics: &aws::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut answer = String::new();

        // Convert question to lowercase for easier matching
        let question = question.to_lowercase();

        if question.contains("cpu") || question.contains("processor") {
            if let Some(cpu_metric) = self.find_metric(metrics, "CPUUtilization") {
                let (avg, max) = self.calculate_statistics(&cpu_metric.datapoints);
                answer.push_str(&format!(
                    "The EC2 instance's CPU usage:\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                    avg, max
                ));
            }
        }

        if question.contains("memory") || question.contains("ram") {
            if let Some(mem_metric) = self.find_metric(metrics, "MemoryUtilization") {
                let (avg, max) = self.calculate_statistics(&mem_metric.datapoints);
                answer.push_str(&format!(
                    "Memory utilization:\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                    avg, max
                ));
            }
        }

        if answer.is_empty() {
            answer = "I apologize, but I don't have enough information to answer that specific question about the EC2 instance.".to_string();
        }

        Ok(answer)
    }

    async fn answer_rds_question(
        &self,
        _resource: &aws_resource::Model,  // Using _ prefix to indicate intentionally unused
        question: &str,
        metrics: &aws::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut answer = String::new();

        let question = question.to_lowercase();

        if question.contains("cpu") {
            if let Some(cpu_metric) = self.find_metric(metrics, "CPUUtilization") {
                let (avg, max) = self.calculate_statistics(&cpu_metric.datapoints);
                answer.push_str(&format!(
                    "The RDS instance's CPU usage:\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                    avg, max
                ));
            }
        }

        if question.contains("memory") || question.contains("ram") {
            if let Some(mem_metric) = self.find_metric(metrics, "FreeableMemory") {
                let (avg, _) = self.calculate_statistics(&mem_metric.datapoints);
                answer.push_str(&format!(
                    "Memory status:\n- Average Free Memory: {:.2} MB\n\n",
                    avg / (1024.0 * 1024.0)
                ));
            }
        }

        if answer.is_empty() {
            answer = "I apologize, but I don't have enough information to answer that specific question about the RDS instance.".to_string();
        }

        Ok(answer)
    }

    async fn analyze_s3_bucket(
        &self,
        resource: &aws_resource::Model,
        workflow: &ResourceAnalysisWorkflow,
        metrics: &aws::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut analysis = String::new();

        match workflow {
            ResourceAnalysisWorkflow::Cost => {
                analysis.push_str("# S3 Bucket Cost Analysis\n\n");
                // Implement cost analysis for S3...
            },
            _ => return Err(AppError::BadRequest(
                format!("Unsupported workflow for S3: {:?}", workflow)
            )),
        }

        Ok(analysis)
    }

    async fn get_metrics_request(
        &self,
        resource_id: String,
        resource_type: String,
        region: String,
        time_range: Option<String>,
    ) -> aws::CloudWatchMetricsRequest {
        aws::CloudWatchMetricsRequest {
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

    // Helper methods for metrics analysis
    fn find_metric<'a>(
        &self,
        metrics: &'a aws::CloudWatchMetricsResult,
        name: &str
    ) -> Option<&'a aws::CloudWatchMetricData> {
        metrics.metrics.iter().find(|m| m.metric_name == name)
    }

    fn calculate_statistics(&self, datapoints: &[aws::CloudWatchDatapoint]) -> (f64, f64) {
        if datapoints.is_empty() {
            return (0.0, 0.0);
        }

        let sum: f64 = datapoints.iter().map(|d| d.value).sum();
        let max: f64 = datapoints.iter().map(|d| d.value).fold(f64::NEG_INFINITY, f64::max);
        let avg = sum / datapoints.len() as f64;

        (avg, max)
    }

    fn analyze_network_metrics(
        &self,
        analysis: &mut String,
        metrics: &aws::CloudWatchMetricsResult,
    ) {
        analysis.push_str("## Network Performance\n");

        if let Some(net_in) = self.find_metric(metrics, "NetworkIn") {
            let (avg, max) = self.calculate_statistics(&net_in.datapoints);
            analysis.push_str(&format!(
                "Network In:\n- Average: {:.2} MB/s\n- Peak: {:.2} MB/s\n\n",
                avg / (1024.0 * 1024.0),
                max / (1024.0 * 1024.0)
            ));
        }

        if let Some(net_out) = self.find_metric(metrics, "NetworkOut") {
            let (avg, max) = self.calculate_statistics(&net_out.datapoints);
            analysis.push_str(&format!(
                "Network Out:\n- Average: {:.2} MB/s\n- Peak: {:.2} MB/s\n\n",
                avg / (1024.0 * 1024.0),
                max / (1024.0 * 1024.0)
            ));
        }
    }

    fn get_common_questions(&self, resource_type: &str) -> Vec<String> {
        match resource_type {
            "EC2Instance" => vec![
                "What is the CPU utilization?".to_string(),
                "How much memory is being used?".to_string(),
                "What is the network performance?".to_string(),
                "Are there any performance bottlenecks?".to_string(),
            ],
            "RdsInstance" => vec![
                "What is the database CPU usage?".to_string(),
                "How much free memory is available?".to_string(),
                "What is the storage usage trend?".to_string(),
                "Are there any slow queries?".to_string(),
            ],
            "DynamoDbTable" => vec![
                "What is the consumed read capacity?".to_string(),
                "What is the consumed write capacity?".to_string(),
                "Are there any throttled requests?".to_string(),
                "What is the storage usage?".to_string(),
            ],
            "ElasticacheCluster" => vec![
                "What is the cache hit rate?".to_string(),
                "How much memory is being used?".to_string(),
                "Are there any evictions?".to_string(),
                "What is the network bandwidth usage?".to_string(),
            ],
            "KinesisStream" => vec![
                "What is the consumer lag?".to_string(),
                "How many records are being processed?".to_string(),
                "Are there any throughput exceeded events?".to_string(),
                "Do I need to increase my shard count?".to_string(),
            ],
            _ => vec![],
        }
    }

    fn generate_related_questions(&self, resource_type: &str, workflow: &str) -> Vec<String> {
        let mut questions = self.get_common_questions(resource_type);

        // Add workflow-specific questions
        match workflow.to_lowercase().as_str() {
            "performance" => questions.extend(vec![
                "What are the peak usage times?".to_string(),
                "Are there any performance bottlenecks?".to_string(),
                "How does the current performance compare to last week?".to_string(),
            ]),
            "cost" => questions.extend(vec![
                "What is the monthly cost trend?".to_string(),
                "Are there any cost optimization opportunities?".to_string(),
                "How does the cost compare to similar resources?".to_string(),
            ]),
            _ => {},
        }

        questions
    }

    async fn analyze_kinesis_stream(
        &self,
        resource: &aws_resource::Model,
        workflow: &ResourceAnalysisWorkflow,
        metrics: &aws::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut analysis = String::new();

        match workflow {
            ResourceAnalysisWorkflow::Performance => {
                analysis.push_str("# Kinesis Stream Performance Analysis\n\n");

                // Add stream info
                analysis.push_str(&format!("## Stream Information\n"));
                analysis.push_str(&format!("- Stream: {}\n", resource.resource_id));
                analysis.push_str(&format!("- ARN: {}\n", resource.arn));
                analysis.push_str(&format!("- Region: {}\n\n", resource.region));

                // Analyze GetRecords.IteratorAgeMilliseconds - measures how far behind the consumer is
                if let Some(iterator_age) = self.find_metric(metrics, "GetRecords.IteratorAgeMilliseconds") {
                    let (avg, max) = self.calculate_statistics(&iterator_age.datapoints);
                    analysis.push_str(&format!(
                        "## Consumer Lag\n- Average Iterator Age: {:.2} ms\n- Maximum Iterator Age: {:.2} ms\n\n",
                        avg, max
                    ));

                    if max > 30000.0 {
                        analysis.push_str("⚠️ **High Consumer Lag Detected**\n");
                        analysis.push_str("Your consumers are falling behind processing the stream. Consider:\n");
                        analysis.push_str("1. Increasing the number of consumer instances\n");
                        analysis.push_str("2. Optimizing consumer code for faster processing\n");
                        analysis.push_str("3. Increasing the resource allocation for consumers\n\n");
                    }
                }

                // Analyze throughput
                if let Some(incoming_records) = self.find_metric(metrics, "IncomingRecords") {
                    let (avg, max) = self.calculate_statistics(&incoming_records.datapoints);
                    analysis.push_str(&format!(
                        "## Incoming Records\n- Average: {:.2} records/second\n- Peak: {:.2} records/second\n\n",
                        avg, max
                    ));
                }

                if let Some(incoming_bytes) = self.find_metric(metrics, "IncomingBytes") {
                    let (avg, max) = self.calculate_statistics(&incoming_bytes.datapoints);
                    analysis.push_str(&format!(
                        "## Incoming Data\n- Average: {:.2} KB/s\n- Peak: {:.2} KB/s\n\n",
                        avg / 1024.0, max / 1024.0
                    ));
                }

                // Add shard metrics if available
                if let Some(read_throughput) = self.find_metric(metrics, "ReadProvisionedThroughputExceeded") {
                    let (avg, max) = self.calculate_statistics(&read_throughput.datapoints);
                    if max > 0.0 {
                        analysis.push_str("⚠️ **Throughput Exceeded**\n");
                        analysis.push_str("Your stream has experienced throttling events. Consider:\n");
                        analysis.push_str("1. Increasing the number of shards\n");
                        analysis.push_str("2. Implementing a more even distribution of partition keys\n\n");
                    } else {
                        analysis.push_str("✅ **No Throughput Exceeded Events**\n");
                        analysis.push_str("Your stream is handling the current load without throttling.\n\n");
                    }
                }

                // Add recommendations section
                analysis.push_str("## Recommendations\n");
                analysis.push_str("1. Monitor the Iterator Age metric closely to ensure consumers keep up with producers\n");
                analysis.push_str("2. Implement auto-scaling for consumers based on Iterator Age\n");
                analysis.push_str("3. Consider Enhanced Fan-Out for high-throughput applications\n");
                analysis.push_str("4. Review your shard count to ensure adequate capacity\n");
            },
            ResourceAnalysisWorkflow::Cost => {
                analysis.push_str("# Kinesis Stream Cost Analysis\n\n");

                // Add stream info
                analysis.push_str(&format!("## Stream Information\n"));
                analysis.push_str(&format!("- Stream: {}\n", resource.resource_id));
                analysis.push_str(&format!("- ARN: {}\n", resource.arn));
                analysis.push_str(&format!("- Region: {}\n\n", resource.region));

                // Extract number of shards if available in resource data
                let shard_count = resource.resource_data.get("ShardCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1);

                // Calculate estimated costs
                let hourly_shard_cost = match resource.region.as_str() {
                    "us-east-1" => 0.015,  // USD per shard hour
                    "us-west-2" => 0.015,
                    "eu-west-1" => 0.017,
                    _ => 0.016,  // Average cost for other regions
                };

                let monthly_shard_cost = hourly_shard_cost * 24.0 * 30.0;
                let total_monthly_cost = monthly_shard_cost * shard_count as f64;

                analysis.push_str(&format!("## Cost Analysis\n"));
                analysis.push_str(&format!("- Shard Count: {}\n", shard_count));
                analysis.push_str(&format!("- Cost per Shard per Hour: ${:.4}\n", hourly_shard_cost));
                analysis.push_str(&format!("- Estimated Monthly Cost: ${:.2}\n\n", total_monthly_cost));

                // Add PUT payload units analysis if available
                if let Some(incoming_bytes) = self.find_metric(metrics, "IncomingBytes") {
                    let (avg_bytes, _) = self.calculate_statistics(&incoming_bytes.datapoints);
                    let avg_bytes_per_month = avg_bytes * 60.0 * 60.0 * 24.0 * 30.0;
                    let million_put_payload_units = avg_bytes_per_month / (1024.0 * 1024.0);

                    // Cost per million PUT payload units (25KB = 1 PUT payload unit)
                    let put_cost_per_million = 0.014;  // USD per million PUT payload units
                    let put_cost = (million_put_payload_units / 25.0) * put_cost_per_million;

                    analysis.push_str(&format!("## Data Transfer Costs\n"));
                    analysis.push_str(&format!("- Estimated Monthly Data Transfer: {:.2} GB\n", avg_bytes_per_month / (1024.0 * 1024.0 * 1024.0)));
                    analysis.push_str(&format!("- Estimated PUT Cost: ${:.2}\n\n", put_cost));

                    analysis.push_str(&format!("## Total Estimated Monthly Cost\n"));
                    analysis.push_str(&format!("- Shard Cost: ${:.2}\n", total_monthly_cost));
                    analysis.push_str(&format!("- PUT Cost: ${:.2}\n", put_cost));
                    analysis.push_str(&format!("- Total: ${:.2}\n\n", total_monthly_cost + put_cost));
                }

                // Add cost optimization recommendations
                analysis.push_str("## Cost Optimization Recommendations\n");

                if shard_count > 1 {
                    // Check if shards are underutilized
                    if let Some(incoming_records) = self.find_metric(metrics, "IncomingRecords") {
                        let (avg_records, max_records) = self.calculate_statistics(&incoming_records.datapoints);

                        // 1000 records per second per shard is a common threshold
                        let max_capacity = shard_count as f64 * 1000.0;
                        let utilization_pct = (max_records / max_capacity) * 100.0;

                        if utilization_pct < 50.0 {
                            analysis.push_str("1. **Consider Reducing Shard Count**\n");
                            analysis.push_str(&format!("   - Current peak utilization: {:.1} % of capacity\n", utilization_pct));
                            analysis.push_str(&format!("   - Potential savings: ${:.2} per month by reducing shards\n\n",
                                                       total_monthly_cost * 0.5));
                        }
                    }
                }

                analysis.push_str("2. **Evaluate Enhanced Fan-Out Necessity**\n");
                analysis.push_str("   - Enhanced Fan-Out costs $0.015 per consumer-shard hour\n");
                analysis.push_str("   - Only use for applications requiring dedicated throughput\n\n");

                analysis.push_str("3. **Consider Kinesis Analytics**\n");
                analysis.push_str("   - For real-time analytics, Kinesis Analytics may be more cost-effective than maintaining multiple consumers\n\n");

                analysis.push_str("4. **Review Data Retention Period**\n");
                analysis.push_str("   - Extended retention beyond 24 hours costs $0.02 per shard per hour\n");
                analysis.push_str("   - Only extend retention if absolutely necessary\n");
            },
            _ => return Err(AppError::BadRequest(
                "Unsupported workflow type for Kinesis Stream".to_string()
            )),
        }

        Ok(analysis)
    }

    fn generate_followup_questions(&self, resource_type: &str, original_question: &str) -> Vec<String> {
        let mut questions = Vec::new();
        let question = original_question.to_lowercase();

        if question.contains("cpu") || question.contains("performance") {
            questions.extend(vec![
                "What is the memory usage like?".to_string(),
                "Are there any correlated metrics?".to_string(),
                "What time periods show the highest usage?".to_string(),
            ]);
        }

        if question.contains("memory") || question.contains("ram") {
            questions.extend(vec![
                "Is there any swap usage?".to_string(),
                "What processes are using the most memory?".to_string(),
                "Has memory usage been increasing over time?".to_string(),
            ]);
        }

        // Add resource-specific follow-up questions
        questions.extend(self.get_common_questions(resource_type));

        questions
    }
}