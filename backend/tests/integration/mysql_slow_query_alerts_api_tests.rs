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
use mayyam::controllers::database_alerts::get_mysql_slow_query_alerts;
use mayyam::middleware::auth::Claims;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;

#[tokio::test]
async fn mysql_slow_query_alerts_contract() {
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
                "/api/databases/mysql/slow-query-alerts",
                web::get().to(get_mysql_slow_query_alerts),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/databases/mysql/slow-query-alerts")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "MySqlSlowQueryAlert");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert_eq!(body["resources_evaluated"], 0);
    assert_eq!(body["active_alerts"], 0);
    assert_eq!(body["insufficient_data_alerts"], 0);
    assert_eq!(body["alerts"].as_array().map(Vec::len), Some(0));
    assert_eq!(body["notification_workflow"]["status"], "healthy");
    assert_eq!(body["health_workflow"]["status"], "healthy");
    assert_eq!(body["reporting"]["total_alerts"], 0);
}
