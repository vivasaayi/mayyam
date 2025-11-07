use crate::integration::helpers::TestHarness;
use anyhow::{anyhow, ensure, Context, Result};
use chrono::{Datelike, Months, NaiveDate, Utc};
use mayyam::models::aws_cost_anomalies;
use mayyam::models::aws_cost_anomalies::Entity as CostAnomaliesEntity;
use mayyam::models::aws_monthly_cost_aggregates;
use mayyam::models::aws_monthly_cost_aggregates::Entity as MonthlyAggregatesEntity;
use reqwest::StatusCode;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Database, DatabaseConnection, EntityTrait,
    QueryFilter,
};
use serde_json::Value;
use uuid::Uuid;

const SERVICE_NAME: &str = "AmazonEC2";

async fn connect_test_database() -> Option<DatabaseConnection> {
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Skipping cost analytics API tests: DATABASE_URL not set");
            return None;
        }
    };

    match Database::connect(&database_url).await {
        Ok(conn) => Some(conn),
        Err(err) => {
            eprintln!(
                "Skipping cost analytics API tests: unable to connect to DATABASE_URL ({err})"
            );
            None
        }
    }
}

async fn seed_cost_analytics_data(
    db: &DatabaseConnection,
    account_id: &str,
) -> Result<Uuid, sea_orm::DbErr> {
    use aws_cost_anomalies::ActiveModel as CostAnomalyActiveModel;
    use aws_monthly_cost_aggregates::ActiveModel as MonthlyAggregateActiveModel;

    let today = Utc::now().naive_utc().date();
    let current_month =
        NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or_else(|| {
            NaiveDate::from_ymd_opt(today.year(), today.month(), today.day()).unwrap()
        });

    let month_sequence = [
        current_month
            .checked_sub_months(Months::new(2))
            .unwrap_or(current_month),
        current_month
            .checked_sub_months(Months::new(1))
            .unwrap_or(current_month),
        current_month,
    ];

    let monthly_costs = [80.0_f64, 110.0_f64, 150.0_f64];

    for (date, cost) in month_sequence.iter().zip(monthly_costs.iter()) {
        let aggregate = MonthlyAggregateActiveModel {
            id: ActiveValue::Set(Uuid::new_v4()),
            account_id: ActiveValue::Set(account_id.to_string()),
            service_name: ActiveValue::Set(SERVICE_NAME.to_string()),
            month_year: ActiveValue::Set(*date),
            total_cost: ActiveValue::Set(
                sea_orm::prelude::Decimal::from_f64_retain(*cost)
                    .expect("valid decimal for total_cost"),
            ),
            usage_amount: ActiveValue::Set(None),
            usage_unit: ActiveValue::Set(None),
            cost_change_pct: ActiveValue::Set(None),
            cost_change_amount: ActiveValue::Set(None),
            anomaly_score: ActiveValue::Set(None),
            is_anomaly: ActiveValue::Set(false),
            tags_summary: ActiveValue::Set(None),
            created_at: ActiveValue::Set(Utc::now().into()),
            updated_at: ActiveValue::Set(Utc::now().into()),
        };

        aggregate.insert(db).await?;
    }

    let anomaly = CostAnomalyActiveModel {
        id: ActiveValue::Set(Uuid::new_v4()),
        account_id: ActiveValue::Set(account_id.to_string()),
        service_name: ActiveValue::Set(SERVICE_NAME.to_string()),
        anomaly_type: ActiveValue::Set("spike".to_string()),
        severity: ActiveValue::Set("high".to_string()),
        detected_date: ActiveValue::Set(current_month),
        anomaly_score: ActiveValue::Set(
            sea_orm::prelude::Decimal::from_f64_retain(7.5)
                .expect("valid decimal for anomaly_score"),
        ),
        baseline_cost: ActiveValue::Set(Some(
            sea_orm::prelude::Decimal::from_f64_retain(100.0)
                .expect("valid decimal for baseline_cost"),
        )),
        actual_cost: ActiveValue::Set(
            sea_orm::prelude::Decimal::from_f64_retain(150.0)
                .expect("valid decimal for actual_cost"),
        ),
        cost_difference: ActiveValue::Set(Some(
            sea_orm::prelude::Decimal::from_f64_retain(50.0)
                .expect("valid decimal for cost_difference"),
        )),
        percentage_change: ActiveValue::Set(Some(
            sea_orm::prelude::Decimal::from_f64_retain(33.3)
                .expect("valid decimal for percentage_change"),
        )),
        description: ActiveValue::Set(Some("API test anomaly".to_string())),
        status: ActiveValue::Set("open".to_string()),
        created_at: ActiveValue::Set(Utc::now().into()),
        updated_at: ActiveValue::Set(Utc::now().into()),
    };

    let inserted = anomaly.insert(db).await?;
    Ok(inserted.id)
}

async fn cleanup_cost_analytics_data(
    db: &DatabaseConnection,
    account_id: &str,
) -> Result<(), sea_orm::DbErr> {
    MonthlyAggregatesEntity::delete_many()
        .filter(aws_monthly_cost_aggregates::Column::AccountId.eq(account_id))
        .exec(db)
        .await?;

    CostAnomaliesEntity::delete_many()
        .filter(aws_cost_anomalies::Column::AccountId.eq(account_id))
        .exec(db)
        .await?;

    Ok(())
}

#[tokio::test]
async fn cost_forecast_api_returns_expected_projection() -> Result<()> {
    let Some(db) = connect_test_database().await else {
        return Ok(());
    };

    let account_id = format!("cost-api-test-{}", Uuid::new_v4());
    seed_cost_analytics_data(&db, &account_id)
        .await
        .context("seeding cost analytics data")?;

    let harness = TestHarness::new().await;
    harness.test_delay().await;

    let test_result: Result<()> = async {
        let response = harness
            .client()
            .get(&harness.build_url(&format!("/api/cost-analytics/{}/forecast", account_id)))
            .header("Authorization", format!("Bearer {}", harness.auth_token()))
            .query(&[("days_ahead", 90)])
            .send()
            .await
            .context("calling forecast endpoint")?;

        ensure!(
            response.status() == StatusCode::OK,
            "unexpected forecast status: {}",
            response.status()
        );

        let body: Value = response.json().await.context("parsing forecast response")?;
        ensure!(
            body.get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            "forecast response reported failure"
        );

        let data = body
            .get("data")
            .ok_or_else(|| anyhow!("missing data field in forecast response"))?;
        let forecast_block = data
            .get("forecast")
            .ok_or_else(|| anyhow!("missing forecast block"))?;
        let forecasts = forecast_block
            .get("forecasts")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("missing forecasts array"))?;

        ensure!(
            forecasts.len() == 3,
            "expected 3 forecast points, found {}",
            forecasts.len()
        );

        let trend_direction = data
            .get("trend_analysis")
            .and_then(|v| v.get("trend_direction"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        ensure!(
            !trend_direction.is_empty(),
            "trend analysis did not include a trend direction"
        );

        Ok(())
    }
    .await;

    cleanup_cost_analytics_data(&db, &account_id)
        .await
        .context("cleaning up cost analytics data")?;

    test_result
}

#[tokio::test]
async fn cost_anomalies_api_returns_seeded_anomaly() -> Result<()> {
    let Some(db) = connect_test_database().await else {
        return Ok(());
    };

    let account_id = format!("cost-api-test-{}", Uuid::new_v4());
    let anomaly_id = seed_cost_analytics_data(&db, &account_id)
        .await
        .context("seeding cost analytics data")?;

    let harness = TestHarness::new().await;
    harness.test_delay().await;

    let test_result: Result<()> = async {
        let response = harness
            .client()
            .get(&harness.build_url("/api/cost-analytics/anomalies"))
            .header("Authorization", format!("Bearer {}", harness.auth_token()))
            .query(&[("account_id", account_id.as_str()), ("severity", "high")])
            .send()
            .await
            .context("calling anomalies endpoint")?;

        ensure!(
            response.status() == StatusCode::OK,
            "unexpected anomalies status: {}",
            response.status()
        );

        let body: Value = response
            .json()
            .await
            .context("parsing anomalies response")?;
        ensure!(
            body.get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            "anomalies response reported failure"
        );

        let anomalies = body
            .pointer("/data/anomalies")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("missing anomalies array"))?;

        ensure!(
            anomalies.len() == 1,
            "expected exactly one anomaly, found {}",
            anomalies.len()
        );

        let anomaly = anomalies[0]
            .as_object()
            .ok_or_else(|| anyhow!("anomaly entry is not an object"))?;

        let returned_id = anomaly
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("anomaly missing id"))?;

        ensure!(
            returned_id == anomaly_id.to_string(),
            "unexpected anomaly id: expected {}, found {}",
            anomaly_id,
            returned_id
        );

        ensure!(
            anomaly
                .get("service_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                == SERVICE_NAME,
            "unexpected service name in anomaly response"
        );

        Ok(())
    }
    .await;

    cleanup_cost_analytics_data(&db, &account_id)
        .await
        .context("cleaning up cost analytics data")?;

    test_result
}
