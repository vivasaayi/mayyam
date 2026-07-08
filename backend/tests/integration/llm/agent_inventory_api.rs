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
use mayyam::models::prompt_template::{
    CreatePromptTemplateDto, PromptCategory, PromptStatus, PromptType,
};
use mayyam::repositories::llm_model::LlmProviderModelRepository;
use mayyam::repositories::prompt_template::PromptTemplateRepository;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use serde_json::json;
use uuid::Uuid;

#[actix_web::test]
async fn llm_agent_inventory_pillar_reports_contract() {
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
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        r#"
        CREATE TABLE prompt_templates (
            id BLOB PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            category TEXT NOT NULL,
            resource_type TEXT NULL,
            workflow_type TEXT NULL,
            prompt_template TEXT NOT NULL,
            variables TEXT NOT NULL,
            version TEXT NOT NULL,
            is_active BOOLEAN NOT NULL,
            is_system BOOLEAN NOT NULL,
            description TEXT NULL,
            tags TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            created_by BLOB NULL
        )
        "#
        .to_string(),
    ))
    .await
    .expect("create prompt_templates");

    let model_repo = Arc::new(LlmProviderModelRepository::new(Arc::new(db.clone())));
    let prompt_repo = Arc::new(PromptTemplateRepository::new(db));
    prompt_repo
        .create(CreatePromptTemplateDto {
            name: "agent-investigation".to_string(),
            category: PromptCategory::General,
            resource_type: Some("AI".to_string()),
            workflow_type: Some("Agentic Investigation".to_string()),
            prompt_template:
                "Use evidence, read-only tools first, audit every tool call, require approval"
                    .to_string(),
            variables: vec![],
            version: Some("1.0".to_string()),
            is_active: Some(true),
            is_system: Some(true),
            description: Some("Agent investigation prompt".to_string()),
            tags: vec!["agent".to_string(), "cost-center=ai-platform".to_string()],
            prompt_type: Some(PromptType::Analysis),
            template_content: None,
            is_system_prompt: Some(true),
            parent_id: None,
            status: Some(PromptStatus::Active),
            usage_count: None,
            last_used_at: None,
        })
        .await
        .expect("create agent prompt");
    model_repo
        .create(
            Uuid::new_v4(),
            "deepseek-agent".to_string(),
            json!({
                "owner": "sre-ai",
                "labels": ["cost-center=ai-platform"],
                "agent_monthly_budget_usd": 1200.0,
                "max_tokens_per_run": 8000,
                "timeout_ms": 30000,
                "max_iterations": 8,
                "allowed_tools": ["kafka.read", "kubernetes.read"],
                "provider_failover_policy": "fail over to approved standby provider after dry-run health check",
                "tool_policy": "read-only diagnostics by default",
                "approval_policy": "approval required for mutations",
                "model_routing_policy": "route regulated workloads to approved provider set",
                "audit_enabled": true
            }),
            true,
        )
        .await
        .expect("create healthy agent route");
    model_repo
        .create(
            Uuid::new_v4(),
            "unsafe-agent".to_string(),
            json!({"temperature": 0.2}),
            false,
        )
        .await
        .expect("create incomplete agent route");

    let controller = Arc::new(LlmModelController::with_prompt_template_repository(
        model_repo,
        prompt_repo,
    ));
    let app = test::init_service(App::new().app_data(web::Data::from(controller)).route(
        "/api/v1/llm-providers/agent-inventory/pillars",
        web::get().to(LlmModelController::agent_inventory_pillar_reports),
    ))
    .await;

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/agent-inventory/pillars")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["resource_type"], "AiLlmAgent");
    assert_eq!(body["resources_evaluated"].as_u64().unwrap_or(0), 2);
    assert_eq!(body["reports"].as_array().unwrap().len(), 3);
    assert_eq!(
        body["control_workflow"]["workflow_id"],
        "ai_llm_agent_budget_stop_control_workflow"
    );
    assert_eq!(
        body["control_workflow"]["approval_gate_permission"],
        "ai.llm.agent.controls.approve"
    );
    assert_eq!(
        body["governance_workflow"]["workflow_id"],
        "ai_llm_agent_governance_workflow"
    );
    assert_eq!(
        body["governance_workflow"]["approval_gate_permission"],
        "ai.llm.agent.governance.approve"
    );
    assert_eq!(body["governance_workflow"]["read_only_mode"], true);
    assert_eq!(body["control_workflow"]["read_only_mode"], true);
    assert_eq!(body["control_workflow"]["dry_run_only"], true);
    let actions = body["control_workflow"]["actions"].as_array().unwrap();
    assert_eq!(actions.len(), 2);
    assert!(actions.iter().all(|action| action["dry_run"] == true));
    assert!(actions
        .iter()
        .any(|action| action["status"] == "ready_for_approval"));
    let blocked_action = actions
        .iter()
        .find(|action| action["model_name"] == "unsafe-agent")
        .expect("unsafe agent workflow action");
    assert_eq!(blocked_action["status"], "blocked_missing_evidence");
    assert!(blocked_action["required_evidence"]
        .as_array()
        .unwrap()
        .contains(&json!("monthly_budget_usd")));
    assert!(blocked_action["required_evidence"]
        .as_array()
        .unwrap()
        .contains(&json!("max_iterations_or_stop_condition")));
    let blocked_governance_action = body["governance_workflow"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["model_name"] == "unsafe-agent")
        .expect("unsafe governance action");
    assert_eq!(
        blocked_governance_action["status"],
        "blocked_missing_evidence"
    );
    assert!(blocked_governance_action["required_evidence"]
        .as_array()
        .unwrap()
        .contains(&json!("approval_policy")));
    assert!(blocked_governance_action["required_evidence"]
        .as_array()
        .unwrap()
        .contains(&json!("model_routing_policy")));
    assert!(blocked_governance_action["required_evidence"]
        .as_array()
        .unwrap()
        .contains(&json!("provider_failover_policy")));

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/agent-inventory/pillars?pillar=resilience,security")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    let reports = body["reports"].as_array().unwrap();
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(reports[1]["pillar"], "security");

    let req = test::TestRequest::get()
        .uri("/api/v1/llm-providers/agent-inventory/pillars?pillar=latency")
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
