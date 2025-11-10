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


use crate::controllers::query_fingerprint;
use actix_web::{web, HttpResponse};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub fn configure(cfg: &mut web::ServiceConfig, db: Arc<DatabaseConnection>) {
    let fingerprint_controller = query_fingerprint::QueryFingerprintController::new(db.clone());

    cfg.service(
        web::scope("/api/query-fingerprints")
            .app_data(web::Data::new(fingerprint_controller))
            .service(
                web::resource("")
                    .route(web::get().to(query_fingerprint::get_fingerprints)),
            )
            .service(
                web::resource("/{id}")
                    .route(web::get().to(query_fingerprint::get_fingerprint)),
            )
            .service(
                web::resource("/{id}/analysis")
                    .route(web::get().to(query_fingerprint::analyze_fingerprint)),
            )
            .service(
                web::resource("/patterns/{cluster_id}")
                    .route(web::get().to(query_fingerprint::get_fingerprint_patterns)),
            ),
    );
}