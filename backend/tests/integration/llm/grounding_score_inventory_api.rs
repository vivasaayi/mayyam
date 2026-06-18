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
async fn llm_grounding_score_inventory_pillar_reports_contract() {
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
        "rag-answer-model".to_string(),
        json!({
            "owner": "sre-ai",
            "labels": ["cost-center=ai-platform"],
            "grounding_score": 0.91,
            "minimum_grounding_score": 0.85,
            "citation_coverage": 0.96,
            "grounding_sample_count": 900,
            "grounding_evidence_source_count": 12,
            "grounding_scorer_version": "grounding-rubric-2026-06",
            "grounding_scoring_window": "rolling 7d",
            "grounding_score_cost_usd": 70.0,
            "grounding_score_budget_usd": 120.0,
            "grounding_audit_enabled": true,
            "grounding_source_access_policy": "approved retrieval indexes only",
            "grounding_evidence_retention_policy": "retain cited evidence hashes for 30d"
        }),
        true,
    )
    .await
    .expect("create healthy grounding score route");
    repo.create(
        Uuid::new_v4(),
        "ungrounded-answer-model".to_string(),
        json!({"grounding_score_cost_usd": 300.0}),
        true,
    )
    .await
    .expect("create incomplete grounding score route");

    let controller = Arc::new(LlmModelController::new(repo));
    let app = test::init_service(App::new().app_data(web::Data::from(controller)).route(
        "/api/v1/llm-providers/grounding-score-inventory/pillars",
        web::get().to(LlmModelController::grounding_score_inventory_pillar_reports),
    ))
    .await;

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/grounding-score-inventory/pillars")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["resource_type"], "AiLlmGroundingScore");
    assert_eq!(body["resources_evaluated"].as_u64().unwrap_or(0), 2);
    assert_eq!(body["reports"].as_array().unwrap().len(), 3);

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/grounding-score-inventory/pillars?pillar=resilience,security")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    let reports = body["reports"].as_array().unwrap();
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(reports[1]["pillar"], "security");

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/grounding-score-inventory/pillars?pillar=durability")
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
