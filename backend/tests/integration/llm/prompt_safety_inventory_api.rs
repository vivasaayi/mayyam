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

use actix_web::{http::StatusCode, test, web, App};
use mayyam::controllers::llm_model::LlmModelController;
use mayyam::repositories::llm_model::LlmProviderModelRepository;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use serde_json::json;
use uuid::Uuid;

async fn build_repo() -> Arc<LlmProviderModelRepository> {
    let db = Database::connect("sqlite::memory:?cache=shared")
        .await
        .expect("connect sqlite");
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE llm_provider_models (
            id TEXT PRIMARY KEY NOT NULL,
            provider_id TEXT NOT NULL,
            model_name TEXT NOT NULL,
            model_config TEXT NOT NULL,
            enabled BOOLEAN NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        "#
        .to_string(),
    ))
    .await
    .expect("create llm_provider_models");

    Arc::new(LlmProviderModelRepository::new(Arc::new(db)))
}

#[actix_web::test]
async fn llm_prompt_injection_inventory_pillar_reports_contract() {
    let repo = build_repo().await;
    repo.create(
        Uuid::new_v4(),
        "guard-model".to_string(),
        json!({
            "owner": "sre-ai",
            "labels": ["cost-center=ai-platform"],
            "prompt_injection_detection_enabled": true,
            "prompt_firewall_enabled": true,
            "prompt_injection_policy_version": "prompt-firewall-2026-06",
            "prompt_injection_sample_count": 800,
            "prompt_injection_review_window": "rolling 7d",
            "prompt_injection_monthly_cost_usd": 60.0,
            "prompt_injection_budget_usd": 120.0,
            "prompt_injection_audit_enabled": true,
            "prompt_injection_review_policy": "review blocked prompts before suppressing",
            "prompt_injection_tamper_evidence_enabled": true
        }),
        true,
    )
    .await
    .expect("create healthy prompt injection detector");
    repo.create(
        Uuid::new_v4(),
        "unguarded-model".to_string(),
        json!({"prompt_injection_monthly_cost_usd": 250.0}),
        true,
    )
    .await
    .expect("create incomplete prompt injection detector");

    let controller = Arc::new(LlmModelController::new(repo));
    let app = test::init_service(App::new().app_data(web::Data::from(controller)).route(
        "/api/v1/llm-providers/prompt-injection-inventory/pillars",
        web::get().to(LlmModelController::prompt_injection_inventory_pillar_reports),
    ))
    .await;

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/prompt-injection-inventory/pillars")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["resource_type"], "AiLlmPromptInjectionDetection");
    assert_eq!(body["resources_evaluated"].as_u64().unwrap_or(0), 2);
    assert_eq!(body["reports"].as_array().unwrap().len(), 3);

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/prompt-injection-inventory/pillars?pillar=resilience,security")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    let reports = body["reports"].as_array().unwrap();
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(reports[1]["pillar"], "security");

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/prompt-injection-inventory/pillars?pillar=durability")
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn llm_sensitive_data_leakage_inventory_pillar_reports_contract() {
    let repo = build_repo().await;
    repo.create(
        Uuid::new_v4(),
        "guard-model".to_string(),
        json!({
            "owner": "sre-ai",
            "labels": ["cost-center=ai-platform"],
            "sensitive_data_leakage_detection_enabled": true,
            "sensitive_data_pattern_set_version": "pii-patterns-2026-06",
            "sensitive_data_leakage_sample_count": 900,
            "sensitive_data_review_window": "rolling 7d",
            "sensitive_data_leakage_monthly_cost_usd": 70.0,
            "sensitive_data_leakage_budget_usd": 120.0,
            "sensitive_data_leakage_audit_enabled": true,
            "sensitive_data_redaction_policy": "redact secrets and customer identifiers",
            "sensitive_data_quarantine_action": "block delivery and quarantine transcript",
            "sensitive_data_tamper_evidence_enabled": true
        }),
        true,
    )
    .await
    .expect("create healthy sensitive data detector");
    repo.create(
        Uuid::new_v4(),
        "unguarded-model".to_string(),
        json!({"sensitive_data_leakage_monthly_cost_usd": 300.0}),
        true,
    )
    .await
    .expect("create incomplete sensitive data detector");

    let controller = Arc::new(LlmModelController::new(repo));
    let app = test::init_service(App::new().app_data(web::Data::from(controller)).route(
        "/api/v1/llm-providers/sensitive-data-leakage-inventory/pillars",
        web::get().to(LlmModelController::sensitive_data_leakage_inventory_pillar_reports),
    ))
    .await;

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/sensitive-data-leakage-inventory/pillars")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["resource_type"], "AiLlmSensitiveDataLeakage");
    assert_eq!(body["resources_evaluated"].as_u64().unwrap_or(0), 2);
    assert_eq!(body["reports"].as_array().unwrap().len(), 3);

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/sensitive-data-leakage-inventory/pillars?pillar=resilience,security")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    let reports = body["reports"].as_array().unwrap();
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(reports[1]["pillar"], "security");

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/sensitive-data-leakage-inventory/pillars?pillar=durability")
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
