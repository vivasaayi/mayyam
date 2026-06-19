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

use std::sync::Arc;

use actix_web::{test, web, App};
use chrono::Utc;
use mayyam::controllers::cost_alerts::CostAlertsController;
use mayyam::repositories::cost_analytics::CostAnalyticsRepository;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use serde_json::json;
use uuid::Uuid;

#[actix_web::test]
async fn cost_anomaly_alerts_contract() {
    let db = Database::connect("sqlite::memory:?cache=shared")
        .await
        .expect("connect sqlite");
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE aws_cost_anomalies (
            id TEXT PRIMARY KEY NOT NULL,
            account_id TEXT NOT NULL,
            service_name TEXT NOT NULL,
            anomaly_type TEXT NOT NULL,
            severity TEXT NOT NULL,
            detected_date TEXT NOT NULL,
            anomaly_score DECIMAL NOT NULL,
            baseline_cost DECIMAL NULL,
            actual_cost DECIMAL NOT NULL,
            cost_difference DECIMAL NULL,
            percentage_change DECIMAL NULL,
            description TEXT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        "#
        .to_string(),
    ))
    .await
    .expect("create aws_cost_anomalies");
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE aws_cost_insights (
            id TEXT PRIMARY KEY NOT NULL,
            anomaly_id TEXT NULL,
            aggregate_id TEXT NULL,
            account_id TEXT NOT NULL,
            insight_type TEXT NOT NULL,
            prompt_template TEXT NOT NULL,
            llm_provider TEXT NOT NULL,
            llm_model TEXT NOT NULL,
            llm_response TEXT NOT NULL,
            summary TEXT NULL,
            recommendations TEXT NULL,
            confidence_score DECIMAL NULL,
            tokens_used INTEGER NULL,
            processing_time_ms INTEGER NULL,
            created_at TEXT NOT NULL
        )
        "#
        .to_string(),
    ))
    .await
    .expect("create aws_cost_insights");

    let account_id = "123456789012";
    let anomaly_id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();

    insert_cost_anomaly(
        &db,
        anomaly_id,
        account_id,
        "AmazonEC2",
        "spike",
        "high",
        "open",
        &now,
    )
    .await;
    insert_cost_insight(
        &db,
        Uuid::new_v4(),
        anomaly_id,
        account_id,
        Some("Investigate recent EC2 scale-out".to_string()),
        Some(0.55),
        &now,
    )
    .await;
    insert_cost_anomaly(
        &db,
        Uuid::new_v4(),
        account_id,
        "AWSLambda",
        "spike",
        "medium",
        "open",
        &now,
    )
    .await;

    let repo = Arc::new(CostAnalyticsRepository::new(Arc::new(db)));
    let controller = Arc::new(CostAlertsController::new(repo));
    let app = test::init_service(App::new().app_data(web::Data::from(controller)).route(
        "/api/v1/cost-alerts/anomalies",
        web::get().to(CostAlertsController::anomaly_alerts),
    ))
    .await;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/cost-alerts/anomalies?account_id={}",
            account_id
        ))
        .to_request();
    let response = test::call_service(&app, req).await;
    let status = response.status();
    let body_bytes = test::read_body(response).await;
    if !status.is_success() {
        panic!(
            "unexpected status {} body {}",
            status,
            String::from_utf8_lossy(&body_bytes)
        );
    }
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("cost alert response json");

    assert_eq!(body["resource_type"], "AwsCostAnomalyAlert");
    assert_eq!(body["resources_evaluated"], 2);
    assert_eq!(body["active_alerts"], 3);
    assert_eq!(body["insufficient_data_alerts"], 2);
    assert_eq!(body["alerts"].as_array().unwrap().len(), 5);
    assert_eq!(
        body["notification_workflow"]["workflow_id"],
        "cost_anomaly_notification"
    );
    assert_eq!(body["health_workflow"]["status"], "action_required");
    assert_eq!(
        body["reporting"]["report_id"],
        "aws-cost-anomaly-alert-summary"
    );
    assert!(body["reporting"]["services_with_alerts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "AmazonEC2"));
    assert!(body["alerts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|alert| alert["reason_code"] == "AWS_COST_ANOMALY_ALERT_HIGH_SEVERITY"));
    assert!(body["alerts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|alert| alert["reason_code"] == "AWS_COST_ANOMALY_ALERT_MISSING_INSIGHT"));
}

async fn insert_cost_anomaly(
    db: &sea_orm::DatabaseConnection,
    anomaly_id: Uuid,
    account_id: &str,
    service_name: &str,
    anomaly_type: &str,
    severity: &str,
    status: &str,
    now: &str,
) {
    let anomaly_hex = uuid_hex(anomaly_id);
    let statement = format!(
        "INSERT INTO aws_cost_anomalies (id, account_id, service_name, anomaly_type, severity, detected_date, anomaly_score, baseline_cost, actual_cost, cost_difference, percentage_change, description, status, created_at, updated_at) VALUES (X'{}', '{}', '{}', '{}', '{}', '2026-06-01', 8.25, 120.55, 280.75, 160.20, 133.35, 'Cost anomaly', '{}', '{}', '{}')",
        anomaly_hex,
        account_id,
        service_name,
        anomaly_type,
        severity,
        status,
        now,
        now,
    );
    db.execute(Statement::from_string(DbBackend::Sqlite, statement))
        .await
        .expect("insert anomaly");
}

async fn insert_cost_insight(
    db: &sea_orm::DatabaseConnection,
    insight_id: Uuid,
    anomaly_id: Uuid,
    account_id: &str,
    summary: Option<String>,
    confidence_score: Option<f64>,
    now: &str,
) {
    let insight_hex = uuid_hex(insight_id);
    let anomaly_hex = uuid_hex(anomaly_id);
    let summary_sql = summary
        .map(|value| format!("'{}'", value.replace('\'', "''")))
        .unwrap_or_else(|| "NULL".to_string());
    let confidence_sql = confidence_score
        .map(|value| value.to_string())
        .unwrap_or_else(|| "NULL".to_string());
    let recommendations_sql = json!(["review scaling events", "check deployment history"])
        .to_string()
        .replace('\'', "''");
    let statement = format!(
        "INSERT INTO aws_cost_insights (id, anomaly_id, aggregate_id, account_id, insight_type, prompt_template, llm_provider, llm_model, llm_response, summary, recommendations, confidence_score, tokens_used, processing_time_ms, created_at) VALUES (X'{}', X'{}', NULL, '{}', 'anomaly_analysis', 'cost-anomaly-v1', 'openai', 'gpt-5', '{{}}', {}, '{}', {}, 512, 1200, '{}')",
        insight_hex,
        anomaly_hex,
        account_id,
        summary_sql,
        recommendations_sql,
        confidence_sql,
        now,
    );
    db.execute(Statement::from_string(DbBackend::Sqlite, statement))
        .await
        .expect("insert insight");
}

fn uuid_hex(id: Uuid) -> String {
    id.as_bytes()
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}
