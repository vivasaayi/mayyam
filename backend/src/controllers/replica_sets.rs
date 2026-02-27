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

use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::controllers::kubernetes::get_cluster_config_by_id;
use crate::services::kubernetes::replica_sets_service::ReplicaSetsService;
use actix_web::{web, HttpResponse, Responder};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::debug;

pub async fn list_replica_sets_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    rs_service: web::Data<Arc<ReplicaSetsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    debug!(target: "mayyam::controllers::replica_sets", user_id = %claims.username, %cluster_id, %namespace_name, "Attempting to list ReplicaSets");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let replica_sets = rs_service.list_replica_sets(&cluster_config, &namespace_name).await?;
    Ok(HttpResponse::Ok().json(replica_sets))
}

pub async fn get_replica_set_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, rs_name)
    rs_service: web::Data<Arc<ReplicaSetsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, rs_name) = path.into_inner();
    debug!(target: "mayyam::controllers::replica_sets", user_id = %claims.username, %cluster_id, %namespace_name, %rs_name, "Attempting to get ReplicaSet details");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let details = rs_service.get_replica_set_details(&cluster_config, &namespace_name, &rs_name).await?;
    Ok(HttpResponse::Ok().json(details))
}
