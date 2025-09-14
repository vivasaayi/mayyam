use actix_web::{web, HttpResponse, Responder};
use sea_orm::{DatabaseConnection, EntityTrait};
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::authz_service::AuthorizationService;

#[derive(serde::Deserialize)]
pub struct AuthzRequest {
    pub namespace: Option<String>,
    pub verb: String,
    pub group: Option<String>,
    pub resource: String,
    pub subresource: Option<String>,
    pub name: Option<String>,
}

async fn get_cluster_config_by_id(db: &DatabaseConnection, cluster_id_str: &str) -> Result<KubernetesClusterConfig, AppError> {
    let cluster_id = Uuid::parse_str(cluster_id_str).map_err(|_| AppError::BadRequest("Invalid cluster ID format".to_string()))?;
    let cluster_model = crate::models::cluster::Entity::find_by_id(cluster_id)
        .one(db)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("Cluster with ID {} not found", cluster_id)))?;
    if cluster_model.cluster_type != "kubernetes" { return Err(AppError::BadRequest("Cluster is not a Kubernetes cluster".into())); }
    let value = cluster_model.config;
    if value.is_null() {
        Ok(KubernetesClusterConfig { api_server_url: None, token: None, kube_config_path: None, kube_context: None, certificate_authority_data: None, client_certificate_data: None, client_key_data: None })
    } else { serde_json::from_value(value).map_err(|e| AppError::Internal(format!("Failed to parse cluster config: {}", e))) }
}

pub async fn authz_can_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>,
    body: web::Json<AuthzRequest>,
    svc: web::Data<Arc<AuthorizationService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    let req = body.into_inner();
    debug!(target: "mayyam::controllers::authz", user_id = %claims.username, %cluster_id, verb = %req.verb, resource = %req.resource, "AuthZ can check");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let status = svc.can(
        &cfg,
        req.namespace,
        &req.verb,
        req.group.as_deref(),
        &req.resource,
        req.subresource.as_deref(),
        req.name.as_deref(),
    ).await?;
    Ok(HttpResponse::Ok().json(status))
}
