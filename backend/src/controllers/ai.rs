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


use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::services::llm::interface::LlmRequestBuilder;
use crate::services::llm::manager::{LlmGenerationRequest, UnifiedLlmManager};
use actix_web::web::Bytes;
use actix_web::{web, HttpResponse, Responder};
use futures::{stream, StreamExt};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogAnalysisRequest {
    pub logs: String,
    pub context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetricAnalysisRequest {
    pub metrics: Vec<Metric>,
    pub timeframe: String,
    pub context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub values: Vec<f64>,
    pub timestamps: Vec<i64>,
    pub unit: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryOptimizationRequest {
    pub query: String,
    pub db_type: String,
    pub schema: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KubernetesExplainRequest {
    pub resource: String,
    pub resource_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TroubleshootRequest {
    pub issue: String,
    pub context: String,
    pub logs: Option<String>,
    pub metrics: Option<Vec<Metric>>,
}

// Types for RDS Analysis

#[derive(Debug, Serialize, Deserialize)]
pub struct RdsAnalysisRequest {
    pub instance_id: String,
    pub workflow: String,
    pub time_range: Option<String>,
    pub additional_context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RdsAnalysisResponse {
    pub format: String,
    pub content: String,
    pub related_questions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RelatedQuestionRequest {
    pub instance_id: String,
    pub question: String,
    pub workflow: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DynamoDBAnalysisRequest {
    pub table_id: String,
    pub workflow: String,
    pub time_range: Option<String>,
    pub additional_context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DynamoDBAnalysisResponse {
    pub format: String,
    pub content: String,
    pub related_questions: Vec<String>,
}

pub async fn chat(
    req: web::Json<ChatRequest>,
    config: Option<web::Data<crate::config::Config>>,
    llm_integration_service: Option<web::Data<Arc<crate::services::llm::LlmIntegrationService>>>,
    llm_provider_repo: Option<
        web::Data<Arc<crate::repositories::llm_provider::LlmProviderRepository>>,
    >,
    _claims: Option<web::ReqData<Claims>>,
) -> Result<impl Responder, AppError> {
    // Basic input validation & limits
    const MAX_MESSAGE_LEN: usize = 4000;
    const MAX_MESSAGES: usize = 50;
    if req.messages.is_empty() {
        return Err(AppError::BadRequest(
            "At least one message is required".to_string(),
        ));
    }
    if req.messages.len() > MAX_MESSAGES {
        return Err(AppError::BadRequest(format!(
            "Too many messages (max {})",
            MAX_MESSAGES
        )));
    }
    let too_long = req
        .messages
        .iter()
        .any(|m| m.content.len() > MAX_MESSAGE_LEN);
    if too_long {
        return Err(AppError::BadRequest(format!(
            "Message too long (max {} chars)",
            MAX_MESSAGE_LEN
        )));
    }
    // Optional simple sanitization to avoid accidental HTML/script injection echoes
    let strip_html = Regex::new(r"<[^>]+>").ok();
    // Find provider by model name (or fallback to default)
    let config =
        config.ok_or_else(|| AppError::Internal("Missing Config in app state".to_string()))?;
    let llm_provider_repo = llm_provider_repo.ok_or_else(|| {
        AppError::Internal("Missing LlmProviderRepository in app state".to_string())
    })?;
    let llm_integration_service = llm_integration_service.ok_or_else(|| {
        AppError::Internal("Missing LlmIntegrationService in app state".to_string())
    })?;
    let model_name = req.model.clone().unwrap_or_else(|| config.ai.model.clone());
    let provider = llm_provider_repo
        .find_by_model_name(&model_name)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("LLM provider for model '{}' not found", model_name))
        })?;
    let provider_id = provider.id;

    // Compose prompt from chat history (simple: join user messages)
    let prompt = req
        .messages
        .iter()
        .filter(|m| m.role == "user")
        .map(|m| {
            let mut c = m.content.clone();
            if let Some(re) = &strip_html {
                c = re.replace_all(&c, "").to_string();
            }
            c
        })
        .collect::<Vec<_>>()
        .join("\n");

    let system_prompt = req.messages.iter().find(|m| m.role == "system").map(|m| {
        let mut c = m.content.clone();
        if let Some(re) = &strip_html {
            c = re.replace_all(&c, "").to_string();
        }
        c
    });

    let llm_request = crate::services::llm::LlmRequest {
        prompt,
        system_prompt,
        temperature: req.temperature,
        max_tokens: None,
        variables: None,
    };

    let llm_response = llm_integration_service
        .generate_response(provider_id, llm_request)
        .await?;

    let response = serde_json::json!({
        "id": format!("chatcmpl-{}", provider_id),
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": model_name,
        "choices": [{
            "message": {
                "role": "assistant",
                "content": llm_response.content
            },
            "index": 0,
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": llm_response.tokens_used.unwrap_or(0),
            "completion_tokens": 0,
            "total_tokens": llm_response.tokens_used.unwrap_or(0)
        }
    });
    Ok(HttpResponse::Ok().json(response))
}

/// Streaming chat via Server-Sent Events (SSE)
pub async fn chat_stream(
    req: web::Json<ChatRequest>,
    llm_manager: Option<web::Data<Arc<UnifiedLlmManager>>>,
    llm_provider_repo: Option<
        web::Data<Arc<crate::repositories::llm_provider::LlmProviderRepository>>,
    >,
    _claims: Option<web::ReqData<Claims>>,
    config: Option<web::Data<crate::config::Config>>,
) -> Result<HttpResponse, AppError> {
    // Validation (same limits as non-streaming)
    const MAX_MESSAGE_LEN: usize = 4000;
    const MAX_MESSAGES: usize = 50;
    if req.messages.is_empty() {
        return Err(AppError::BadRequest(
            "At least one message is required".to_string(),
        ));
    }
    if req.messages.len() > MAX_MESSAGES {
        return Err(AppError::BadRequest(format!(
            "Too many messages (max {})",
            MAX_MESSAGES
        )));
    }
    if req
        .messages
        .iter()
        .any(|m| m.content.len() > MAX_MESSAGE_LEN)
    {
        return Err(AppError::BadRequest(format!(
            "Message too long (max {} chars)",
            MAX_MESSAGE_LEN
        )));
    }

    // Only resolve dependencies after validation to keep tests simple
    let config =
        config.ok_or_else(|| AppError::Internal("Missing Config in app state".to_string()))?;
    let llm_provider_repo = llm_provider_repo.ok_or_else(|| {
        AppError::Internal("Missing LlmProviderRepository in app state".to_string())
    })?;
    let llm_manager = llm_manager
        .ok_or_else(|| AppError::Internal("Missing UnifiedLlmManager in app state".to_string()))?;

    let model_name = req.model.clone().unwrap_or_else(|| config.ai.model.clone());
    let provider = llm_provider_repo
        .find_by_model_name(&model_name)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("LLM provider for model '{}' not found", model_name))
        })?;

    // Build prompt/system
    let strip_html = Regex::new(r"<[^>]+>").ok();
    let prompt = req
        .messages
        .iter()
        .filter(|m| m.role == "user")
        .map(|m| {
            let mut c = m.content.clone();
            if let Some(re) = &strip_html {
                c = re.replace_all(&c, "").to_string();
            }
            c
        })
        .collect::<Vec<_>>()
        .join("\n");
    let system_prompt = req.messages.iter().find(|m| m.role == "system").map(|m| {
        let mut c = m.content.clone();
        if let Some(re) = &strip_html {
            c = re.replace_all(&c, "").to_string();
        }
        c
    });

    let mut builder = LlmRequestBuilder::new()
        .prompt(prompt)
        .temperature(req.temperature.unwrap_or(1.0))
        .max_tokens(1000)
        .stream(true);
    if let Some(sp) = system_prompt {
        builder = builder.system_prompt(sp);
    }
    let llm_request = builder.build();

    // Request streaming from the provider via manager
    let rx = llm_manager
        .generate_stream(LlmGenerationRequest {
            provider: provider.id.to_string(),
            model: Some(provider.model_name.clone()),
            request: llm_request,
            format_response: Some(false),
            formatting_options: None,
        })
        .await?;

    // Convert mpsc Receiver into SSE stream
    let sse_stream = stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Some(Ok(chunk)) => {
                let line = format!("data: {}\n\n", chunk);
                Some((Ok::<Bytes, actix_web::Error>(Bytes::from(line)), rx))
            }
            Some(Err(e)) => {
                let line = format!("event: error\ndata: {}\n\n", e.to_string());
                Some((Ok::<Bytes, actix_web::Error>(Bytes::from(line)), rx))
            }
            None => None,
        }
    });

    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .streaming(sse_stream))
}

pub async fn analyze_logs(
    req: web::Json<LogAnalysisRequest>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    // In a real implementation, we would process the logs and call an AI API
    // For now, simulate a response

    let analysis = serde_json::json!({
        "summary": "Simulated log analysis summary",
        "issues": [
            {
                "severity": "high",
                "message": "Multiple authentication failures detected",
                "occurrence_count": 12,
                "first_timestamp": "2023-05-01T12:34:56Z",
                "last_timestamp": "2023-05-01T12:45:23Z"
            },
            {
                "severity": "medium",
                "message": "Increased latency in database responses",
                "occurrence_count": 8,
                "first_timestamp": "2023-05-01T12:40:00Z",
                "last_timestamp": "2023-05-01T12:48:12Z"
            }
        ],
        "recommendations": [
            "Check for potential brute force attacks on authentication endpoints",
            "Review database query performance and consider optimization"
        ]
    });

    Ok(HttpResponse::Ok().json(analysis))
}

pub async fn analyze_metrics(
    req: web::Json<MetricAnalysisRequest>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    // In a real implementation, we would process the metrics and call an AI API
    // For now, simulate a response

    let analysis = serde_json::json!({
        "summary": "Simulated metrics analysis summary",
        "anomalies": [
            {
                "metric": "cpu_usage",
                "timestamp": "2023-05-01T12:42:10Z",
                "value": 95.5,
                "expected_range": [10.0, 70.0],
                "severity": "high"
            },
            {
                "metric": "memory_usage",
                "timestamp": "2023-05-01T12:43:22Z",
                "value": 87.2,
                "expected_range": [20.0, 80.0],
                "severity": "medium"
            }
        ],
        "trends": [
            {
                "metric": "request_latency",
                "trend": "increasing",
                "rate": "+15% over the timeframe",
                "concern_level": "medium"
            }
        ],
        "recommendations": [
            "Investigate CPU spike at 12:42:10",
            "Consider scaling resources if latency trend continues"
        ]
    });

    Ok(HttpResponse::Ok().json(analysis))
}

pub async fn optimize_query(
    req: web::Json<QueryOptimizationRequest>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    // In a real implementation, we would analyze the query and call an AI API
    // For now, simulate a response

    // Example original query
    let original_query = &req.query;

    let optimization = serde_json::json!({
        "original_query": original_query,
        "optimized_query": "SELECT u.id, u.name, COUNT(o.id) as order_count FROM users u LEFT JOIN orders o ON u.id = o.user_id WHERE u.status = 'active' GROUP BY u.id, u.name",
        "explanation": [
            "Added index hint to use idx_user_id on the orders table",
            "Removed unnecessary columns from the SELECT clause",
            "Changed the JOIN condition to improve efficiency"
        ],
        "estimated_improvement": "60% reduced execution time",
        "recommended_indexes": [
            {
                "table": "users",
                "columns": ["status"],
                "name": "idx_user_status"
            }
        ]
    });

    Ok(HttpResponse::Ok().json(optimization))
}

pub async fn explain_kubernetes(
    req: web::Json<KubernetesExplainRequest>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    // In a real implementation, we would analyze the K8s resource and call an AI API
    // For now, simulate a response

    let explanation = serde_json::json!({
        "resource_type": req.resource_type,
        "explanation": "This Kubernetes Deployment manages a replicated application, ensuring that the specified number of pod replicas are running at all times. It uses a selector to identify which pods it manages, and a template that defines the pod specification.",
        "key_components": [
            {
                "field": "spec.replicas",
                "value": "3",
                "explanation": "Specifies that 3 pod replicas should be maintained"
            },
            {
                "field": "spec.strategy",
                "value": "RollingUpdate",
                "explanation": "Specifies that updates should be rolled out gradually rather than all at once"
            },
            {
                "field": "spec.template.spec.containers[0].resources",
                "explanation": "Resource requests and limits seem appropriate for the workload"
            }
        ],
        "potential_issues": [
            {
                "severity": "medium",
                "issue": "No liveness probe defined",
                "recommendation": "Add a liveness probe to detect and restart unhealthy containers"
            },
            {
                "severity": "low",
                "issue": "No pod disruption budget defined",
                "recommendation": "Consider adding a PodDisruptionBudget to ensure availability during voluntary disruptions"
            }
        ],
        "best_practices": [
            "Add resource requests and limits for all containers",
            "Implement liveness and readiness probes",
            "Use network policies to restrict traffic"
        ]
    });

    Ok(HttpResponse::Ok().json(explanation))
}

pub async fn troubleshoot(
    req: web::Json<TroubleshootRequest>,
    config: web::Data<crate::config::Config>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    // In a real implementation, we would analyze the issue and call an AI API
    // For now, simulate a response

    let troubleshooting = serde_json::json!({
        "issue_summary": req.issue,
        "diagnosis": [
            {
                "probability": "high",
                "cause": "Connection timeout between service A and database",
                "evidence": "Error logs show repeated timeout exceptions and the metrics indicate increased latency"
            },
            {
                "probability": "medium",
                "cause": "Resource constraint in database server",
                "evidence": "CPU usage spiked to 95% before the failures started"
            }
        ],
        "recommended_actions": [
            {
                "priority": "high",
                "action": "Check database connection pool settings and increase timeout",
                "details": "Current timeout appears to be 5s which may be insufficient during peak load"
            },
            {
                "priority": "medium",
                "action": "Scale up database resources",
                "details": "Current CPU utilization is consistently above 80% during peak hours"
            },
            {
                "priority": "medium",
                "action": "Review recent queries for performance issues",
                "details": "Look for long-running queries that might be blocking others"
            }
        ],
        "queries_to_run": [
            "SHOW PROCESSLIST;",
            "SELECT * FROM performance_schema.events_statements_summary_by_digest ORDER BY sum_timer_wait DESC LIMIT 10;"
        ]
    });

    Ok(HttpResponse::Ok().json(troubleshooting))
}

pub async fn analyze_rds_instance(
    path: web::Path<(String, String)>,
    claims: web::ReqData<Claims>,
    config: web::Data<crate::config::Config>,
    llm_integration_service: web::Data<Arc<crate::services::llm::LlmIntegrationService>>,
    llm_provider_repo: web::Data<Arc<crate::repositories::llm_provider::LlmProviderRepository>>,
) -> Result<impl Responder, AppError> {
    let (instance_id, workflow) = path.into_inner();

    info!(
        "Analyzing RDS instance {} with workflow {}",
        instance_id, workflow
    );

    // Get the default LLM provider
    let model_name = config.ai.model.clone();
    let provider = llm_provider_repo
        .find_by_model_name(&model_name)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("LLM provider for model '{}' not found", model_name))
        })?;

    // Create real LLM analysis based on workflow
    let (prompt, related_questions) = match workflow.as_str() {
        "memory-usage" => (
            format!("Analyze the memory usage patterns for RDS instance '{}'. Provide detailed insights on current memory utilization, identify potential memory bottlenecks, suggest optimization strategies, and recommend proper memory configuration settings. Format the response in markdown.", instance_id),
            vec![
                "How can I optimize my memory configuration?".to_string(),
                "Is my current memory allocation sufficient?".to_string(),
                "What are the peak memory usage patterns?".to_string(),
            ]
        ),
        "cpu-usage" => (
            format!("Analyze the CPU usage patterns for RDS instance '{}'. Examine CPU utilization trends, identify resource-intensive queries, suggest CPU optimization strategies, and provide recommendations for scaling CPU resources. Format the response in markdown.", instance_id),
            vec![
                "Which queries are consuming the most CPU?".to_string(),
                "How to scale my CPU resources efficiently?".to_string(),
                "When do CPU spikes occur most frequently?".to_string(),
            ]
        ),
        "disk-usage" => (
            format!("Analyze the disk usage and I/O patterns for RDS instance '{}'. Review storage utilization, identify I/O bottlenecks, suggest storage optimizations, and recommend appropriate storage configurations. Format the response in markdown.", instance_id),
            vec![
                "What objects are taking up the most space?".to_string(),
                "How can I improve IO performance?".to_string(),
                "Should I consider storage autoscaling?".to_string(),
            ]
        ),
        "performance" => (
            format!("Perform a comprehensive performance analysis for RDS instance '{}'. Analyze overall database performance metrics, identify performance bottlenecks, review configuration settings against best practices, and provide actionable optimization recommendations. Format the response in markdown.", instance_id),
            vec![
                "What is affecting my overall database performance?".to_string(),
                "How do my current settings compare to best practices?".to_string(),
                "What optimizations would provide the biggest performance gains?".to_string(),
            ]
        ),
        "slow-queries" => (
            format!("Analyze slow query patterns for RDS instance '{}'. Identify the slowest-performing queries, analyze execution plans, suggest query optimizations, recommend index strategies, and provide best practices for query performance. Format the response in markdown.", instance_id),
            vec![
                "How can I optimize my slowest queries?".to_string(),
                "What indexes should I add to improve performance?".to_string(),
                "Are there query patterns that should be redesigned?".to_string(),
            ]
        ),
        _ => return Err(AppError::BadRequest(format!("Unknown workflow: {}", workflow))),
    };

    // Make real LLM call
    let llm_request = crate::services::llm::LlmRequest {
        prompt,
        system_prompt: Some("You are an expert AWS RDS database performance analyst. Provide detailed, actionable analysis and recommendations.".to_string()),
        temperature: Some(0.7),
        max_tokens: Some(2000),
        variables: None,
    };

    let llm_response = llm_integration_service
        .generate_response(provider.id, llm_request)
        .await?;

    let response = RdsAnalysisResponse {
        format: "markdown".to_string(),
        content: llm_response.content,
        related_questions,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub async fn answer_rds_question(
    req: web::Json<RelatedQuestionRequest>,
    _claims: web::ReqData<Claims>,
    config: web::Data<crate::config::Config>,
    llm_integration_service: web::Data<Arc<crate::services::llm::LlmIntegrationService>>,
    llm_provider_repo: web::Data<Arc<crate::repositories::llm_provider::LlmProviderRepository>>,
) -> Result<impl Responder, AppError> {
    info!(
        "Answering question about RDS instance {}: {}",
        req.instance_id, req.question
    );

    // Get the default LLM provider
    let model_name = config.ai.model.clone();
    let provider = llm_provider_repo
        .find_by_model_name(&model_name)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("LLM provider for model '{}' not found", model_name))
        })?;

    let prompt = format!(
        "Answer this specific question about RDS instance '{}': {}\n\nProvide a detailed, actionable response based on AWS RDS best practices and performance optimization principles. Format the response in markdown.",
        req.instance_id, req.question
    );

    // Make real LLM call
    let llm_request = crate::services::llm::LlmRequest {
        prompt,
        system_prompt: Some("You are an expert AWS RDS database administrator. Answer user questions with detailed, practical advice.".to_string()),
        temperature: Some(0.7),
        max_tokens: Some(1500),
        variables: None,
    };

    let llm_response = llm_integration_service
        .generate_response(provider.id, llm_request)
        .await?;

    let response = RdsAnalysisResponse {
        format: "markdown".to_string(),
        content: llm_response.content,
        related_questions: vec![
            "How does this compare to other similar workloads?".to_string(),
            "What metrics should I monitor after applying these changes?".to_string(),
            "How can I automate this optimization process?".to_string(),
        ],
    };

    Ok(HttpResponse::Ok().json(response))
}

// New endpoint for analyzing DynamoDB tables
pub async fn analyze_dynamodb_table(
    path: web::Path<(String, String)>,
    _claims: web::ReqData<Claims>,
    config: web::Data<crate::config::Config>,
    llm_integration_service: web::Data<Arc<crate::services::llm::LlmIntegrationService>>,
    llm_provider_repo: web::Data<Arc<crate::repositories::llm_provider::LlmProviderRepository>>,
) -> Result<impl Responder, AppError> {
    let (table_id, workflow) = path.into_inner();

    info!(
        "Analyzing DynamoDB table {} with workflow {}",
        table_id, workflow
    );

    // Get the default LLM provider
    let model_name = config.ai.model.clone();
    let provider = llm_provider_repo
        .find_by_model_name(&model_name)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("LLM provider for model '{}' not found", model_name))
        })?;

    // Create real LLM analysis based on workflow
    let (prompt, related_questions) = match workflow.as_str() {
        "provisioned-capacity" => (
            format!("Analyze the provisioned capacity configuration for DynamoDB table '{}'. Review current read/write capacity units, examine utilization patterns, identify over or under-provisioning, suggest optimal capacity settings, and compare with on-demand pricing. Format the response in markdown.", table_id),
            vec![
                "How does my current capacity compare to usage?".to_string(),
                "When do I experience throttling?".to_string(),
                "Should I consider on-demand pricing?".to_string(),
            ]
        ),
        "read-patterns" => (
            format!("Analyze the read access patterns for DynamoDB table '{}'. Examine query and scan operations, identify inefficient read patterns, analyze partition key distribution, suggest query optimizations, and recommend best practices for read performance. Format the response in markdown.", table_id),
            vec![
                "Which queries are most expensive?".to_string(),
                "Are my partition keys distributed well?".to_string(),
                "How can I optimize my scan operations?".to_string(),
            ]
        ),
        "write-patterns" => (
            format!("Analyze the write access patterns for DynamoDB table '{}'. Review write operations, identify hot partitions, examine batch write efficiency, analyze write throttling patterns, and suggest optimizations for write performance. Format the response in markdown.", table_id),
            vec![
                "What are my peak write periods?".to_string(),
                "Am I having hot partition issues?".to_string(),
                "How can I optimize batch writes?".to_string(),
            ]
        ),
        "hotspots-analysis" => (
            format!("Analyze partition hotspots and access patterns for DynamoDB table '{}'. Identify hot partitions, examine uneven data distribution, analyze access pattern irregularities, suggest partition key strategy improvements, and recommend solutions for balancing load. Format the response in markdown.", table_id),
            vec![
                "How do I reduce partition contention?".to_string(),
                "Should I revise my partition key strategy?".to_string(),
                "What's causing the uneven access patterns?".to_string(),
            ]
        ),
        "cost-optimization" => (
            format!("Perform cost optimization analysis for DynamoDB table '{}'. Analyze current pricing model, examine capacity utilization vs cost, suggest cost-saving strategies, recommend optimal capacity mode, and provide insights on features that can reduce costs. Format the response in markdown.", table_id),
            vec![
                "How can I reduce my DynamoDB costs?".to_string(),
                "Am I using the right capacity mode?".to_string(),
                "Which features can save me money?".to_string(),
            ]
        ),
        _ => return Err(AppError::BadRequest(format!("Unknown workflow: {}", workflow))),
    };

    // Make real LLM call
    let llm_request = crate::services::llm::LlmRequest {
        prompt,
        system_prompt: Some("You are an expert AWS DynamoDB performance and cost optimization analyst. Provide detailed, actionable analysis and recommendations.".to_string()),
        temperature: Some(0.7),
        max_tokens: Some(2000),
        variables: None,
    };

    let llm_response = llm_integration_service
        .generate_response(provider.id, llm_request)
        .await?;

    let response = DynamoDBAnalysisResponse {
        format: "markdown".to_string(),
        content: llm_response.content,
        related_questions,
    };

    Ok(HttpResponse::Ok().json(response))
}

// Answer follow-up questions about a DynamoDB table
pub async fn answer_dynamodb_question(
    req: web::Json<RelatedQuestionRequest>,
    _claims: web::ReqData<Claims>,
    config: web::Data<crate::config::Config>,
    llm_integration_service: web::Data<Arc<crate::services::llm::LlmIntegrationService>>,
    llm_provider_repo: web::Data<Arc<crate::repositories::llm_provider::LlmProviderRepository>>,
) -> Result<impl Responder, AppError> {
    info!(
        "Answering question about DynamoDB table {}: {}",
        req.instance_id, req.question
    );

    // Get the default LLM provider
    let model_name = config.ai.model.clone();
    let provider = llm_provider_repo
        .find_by_model_name(&model_name)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("LLM provider for model '{}' not found", model_name))
        })?;

    let prompt = format!(
        "Answer this specific question about DynamoDB table '{}': {}\n\nProvide a detailed, actionable response based on AWS DynamoDB best practices, performance optimization, and cost management principles. Format the response in markdown.",
        req.instance_id, req.question
    );

    // Make real LLM call
    let llm_request = crate::services::llm::LlmRequest {
        prompt,
        system_prompt: Some("You are an expert AWS DynamoDB architect and performance analyst. Answer user questions with detailed, practical advice.".to_string()),
        temperature: Some(0.7),
        max_tokens: Some(1500),
        variables: None,
    };

    let llm_response = llm_integration_service
        .generate_response(provider.id, llm_request)
        .await?;

    let response = DynamoDBAnalysisResponse {
        format: "markdown".to_string(),
        content: llm_response.content,
        related_questions: vec![
            "How does this compare to best practices?".to_string(),
            "What monitoring should I set up for this?".to_string(),
            "How can I automate these optimizations?".to_string(),
        ],
    };

    Ok(HttpResponse::Ok().json(response))
}

// Mock response content generators
fn get_mock_memory_analysis() -> String {
    r#"}"#.to_string()
}

fn get_mock_cpu_analysis() -> String {
    r#"# CPU Usage Analysis

## Current Status
Your RDS instance is experiencing periodic CPU spikes, with an average utilization of 45% and peaks of 92%.

## Findings
- **Top CPU Consumers**: 
  1. Query scans without proper indexes (35%)
  2. Batch processing jobs (25%)
  3. Reporting queries (20%)
- **CPU Credit Balance**: Decreasing (t3 instance types only)
- **CPU Throttling Events**: 2 in the past week

## Recommendations
1. **Query Optimization**: Add indexes for frequently run queries on tables: users, orders, products
2. **Workload Distribution**: Schedule batch jobs during off-peak hours (currently running at 2PM daily)
3. **Consider Upgrading Instance**: If consistently hitting >80% CPU, upgrade from t3 to m5 instance class

## Monitoring Improvements
Set up alerts for sustained CPU usage above 80% for more than 10 minutes."#.to_string()
}

fn get_mock_disk_analysis() -> String {
    r#"# Disk Usage Analysis

## Current Status
Storage usage is at 68% of allocated capacity with a growth rate of approximately 2GB per week.

## Performance Metrics
- **Read IOPS**: Avg 120, Peak 550
- **Write IOPS**: Avg 85, Peak 320
- **Read Latency**: Avg 1.2ms
- **Write Latency**: Avg 2.8ms

## Top Storage Consumers
1. Table 'order_history': 35% (142GB)
2. Table 'product_catalog': 22% (89GB)
3. Table 'user_activity_logs': 18% (73GB)

## Recommendations
1. **Archiving Strategy**: Implement data archiving for orders older than 12 months
2. **Storage Optimization**: Consider partitioning the 'user_activity_logs' table by date
3. **IOPS Management**: Current provisioned IOPS are sufficient, no changes needed
4. **Monitoring**: Enable storage auto-scaling to prevent reaching capacity limits

## Projected Growth
At current growth rate, you will reach 85% capacity in approximately 14 weeks."#
        .to_string()
}

fn get_mock_performance_analysis() -> String {
    r#"# Comprehensive Performance Analysis

## Overall Health Score: 72/100

## Key Performance Indicators
- **Response Time**: Avg 45ms (Acceptable)
- **Throughput**: 1,250 queries/second (Good)
- **Connection Utilization**: 65% of maximum (Healthy)
- **Cache Hit Ratio**: 78% (Could be improved)
- **Deadlocks**: 3 per day (Higher than recommended)

## Critical Issues
1. **Connection Spikes**: Experiencing periodic connection spikes that consume available connections
2. **Inefficient Queries**: 5 queries identified with full table scans on large tables
3. **Lock Contention**: Moderate contention on the 'orders' table during peak hours

## Recommendations
1. **Connection Pooling**: Implement or tune connection pooling in your application
2. **Index Optimization**: Create composite indexes on (customer_id, order_date) for the orders table
3. **Query Rewriting**: Rewrite the report queries to use CTEs instead of nested subqueries
4. **Parameter Tuning**: Adjust max_connections to 200 (currently 100)
5. **Monitoring**: Set up alerts for connection spikes and deadlocks

## Long-term Strategic Recommendations
Consider implementing read replicas for reporting workloads to offload the primary instance."#.to_string()
}

fn get_mock_slow_query_analysis() -> String {
    r#"# Slow Query Analysis

## Top 5 Slow Queries Identified

### 1. Product Search Query (Avg: 2.8s)
```sql
SELECT p.*, c.name as category_name 
FROM products p 
JOIN categories c ON p.category_id = c.id
WHERE p.name LIKE '%keyword%'
ORDER BY p.popularity DESC
```

**Issue**: Full table scan with sorting
**Recommendation**: Add full-text search index on products.name

### 2. Order History Query (Avg: 1.9s)
```sql
SELECT o.*, oi.product_id, oi.quantity
FROM orders o
JOIN order_items oi ON o.id = oi.order_id
WHERE o.customer_id = ?
AND o.order_date BETWEEN ? AND ?
```

**Issue**: Missing index on order_date
**Recommendation**: Add composite index on (customer_id, order_date)

### 3. Dashboard Analytics Query (Avg: 3.5s)
```sql
SELECT 
  DATE(created_at) as date,
  COUNT(*) as order_count,
  SUM(total_amount) as revenue
FROM orders
GROUP BY DATE(created_at)
ORDER BY date DESC
LIMIT 30
```

**Issue**: Function on indexed column prevents index usage
**Recommendation**: Create a computed column or materialized view

## Overall Recommendations
1. Implement query caching for repeated analytical queries
2. Review application ORM settings to prevent N+1 query patterns
3. Consider creating read replicas for reporting and analytics workloads
4. Schedule regular EXPLAIN ANALYZE on critical queries"#
        .to_string()
}

fn get_mock_dynamodb_capacity_analysis() -> String {
    r#"# Provisioned Capacity Analysis

## Current Status
Your DynamoDB table is using provisioned capacity mode with:
- Read Capacity: 50 RCUs 
- Write Capacity: 25 WCUs
- AutoScaling: Enabled

## Utilization Patterns
- **Read Usage**: Averaging 35 RCUs (70%)
  - Peak: 45 RCUs at 14:00 UTC
  - Low: 12 RCUs at 02:00 UTC
- **Write Usage**: Averaging 15 WCUs (60%)
  - Peak: 22 WCUs at 15:30 UTC
  - Low: 5 WCUs at 03:00 UTC

## Throttling Events
- Read Throttling: None
- Write Throttling: 2 events in last 24h

## Recommendations
1. **Optimize AutoScaling**
   - Increase minimum RCUs to 40 during business hours
   - Adjust scale-out cooldown period to 60s
2. **Consider On-Demand**
   - Your utilization patterns suggest on-demand might be more cost-effective
   - Estimated cost savings: 15%

## Monitoring Improvements
Set up CloudWatch alarms for:
- Throttling events > 5 in 5 minutes
- Sustained capacity above 80% for 15 minutes"#
        .to_string()
}

fn get_mock_dynamodb_read_analysis() -> String {
    r#"# Read Pattern Analysis

## Query Patterns
- 65% Query operations
- 30% GetItem operations
- 5% Scan operations

## Top Access Patterns
1. Query by userId + timestamp (45% of reads)
2. GetItem by orderId (25% of reads)
3. Scan with filter on status (5% of reads)

## Performance Metrics
- Average Query latency: 8ms
- Average GetItem latency: 4ms
- Average Scan latency: 250ms

## Identified Issues
1. **Frequent Table Scans**
   - Weekly report query scanning entire table
   - Consider creating a GSI for this access pattern
2. **Hot Partition**
   - High read activity on recent orders
   - Consider caching frequently accessed items

## Optimization Opportunities
1. Create a GSI with status as partition key
2. Implement DynamoDB Accelerator (DAX)
3. Add TTL for old items
4. Use Parallel Scan for large datasets"#
        .to_string()
}

fn get_mock_dynamodb_write_analysis() -> String {
    r#"# Write Pattern Analysis

## Write Operation Distribution
- PutItem: 45%
- UpdateItem: 35%
- BatchWriteItem: 20%

## Peak Write Periods
1. 14:00-16:00 UTC: Order processing spike
2. 00:00-01:00 UTC: Batch updates
3. 08:00-09:00 UTC: Morning rush

## Performance Metrics
- Average PutItem latency: 10ms
- Average UpdateItem latency: 12ms
- Average BatchWriteItem latency: 45ms

## Write Efficiency
- Successful BatchWrite operations: 98%
- Average items per batch: 15
- Write capacity consumption: 65%

## Recommendations
1. **Batch Operation Optimization**
   - Increase batch size to 25 items
   - Implement exponential backoff retry
2. **Capacity Management**
   - Schedule AutoScaling for known peak periods
   - Consider reserved capacity"#
        .to_string()
}

fn get_mock_dynamodb_hotspots_analysis() -> String {
    r#"# Partition Hotspot Analysis

## Current Hotspots
1. **Recent Orders Partition**
   - Partition key: current_date
   - 45% of all operations
   - Experiencing throttling during peak

2. **Active Users Partition**
   - Partition key: status=ACTIVE
   - 30% of read operations
   - Sub-optimal key distribution

## Impact Assessment
- Throttled requests: 150/day
- Increased latency: +40ms during peaks
- Affected operations: Mostly reads

## Root Causes
1. Temporal data clustering
2. High-cardinality partition keys
3. Uneven workload distribution

## Recommendations
1. **Partition Key Strategy**
   - Add random suffix to high-volume dates
   - Implement key randomization for active users
2. **Data Access Patterns**
   - Cache hot items in DAX
   - Implement write sharding
3. **Monitoring**
   - Track PartitionCount metric
   - Alert on throttling events"#
        .to_string()
}

fn get_mock_dynamodb_cost_analysis() -> String {
    r#"# Cost Optimization Analysis

## Current Costs
- Provisioned Capacity: $250/month
- Data Storage: $75/month
- Data Transfer: $45/month
- Reserved Capacity: None

## Usage Efficiency
- Read Capacity: 62% utilized
- Write Capacity: 58% utilized
- Storage Growth: 15% monthly

## Cost-Saving Opportunities

### 1. Capacity Mode
Switching to on-demand pricing could save 20%
- Current cost: $250/month
- Projected on-demand: $200/month
- Risk: Spikes could increase costs

### 2. Data Lifecycle
Implement TTL for old data
- Potential storage savings: 35%
- Reduced backup costs
- Improved query performance

### 3. Reserved Capacity
1-year commitment recommendations:
- 40 RCUs: $180/month
- 20 WCUs: $90/month
- Total savings: ~30%

## Action Plan
1. Enable TTL immediately
2. Test on-demand pricing for 2 weeks
3. Review reserved capacity options
4. Implement DAX for frequent queries"#
        .to_string()
}
