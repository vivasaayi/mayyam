use serde_json::{json, Value};
use tracing::{debug, error};
use crate::{errors::AppError, models::aws_account::AwsAccountDto};
use super::base::AwsCostService;
use aws_sdk_costexplorer::types::{DateInterval, Granularity};

pub trait CostForecasting {
    async fn get_cost_forecast(
        &self,
        aws_account_dto: &AwsAccountDto,
        start_date: &str,
        end_date: &str,
        metric: Option<&str>,
        granularity: Option<Granularity>,
    ) -> Result<Value, AppError>;

    async fn get_monthly_forecast(
        &self,
        aws_account_dto: &AwsAccountDto,
        months_ahead: u32,
    ) -> Result<Value, AppError>;
}

impl CostForecasting for AwsCostService {
    async fn get_cost_forecast(
        &self,
        aws_account_dto: &AwsAccountDto,
        start_date: &str,
        end_date: &str,
        metric: Option<&str>,
        granularity: Option<Granularity>,
    ) -> Result<Value, AppError> {
        let client = self.create_client(aws_account_dto).await?;
        
        let time_period = DateInterval::builder()
            .start(start_date)
            .end(end_date)
            .build()
            .map_err(|e| AppError::ExternalService(format!("Failed to build time period: {}", e)))?;
        
        let metric_str = metric.unwrap_or("UNBLENDED_COST");
        
        debug!("Getting cost forecast from {} to {}", start_date, end_date);
        
        let response = match client.get_cost_forecast()
            .time_period(time_period)
            .metric(metric_str.into())
            .granularity(granularity.unwrap_or(Granularity::Monthly))
            .send()
            .await {
            Ok(resp) => resp,
            Err(e) => {
                error!("Error getting cost forecast: {:?}", e);
                return Err(AppError::ExternalService(format!("Failed to get cost forecast: {}", e)));
            }
        };
        
        let mut result = json!({
            "account_id": aws_account_dto.account_id.clone(),
            "metric": metric_str,
            "forecast": {
                "total": response.total().map(|t| json!({
                    "amount": t.amount().unwrap_or_default(),
                    "unit": t.unit().unwrap_or_default()
                })).unwrap_or_else(|| json!({})),
                "predictions": []
            }
        });
        
        let predictions = result["forecast"]["predictions"].as_array_mut().unwrap();
        
        for forecast_result in response.forecast_results_by_time() {
            predictions.push(json!({
                "timePeriod": forecast_result.time_period().map(|tp| json!({
                    "start": tp.start(),
                    "end": tp.end()
                })).unwrap_or_else(|| json!({})),
                "meanValue": forecast_result.mean_value().unwrap_or_default(),
                "predictionIntervals": forecast_result.prediction_interval_lower_bound().map(|lower| {
                    json!({
                        "lowerBound": lower,
                        "upperBound": forecast_result.prediction_interval_upper_bound().unwrap_or_default()
                    })
                }).unwrap_or_else(|| json!({}))
            }));
        }
        
        Ok(result)
    }

    async fn get_monthly_forecast(
        &self,
        aws_account_dto: &AwsAccountDto,
        months_ahead: u32,
    ) -> Result<Value, AppError> {
        use chrono::{Local, Duration};
        
        let start_date = Local::now().format("%Y-%m-%d").to_string();
        let end_date = (Local::now() + Duration::days(30 * months_ahead as i64))
            .format("%Y-%m-%d")
            .to_string();
        
        self.get_cost_forecast(
            aws_account_dto,
            &start_date,
            &end_date,
            Some("UNBLENDED_COST"),
            Some(Granularity::Monthly),
        ).await
    }
}
