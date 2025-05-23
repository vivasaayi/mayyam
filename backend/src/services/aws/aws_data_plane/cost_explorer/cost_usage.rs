// filepath: /Users/rajanpanneerselvam/work/mayyam/backend/src/services/aws/aws_data_plane/cost_explorer/cost_usage.rs
use serde_json::{json, Value};
use tracing::error;
use chrono::{NaiveDateTime, Local, Duration, Datelike};
use crate::errors::AppError;
use super::base::AwsCostService;
use aws_sdk_costexplorer::types::{DateInterval, GroupDefinition, Granularity};

#[derive(Debug, Clone)]
pub enum DatePreset {
    Today,
    Yesterday,
    LastWeek,
    ThisWeek,
    LastMonth,
    ThisMonth,
    LastYear,
    ThisYear,
    Custom(String, String), // start_date, end_date in YYYY-MM-DD format
}

/// Trait for cost and usage related functionality
pub trait CostAndUsage {
    async fn get_cost_and_usage(
        &self,
        account_id: &str,
        profile: Option<&str>,
        region: &str,
        start_date: &str,
        end_date: &str,
        granularity: Option<Granularity>,
        metrics: Vec<&str>,
        group_by: Option<Vec<GroupDefinition>>,
    ) -> Result<Value, AppError>;
    
    async fn get_cost_for_date(
        &self,
        account_id: &str,
        profile: Option<&str>,
        region: &str,
        date: &str,
        group_by: Option<Vec<GroupDefinition>>,
    ) -> Result<Value, AppError>;
    
    async fn get_cost_for_hour(
        &self,
        account_id: &str,
        profile: Option<&str>,
        region: &str,
        date: &str,
        hour: u32,
        group_by: Option<Vec<GroupDefinition>>,
    ) -> Result<Value, AppError>;
    
    async fn get_cost_for_month(
        &self,
        account_id: &str,
        profile: Option<&str>,
        region: &str,
        year: i32,
        month: u32,
        group_by: Option<Vec<GroupDefinition>>,
    ) -> Result<Value, AppError>;

    async fn get_cost_for_preset(
        &self,
        account_id: &str,
        profile: Option<&str>,
        region: &str,
        preset: DatePreset,
        group_by: Option<Vec<GroupDefinition>>,
    ) -> Result<Value, AppError>;

    async fn compare_costs(
        &self,
        account_id: &str,
        profile: Option<&str>,
        region: &str,
        date1: DatePreset,
        date2: DatePreset,
        group_by: Option<Vec<GroupDefinition>>,
    ) -> Result<Value, AppError>;
}

impl CostAndUsage for AwsCostService {
    async fn get_cost_and_usage(
        &self,
        account_id: &str,
        profile: Option<&str>,
        region: &str,
        start_date: &str,
        end_date: &str,
        granularity: Option<Granularity>,
        metrics: Vec<&str>,
        group_by: Option<Vec<GroupDefinition>>,
    ) -> Result<Value, AppError> {
        let client = self.create_client(profile, region).await?;
        
        let time_period = DateInterval::builder()
            .start(start_date)
            .end(end_date)
            .build();
        
        let mut request = client.get_cost_and_usage()
            .time_period(time_period)
            .granularity(granularity.unwrap_or(Granularity::Daily));
            
        // Add metrics
        for metric in metrics {
            request = request.metrics(metric);
        }
        
        // Add group by if provided
        if let Some(groups) = group_by {
            for group in groups {
                request = request.group_by(group);
            }
        }
        
        let response = match request.send().await {
            Ok(resp) => resp,
            Err(e) => {
                error!("Error getting cost and usage data: {:?}", e);
                return Err(AppError::ExternalService(format!("Failed to get cost data: {}", e)));
            }
        };
        
        // Convert the response to a JSON object
        let mut results = Vec::new();
        
        // Iterate over the results by time
        if let Some(results_by_time) = response.results_by_time() {
            for result_by_time in results_by_time {
                let mut result_json = json!({});
                
                // Handle time period
                if let Some(time_period) = result_by_time.time_period() {
                    result_json["timePeriod"] = json!({
                        "start": time_period.start().unwrap_or_default(),
                        "end": time_period.end().unwrap_or_default(),
                    });
                }
                
                // Handle totals
                if let Some(total) = result_by_time.total() {
                    let mut total_json = json!({});
                    for (metric_name, metric_value) in total {
                        total_json[metric_name] = json!({
                            "amount": metric_value.amount().unwrap_or("0"),
                            "unit": metric_value.unit().unwrap_or("USD")
                        });
                    }
                    result_json["total"] = total_json;
                }
                
                // Handle groups
                if let Some(groups) = result_by_time.groups() {
                    let mut groups_json = Vec::new();
                    for group in groups {
                        let mut group_json = json!({});
                        
                        // Handle keys
                        if let Some(keys) = group.keys() {
                            group_json["keys"] = json!(keys);
                        }
                        
                        // Handle metrics
                        if let Some(metrics) = group.metrics() {
                            let mut metrics_json = json!({});
                            for (metric_name, metric_value) in metrics {
                                metrics_json[metric_name] = json!({
                                    "amount": metric_value.amount().unwrap_or("0"),
                                    "unit": metric_value.unit().unwrap_or("USD")
                                });
                            }
                            group_json["metrics"] = metrics_json;
                        }
                        
                        groups_json.push(group_json);
                    }
                    result_json["groups"] = json!(groups_json);
                }
                
                results.push(result_json);
            }
        }
        
        Ok(json!({
            "results": results,
            "account_id": account_id
        }))
    }

    async fn get_cost_for_date(
        &self,
        account_id: &str,
        profile: Option<&str>,
        region: &str,
        date: &str,
        group_by: Option<Vec<GroupDefinition>>,
    ) -> Result<Value, AppError> {
        self.get_cost_and_usage(
            account_id,
            profile,
            region,
            date,
            date,
            Some(Granularity::Daily),
            vec!["AmortizedCost", "UnblendedCost", "UsageQuantity"],
            group_by,
        ).await
    }

    async fn get_cost_for_hour(
        &self,
        account_id: &str,
        profile: Option<&str>,
        region: &str,
        date: &str,
        hour: u32,
        group_by: Option<Vec<GroupDefinition>>,
    ) -> Result<Value, AppError> {
        // AWS Cost Explorer doesn't support hourly granularity directly
        // We'll get daily data and add a note about this limitation
        let result = self.get_cost_for_date(account_id, profile, region, date, group_by).await?;
        
        let result_with_note = json!({
            "note": "AWS Cost Explorer does not support hourly granularity. Showing daily cost instead.",
            "requested_hour": hour,
            "data": result
        });
        
        Ok(result_with_note)
    }

    async fn get_cost_for_month(
        &self,
        account_id: &str,
        profile: Option<&str>,
        region: &str,
        year: i32,
        month: u32,
        group_by: Option<Vec<GroupDefinition>>,
    ) -> Result<Value, AppError> {
        let start_date = format!("{:04}-{:02}-01", year, month);
        let end_date = format!("{:04}-{:02}-{:02}", year, month, days_in_month(year, month));
        
        self.get_cost_and_usage(
            account_id,
            profile,
            region,
            &start_date,
            &end_date,
            Some(Granularity::Monthly),
            vec!["AmortizedCost", "UnblendedCost", "UsageQuantity"],
            group_by,
        ).await
    }

    async fn get_cost_for_preset(
        &self,
        account_id: &str,
        profile: Option<&str>,
        region: &str,
        preset: DatePreset,
        group_by: Option<Vec<GroupDefinition>>,
    ) -> Result<Value, AppError> {
        let (start_date, end_date) = get_preset_dates(preset)?;
        
        self.get_cost_and_usage(
            account_id,
            profile,
            region,
            &start_date,
            &end_date,
            Some(Granularity::Daily),
            vec!["AmortizedCost", "UnblendedCost", "UsageQuantity"],
            group_by,
        ).await
    }

    async fn compare_costs(
        &self,
        account_id: &str,
        profile: Option<&str>,
        region: &str,
        date1: DatePreset,
        date2: DatePreset,
        group_by: Option<Vec<GroupDefinition>>,
    ) -> Result<Value, AppError> {
        let result1 = self.get_cost_for_preset(account_id, profile, region, date1.clone(), group_by.clone()).await?;
        let result2 = self.get_cost_for_preset(account_id, profile, region, date2.clone(), group_by.clone()).await?;

        Ok(json!({
            "comparison": {
                "period1": {
                    "preset": format!("{:?}", date1),
                    "data": result1
                },
                "period2": {
                    "preset": format!("{:?}", date2),
                    "data": result2
                }
            }
        }))
    }
}

// Helper function to get the number of days in a month
fn days_in_month(year: i32, month: u32) -> u32 {
    let next_month = if month == 12 {
        NaiveDateTime::new(
            chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap(),
            chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
        )
    } else {
        NaiveDateTime::new(
            chrono::NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap(),
            chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
        )
    };
    
    let this_month = NaiveDateTime::new(
        chrono::NaiveDate::from_ymd_opt(year, month, 1).unwrap(),
        chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    );
    
    (next_month - this_month).num_days() as u32
}

// Helper function to convert date presets to actual dates
fn get_preset_dates(preset: DatePreset) -> Result<(String, String), AppError> {
    let today = Local::now();
    
    match preset {
        DatePreset::Today => {
            let date = today.format("%Y-%m-%d").to_string();
            Ok((date.clone(), date))
        },
        DatePreset::Yesterday => {
            let date = (today - Duration::days(1)).format("%Y-%m-%d").to_string();
            Ok((date.clone(), date))
        },
        DatePreset::ThisWeek => {
            let start = today - Duration::days(today.weekday().num_days_from_monday() as i64);
            let end = today.format("%Y-%m-%d").to_string();
            Ok((start.format("%Y-%m-%d").to_string(), end))
        },
        DatePreset::LastWeek => {
            let end = today - Duration::days(today.weekday().num_days_from_monday() as i64 + 1);
            let start = end - Duration::days(6);
            Ok((start.format("%Y-%m-%d").to_string(), end.format("%Y-%m-%d").to_string()))
        },
        DatePreset::ThisMonth => {
            let start = today.with_day(1).unwrap();
            let end = today.format("%Y-%m-%d").to_string();
            Ok((start.format("%Y-%m-%d").to_string(), end))
        },
        DatePreset::LastMonth => {
            let last_month = if today.month() == 1 {
                today.with_year(today.year() - 1).unwrap().with_month(12).unwrap()
            } else {
                today.with_month(today.month() - 1).unwrap()
            };
            let start = last_month.with_day(1).unwrap();
            let end = last_month.with_day(days_in_month(last_month.year(), last_month.month()) as u32).unwrap();
            Ok((start.format("%Y-%m-%d").to_string(), end.format("%Y-%m-%d").to_string()))
        },
        DatePreset::ThisYear => {
            let start = today.with_month(1).unwrap().with_day(1).unwrap();
            let end = today.format("%Y-%m-%d").to_string();
            Ok((start.format("%Y-%m-%d").to_string(), end))
        },
        DatePreset::LastYear => {
            let last_year = today.with_year(today.year() - 1).unwrap();
            let start = last_year.with_month(1).unwrap().with_day(1).unwrap();
            let end = last_year.with_month(12).unwrap().with_day(31).unwrap();
            Ok((start.format("%Y-%m-%d").to_string(), end.format("%Y-%m-%d").to_string()))
        },
        DatePreset::Custom(start, end) => Ok((start, end)),
    }
}
