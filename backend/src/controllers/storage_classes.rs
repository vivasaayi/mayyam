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
use crate::services::kubernetes::storage_classes_service::StorageClassesService;
use actix_web::{web, HttpResponse, Responder};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::debug;

pub async fn list_storage_classes_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>, // cluster_id
    sc_service: web::Data<Arc<StorageClassesService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    debug!(target: "mayyam::controllers::storage_classes", user_id = %claims.username, %cluster_id, "Attempting to list StorageClasses");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let storage_classes = sc_service.list_storage_classes(&cluster_config).await?;
    Ok(HttpResponse::Ok().json(storage_classes))
}

pub async fn get_storage_class_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, sc_name)
    sc_service: web::Data<Arc<StorageClassesService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, sc_name) = path.into_inner();
    debug!(target: "mayyam::controllers::storage_classes", user_id = %claims.username, %cluster_id, %sc_name, "Attempting to get StorageClass details");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let details = sc_service.get_storage_class_details(&cluster_config, &sc_name).await?;
    Ok(HttpResponse::Ok().json(details))
}
