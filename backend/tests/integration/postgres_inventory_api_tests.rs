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

#![cfg(feature = "integration-tests")]

use actix_web::{dev::Service as _, http::StatusCode, test, web, App, HttpMessage};
use mayyam::config::Config;
use mayyam::controllers::database::{
    get_postgres_pg_locks_inventory_pillar_reports,
    get_postgres_pg_stat_activity_inventory_pillar_reports,
    get_postgres_pg_stat_database_inventory_pillar_reports,
    get_postgres_pg_stat_io_inventory_pillar_reports,
    get_postgres_pg_stat_statements_inventory_pillar_reports,
    get_postgres_pg_stat_wal_inventory_pillar_reports,
};
use mayyam::middleware::auth::Claims;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;

#[tokio::test]
async fn postgres_pg_stat_activity_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(Config::default()))
            .route(
                "/api/databases/postgres/pg-stat-activity/pillars",
                web::get().to(get_postgres_pg_stat_activity_inventory_pillar_reports),
            )
            .route(
                "/api/databases/postgres/pg-stat-statements/pillars",
                web::get().to(get_postgres_pg_stat_statements_inventory_pillar_reports),
            )
            .route(
                "/api/databases/postgres/pg-stat-database/pillars",
                web::get().to(get_postgres_pg_stat_database_inventory_pillar_reports),
            )
            .route(
                "/api/databases/postgres/pg-stat-io/pillars",
                web::get().to(get_postgres_pg_stat_io_inventory_pillar_reports),
            )
            .route(
                "/api/databases/postgres/pg-stat-wal/pillars",
                web::get().to(get_postgres_pg_stat_wal_inventory_pillar_reports),
            )
            .route(
                "/api/databases/postgres/pg-locks/pillars",
                web::get().to(get_postgres_pg_locks_inventory_pillar_reports),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-activity/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "PostgresPgStatActivity");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert_eq!(body["resources_evaluated"], 0);
    assert_eq!(body["stale_resources"], 0);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["resources_evaluated"].is_number());
        assert!(report["stale_resources"].is_number());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-activity/pillars?pillar=cost,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-activity/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn postgres_pg_stat_database_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(Config::default()))
            .route(
                "/api/databases/postgres/pg-stat-database/pillars",
                web::get().to(get_postgres_pg_stat_database_inventory_pillar_reports),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-database/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "PostgresPgStatDatabase");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert_eq!(body["resources_evaluated"], 0);
    assert_eq!(body["stale_resources"], 0);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["resources_evaluated"].is_number());
        assert!(report["stale_resources"].is_number());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-database/pillars?pillar=resilience,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-database/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn postgres_pg_stat_io_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(Config::default()))
            .route(
                "/api/databases/postgres/pg-stat-io/pillars",
                web::get().to(get_postgres_pg_stat_io_inventory_pillar_reports),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-io/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "PostgresPgStatIo");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert_eq!(body["resources_evaluated"], 0);
    assert_eq!(body["stale_resources"], 0);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["resources_evaluated"].is_number());
        assert!(report["stale_resources"].is_number());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-io/pillars?pillar=cost,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-io/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn postgres_pg_stat_wal_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(Config::default()))
            .route(
                "/api/databases/postgres/pg-stat-wal/pillars",
                web::get().to(get_postgres_pg_stat_wal_inventory_pillar_reports),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-wal/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "PostgresPgStatWal");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert_eq!(body["resources_evaluated"], 0);
    assert_eq!(body["stale_resources"], 0);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["resources_evaluated"].is_number());
        assert!(report["stale_resources"].is_number());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-wal/pillars?pillar=cost,resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-wal/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn postgres_pg_locks_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(Config::default()))
            .route(
                "/api/databases/postgres/pg-locks/pillars",
                web::get().to(get_postgres_pg_locks_inventory_pillar_reports),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-locks/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "PostgresPgLocks");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert_eq!(body["resources_evaluated"], 0);
    assert_eq!(body["stale_resources"], 0);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["resources_evaluated"].is_number());
        assert!(report["stale_resources"].is_number());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-locks/pillars?pillar=resilience,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-locks/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn postgres_pg_stat_statements_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(Config::default()))
            .route(
                "/api/databases/postgres/pg-stat-statements/pillars",
                web::get().to(get_postgres_pg_stat_statements_inventory_pillar_reports),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-statements/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "PostgresPgStatStatements");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert_eq!(body["resources_evaluated"], 0);
    assert_eq!(body["stale_resources"], 0);
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["resources_evaluated"].is_number());
        assert!(report["stale_resources"].is_number());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-statements/pillars?pillar=cost,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/databases/postgres/pg-stat-statements/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
