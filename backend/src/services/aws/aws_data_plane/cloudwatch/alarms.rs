use serde_json::{json, Value};
use tracing::{debug, error};
use aws_sdk_cloudwatch::types::{ComparisonOperator, Dimension, Statistic};
use crate::errors::AppError;
use super::base::CloudWatchService;
use super::types::CloudWatchAlarmDetails;

pub trait CloudWatchAlarms {
    async fn create_metric_alarm(
        &self,
        profile: Option<&str>,
        region: &str,
        alarm_details: CloudWatchAlarmDetails,
        dimensions: Vec<Dimension>,
    ) -> Result<(), AppError>;
    
    async fn get_alarms_by_resource(
        &self,
        profile: Option<&str>,
        region: &str,
        resource_id: &str,
    ) -> Result<Vec<Value>, AppError>;
}

impl CloudWatchAlarms for CloudWatchService {
    async fn create_metric_alarm(
        &self,
        profile: Option<&str>,
        region: &str,
        alarm_details: CloudWatchAlarmDetails,
        dimensions: Vec<Dimension>,
    ) -> Result<(), AppError> {
        let client = self.create_cloudwatch_client(profile, region).await?;
        
        let operator = match alarm_details.comparison_operator.as_str() {
            "GreaterThanThreshold" => ComparisonOperator::GreaterThanThreshold,
            "GreaterThanOrEqualToThreshold" => ComparisonOperator::GreaterThanOrEqualToThreshold,
            "LessThanThreshold" => ComparisonOperator::LessThanThreshold,
            "LessThanOrEqualToThreshold" => ComparisonOperator::LessThanOrEqualToThreshold,
            _ => return Err(AppError::BadRequest(format!(
                "Invalid comparison operator: {}", 
                alarm_details.comparison_operator
            ))),
        };
        
        let stat = match alarm_details.statistic.as_str() {
            "Average" => Statistic::Average,
            "Maximum" => Statistic::Maximum,
            "Minimum" => Statistic::Minimum,
            "Sum" => Statistic::Sum,
            "SampleCount" => Statistic::SampleCount,
            _ => return Err(AppError::BadRequest(format!(
                "Invalid statistic: {}", 
                alarm_details.statistic
            ))),
        };
        
        debug!("Creating CloudWatch alarm: {}", alarm_details.alarm_name);
        
        client.put_metric_alarm()
            .alarm_name(&alarm_details.alarm_name)
            .namespace(&alarm_details.namespace)
            .metric_name(&alarm_details.metric_name)
            .set_dimensions(Some(dimensions))
            .threshold(alarm_details.threshold)
            .comparison_operator(operator)
            .evaluation_periods(alarm_details.evaluation_periods)
            .period(alarm_details.period)
            .statistic(stat)
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to create CloudWatch alarm: {}", e)))?;
            
        Ok(())
    }
    
    async fn get_alarms_by_resource(
        &self,
        profile: Option<&str>,
        region: &str,
        resource_id: &str,
    ) -> Result<Vec<Value>, AppError> {
        let client = self.create_cloudwatch_client(profile, region).await?;
        
        let response = client.describe_alarms()
            .send()
            .await
            .map_err(|e| AppError::ExternalService(format!("Failed to get CloudWatch alarms: {}", e)))?;
            
        let mut alarms = Vec::new();
        
        if let Some(metric_alarms) = response.metric_alarms() {
            for alarm in metric_alarms {
                // Check if alarm is associated with the resource
                if let Some(dimensions) = alarm.dimensions() {
                    for dimension in dimensions {
                        if dimension.value() == Some(resource_id) {
                            alarms.push(json!({
                                "alarmName": alarm.alarm_name().unwrap_or_default(),
                                "namespace": alarm.namespace().unwrap_or_default(),
                                "metricName": alarm.metric_name().unwrap_or_default(),
                                "dimensions": dimensions.iter().map(|d| json!({
                                    "name": d.name().unwrap_or_default(),
                                    "value": d.value().unwrap_or_default()
                                })).collect::<Vec<_>>(),
                                "statistic": alarm.statistic().map(|s| s.as_str()),
                                "period": alarm.period(),
                                "threshold": alarm.threshold(),
                                "comparisonOperator": alarm.comparison_operator().map(|c| c.as_str()),
                                "evaluationPeriods": alarm.evaluation_periods(),
                                "state": alarm.state_value().map(|s| s.as_str()),
                                "stateReason": alarm.state_reason().unwrap_or_default(),
                            }));
                            break;
                        }
                    }
                }
            }
        }
        
        Ok(alarms)
    }
}
