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
use mayyam::controllers::llm_model::LlmModelController;
use mayyam::repositories::llm_model::LlmProviderModelRepository;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use serde_json::json;
use uuid::Uuid;

#[actix_web::test]
async fn llm_ai_spend_report_contract() {
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
            "monthly_token_budget": 100000,
            "max_tokens_per_request": 8000,
            "rate_limit_tokens_per_minute": 60000,
            "burst_guardrail": "throttle large prompts",
            "audit_enabled": true,
            "cost_redaction_policy": "redact prompt and response text",
            "usage_redaction_policy": "redact prompt and response text"
        }),
        true,
    )
    .await
    .expect("create healthy spend route");
    repo.create(
        Uuid::new_v4(),
        "expensive-model".to_string(),
        json!({
            "estimated_monthly_cost_usd": 1200.0,
            "total_tokens": 120000
        }),
        true,
    )
    .await
    .expect("create incomplete spend route");

    let controller = Arc::new(LlmModelController::new(repo));
    let app = test::init_service(App::new().app_data(web::Data::from(controller)).route(
        "/api/v1/llm-providers/ai-spend-report",
        web::get().to(LlmModelController::ai_spend_report),
    ))
    .await;

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/ai-spend-report")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;

    assert_eq!(body["workflow_id"], "ai_llm_spend_reporting");
    assert_eq!(body["saved_view_id"], "ai-llm-spend-report");
    assert_eq!(body["resources_evaluated"].as_u64().unwrap_or(0), 2);
    assert_eq!(
        body["model_cost_resources_evaluated"].as_u64().unwrap_or(0),
        2
    );
    assert_eq!(
        body["token_usage_resources_evaluated"]
            .as_u64()
            .unwrap_or(0),
        2
    );
    assert_eq!(
        body["scheduled_delivery_state"],
        "ready_with_cost_evidence_gaps"
    );
    assert_eq!(
        body["executive_summary"]["total_estimated_monthly_cost_usd"]
            .as_f64()
            .unwrap_or(0.0),
        1200.08
    );
    assert_eq!(
        body["executive_summary"]["total_tokens"]
            .as_u64()
            .unwrap_or(0),
        180000
    );
    assert!(body["missing_data_reason_codes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "AI_LLM_MODEL_COST_PRICE_MISSING"));
    assert!(body["engineering_backlog"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value["source_inventory"] == "model_cost_inventory"));
    assert!(body["engineering_backlog"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value["source_inventory"] == "token_usage_inventory"));
}
