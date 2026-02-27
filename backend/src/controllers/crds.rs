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
use crate::services::kubernetes::crds_service::CrdsService;
use actix_web::{web, HttpResponse, Responder};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::debug;
use serde::Deserialize;

pub async fn list_crds_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>, // cluster_id
    crds_service: web::Data<Arc<CrdsService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    debug!(target: "mayyam::controllers::crds", user_id = %claims.username, %cluster_id, "Attempting to list CRDs");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let crds = crds_service.list_crds(&cluster_config).await?;
    Ok(HttpResponse::Ok().json(crds))
}

pub async fn get_crd_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, crd_name)
    crds_service: web::Data<Arc<CrdsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, crd_name) = path.into_inner();
    debug!(target: "mayyam::controllers::crds", user_id = %claims.username, %cluster_id, %crd_name, "Attempting to get CRD details");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let details = crds_service.get_crd_details(&cluster_config, &crd_name).await?;
    Ok(HttpResponse::Ok().json(details))
}

#[derive(Deserialize)]
pub struct CustomResourceQuery {
    pub namespace: Option<String>,
}

pub async fn list_custom_resources_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String, String)>, // (cluster_id, group, version, plural)
    query: web::Query<CustomResourceQuery>,
    crds_service: web::Data<Arc<CrdsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, group, version, plural) = path.into_inner();
    let query = query.into_inner();
    let ns_ref = query.namespace.as_deref();
    
    debug!(
        target: "mayyam::controllers::crds",
        user_id = %claims.username,
        %cluster_id,
        %group,
        %version,
        %plural,
        namespace = ?ns_ref,
        "Attempting to list CustomResources"
    );
    
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let resources = crds_service.list_custom_resources(&cluster_config, &group, &version, &plural, ns_ref).await?;
    Ok(HttpResponse::Ok().json(resources))
}
