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
use crate::models::aws_account::AwsAccountDto;
use crate::models::aws_resource::{AwsResourceDto, AwsResourceType, Model as AwsResourceModel};
use crate::services::aws::aws_data_plane::cloudwatch::{
    CloudWatchMetrics, CloudWatchMetricsRequest, CloudWatchService,
};
use crate::services::aws::client_factory::AwsClientFactory;
use crate::services::AwsService;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

pub struct LambdaControlPlane {
    aws_service: Arc<AwsService>,
}

impl LambdaControlPlane {
    pub fn new(aws_service: Arc<AwsService>) -> Self {
        Self { aws_service }
    }

    // NOTE: Align with other control planes: do not persist here.
    // Return resource models and let the orchestrator persist a new row per sync.
    pub async fn sync_functions(
        &self,
        aws_account_dto: &AwsAccountDto,
        sync_id: Uuid,
    ) -> Result<Vec<AwsResourceModel>, AppError> {
        let client = self
            .aws_service
            .create_lambda_client(aws_account_dto)
            .await?;
        let cloudwatch_service = CloudWatchService::new(self.aws_service.clone());

        let mut functions: Vec<AwsResourceDto> = Vec::new();
        let mut marker = None;

        // Paginate through all Lambda functions
        loop {
            // Build list functions request
            let mut request = client.list_functions();

            // Add marker for pagination if it exists
            if let Some(marker_val) = &marker {
                request = request.marker(marker_val);
            }

            // Send request to AWS
            let response = request.send().await.map_err(|e| {
                AppError::ExternalService(format!("Failed to list Lambda functions: {}", e))
            })?;

            // Process functions in the response
            for aws_function in response.functions() {
                if let Some(function_name) = aws_function.function_name() {
                    // Extract function ARN
                    let function_arn = aws_function.function_arn().unwrap_or("");

                    // Get function tags
                    let tags_response = client
                        .list_tags()
                        .resource(function_arn)
                        .send()
                        .await
                        .map_err(|e| {
                            AppError::ExternalService(format!(
                                "Failed to get tags for Lambda function {}: {}",
                                function_name, e
                            ))
                        })?;

                    // Process tags
                    let mut tags_map = serde_json::Map::new();
                    let mut name = None;

                    if let Some(tags) = tags_response.tags() {
                        for (key, value) in tags {
                            if key == "Name" {
                                name = Some(value.to_string());
                            }
                            tags_map.insert(key.to_string(), json!(value));
                        }
                    }

                    // If no name tag was found, use the function name
                    if name.is_none() {
                        name = Some(function_name.to_string());
                    }

                    // Build function data
                    let mut function_data = serde_json::Map::new();

                    function_data.insert("function_name".to_string(), json!(function_name));
                    function_data.insert("function_arn".to_string(), json!(function_arn));

                    if let Some(runtime) = aws_function.runtime().map(|r| r.as_str()) {
                        function_data.insert("runtime".to_string(), json!(runtime));
                    }

                    if let Some(role) = aws_function.role() {
                        function_data.insert("role".to_string(), json!(role));
                    }

                    if let Some(handler) = aws_function.handler() {
                        function_data.insert("handler".to_string(), json!(handler));
                    }

                    // code_size is not an Option
                    function_data.insert("code_size".to_string(), json!(aws_function.code_size()));

                    if let Some(description) = aws_function.description() {
                        function_data.insert("description".to_string(), json!(description));
                    }

                    if let Some(timeout) = aws_function.timeout() {
                        function_data.insert("timeout".to_string(), json!(timeout));
                    }

                    if let Some(memory_size) = aws_function.memory_size() {
                        function_data.insert("memory_size".to_string(), json!(memory_size));
                    }

                    if let Some(last_modified) = aws_function.last_modified() {
                        function_data.insert("last_modified".to_string(), json!(last_modified));
                    }

                    if let Some(code_sha) = aws_function.code_sha256() {
                        function_data.insert("code_sha256".to_string(), json!(code_sha));
                    }

                    // Handle environment variables
                    if let Some(env) = aws_function.environment() {
                        if let Some(vars) = env.variables() {
                            function_data.insert(
                                "environment".to_string(),
                                json!({
                                    "variables": vars
                                }),
                            );
                        }
                    }

                    if let Some(version) = aws_function.version() {
                        function_data.insert("version".to_string(), json!(version));
                    }

                    let architectures = aws_function.architectures();
                    if !architectures.is_empty() {
                        let arch_list: Vec<String> = architectures
                            .iter()
                            .map(|a| a.as_str().to_string())
                            .collect();

                        function_data.insert("architectures".to_string(), json!(arch_list));
                    }

                    self.attach_cloudwatch_telemetry(
                        &cloudwatch_service,
                        aws_account_dto,
                        function_name,
                        &mut function_data,
                    )
                    .await;

                    // Create resource DTO
                    let function = AwsResourceDto {
                        id: None,
                        sync_id: Some(sync_id),
                        account_id: aws_account_dto.account_id.clone(),
                        profile: aws_account_dto.profile.clone(),
                        region: aws_account_dto.default_region.clone(),
                        resource_type: AwsResourceType::LambdaFunction.to_string(),
                        resource_id: function_name.to_string(),
                        arn: function_arn.to_string(),
                        name,
                        tags: serde_json::Value::Object(tags_map),
                        resource_data: serde_json::Value::Object(function_data),
                    };

                    functions.push(function);
                }
            }

            // Check if there are more functions to fetch
            marker = response.next_marker().map(|s| s.to_string());
            if marker.is_none() {
                break;
            }
        }
        // Convert DTOs into Models for uniform handling by orchestrator
        Ok(functions.into_iter().map(|f| f.into()).collect())
    }

    async fn attach_cloudwatch_telemetry(
        &self,
        cloudwatch_service: &CloudWatchService,
        aws_account_dto: &AwsAccountDto,
        function_name: &str,
        resource_data: &mut serde_json::Map<String, Value>,
    ) {
        let collection_started_at = Utc::now();
        let telemetry_result = self
            .collect_cloudwatch_metric_sample(cloudwatch_service, aws_account_dto, function_name)
            .await;
        let collection_completed_at = Utc::now();
        let duration_ms = (collection_completed_at - collection_started_at).num_milliseconds();

        resource_data.insert(
            "telemetry_collection_started_at".to_string(),
            json!(collection_started_at.to_rfc3339()),
        );
        resource_data.insert(
            "telemetry_collection_completed_at".to_string(),
            json!(collection_completed_at.to_rfc3339()),
        );
        resource_data.insert(
            "telemetry_collection_duration_ms".to_string(),
            json!(duration_ms.max(0)),
        );

        match telemetry_result {
            Ok(cloudwatch_metrics) => {
                let metric_names: Vec<String> = cloudwatch_metrics
                    .get("metrics")
                    .and_then(|metrics| metrics.as_array())
                    .map(|metrics| {
                        metrics
                            .iter()
                            .filter_map(|metric| {
                                metric
                                    .get("metric_name")
                                    .and_then(|name| name.as_str())
                                    .map(|name| name.to_string())
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let metric_count = metric_names.len();

                resource_data.insert("cloudwatch_metrics".to_string(), cloudwatch_metrics);
                resource_data.insert("cloudwatch_metric_names".to_string(), json!(metric_names));
                resource_data.insert("cloudwatch_metric_count".to_string(), json!(metric_count));
                resource_data.insert(
                    "lambda_invocation_metric_observed".to_string(),
                    json!(has_metric(resource_data, "Invocations")),
                );
                resource_data.insert(
                    "lambda_duration_metric_observed".to_string(),
                    json!(has_metric(resource_data, "Duration")),
                );
                resource_data.insert(
                    "lambda_error_metric_observed".to_string(),
                    json!(has_metric(resource_data, "Errors")),
                );
                resource_data.insert(
                    "lambda_throttle_metric_observed".to_string(),
                    json!(has_metric(resource_data, "Throttles")),
                );
                resource_data.insert("telemetry_collection_success_count".to_string(), json!(1));
                resource_data.insert("telemetry_collection_failure_count".to_string(), json!(0));
                resource_data.insert("telemetry_collection_error_count".to_string(), json!(0));
                resource_data.insert("telemetry_collection_errors".to_string(), json!([]));
            }
            Err(error) => {
                resource_data.insert("cloudwatch_metrics".to_string(), json!({ "metrics": [] }));
                resource_data.insert("cloudwatch_metric_names".to_string(), json!([]));
                resource_data.insert("cloudwatch_metric_count".to_string(), json!(0));
                resource_data.insert(
                    "lambda_invocation_metric_observed".to_string(),
                    json!(false),
                );
                resource_data.insert("lambda_duration_metric_observed".to_string(), json!(false));
                resource_data.insert("lambda_error_metric_observed".to_string(), json!(false));
                resource_data.insert("lambda_throttle_metric_observed".to_string(), json!(false));
                resource_data.insert("telemetry_collection_success_count".to_string(), json!(0));
                resource_data.insert("telemetry_collection_failure_count".to_string(), json!(1));
                resource_data.insert("telemetry_collection_error_count".to_string(), json!(1));
                resource_data.insert(
                    "telemetry_collection_errors".to_string(),
                    json!([{
                        "source": "cloudwatch",
                        "operation": "GetMetricData",
                        "error": error.to_string(),
                    }]),
                );
            }
        }
    }

    async fn collect_cloudwatch_metric_sample(
        &self,
        cloudwatch_service: &CloudWatchService,
        aws_account_dto: &AwsAccountDto,
        function_name: &str,
    ) -> Result<Value, AppError> {
        let end_time = Utc::now();
        let start_time = end_time - Duration::hours(3);
        let metrics = lambda_cost_metric_names()
            .iter()
            .map(|metric| metric.to_string())
            .collect::<Vec<_>>();
        let request = CloudWatchMetricsRequest {
            resource_type: AwsResourceType::LambdaFunction.to_string(),
            resource_id: function_name.to_string(),
            region: aws_account_dto.default_region.clone(),
            metrics: metrics.clone(),
            start_time,
            end_time,
            period: 300,
        };

        let result = cloudwatch_service
            .get_metrics(aws_account_dto, &request)
            .await?;
        let metric_samples = result
            .metrics
            .into_iter()
            .map(|metric| {
                json!({
                    "namespace": metric.namespace,
                    "metric_name": metric.metric_name,
                    "unit": metric.unit,
                    "datapoints": metric
                        .datapoints
                        .into_iter()
                        .map(|datapoint| {
                            json!({
                                "timestamp": datapoint.timestamp.to_rfc3339(),
                                "value": datapoint.value,
                                "unit": datapoint.unit,
                            })
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();

        Ok(json!({
            "source": "CloudWatch",
            "namespace": "AWS/Lambda",
            "resource_id": function_name,
            "dimension_name": "FunctionName",
            "lookback_hours": 3,
            "period_seconds": 300,
            "requested_metrics": metrics,
            "metrics": metric_samples,
            "collected_at": end_time.to_rfc3339(),
        }))
    }
}

fn lambda_cost_metric_names() -> [&'static str; 4] {
    ["Invocations", "Duration", "Errors", "Throttles"]
}

fn has_metric(resource_data: &serde_json::Map<String, Value>, metric_name: &str) -> bool {
    resource_data
        .get("cloudwatch_metrics")
        .and_then(|cloudwatch| cloudwatch.get("metrics"))
        .and_then(|metrics| metrics.as_array())
        .map(|metrics| {
            metrics.iter().any(|metric| {
                metric
                    .get("metric_name")
                    .or_else(|| metric.get("MetricName"))
                    .and_then(|name| name.as_str())
                    .map(|name| name == metric_name)
                    .unwrap_or(false)
                    && metric
                        .get("datapoints")
                        .or_else(|| metric.get("Datapoints"))
                        .and_then(|datapoints| datapoints.as_array())
                        .map(|datapoints| !datapoints.is_empty())
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}
