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
use mayyam::controllers::prompt_template::PromptTemplateController;
use mayyam::models::prompt_template::{
    CreatePromptTemplateDto, PromptCategory, PromptStatus, PromptType,
};
use mayyam::repositories::prompt_template::PromptTemplateRepository;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

#[actix_web::test]
async fn llm_prompt_inventory_pillar_reports_contract() {
    let db = Database::connect("sqlite::memory:?cache=shared")
        .await
        .expect("connect sqlite");
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

    let repo = Arc::new(PromptTemplateRepository::new(db));
    repo.create(CreatePromptTemplateDto {
        name: "incident-summary".to_string(),
        category: PromptCategory::General,
        resource_type: Some("AI".to_string()),
        workflow_type: Some("Triage".to_string()),
        prompt_template:
            "Use evidence, redact secrets, require approval for mutations, max_tokens 800"
                .to_string(),
        variables: vec![],
        version: Some("1.0".to_string()),
        is_active: Some(true),
        is_system: Some(true),
        description: Some("Incident summary prompt".to_string()),
        tags: vec!["cost-center=ai-platform".to_string()],
        prompt_type: Some(PromptType::Analysis),
        template_content: None,
        is_system_prompt: Some(true),
        parent_id: None,
        status: Some(PromptStatus::Active),
        usage_count: None,
        last_used_at: None,
    })
    .await
    .expect("create healthy prompt");
    repo.create(CreatePromptTemplateDto {
        name: "unsafe-summary".to_string(),
        category: PromptCategory::General,
        resource_type: Some("AI".to_string()),
        workflow_type: Some("Triage".to_string()),
        prompt_template: "Summarize this issue".to_string(),
        variables: vec![],
        version: Some("1.0".to_string()),
        is_active: Some(false),
        is_system: Some(true),
        description: None,
        tags: vec![],
        prompt_type: Some(PromptType::Analysis),
        template_content: None,
        is_system_prompt: Some(true),
        parent_id: None,
        status: Some(PromptStatus::Inactive),
        usage_count: None,
        last_used_at: None,
    })
    .await
    .expect("create incomplete prompt");
    let controller = Arc::new(PromptTemplateController::new(repo));
    let app = test::init_service(App::new().app_data(web::Data::from(controller)).route(
        "/api/v1/prompt-templates/inventory/pillars",
        web::get().to(PromptTemplateController::prompt_inventory_pillar_reports),
    ))
    .await;

    let req = test::TestRequest::get()
        .uri("/api/v1/prompt-templates/inventory/pillars")
        .to_request();
    let response = test::call_service(&app, req).await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "body: {}",
        String::from_utf8_lossy(&bytes)
    );
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    assert_eq!(body["resource_type"], "AiLlmPromptTemplate");
    assert_eq!(body["resources_evaluated"].as_u64().unwrap_or(0), 2);
    assert_eq!(body["reports"].as_array().unwrap().len(), 3);

    let req = test::TestRequest::get()
        .uri("/api/v1/prompt-templates/inventory/pillars?pillar=resilience,security")
        .to_request();
    let body: serde_json::Value = test::call_and_read_body_json(&app, req).await;
    let reports = body["reports"].as_array().unwrap();
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(reports[1]["pillar"], "security");

    let req = test::TestRequest::get()
        .uri("/api/v1/prompt-templates/inventory/pillars?pillar=latency")
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
