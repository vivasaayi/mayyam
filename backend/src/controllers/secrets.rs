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
use sea_orm::{DatabaseConnection, EntityTrait};
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::secrets_service::SecretsService;

async fn get_cluster_config_by_id(
    db: &DatabaseConnection,
    cluster_id_str: &str,
) -> Result<KubernetesClusterConfig, AppError> {
    let cluster_id = Uuid::parse_str(cluster_id_str)
        .map_err(|_| AppError::BadRequest("Invalid cluster ID format".to_string()))?;
    let cluster_model = crate::models::cluster::Entity::find_by_id(cluster_id)
        .one(db)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("Cluster with ID {} not found", cluster_id)))?;
    if cluster_model.cluster_type != "kubernetes" {
        return Err(AppError::BadRequest(
            "Cluster is not a Kubernetes cluster".to_string(),
        ));
    }
    let value = cluster_model.config;
    if value.is_null() {
        Ok(KubernetesClusterConfig {
            api_server_url: None,
            token: None,
            kube_config_path: None,
            kube_context: None,
            certificate_authority_data: None,
            client_certificate_data: None,
            client_key_data: None,
        })
    } else {
        serde_json::from_value(value)
            .map_err(|e| AppError::Internal(format!("Failed to parse cluster config: {}", e)))
    }
}

#[derive(serde::Deserialize)]
pub struct UpsertSecretRequest {
    pub type_field: Option<String>,
    pub data: std::collections::BTreeMap<String, String>,
    pub labels: Option<std::collections::BTreeMap<String, String>>,
    pub annotations: Option<std::collections::BTreeMap<String, String>>,
}

pub async fn list_secrets_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace)
    query: web::Query<std::collections::HashMap<String, String>>, // labelSelector, fieldSelector, limit, continue
    svc: web::Data<Arc<SecretsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns) = path.into_inner();
    debug!(target: "mayyam::controllers::secrets", user_id = %claims.username, %cluster_id, %ns, "List Secrets");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let label_selector = query.get("labelSelector").cloned();
    let field_selector = query.get("fieldSelector").cloned();
    let limit = query.get("limit").and_then(|s| s.parse::<u32>().ok());
    let continue_token = query.get("continue").cloned();
    let list = svc
        .list(
            &cfg,
            &ns,
            label_selector,
            field_selector,
            limit,
            continue_token,
        )
        .await?;
    Ok(HttpResponse::Ok().json(list))
}

pub async fn get_secret_redacted_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace, name)
    svc: web::Data<Arc<SecretsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns, name) = path.into_inner();
    debug!(target: "mayyam::controllers::secrets", user_id = %claims.username, %cluster_id, %ns, %name, "Get Secret redacted");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let s = svc.get_redacted(&cfg, &ns, &name).await?;
    Ok(HttpResponse::Ok().json(s))
}

pub async fn upsert_secret_plaintext_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace, name)
    body: web::Json<UpsertSecretRequest>,
    svc: web::Data<Arc<SecretsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns, name) = path.into_inner();
    let req = body.into_inner();
    debug!(target: "mayyam::controllers::secrets", user_id = %claims.username, %cluster_id, %ns, %name, "Upsert Secret plaintext");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let s = svc
        .upsert_plaintext(
            &cfg,
            &ns,
            &name,
            req.type_field,
            req.data,
            req.labels,
            req.annotations,
        )
        .await?;
    Ok(HttpResponse::Ok().json(s))
}

pub async fn delete_secret_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace, name)
    svc: web::Data<Arc<SecretsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns, name) = path.into_inner();
    debug!(target: "mayyam::controllers::secrets", user_id = %claims.username, %cluster_id, %ns, %name, "Delete Secret");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    svc.delete(&cfg, &ns, &name).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"deleted": true})))
}
