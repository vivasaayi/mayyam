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


use aws_sdk_cloudwatch::{types::Dimension, Client as CloudWatchClient};
use chrono::{DateTime, Utc};
use reqwest::Client as HttpClient;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use crate::errors::AppError;
use crate::models::data_source::{Model as DataSourceModel, ResourceType, SourceType};
use crate::repositories::data_source::DataSourceRepository;
use crate::utils::time_conversion::{from_aws_datetime, to_aws_datetime};

#[derive(Debug, Clone)]
pub struct MetricData {
    pub resource_id: String,
    pub metric_name: String,
    pub value: f64,
    pub unit: String,
    pub timestamp: DateTime<Utc>,
    pub dimensions: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct DataCollectionRequest {
    pub data_source_id: uuid::Uuid,
    pub resource_ids: Vec<String>,
    pub metric_names: Vec<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub period: Option<i32>, // in seconds
}

#[derive(Debug, Clone)]
pub struct DataCollectionResponse {
    pub metrics: Vec<MetricData>,
    pub metadata: HashMap<String, Value>,
    pub timestamp: DateTime<Utc>,
}

pub struct DataCollectionService {
    data_source_repo: Arc<DataSourceRepository>,
    cloudwatch_client: Option<CloudWatchClient>,
    http_client: HttpClient,
    config: Config,
}

impl DataCollectionService {
    pub fn new(
        data_source_repo: Arc<DataSourceRepository>,
        cloudwatch_client: Option<CloudWatchClient>,
        config: Config,
    ) -> Self {
        Self {
            data_source_repo,
            cloudwatch_client,
            http_client: HttpClient::new(),
            config,
        }
    }

    pub async fn collect_data(
        &self,
        request: DataCollectionRequest,
    ) -> Result<DataCollectionResponse, AppError> {
        let data_source = self
            .data_source_repo
            .find_by_id(request.data_source_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Data source not found".to_string()))?;

        let source_type = self
            .get_source_type_enum(&data_source)
            .ok_or_else(|| AppError::InternalServerError("Invalid source_type".to_string()))?;

        match source_type {
            SourceType::CloudWatch => self.collect_cloudwatch_data(&data_source, &request).await,
            SourceType::Dynatrace => self.collect_dynatrace_data(&data_source, &request).await,
            SourceType::Splunk => self.collect_splunk_data(&data_source, &request).await,
            SourceType::Prometheus => self.collect_prometheus_data(&data_source, &request).await,
            SourceType::Custom => self.collect_custom_data(&data_source, &request).await,
        }
    }

    pub async fn get_available_metrics(
        &self,
        data_source_id: uuid::Uuid,
        resource_type: ResourceType,
    ) -> Result<Vec<String>, AppError> {
        let data_source = self
            .data_source_repo
            .find_by_id(data_source_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Data source not found".to_string()))?;

        let source_type = self
            .get_source_type_enum(&data_source)
            .ok_or_else(|| AppError::InternalServerError("Invalid source_type".to_string()))?;

        match (source_type, resource_type) {
            (SourceType::CloudWatch, ResourceType::DynamoDB) => Ok(vec![
                "ConsumedReadCapacityUnits".to_string(),
                "ConsumedWriteCapacityUnits".to_string(),
                "ProvisionedReadCapacityUnits".to_string(),
                "ProvisionedWriteCapacityUnits".to_string(),
                "ReadThrottleEvents".to_string(),
                "WriteThrottleEvents".to_string(),
                "SystemErrors".to_string(),
                "UserErrors".to_string(),
                "SuccessfulRequestLatency".to_string(),
            ]),
            (SourceType::CloudWatch, ResourceType::RDS) => Ok(vec![
                "CPUUtilization".to_string(),
                "DatabaseConnections".to_string(),
                "FreeableMemory".to_string(),
                "FreeStorageSpace".to_string(),
                "ReadIOPS".to_string(),
                "WriteIOPS".to_string(),
                "ReadLatency".to_string(),
                "WriteLatency".to_string(),
            ]),
            (SourceType::CloudWatch, ResourceType::Kubernetes) => Ok(vec![
                "cluster_node_count".to_string(),
                "cluster_pod_count".to_string(),
                "node_cpu_utilization".to_string(),
                "node_memory_utilization".to_string(),
                "pod_cpu_utilization".to_string(),
                "pod_memory_utilization".to_string(),
            ]),
            _ => {
                if let Some(metric_config) = &data_source.metric_config {
                    if let serde_json::Value::Object(config) = metric_config {
                        if let Some(serde_json::Value::Array(metrics)) =
                            config.get("available_metrics")
                        {
                            let metric_names: Vec<String> = metrics
                                .iter()
                                .filter_map(|m| m.as_str().map(|s| s.to_string()))
                                .collect();
                            return Ok(metric_names);
                        }
                    }
                }
                Ok(vec![])
            }
        }
    }

    async fn collect_cloudwatch_data(
        &self,
        data_source: &DataSourceModel,
        request: &DataCollectionRequest,
    ) -> Result<DataCollectionResponse, AppError> {
        let cloudwatch = self.cloudwatch_client.as_ref().ok_or_else(|| {
            AppError::InternalServerError("CloudWatch client not configured".to_string())
        })?;

        let mut all_metrics = Vec::new();
        let mut metadata = HashMap::new();

        let connection_config = &data_source.connection_config;
        // Use enum conversion for resource_type
        let resource_type_enum = self
            .get_resource_type_enum(data_source)
            .ok_or_else(|| AppError::InternalServerError("Invalid resource_type".to_string()))?;
        let namespace = connection_config
            .get("namespace")
            .and_then(|v| v.as_str())
            .unwrap_or(self.get_default_namespace(&resource_type_enum));

        for resource_id in &request.resource_ids {
            for metric_name in &request.metric_names {
                let dimensions = self.build_cloudwatch_dimensions(&resource_type_enum, resource_id);
                let result = cloudwatch
                    .get_metric_statistics()
                    .namespace(namespace)
                    .metric_name(metric_name)
                    .set_dimensions(Some(dimensions))
                    .start_time(to_aws_datetime(&request.start_time))
                    .end_time(to_aws_datetime(&request.end_time))
                    .period(request.period.unwrap_or(300))
                    .set_statistics(Some(vec![
                        aws_sdk_cloudwatch::types::Statistic::Average,
                        aws_sdk_cloudwatch::types::Statistic::Maximum,
                        aws_sdk_cloudwatch::types::Statistic::Sum,
                    ]))
                    .send()
                    .await
                    .map_err(|e| {
                        AppError::ExternalServiceError(format!("CloudWatch API error: {}", e))
                    })?;

                if let Some(datapoints) = result.datapoints {
                    for datapoint in datapoints {
                        if let (Some(timestamp), Some(average)) =
                            (datapoint.timestamp, datapoint.average)
                        {
                            let mut dimensions_map = HashMap::new();
                            dimensions_map.insert("ResourceId".to_string(), resource_id.clone());
                            all_metrics.push(MetricData {
                                resource_id: resource_id.clone(),
                                metric_name: metric_name.clone(),
                                value: average,
                                unit: datapoint
                                    .unit
                                    .map(|u| u.as_str().to_string())
                                    .unwrap_or_default(),
                                timestamp: from_aws_datetime(&timestamp),
                                dimensions: dimensions_map,
                            });
                        }
                    }
                }
            }
        }
        metadata.insert("source".to_string(), json!("CloudWatch"));
        metadata.insert("namespace".to_string(), json!(namespace));
        metadata.insert("period".to_string(), json!(request.period.unwrap_or(300)));
        Ok(DataCollectionResponse {
            metrics: all_metrics,
            metadata,
            timestamp: Utc::now(),
        })
    }

    async fn collect_dynatrace_data(
        &self,
        data_source: &DataSourceModel,
        request: &DataCollectionRequest,
    ) -> Result<DataCollectionResponse, AppError> {
        let connection_config = &data_source.connection_config;
        let api_token = connection_config
            .get("api_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::BadRequest("Dynatrace API token not configured".to_string())
            })?;

        let base_url = connection_config
            .get("base_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::BadRequest("Dynatrace base URL not configured".to_string()))?;

        let mut all_metrics = Vec::new();
        let mut metadata = HashMap::new();

        // Dynatrace API v2 metrics query
        let metrics_selector = request.metric_names.join(",");
        let resource_type_enum = self
            .get_resource_type_enum(data_source)
            .ok_or_else(|| AppError::InternalServerError("Invalid resource_type".to_string()))?;
        let entity_selector = format!(
            "type({})",
            self.get_dynatrace_entity_type(&resource_type_enum)
        );

        let url = format!("{}/api/v2/metrics/query", base_url);
        let params = [
            ("metricSelector", metrics_selector.as_str()),
            ("entitySelector", entity_selector.as_str()),
            ("from", &request.start_time.timestamp_millis().to_string()),
            ("to", &request.end_time.timestamp_millis().to_string()),
        ];

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Api-Token {}", api_token))
            .query(&params)
            .send()
            .await
            .map_err(|e| AppError::ExternalServiceError(format!("Dynatrace API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalServiceError(format!(
                "Dynatrace API error: {}",
                error_text
            )));
        }

        let response_data: Value = response.json().await.map_err(|e| {
            AppError::ExternalServiceError(format!("Failed to parse Dynatrace response: {}", e))
        })?;

        // Parse Dynatrace response and convert to MetricData
        if let Some(result) = response_data.get("result") {
            if let Value::Array(results) = result {
                for result_item in results {
                    if let Some(data) = result_item.get("data") {
                        if let Value::Array(data_points) = data {
                            for _ in data_points {
                                // Parse Dynatrace data point format
                                // This is a simplified implementation
                                all_metrics.push(MetricData {
                                    resource_id: "dynatrace_entity".to_string(),
                                    metric_name: "sample_metric".to_string(),
                                    value: 0.0,
                                    unit: "count".to_string(),
                                    timestamp: Utc::now(),
                                    dimensions: HashMap::new(),
                                });
                            }
                        }
                    }
                }
            }
        }

        metadata.insert("source".to_string(), json!("Dynatrace"));
        Ok(DataCollectionResponse {
            metrics: all_metrics,
            metadata,
            timestamp: Utc::now(),
        })
    }

    async fn collect_splunk_data(
        &self,
        data_source: &DataSourceModel,
        _request: &DataCollectionRequest,
    ) -> Result<DataCollectionResponse, AppError> {
        let connection_config = &data_source.connection_config;
        connection_config
            .get("base_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::BadRequest("Splunk base URL not configured".to_string()))?;
        connection_config
            .get("username")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::BadRequest("Splunk username not configured".to_string()))?;
        connection_config
            .get("password")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::BadRequest("Splunk password not configured".to_string()))?;
        // Placeholder implementation for Splunk data collection
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), json!("Splunk"));
        Ok(DataCollectionResponse {
            metrics: vec![],
            metadata,
            timestamp: Utc::now(),
        })
    }

    async fn collect_prometheus_data(
        &self,
        data_source: &DataSourceModel,
        request: &DataCollectionRequest,
    ) -> Result<DataCollectionResponse, AppError> {
        let connection_config = &data_source.connection_config;
        let base_url = connection_config
            .get("base_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::BadRequest("Prometheus base URL not configured".to_string())
            })?;

        let mut all_metrics = Vec::new();
        let mut metadata = HashMap::new();

        for metric_name in &request.metric_names {
            let url = format!("{}/api/v1/query_range", base_url);
            let params = [
                ("query", metric_name.as_str()),
                ("start", &request.start_time.timestamp().to_string()),
                ("end", &request.end_time.timestamp().to_string()),
                ("step", &request.period.unwrap_or(300).to_string()),
            ];

            let response = self
                .http_client
                .get(&url)
                .query(&params)
                .send()
                .await
                .map_err(|e| {
                    AppError::ExternalServiceError(format!("Prometheus API error: {}", e))
                })?;

            if !response.status().is_success() {
                let error_text = response.text().await.unwrap_or_default();
                return Err(AppError::ExternalServiceError(format!(
                    "Prometheus API error: {}",
                    error_text
                )));
            }

            let response_data: Value = response.json().await.map_err(|e| {
                AppError::ExternalServiceError(format!(
                    "Failed to parse Prometheus response: {}",
                    e
                ))
            })?;

            // Parse Prometheus response format
            if let Some(data) = response_data.get("data") {
                if let Some(result) = data.get("result") {
                    if let Value::Array(results) = result {
                        for result_item in results {
                            if let Some(values) = result_item.get("values") {
                                if let Value::Array(value_pairs) = values {
                                    for value_pair in value_pairs {
                                        if let Value::Array(pair) = value_pair {
                                            if pair.len() >= 2 {
                                                if let (Some(timestamp), Some(value_str)) =
                                                    (pair[0].as_f64(), pair[1].as_str())
                                                {
                                                    if let Ok(value) = value_str.parse::<f64>() {
                                                        all_metrics.push(MetricData {
                                                            resource_id: "prometheus_metric"
                                                                .to_string(),
                                                            metric_name: metric_name.clone(),
                                                            value,
                                                            unit: "count".to_string(),
                                                            timestamp: DateTime::from_timestamp(
                                                                timestamp as i64,
                                                                0,
                                                            )
                                                            .unwrap_or(Utc::now()),
                                                            dimensions: HashMap::new(),
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        metadata.insert("source".to_string(), json!("Prometheus"));
        Ok(DataCollectionResponse {
            metrics: all_metrics,
            metadata,
            timestamp: Utc::now(),
        })
    }

    async fn collect_custom_data(
        &self,
        _data_source: &DataSourceModel,
        _request: &DataCollectionRequest,
    ) -> Result<DataCollectionResponse, AppError> {
        // Placeholder for custom data source implementation
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), json!("Custom"));
        Ok(DataCollectionResponse {
            metrics: vec![],
            metadata,
            timestamp: Utc::now(),
        })
    }

    fn get_default_namespace(&self, resource_type: &ResourceType) -> &'static str {
        match resource_type {
            ResourceType::DynamoDB => "AWS/DynamoDB",
            ResourceType::RDS => "AWS/RDS",
            ResourceType::Kubernetes => "AWS/EKS",
            _ => "AWS/Other",
        }
    }

    fn build_cloudwatch_dimensions(
        &self,
        resource_type: &ResourceType,
        resource_id: &str,
    ) -> Vec<Dimension> {
        match resource_type {
            ResourceType::DynamoDB => {
                vec![Dimension::builder()
                    .name("TableName")
                    .value(resource_id)
                    .build()]
            }
            ResourceType::RDS => {
                vec![Dimension::builder()
                    .name("DBInstanceIdentifier")
                    .value(resource_id)
                    .build()]
            }
            ResourceType::Kubernetes => {
                vec![Dimension::builder()
                    .name("ClusterName")
                    .value(resource_id)
                    .build()]
            }
            _ => vec![],
        }
    }

    fn get_dynatrace_entity_type(&self, resource_type: &ResourceType) -> &'static str {
        match resource_type {
            ResourceType::DynamoDB => "DYNAMO_DB_TABLE",
            ResourceType::RDS => "DATABASE",
            ResourceType::Kubernetes => "KUBERNETES_CLUSTER",
            _ => "GENERIC_RESOURCE",
        }
    }

    pub async fn test_data_source_connection(
        &self,
        data_source_id: uuid::Uuid,
    ) -> Result<bool, AppError> {
        let data_source = self
            .data_source_repo
            .find_by_id(data_source_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Data source not found".to_string()))?;

        let source_type = self
            .get_source_type_enum(&data_source)
            .ok_or_else(|| AppError::InternalServerError("Invalid source_type".to_string()))?;

        match source_type {
            SourceType::CloudWatch => {
                // Test CloudWatch connection by listing metrics
                if let Some(cloudwatch) = &self.cloudwatch_client {
                    let result = cloudwatch.list_metrics().send().await;
                    Ok(result.is_ok())
                } else {
                    Ok(false)
                }
            }
            SourceType::Dynatrace => {
                // Test Dynatrace API connection
                let connection_config = &data_source.connection_config;
                if let (Some(base_url), Some(api_token)) = (
                    connection_config.get("base_url").and_then(|v| v.as_str()),
                    connection_config.get("api_token").and_then(|v| v.as_str()),
                ) {
                    let url = format!("{}/api/v2/metrics", base_url);
                    let response = self
                        .http_client
                        .get(&url)
                        .header("Authorization", format!("Api-Token {}", api_token))
                        .send()
                        .await;
                    Ok(response.map(|r| r.status().is_success()).unwrap_or(false))
                } else {
                    Ok(false)
                }
            }
            SourceType::Prometheus => {
                // Test Prometheus connection
                let connection_config = &data_source.connection_config;
                if let Some(base_url) = connection_config.get("base_url").and_then(|v| v.as_str()) {
                    let url = format!("{}/api/v1/label/__name__/values", base_url);
                    let response = self.http_client.get(&url).send().await;
                    Ok(response.map(|r| r.status().is_success()).unwrap_or(false))
                } else {
                    Ok(false)
                }
            }
            _ => Ok(true), // Default to true for other types
        }
    }

    // Helper to get ResourceType from a DataSourceModel
    fn get_resource_type_enum(&self, data_source: &DataSourceModel) -> Option<ResourceType> {
        ResourceType::from_str_case_insensitive(&data_source.resource_type)
    }
    // Helper to get SourceType from a DataSourceModel
    fn get_source_type_enum(&self, data_source: &DataSourceModel) -> Option<SourceType> {
        SourceType::from_str_case_insensitive(&data_source.source_type)
    }
}
