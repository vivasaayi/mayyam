use actix_web::{web, HttpResponse, Responder};
use sea_orm::{DatabaseConnection, EntityTrait};
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::cluster::KubernetesClusterConfig;
use crate::services::kubernetes::endpoints_service::EndpointsService;
use k8s_openapi::api::core::v1::Endpoints;

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

pub async fn list_endpoints_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    svc: web::Data<Arc<EndpointsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns) = path.into_inner();
    debug!(target: "mayyam::controllers::endpoints", user_id = %claims.username, %cluster_id, %ns, "List Endpoints");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let list = svc.list_endpoints(&cfg, &ns).await?;
    Ok(HttpResponse::Ok().json(list))
}

pub async fn list_endpoint_slices_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    svc: web::Data<Arc<EndpointsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns) = path.into_inner();
    debug!(target: "mayyam::controllers::endpoints", user_id = %claims.username, %cluster_id, %ns, "List EndpointSlices");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let list = svc.list_endpoint_slices(&cfg, &ns).await?;
    Ok(HttpResponse::Ok().json(list))
}

pub async fn get_endpoints_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>,
    svc: web::Data<Arc<EndpointsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns, name) = path.into_inner();
    debug!(target: "mayyam::controllers::endpoints", user_id = %claims.username, %cluster_id, %ns, %name, "Get Endpoints");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let item = svc.get_endpoints(&cfg, &ns, &name).await?;
    Ok(HttpResponse::Ok().json(item))
}

pub async fn upsert_endpoints_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>,
    body: web::Json<Endpoints>,
    svc: web::Data<Arc<EndpointsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns, name) = path.into_inner();
    let mut item = body.into_inner();
    if item.metadata.name.as_deref() != Some(&name) {
        item.metadata.name = Some(name.clone());
    }
    debug!(target: "mayyam::controllers::endpoints", user_id = %claims.username, %cluster_id, %ns, %name, "Upsert Endpoints");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let updated = svc.upsert_endpoints(&cfg, &ns, &item).await?;
    Ok(HttpResponse::Ok().json(updated))
}

pub async fn delete_endpoints_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>,
    svc: web::Data<Arc<EndpointsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, ns, name) = path.into_inner();
    debug!(target: "mayyam::controllers::endpoints", user_id = %claims.username, %cluster_id, %ns, %name, "Delete Endpoints");
    let cfg = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    svc.delete_endpoints(&cfg, &ns, &name).await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({"deleted": true})))
}
