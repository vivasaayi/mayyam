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


use actix_web::{web, HttpResponse, Responder};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::repositories::query_fingerprint_repository::QueryFingerprintRepository;

pub async fn get_top_offending_tables(
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
    _claims: web::ReqData<Claims>,
) -> Result<impl Responder, AppError> {
    let cluster_id = if path.as_str() == "all" {
        None
    } else {
        Some(Uuid::parse_str(&path).map_err(|e| AppError::BadRequest(format!("Invalid cluster UUID: {}", e)))?)
    };

    let hours = query.get("hours")
        .and_then(|h| h.parse::<i64>().ok())
        .unwrap_or(24);
    
    let limit = query.get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(20);

    let repo = QueryFingerprintRepository::new(db.get_ref().clone());
    
    let stats = repo.get_top_offending_tables(cluster_id, hours, limit).await
        .map_err(|e| AppError::InternalServerError(e))?;

    Ok(HttpResponse::Ok().json(stats))
}
