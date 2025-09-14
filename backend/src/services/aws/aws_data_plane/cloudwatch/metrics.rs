use std::sync::Arc;
use serde_json::{json, Value};
use tracing::{debug, error};

use aws_sdk_cloudwatch::{
    Client as CloudWatchClient,
    types::{
        Dimension,
        DimensionFilter,
        Metric,
        MetricDataQuery,
        MetricStat,
        Statistic,
        ComparisonOperator,
        AlarmType,
    },
    operation::{
        get_metric_data::GetMetricDataInput,
        list_metrics::ListMetricsInput,
        get_metric_statistics::GetMetricStatisticsInput,
    }
};

use crate::{errors::AppError, models::aws_account::AwsAccountDto};
use super::base::CloudWatchService;
use super::types::{
    CloudWatchMetricsRequest, CloudWatchMetricsResult, CloudWatchMetricData,
    CloudWatchDatapoint, to_aws_datetime, from_aws_datetime,
};

pub trait CloudWatchMetrics {
    async fn get_metrics(&self, aws_account_dto: &AwsAccountDto, request: &CloudWatchMetricsRequest) 
        -> Result<CloudWatchMetricsResult, AppError>;
        
    async fn get_metrics_with_dimensions(
        &self,
        aws_account_dto: &AwsAccountDto,
        namespace: &str,
        metrics: Vec<&str>,
        dimensions: Vec<Dimension>,
        start_time: chrono::DateTime<chrono::Utc>,
        end_time: chrono::DateTime<chrono::Utc>,
        period: i32,
    ) -> Result<Vec<CloudWatchMetricData>, AppError>;
    
    async fn get_metric_statistics(
        &self,
        aws_account_dto: &AwsAccountDto,
        namespace: &str,
        metric_name: &str,
        dimensions: Vec<Dimension>,
        start_time: chrono::DateTime<chrono::Utc>,
        end_time: chrono::DateTime<chrono::Utc>,
        period: i32,
        statistics: Vec<Statistic>,
    ) -> Result<Vec<CloudWatchDatapoint>, AppError>;
    
    async fn list_metrics(
        &self,
        aws_account_dto: &AwsAccountDto,
        namespace: Option<&str>,
        metric_name: Option<&str>,
        dimensions: Option<Vec<Dimension>>,
    ) -> Result<Vec<Metric>, AppError>;
    
    async fn get_metric_math(
        &self,
        aws_account_dto: &AwsAccountDto,
        namespace: &str,
        metrics: Vec<&str>,
        dimensions: Vec<Dimension>,
        start_time: chrono::DateTime<chrono::Utc>,
        end_time: chrono::DateTime<chrono::Utc>,
        period: i32,
        math_expressions: Vec<(String, String)>,
    ) -> Result<Vec<CloudWatchMetricData>, AppError>;
    
    async fn detect_metric_anomalies(
        &self,
        aws_account_dto: &AwsAccountDto,
        namespace: &str,
        metric_name: &str,
        dimensions: Vec<Dimension>,
        start_time: chrono::DateTime<chrono::Utc>,
        end_time: chrono::DateTime<chrono::Utc>,
        period: i32,
    ) -> Result<CloudWatchMetricData, AppError>;
}

impl CloudWatchMetrics for CloudWatchService {
    async fn get_metrics(&self, aws_account_dto: &AwsAccountDto, request: &CloudWatchMetricsRequest) 
        -> Result<CloudWatchMetricsResult, AppError> {
        let client = self.create_cloudwatch_client(aws_account_dto).await?;
        
        let namespace = self.get_namespace_for_resource_type(&request.resource_type);
        let dimensions = self.create_dimensions_for_resource(&request.resource_type, &request.resource_id);
        
        let mut metric_data_queries = Vec::new();
        let mut metric_names = Vec::new();
        
        for (i, metric_name) in request.metrics.iter().enumerate() {
            let query_id = format!("m{}", i);
            
            let metric = Metric::builder()
                .namespace(namespace)
                .metric_name(metric_name)
                .set_dimensions(Some(dimensions.clone()))
                .build();
                
            let metric_stat = MetricStat::builder()
                .metric(metric)
                .period(request.period)
                .stat("Average")
                .build();
                
            let query = MetricDataQuery::builder()
                .id(query_id)
                .metric_stat(metric_stat)
                .return_data(true)
                .build();
                
            metric_data_queries.push(query);
            metric_names.push(metric_name.clone());
        }

        debug!("Executing CloudWatch GetMetricData request");
        
        let response = client.get_metric_data()
            .set_start_time(Some(to_aws_datetime(&request.start_time)))
            .set_end_time(Some(to_aws_datetime(&request.end_time)))
            .set_metric_data_queries(Some(metric_data_queries))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to get CloudWatch metrics: {}", e)))?;
            
        let mut result_metrics = Vec::new();
        
        for (i, result) in response.metric_data_results().iter().enumerate() {
            let mut datapoints = Vec::new();
            let timestamps = result.timestamps();
            let values = result.values();
            
            for (j, timestamp) in timestamps.iter().enumerate() {
                if j < values.len() {
                    datapoints.push(CloudWatchDatapoint {
                        timestamp: from_aws_datetime(timestamp),
                        value: values[j],
                        unit: "Count".to_string(),
                    });
                }
            }
            
            if i < metric_names.len() {
                result_metrics.push(CloudWatchMetricData {
                    namespace: namespace.to_string(),
                    metric_name: metric_names[i].clone(),
                    unit: "Count".to_string(),
                    datapoints,
                });
            }
        }
        
        Ok(CloudWatchMetricsResult {
            resource_id: request.resource_id.clone(),
            resource_type: request.resource_type.clone(),
            metrics: result_metrics,
        })
    }

    async fn get_metrics_with_dimensions(
        &self,
        aws_account_dto: &AwsAccountDto,
        namespace: &str,
        metrics: Vec<&str>,
        dimensions: Vec<Dimension>,
        start_time: chrono::DateTime<chrono::Utc>,
        end_time: chrono::DateTime<chrono::Utc>,
        period: i32,
    ) -> Result<Vec<CloudWatchMetricData>, AppError> {
        let client = self.create_cloudwatch_client(aws_account_dto).await?;
        
        let mut metric_data_queries = Vec::new();
        
        for (i, metric_name) in metrics.iter().enumerate() {
            let query_id = format!("m{}", i);
            
            let metric = Metric::builder()
                .namespace(namespace)
                .metric_name(metric_name.clone())
                .set_dimensions(Some(dimensions.clone()))
                .build();
                
            let metric_stat = MetricStat::builder()
                .metric(metric)
                .period(period)
                .stat("Average")
                .build();
                
            let query = MetricDataQuery::builder()
                .id(query_id)
                .metric_stat(metric_stat)
                .return_data(true)
                .build();
                
            metric_data_queries.push(query);
        }

        debug!("Executing CloudWatch GetMetricData request with dimensions");
        
        let response = client.get_metric_data()
            .set_start_time(Some(to_aws_datetime(&start_time)))
            .set_end_time(Some(to_aws_datetime(&end_time)))
            .set_metric_data_queries(Some(metric_data_queries))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to get CloudWatch metrics with dimensions: {}", e)))?;
            
        let mut result_metrics = Vec::new();
        
        for (i, result) in response.metric_data_results().iter().enumerate() {
            if i >= metrics.len() {
                continue;
            }
            
            let mut datapoints = Vec::new();
            let timestamps = result.timestamps();
            let values = result.values();
            
            for (j, timestamp) in timestamps.iter().enumerate() {
                if j < values.len() {
                    datapoints.push(CloudWatchDatapoint {
                        timestamp: from_aws_datetime(timestamp),
                        value: values[j],
                        unit: "Count".to_string(),
                    });
                }
            }
            
            result_metrics.push(CloudWatchMetricData {
                namespace: namespace.to_string(),
                metric_name: metrics[i].to_string(),
                unit: "Count".to_string(),
                datapoints,
            });
        }
        
        Ok(result_metrics)
    }
    
    async fn get_metric_statistics(
        &self,
        aws_account_dto: &AwsAccountDto,
        namespace: &str,
        metric_name: &str,
        dimensions: Vec<Dimension>,
        start_time: chrono::DateTime<chrono::Utc>,
        end_time: chrono::DateTime<chrono::Utc>,
        period: i32,
        statistics: Vec<Statistic>,
    ) -> Result<Vec<CloudWatchDatapoint>, AppError> {
        let client = self.create_cloudwatch_client(aws_account_dto).await?;

        debug!("Getting metric statistics for {}/{}", namespace, metric_name);
        
        let response = client.get_metric_statistics()
            .namespace(namespace)
            .metric_name(metric_name)
            .set_dimensions(Some(dimensions))
            .start_time(to_aws_datetime(&start_time))
            .end_time(to_aws_datetime(&end_time))
            .period(period)
            .set_statistics(Some(statistics))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to get metric statistics: {}", e)))?;
            
        let mut datapoints = Vec::new();
        
        for datapoint in response.datapoints() {
            if let Some(timestamp) = datapoint.timestamp() {
                let value = datapoint.average()
                    .or_else(|| datapoint.sum())
                    .or_else(|| datapoint.maximum())
                    .or_else(|| datapoint.minimum())
                    .unwrap_or(0.0);
                    
                let unit = datapoint.unit()
                    .map(|u| u.as_str().to_string())
                    .unwrap_or_else(|| "Count".to_string());
                    
                datapoints.push(CloudWatchDatapoint {
                    timestamp: from_aws_datetime(timestamp),
                    value,
                    unit,
                });
            }
        }
        
        Ok(datapoints)
    }
    
    async fn list_metrics(
        &self,
        aws_account_dto: &AwsAccountDto,
        namespace: Option<&str>,
        metric_name: Option<&str>,
        dimensions: Option<Vec<Dimension>>,
    ) -> Result<Vec<Metric>, AppError> {
        let client = self.create_cloudwatch_client(aws_account_dto).await?;
        
        debug!("Listing metrics for namespace: {:?}", namespace);
        
        let mut request = client.list_metrics();
        
        if let Some(ns) = namespace {
            request = request.namespace(ns);
        }
        
        if let Some(metric) = metric_name {
            request = request.metric_name(metric);
        }
        
        if let Some(dims) = dimensions {
            for dimension in dims {
                let filter = DimensionFilter::builder()
                    .name(dimension.name().unwrap_or(""))
                    .value(dimension.value().unwrap_or(""))
                    .build();
                request = request.dimensions(filter);
            }
        }
        
        let response = request
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to list metrics: {}", e)))?;
            
        Ok(response.metrics().to_vec())
    }
    
    async fn get_metric_math(
        &self,
        aws_account_dto: &AwsAccountDto,
        namespace: &str,
        metrics: Vec<&str>,
        dimensions: Vec<Dimension>,
        start_time: chrono::DateTime<chrono::Utc>,
        end_time: chrono::DateTime<chrono::Utc>,
        period: i32,
        math_expressions: Vec<(String, String)>,
    ) -> Result<Vec<CloudWatchMetricData>, AppError> {
        let client = self.create_cloudwatch_client(aws_account_dto).await?;

        let mut metric_data_queries = Vec::new();
        
        // Add metric queries
        for (i, metric_name) in metrics.iter().enumerate() {
            let query_id = format!("m{}", i);
            
            let metric = Metric::builder()
                .namespace(namespace)
                .metric_name(metric_name.clone())
                .set_dimensions(Some(dimensions.clone()))
                .build();
                
            let metric_stat = MetricStat::builder()
                .metric(metric)
                .period(period)
                .stat("Average")
                .build();
                
            let query = MetricDataQuery::builder()
                .id(query_id)
                .metric_stat(metric_stat)
                .return_data(false) // Don't return raw metric data
                .build();
                
            metric_data_queries.push(query);
        }
        
        // Add math expression queries
        for (i, (expr_id, expression)) in math_expressions.iter().enumerate() {
            let query = MetricDataQuery::builder()
                .id(expr_id)
                .expression(expression)
                .return_data(true) // Return the calculated results
                .build();
                
            metric_data_queries.push(query);
        }

        debug!("Executing CloudWatch GetMetricData request with math expressions");
        
        let response = client.get_metric_data()
            .set_start_time(Some(to_aws_datetime(&start_time)))
            .set_end_time(Some(to_aws_datetime(&end_time)))
            .set_metric_data_queries(Some(metric_data_queries))
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to get metric math: {}", e)))?;
            
        let mut result_metrics = Vec::new();
        
        for result in response.metric_data_results() {
            if let Some(id) = result.id() {
                let mut datapoints = Vec::new();
                let timestamps = result.timestamps();
                let values = result.values();
                
                for (j, timestamp) in timestamps.iter().enumerate() {
                    if j < values.len() {
                        datapoints.push(CloudWatchDatapoint {
                            timestamp: from_aws_datetime(timestamp),
                            value: values[j],
                            unit: "Count".to_string(),
                        });
                    }
                }
                
                result_metrics.push(CloudWatchMetricData {
                    namespace: namespace.to_string(),
                    metric_name: id.to_string(),
                    unit: "Count".to_string(),
                    datapoints,
                });
            }
        }
        
        Ok(result_metrics)
    }
    
    async fn detect_metric_anomalies(
        &self,
        aws_account_dto: &AwsAccountDto,
        namespace: &str,
        metric_name: &str,
        dimensions: Vec<Dimension>,
        start_time: chrono::DateTime<chrono::Utc>,
        end_time: chrono::DateTime<chrono::Utc>,
        period: i32,
    ) -> Result<CloudWatchMetricData, AppError> {
        let client = self.create_cloudwatch_client(aws_account_dto).await?;

        // For anomaly detection, we'll use a simple approach of getting metric statistics
        // and identifying values that are significantly different from the average
        let statistics = vec![Statistic::Average, Statistic::Maximum, Statistic::Minimum];
        
        let datapoints = self.get_metric_statistics(
            aws_account_dto,
            namespace,
            metric_name,
            dimensions,
            start_time,
            end_time,
            period,
            statistics,
        ).await?;
        
        // Simple anomaly detection: values that are more than 2 standard deviations from the mean
        let values: Vec<f64> = datapoints.iter().map(|dp| dp.value).collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();
        
        let anomalous_datapoints: Vec<CloudWatchDatapoint> = datapoints
            .into_iter()
            .filter(|dp| (dp.value - mean).abs() > 2.0 * std_dev)
            .collect();
        
        debug!("Detected {} anomalous datapoints for {}/{}", 
               anomalous_datapoints.len(), namespace, metric_name);
        
        Ok(CloudWatchMetricData {
            namespace: namespace.to_string(),
            metric_name: metric_name.to_string(),
            unit: "Count".to_string(),
            datapoints: anomalous_datapoints,
        })
    }
}
