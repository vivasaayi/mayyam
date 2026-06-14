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
    get_mysql_aurora_inventory_pillar_reports, get_mysql_binary_log_inventory_pillar_reports,
    get_mysql_connection_threads_inventory_pillar_reports,
    get_mysql_deadlocks_inventory_pillar_reports,
    get_mysql_digest_statistics_inventory_pillar_reports,
    get_mysql_group_replication_inventory_pillar_reports,
    get_mysql_index_cardinality_inventory_pillar_reports,
    get_mysql_innodb_buffer_pool_inventory_pillar_reports,
    get_mysql_metadata_locks_inventory_pillar_reports,
    get_mysql_performance_schema_inventory_pillar_reports, get_mysql_rds_inventory_pillar_reports,
    get_mysql_redo_log_inventory_pillar_reports,
    get_mysql_replication_status_inventory_pillar_reports,
    get_mysql_slow_query_log_inventory_pillar_reports,
    get_mysql_sys_schema_inventory_pillar_reports, get_mysql_undo_log_inventory_pillar_reports,
    get_mysql_wait_events_inventory_pillar_reports,
};
use mayyam::middleware::auth::Claims;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;

#[tokio::test]
async fn mysql_performance_schema_inventory_pillar_reports_contract() {
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
                "/api/databases/mysql/performance-schema/pillars",
                web::get().to(get_mysql_performance_schema_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/sys-schema/pillars",
                web::get().to(get_mysql_sys_schema_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/slow-query-log/pillars",
                web::get().to(get_mysql_slow_query_log_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/digest-statistics/pillars",
                web::get().to(get_mysql_digest_statistics_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/innodb-buffer-pool/pillars",
                web::get().to(get_mysql_innodb_buffer_pool_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/binary-log/pillars",
                web::get().to(get_mysql_binary_log_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/replication-status/pillars",
                web::get().to(get_mysql_replication_status_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/group-replication/pillars",
                web::get().to(get_mysql_group_replication_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/aurora-mysql/pillars",
                web::get().to(get_mysql_aurora_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/rds-mysql/pillars",
                web::get().to(get_mysql_rds_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/connection-threads/pillars",
                web::get().to(get_mysql_connection_threads_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/metadata-locks/pillars",
                web::get().to(get_mysql_metadata_locks_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/deadlocks/pillars",
                web::get().to(get_mysql_deadlocks_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/index-cardinality/pillars",
                web::get().to(get_mysql_index_cardinality_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/redo-log/pillars",
                web::get().to(get_mysql_redo_log_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/undo-log/pillars",
                web::get().to(get_mysql_undo_log_inventory_pillar_reports),
            )
            .route(
                "/api/databases/mysql/wait-events/pillars",
                web::get().to(get_mysql_wait_events_inventory_pillar_reports),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/performance-schema/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlPerformanceSchema");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/performance-schema/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/performance-schema/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/sys-schema/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlSysSchema");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/sys-schema/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/slow-query-log/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlSlowQueryLog");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/slow-query-log/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/digest-statistics/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlDigestStatistics");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/digest-statistics/pillars?pillar=cost,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/innodb-buffer-pool/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlInnoDbBufferPool");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/innodb-buffer-pool/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/binary-log/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlBinaryLog");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/binary-log/pillars?pillar=cost,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/replication-status/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlReplicationStatus");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/replication-status/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/group-replication/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlGroupReplication");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/group-replication/pillars?pillar=cost,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/aurora-mysql/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "AuroraMySql");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/aurora-mysql/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/rds-mysql/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "RdsMySql");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/rds-mysql/pillars?pillar=cost,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/connection-threads/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlConnectionThreads");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/connection-threads/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/metadata-locks/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlMetadataLocks");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/metadata-locks/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/deadlocks/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlDeadlocks");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/deadlocks/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/index-cardinality/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlIndexCardinality");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/index-cardinality/pillars?pillar=cost,resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/redo-log/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlRedoLog");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/redo-log/pillars?pillar=cost,security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[1]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/undo-log/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlUndoLog");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/undo-log/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/wait-events/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlWaitEvents");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/wait-events/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");
}
