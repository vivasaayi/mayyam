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
use crate::services::kubernetes::rbac_service::RbacService;
use k8s_openapi::api::rbac::v1::{ClusterRole, ClusterRoleBinding, Role, RoleBinding};

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
            "Cluster is not a Kubernetes cluster".into(),
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

// Roles
pub async fn list_roles_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns) = path.into_inner();
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, %ns, "List Roles");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let list = svc.list_roles(&cfg, &ns).await?;
    Ok(HttpResponse::Ok().json(list))
}

pub async fn get_role_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns, name) = path.into_inner();
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, %ns, %name, "Get Role");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let item = svc.get_role(&cfg, &ns, &name).await?;
    Ok(HttpResponse::Ok().json(item))
}

pub async fn upsert_role_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>,
    body: web::Json<Role>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns, name) = path.into_inner();
    let mut item = body.into_inner();
    if item.metadata.name.as_deref() != Some(&name) {
        item.metadata.name = Some(name.clone());
    }
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, %ns, %name, "Upsert Role");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let updated = svc.upsert_role(&cfg, &ns, &item).await?;
    Ok(HttpResponse::Ok().json(updated))
}

pub async fn delete_role_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns, name) = path.into_inner();
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, %ns, %name, "Delete Role");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    svc.delete_role(&cfg, &ns, &name).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"deleted": true})))
}

// RoleBindings
pub async fn list_role_bindings_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns) = path.into_inner();
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, %ns, "List RoleBindings");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let list = svc.list_role_bindings(&cfg, &ns).await?;
    Ok(HttpResponse::Ok().json(list))
}

pub async fn get_role_binding_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns, name) = path.into_inner();
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, %ns, %name, "Get RoleBinding");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let item = svc.get_role_binding(&cfg, &ns, &name).await?;
    Ok(HttpResponse::Ok().json(item))
}

pub async fn upsert_role_binding_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>,
    body: web::Json<RoleBinding>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns, name) = path.into_inner();
    let mut item = body.into_inner();
    if item.metadata.name.as_deref() != Some(&name) {
        item.metadata.name = Some(name.clone());
    }
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, %ns, %name, "Upsert RoleBinding");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let updated = svc.upsert_role_binding(&cfg, &ns, &item).await?;
    Ok(HttpResponse::Ok().json(updated))
}

pub async fn delete_role_binding_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns, name) = path.into_inner();
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, %ns, %name, "Delete RoleBinding");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    svc.delete_role_binding(&cfg, &ns, &name).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"deleted": true})))
}

// ClusterRoles
pub async fn list_cluster_roles_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, "List ClusterRoles");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let list = svc.list_cluster_roles(&cfg).await?;
    Ok(HttpResponse::Ok().json(list))
}

pub async fn get_cluster_role_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, name) = path.into_inner();
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, %name, "Get ClusterRole");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let item = svc.get_cluster_role(&cfg, &name).await?;
    Ok(HttpResponse::Ok().json(item))
}

pub async fn upsert_cluster_role_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    body: web::Json<ClusterRole>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, name) = path.into_inner();
    let mut item = body.into_inner();
    if item.metadata.name.as_deref() != Some(&name) {
        item.metadata.name = Some(name.clone());
    }
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, %name, "Upsert ClusterRole");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let updated = svc.upsert_cluster_role(&cfg, &item).await?;
    Ok(HttpResponse::Ok().json(updated))
}

pub async fn delete_cluster_role_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, name) = path.into_inner();
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, %name, "Delete ClusterRole");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    svc.delete_cluster_role(&cfg, &name).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"deleted": true})))
}

// ClusterRoleBindings
pub async fn list_cluster_role_bindings_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, "List ClusterRoleBindings");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let list = svc.list_cluster_role_bindings(&cfg).await?;
    Ok(HttpResponse::Ok().json(list))
}

pub async fn get_cluster_role_binding_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, name) = path.into_inner();
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, %name, "Get ClusterRoleBinding");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let item = svc.get_cluster_role_binding(&cfg, &name).await?;
    Ok(HttpResponse::Ok().json(item))
}

pub async fn upsert_cluster_role_binding_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    body: web::Json<ClusterRoleBinding>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, name) = path.into_inner();
    let mut item = body.into_inner();
    if item.metadata.name.as_deref() != Some(&name) {
        item.metadata.name = Some(name.clone());
    }
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, %name, "Upsert ClusterRoleBinding");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let updated = svc.upsert_cluster_role_binding(&cfg, &item).await?;
    Ok(HttpResponse::Ok().json(updated))
}

pub async fn delete_cluster_role_binding_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    svc: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, name) = path.into_inner();
    debug!(target: "mayyam::controllers::rbac", user_id = %claims.username, %cluster_id, %name, "Delete ClusterRoleBinding");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    svc.delete_cluster_role_binding(&cfg, &name).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"deleted": true})))
}
