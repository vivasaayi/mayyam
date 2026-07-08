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
use mayyam::config::Config;
use mayyam::controllers::cloud_alerts::CloudAlertsController;
use mayyam::repositories::aws_resource::AwsResourceRepository;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use serde_json::json;
use uuid::Uuid;

#[actix_web::test]
async fn cloud_quota_exhaustion_alerts_contract() {
    let db = Database::connect("sqlite::memory:?cache=shared")
        .await
        .expect("connect sqlite");
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE aws_resources (
            id TEXT PRIMARY KEY NOT NULL,
            sync_id TEXT NULL,
            account_id TEXT NOT NULL,
            profile TEXT NULL,
            region TEXT NOT NULL,
            resource_type TEXT NOT NULL,
            resource_id TEXT NOT NULL,
            arn TEXT NOT NULL,
            name TEXT NULL,
            tags TEXT NOT NULL,
            resource_data TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            last_refreshed TEXT NOT NULL
        )
        "#
        .to_string(),
    ))
    .await
    .expect("create aws_resources");

    let account_id = "123456789012";
    let now = Utc::now().to_rfc3339();

    insert_resource(
        &db,
        account_id,
        "us-east-1",
        "Route53HostedZone",
        "hosted-zone",
        "arn:aws:route53:::hostedzone/hosted-zone",
        Some("example.com"),
        json!({"team": "dns"}),
        json!({"resource_record_set_count": 9500}),
        &now,
    )
    .await;

    insert_resource(
        &db,
        account_id,
        "us-east-1",
        "EventBridgeRule",
        "rule-quota",
        "arn:aws:eventbridge:us-east-1:123456789012:rule/rule-quota",
        Some("rule-quota"),
        json!({"team": "platform"}),
        json!({"state": "ENABLED", "target_count": 5}),
        &now,
    )
    .await;

    insert_resource(
        &db,
        account_id,
        "us-east-1",
        "LambdaFunction",
        "quota-near",
        "arn:aws:lambda:us-east-1:123456789012:function:quota-near",
        Some("quota-near"),
        json!({"team": "serverless"}),
        json!({
            "reserved_concurrent_executions": 950,
            "account_concurrency_limit": 1000
        }),
        &now,
    )
    .await;

    insert_resource(
        &db,
        account_id,
        "us-east-1",
        "LambdaFunction",
        "missing-evidence",
        "arn:aws:lambda:us-east-1:123456789012:function:missing-evidence",
        Some("missing-evidence"),
        json!({"team": "serverless"}),
        json!({}),
        &now,
    )
    .await;

    let repo = Arc::new(AwsResourceRepository::new(Arc::new(db), Config::default()));

    let controller = Arc::new(CloudAlertsController::new(repo));
    let app = test::init_service(App::new().app_data(web::Data::from(controller)).route(
        "/api/v1/cloud-alerts/quota-exhaustion",
        web::get().to(CloudAlertsController::quota_exhaustion_alerts),
    ))
    .await;

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/cloud-alerts/quota-exhaustion?account_id={}",
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
        serde_json::from_slice(&body_bytes).expect("cloud alert response json");

    assert_eq!(body["resource_type"], "AwsQuotaExhaustionAlert");
    assert_eq!(body["resources_evaluated"], 4);
    assert_eq!(body["active_alerts"], 3);
    assert_eq!(body["insufficient_data_alerts"], 1);
    assert_eq!(body["alerts"].as_array().unwrap().len(), 4);
    assert_eq!(
        body["notification_workflow"]["workflow_id"],
        "cloud_quota_exhaustion_notification"
    );
    assert_eq!(body["health_workflow"]["status"], "action_required");
    assert_eq!(
        body["reporting"]["report_id"],
        "cloud-quota-exhaustion-summary"
    );
    assert!(body["reporting"]["services_with_alerts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "lambda"));
    assert!(body["alerts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|alert| alert["reason_code"] == "R53_RES_RECORD_QUOTA_NEAR"));
    assert!(body["alerts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|alert| alert["reason_code"] == "LAMBDA_RES_MISSING_QUOTA_LIMIT_EVIDENCE"));
}

async fn insert_resource(
    db: &sea_orm::DatabaseConnection,
    account_id: &str,
    region: &str,
    resource_type: &str,
    resource_id: &str,
    arn: &str,
    name: Option<&str>,
    tags: serde_json::Value,
    resource_data: serde_json::Value,
    now: &str,
) {
    let uuid_hex = Uuid::new_v4()
        .as_bytes()
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<String>();
    let name_sql = name
        .map(|value| format!("'{}'", value.replace('\'', "''")))
        .unwrap_or_else(|| "NULL".to_string());
    let tags_sql = tags.to_string().replace('\'', "''");
    let data_sql = resource_data.to_string().replace('\'', "''");
    let statement = format!(
        "INSERT INTO aws_resources (id, sync_id, account_id, profile, region, resource_type, resource_id, arn, name, tags, resource_data, created_at, updated_at, last_refreshed) VALUES (X'{}', NULL, '{}', NULL, '{}', '{}', '{}', '{}', {}, '{}', '{}', '{}', '{}', '{}')",
        uuid_hex,
        account_id,
        region,
        resource_type,
        resource_id,
        arn,
        name_sql,
        tags_sql,
        data_sql,
        now,
        now,
        now,
    );
    db.execute(Statement::from_string(DbBackend::Sqlite, statement))
        .await
        .expect("insert aws resource");
}
