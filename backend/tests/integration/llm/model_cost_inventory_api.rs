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

#[actix_web::test]
async fn llm_model_cost_inventory_pillar_reports_contract() {
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

    let repo = Arc::new(LlmProviderModelRepository::new(Arc::new(db)));
    repo.create(
        Uuid::new_v4(),
        "deepseek-chat".to_string(),
        json!({
            "owner": "sre-ai",
            "labels": ["cost-center=ai-platform"],
            "input_token_price_usd": 0.000001,
            "output_token_price_usd": 0.000002,
            "prompt_tokens": 40000,
            "completion_tokens": 20000,
            "monthly_budget_usd": 150.0,
            "cost_guardrail": "route high-volume traffic through cached model",
            "fallback_model": "deepseek-reasoner",
            "audit_enabled": true,
            "cost_redaction_policy": "redact prompt and response text"
        }),
        true,
    )
    .await
    .expect("create healthy model cost route");
    repo.create(
        Uuid::new_v4(),
        "expensive-model".to_string(),
        json!({"estimated_monthly_cost_usd": 1200.0}),
        true,
    )
    .await
    .expect("create incomplete model cost route");

    let controller = Arc::new(LlmModelController::new(repo));
    let app = test::init_service(App::new().app_data(web::Data::from(controller)).route(
        "/api/v1/llm-providers/model-cost-inventory/pillars",
        web::get().to(LlmModelController::model_cost_inventory_pillar_reports),
    ))
    .await;

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/model-cost-inventory/pillars")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["resource_type"], "AiLlmModelCost");
    assert_eq!(body["resources_evaluated"].as_u64().unwrap_or(0), 2);
    assert_eq!(body["reports"].as_array().unwrap().len(), 3);

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/model-cost-inventory/pillars?pillar=resilience,security")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    let reports = body["reports"].as_array().unwrap();
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(reports[1]["pillar"], "security");

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/model-cost-inventory/pillars?pillar=durability")
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
