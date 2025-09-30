use actix_web::{web, HttpResponse, Responder};
use sea_orm::{DatabaseConnection, EntityTrait};
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::nodes_ops_service::NodeOpsService;

#[derive(serde::Deserialize)]
pub struct TaintRequest {
    pub key: String,
    pub value: String,
    pub effect: String,
}

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

pub async fn cordon_node_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    svc: web::Data<Arc<NodeOpsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, node) = path.into_inner();
    debug!(target: "mayyam::controllers::node_ops", user_id = %claims.username, %cluster_id, %node, "Cordon node");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let updated = svc.cordon(&cfg, &node).await?;
    Ok(HttpResponse::Ok().json(updated))
}

pub async fn uncordon_node_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    svc: web::Data<Arc<NodeOpsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, node) = path.into_inner();
    debug!(target: "mayyam::controllers::node_ops", user_id = %claims.username, %cluster_id, %node, "Uncordon node");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let updated = svc.uncordon(&cfg, &node).await?;
    Ok(HttpResponse::Ok().json(updated))
}

pub async fn add_taint_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    body: web::Json<TaintRequest>,
    svc: web::Data<Arc<NodeOpsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, node) = path.into_inner();
    let req = body.into_inner();
    debug!(target: "mayyam::controllers::node_ops", user_id = %claims.username, %cluster_id, %node, key=%req.key, effect=%req.effect, "Add taint");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let updated = svc
        .add_taint(&cfg, &node, &req.key, &req.value, &req.effect)
        .await?;
    Ok(HttpResponse::Ok().json(updated))
}

#[derive(serde::Deserialize)]
pub struct RemoveTaintRequest {
    pub key: String,
}

pub async fn remove_taint_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    body: web::Json<RemoveTaintRequest>,
    svc: web::Data<Arc<NodeOpsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, node) = path.into_inner();
    let req = body.into_inner();
    debug!(target: "mayyam::controllers::node_ops", user_id = %claims.username, %cluster_id, %node, key=%req.key, "Remove taint");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let updated = svc.remove_taint(&cfg, &node, &req.key).await?;
    Ok(HttpResponse::Ok().json(updated))
}
