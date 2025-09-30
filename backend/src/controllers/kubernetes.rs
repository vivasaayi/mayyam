use crate::errors::AppError;
use crate::middleware::auth::Claims; // Assuming you have auth middleware
use crate::models::cluster::{CreateKubernetesClusterRequest, KubernetesClusterConfig};
use crate::services::kubernetes::prelude::*;
use actix_web::{web, HttpResponse, Responder};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

// Helper function to get cluster config (you'll need to implement this based on your DB structure)
// This is a simplified example. You'd typically fetch this from a database.
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

    // serde_json::from_value(cluster_model.config).map_err(|e| AppError::Internal(format!("Failed to parse cluster config: {}", e)))
    let config_value = cluster_model.config; // This is serde_json::Value
    if config_value.is_null() {
        // If config from DB is NULL (represented as serde_json::Value::Null),
        // return a default KubernetesClusterConfig with all fields as None.
        // This assumes that a NULL config means no specific overrides are set.
        debug!(target: "mayyam::controllers::kubernetes", cluster_id = %cluster_id_str, "Cluster config is NULL in DB, returning default empty config.");
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
        serde_json::from_value(config_value.clone()).map_err(|e| {
            debug!(
                target: "mayyam::controllers::kubernetes",
                cluster_id = %cluster_id_str,
                error = %e.to_string(), // Log the error message
                config_json = %config_value.to_string(),
                "Failed to parse cluster config from JSON"
            );
            AppError::Internal(format!("Failed to parse cluster config: {}", e))
        })
    }
}

// === Cluster Management Controllers ===
pub async fn list_clusters_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
) -> Result<impl Responder, AppError> {
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, "Attempting to list clusters");
    let clusters = crate::models::cluster::Entity::find()
        .filter(crate::models::cluster::Column::ClusterType.eq("kubernetes"))
        .all(db.get_ref().as_ref())
        .await
        .map_err(AppError::Database)?;
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, count = clusters.len(), "Successfully listed clusters");
    Ok(HttpResponse::Ok().json(clusters))
}

pub async fn create_cluster_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    req: web::Json<CreateKubernetesClusterRequest>,
    user_id: web::ReqData<Uuid>, // Assuming user_id is extracted from claims by auth middleware
) -> Result<impl Responder, AppError> {
    let new_cluster_info = req.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, cluster_name = %new_cluster_info.name, "Attempting to create cluster");
    let cluster_config = KubernetesClusterConfig {
        kube_config_path: new_cluster_info.kube_config_path,
        kube_context: new_cluster_info.kube_context,
        api_server_url: new_cluster_info.api_server_url,
        certificate_authority_data: new_cluster_info.certificate_authority_data,
        client_certificate_data: new_cluster_info.client_certificate_data,
        client_key_data: new_cluster_info.client_key_data,
        token: new_cluster_info.token,
    };

    let new_cluster = crate::models::cluster::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(new_cluster_info.name),
        cluster_type: Set("kubernetes".to_string()),
        config: Set(serde_json::to_value(cluster_config).map_err(|e| {
            AppError::Internal(format!("Failed to serialize cluster config: {}", e))
        })?),
        created_by: Set(user_id.into_inner()),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
        ..Default::default()
    };

    let saved_cluster = new_cluster
        .insert(db.get_ref().as_ref())
        .await
        .map_err(AppError::Database)?;
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, cluster_id = %saved_cluster.id, "Successfully created cluster");
    Ok(HttpResponse::Created().json(saved_cluster))
}

pub async fn get_cluster_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>,
) -> Result<impl Responder, AppError> {
    let cluster_id_str = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, cluster_id = %cluster_id_str, "Attempting to get cluster details");
    let cluster_model = crate::models::cluster::Entity::find_by_id(
        Uuid::parse_str(&cluster_id_str)
            .map_err(|_| AppError::BadRequest("Invalid cluster ID".to_string()))?,
    )
    .one(db.get_ref().as_ref())
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("Cluster with ID {} not found", cluster_id_str)))?;
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, cluster_id = %cluster_id_str, "Successfully retrieved cluster details");
    Ok(HttpResponse::Ok().json(cluster_model))
}

pub async fn update_cluster_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>,
    req: web::Json<CreateKubernetesClusterRequest>, // Using same request for update simplicity
) -> Result<impl Responder, AppError> {
    let cluster_id_str = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, cluster_id = %cluster_id_str, "Attempting to update cluster");
    let cluster_id = Uuid::parse_str(&cluster_id_str)
        .map_err(|_| AppError::BadRequest("Invalid cluster ID format".to_string()))?;
    let update_data = req.into_inner();

    let mut active_cluster: crate::models::cluster::ActiveModel =
        crate::models::cluster::Entity::find_by_id(cluster_id)
            .one(db.get_ref().as_ref())
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "Cluster with ID {} not found for update",
                    cluster_id
                ))
            })?
            .into();

    let cluster_config = KubernetesClusterConfig {
        kube_config_path: update_data.kube_config_path,
        kube_context: update_data.kube_context,
        api_server_url: update_data.api_server_url,
        certificate_authority_data: update_data.certificate_authority_data,
        client_certificate_data: update_data.client_certificate_data,
        client_key_data: update_data.client_key_data,
        token: update_data.token,
    };

    active_cluster.name = Set(update_data.name);
    active_cluster.config = Set(serde_json::to_value(cluster_config)
        .map_err(|e| AppError::Internal(format!("Failed to serialize cluster config: {}", e)))?);
    active_cluster.updated_at = Set(chrono::Utc::now());

    let updated_cluster = active_cluster
        .update(db.get_ref().as_ref())
        .await
        .map_err(AppError::Database)?;
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, cluster_id = %updated_cluster.id, "Successfully updated cluster");
    Ok(HttpResponse::Ok().json(updated_cluster))
}

pub async fn delete_cluster_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>,
) -> Result<impl Responder, AppError> {
    let cluster_id_str = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, cluster_id = %cluster_id_str, "Attempting to delete cluster");
    let cluster_id = Uuid::parse_str(&cluster_id_str)
        .map_err(|_| AppError::BadRequest("Invalid cluster ID format".to_string()))?;

    let delete_result = crate::models::cluster::Entity::delete_by_id(cluster_id)
        .exec(db.get_ref().as_ref())
        .await
        .map_err(AppError::Database)?;

    if delete_result.rows_affected == 0 {
        return Err(AppError::NotFound(format!(
            "Cluster with ID {} not found for deletion",
            cluster_id
        )));
    }
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, cluster_id = %cluster_id_str, "Successfully deleted cluster");
    Ok(HttpResponse::Ok().json(serde_json::json!({ "message": "Cluster deleted successfully" })))
}

// === Kubernetes Resource Controllers ===

pub async fn list_namespaces_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>, // cluster_id
    namespaces_service: web::Data<Arc<NamespacesService>>,
) -> Result<impl Responder, AppError> {
    let original_cluster_id = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, cluster_id = %original_cluster_id, "Attempting to list namespaces");

    let cluster_config =
        get_cluster_config_by_id(db.get_ref().as_ref(), &original_cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", cluster_id = %original_cluster_id, "Successfully retrieved cluster config for listing namespaces");

    let namespaces = namespaces_service.list_namespaces(&cluster_config).await?;
    debug!(target: "mayyam::controllers::kubernetes", cluster_id = %original_cluster_id, count = namespaces.len(), "Successfully listed namespaces");

    Ok(HttpResponse::Ok().json(namespaces))
}

pub async fn get_namespace_details_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    namespaces_service: web::Data<Arc<NamespacesService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, "Attempting to get namespace details");

    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, "Successfully retrieved cluster config for namespace details");

    let namespace_details = namespaces_service
        .get_namespace_details(&cluster_config, &namespace_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, "Successfully retrieved namespace details");

    Ok(HttpResponse::Ok().json(namespace_details))
}

pub async fn list_nodes_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>, // cluster_id
    nodes_service: web::Data<Arc<NodesService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, "Attempting to list nodes");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, "Successfully retrieved cluster config for listing nodes");
    let nodes = nodes_service.list_nodes(&cluster_config).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, count = nodes.len(), "Successfully listed nodes");
    Ok(HttpResponse::Ok().json(nodes))
}

pub async fn get_node_details_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, node_name)
    nodes_service: web::Data<Arc<NodesService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, node_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %node_name, "Attempting to get node details");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %node_name, "Successfully retrieved cluster config for node details");
    let node_details = nodes_service
        .get_node_details(&cluster_config, &node_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %node_name, "Successfully retrieved node details");
    Ok(HttpResponse::Ok().json(node_details))
}

pub async fn list_pods_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, "Attempting to list pods");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, "Successfully retrieved cluster config for listing pods");
    let pods = pod_service
        .list_pods(&cluster_config, &namespace_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, count = pods.len(), "Successfully listed pods");
    Ok(HttpResponse::Ok().json(pods))
}

pub async fn get_pod_details_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, pod_name)
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, pod_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, %pod_name, "Attempting to get pod details");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %pod_name, "Successfully retrieved cluster config for pod details");
    let pod_details = pod_service
        .get_pod_details(&cluster_config, &namespace_name, &pod_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %pod_name, "Successfully retrieved pod details");
    Ok(HttpResponse::Ok().json(pod_details))
}

pub async fn get_pod_events_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, pod_name)
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, pod_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, %pod_name, "Attempting to get pod events");

    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %pod_name, "Successfully retrieved cluster config for pod events");

    let events = pod_service
        .get_pod_events(&cluster_config, &namespace_name, &pod_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %pod_name, count = events.len(), "Successfully fetched pod events via service");
    Ok(HttpResponse::Ok().json(events))
}

#[derive(Deserialize)]
pub struct ExecQuery {
    pub command: String,
    pub container: Option<String>,
    pub tty: Option<bool>,
}

pub async fn exec_pod_command_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, pod_name)
    query: web::Query<ExecQuery>,
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, pod_name) = path.into_inner();
    let ExecQuery {
        command,
        container,
        tty,
    } = query.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, %pod_name, %command, "Exec into pod");

    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let opts = crate::services::kubernetes::pod::ExecOptions {
        command: shlex::split(&command).unwrap_or_else(|| vec![command.clone()]),
        container,
        tty,
        stdin: Some(false),
    };
    let result = pod_service
        .exec_command(&cluster_config, &namespace_name, &pod_name, opts)
        .await?;
    Ok(HttpResponse::Ok().json(result))
}

pub async fn list_services_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    services_service: web::Data<Arc<ServicesService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, "Attempting to list services");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, "Successfully retrieved cluster config for listing services");
    let services = services_service
        .list_services(&cluster_config, &namespace_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, count = services.len(), "Successfully listed services");
    Ok(HttpResponse::Ok().json(services))
}

// New controller to list all services in a cluster, across all namespaces
pub async fn list_all_services_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>, // cluster_id
    services_service: web::Data<Arc<ServicesService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, "Attempting to list all services");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, "Successfully retrieved cluster config for listing all services");
    let services = services_service.list_services(&cluster_config, "").await?; // Empty string for all namespaces
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, count = services.len(), "Successfully listed all services");
    Ok(HttpResponse::Ok().json(services))
}

pub async fn get_service_details_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, service_name)
    services_service: web::Data<Arc<ServicesService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, service_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, %service_name, "Attempting to get service details");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %service_name, "Successfully retrieved cluster config for service details");
    let service_details = services_service
        .get_service_details(&cluster_config, &namespace_name, &service_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %service_name, "Successfully retrieved service details");
    Ok(HttpResponse::Ok().json(service_details))
}

pub async fn list_deployments_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    deployments_service: web::Data<Arc<DeploymentsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, "Attempting to list deployments");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, "Successfully retrieved cluster config for listing deployments");
    let deployments = deployments_service
        .list_deployments(&cluster_config, &namespace_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, count = deployments.len(), "Successfully listed deployments");
    Ok(HttpResponse::Ok().json(deployments))
}

pub async fn get_deployment_details_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, deployment_name)
    deployments_service: web::Data<Arc<DeploymentsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, deployment_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, %deployment_name, "Attempting to get deployment details");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %deployment_name, "Successfully retrieved cluster config for deployment details");
    let deployment_details = deployments_service
        .get_deployment_details(&cluster_config, &namespace_name, &deployment_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %deployment_name, "Successfully retrieved deployment details");
    Ok(HttpResponse::Ok().json(deployment_details))
}

#[derive(Deserialize)]
pub struct ScaleDeploymentBody {
    pub replicas: i32,
}

pub async fn scale_deployment_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, deployment_name)
    body: web::Json<ScaleDeploymentBody>,
    deployments_service: web::Data<Arc<DeploymentsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, deployment_name) = path.into_inner();
    let replicas = body.replicas;
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, %deployment_name, replicas = replicas, "Scaling deployment");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    deployments_service
        .scale_deployment(&cluster_config, &namespace_name, &deployment_name, replicas)
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "scaled",
        "replicas": replicas,
    })))
}

pub async fn restart_deployment_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, deployment_name)
    deployments_service: web::Data<Arc<DeploymentsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, deployment_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, %deployment_name, "Restarting deployment");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    deployments_service
        .restart_deployment(&cluster_config, &namespace_name, &deployment_name)
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "restarted",
    })))
}

#[derive(Deserialize)]
pub struct CreateNamespaceBody {
    pub name: String,
    pub labels: Option<BTreeMap<String, String>>,
}

pub async fn create_namespace_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>, // cluster_id
    body: web::Json<CreateNamespaceBody>,
    namespaces_service: web::Data<Arc<NamespacesService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, ns = %body.name, "Creating namespace");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let ns = namespaces_service
        .create_namespace(&cluster_config, &body.name, body.labels.clone())
        .await?;
    Ok(HttpResponse::Ok().json(ns))
}

pub async fn delete_namespace_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    namespaces_service: web::Data<Arc<NamespacesService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, "Deleting namespace");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    namespaces_service
        .delete_namespace(&cluster_config, &namespace_name)
        .await?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "deleted",
        "name": namespace_name,
    })))
}

// New controller to list all deployments in a cluster, across all namespaces
pub async fn list_all_deployments_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>, // cluster_id
    deployments_service: web::Data<Arc<DeploymentsService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    println!(
        "Listing all deployments for cluster: {}",
        cluster_id.clone()
    );
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, "Attempting to list all deployments");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, "Successfully retrieved cluster config for listing all deployments");
    // Pass None or an empty string for namespace to indicate all namespaces
    let deployments = deployments_service
        .list_deployments(&cluster_config, "")
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, count = deployments.len(), "Successfully listed all deployments");
    Ok(HttpResponse::Ok().json(deployments))
}

pub async fn get_pods_for_deployment_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, deployment_name)
    deployments_service: web::Data<Arc<DeploymentsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, deployment_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, %deployment_name, "Attempting to get pods for deployment");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %deployment_name, "Successfully retrieved cluster config for pods for deployment");

    let pods = deployments_service
        .get_pods_for_deployment(&cluster_config, &namespace_name, &deployment_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %deployment_name, count = pods.len(), "Successfully retrieved pods for deployment");
    Ok(HttpResponse::Ok().json(pods))
}

pub async fn list_stateful_sets_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    stateful_sets_service: web::Data<Arc<StatefulSetsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, "Attempting to list stateful sets");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, "Successfully retrieved cluster config for listing stateful sets");
    let stateful_sets = stateful_sets_service
        .list_stateful_sets(&cluster_config, &namespace_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, count = stateful_sets.len(), "Successfully listed stateful sets");
    Ok(HttpResponse::Ok().json(stateful_sets))
}

pub async fn get_stateful_set_details_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, stateful_set_name)
    stateful_sets_service: web::Data<Arc<StatefulSetsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, stateful_set_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, %stateful_set_name, "Attempting to get stateful set details");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %stateful_set_name, "Successfully retrieved cluster config for stateful set details");
    let details = stateful_sets_service
        .get_stateful_set_details(&cluster_config, &namespace_name, &stateful_set_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %stateful_set_name, "Successfully retrieved stateful set details");
    Ok(HttpResponse::Ok().json(details))
}

pub async fn get_pods_for_stateful_set_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, stateful_set_name)
    stateful_sets_service: web::Data<Arc<StatefulSetsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, stateful_set_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, %stateful_set_name, "Attempting to get pods for stateful set");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %stateful_set_name, "Successfully retrieved cluster config for pods for stateful set");
    let pods = stateful_sets_service
        .get_pods_for_stateful_set(&cluster_config, &namespace_name, &stateful_set_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %stateful_set_name, count = pods.len(), "Successfully retrieved pods for stateful set");
    Ok(HttpResponse::Ok().json(pods))
}

pub async fn list_daemon_sets_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    daemon_sets_service: web::Data<Arc<DaemonSetsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, "Attempting to list daemon sets");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, "Successfully retrieved cluster config for listing daemon sets");
    let daemon_sets = daemon_sets_service
        .list_daemon_sets(&cluster_config, &namespace_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, count = daemon_sets.len(), "Successfully listed daemon sets");
    Ok(HttpResponse::Ok().json(daemon_sets))
}

// New controller to list all daemon sets in a cluster, across all namespaces
pub async fn list_all_daemon_sets_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>, // cluster_id
    daemon_sets_service: web::Data<Arc<DaemonSetsService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, "Attempting to list all daemon sets");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, "Successfully retrieved cluster config for listing all daemon sets");
    let daemon_sets = daemon_sets_service
        .list_daemon_sets(&cluster_config, "")
        .await?; // Empty string for all namespaces
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, count = daemon_sets.len(), "Successfully listed all daemon sets");
    Ok(HttpResponse::Ok().json(daemon_sets))
}

pub async fn get_daemon_set_details_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, daemon_set_name)
    daemon_sets_service: web::Data<Arc<DaemonSetsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, daemon_set_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, %daemon_set_name, "Attempting to get daemon set details");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %daemon_set_name, "Successfully retrieved cluster config for daemon set details");
    let details = daemon_sets_service
        .get_daemon_set_details(&cluster_config, &namespace_name, &daemon_set_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %daemon_set_name, "Successfully retrieved daemon set details");
    Ok(HttpResponse::Ok().json(details))
}

pub async fn get_pods_for_daemon_set_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, daemon_set_name)
    daemon_sets_service: web::Data<Arc<DaemonSetsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, daemon_set_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, %daemon_set_name, "Attempting to get pods for daemon set");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %daemon_set_name, "Successfully retrieved cluster config for pods for daemon set");
    let pods = daemon_sets_service
        .get_pods_for_daemon_set(&cluster_config, &namespace_name, &daemon_set_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %daemon_set_name, count = pods.len(), "Successfully retrieved pods for daemon set");
    Ok(HttpResponse::Ok().json(pods))
}

// New controller to list all stateful sets in a cluster, across all namespaces
pub async fn list_all_stateful_sets_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>, // cluster_id
    stateful_sets_service: web::Data<Arc<StatefulSetsService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, "Attempting to list all stateful sets");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, "Successfully retrieved cluster config for listing all stateful sets");
    let stateful_sets = stateful_sets_service
        .list_stateful_sets(&cluster_config, "")
        .await?; // Empty string for all namespaces
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, count = stateful_sets.len(), "Successfully listed all stateful sets");
    Ok(HttpResponse::Ok().json(stateful_sets))
}

// New controller to list all pvcs in a cluster, across all namespaces
pub async fn list_all_pvcs_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>, // cluster_id
    pvc_service: web::Data<Arc<PersistentVolumeClaimsService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, "Attempting to list all PVCs");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, "Successfully retrieved cluster config for listing all PVCs");
    let pvcs = pvc_service
        .list_persistent_volume_claims(&cluster_config, "")
        .await?; // Empty string for all namespaces
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, count = pvcs.len(), "Successfully listed all PVCs");
    Ok(HttpResponse::Ok().json(pvcs))
}

pub async fn list_pvcs_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    pvc_service: web::Data<Arc<PersistentVolumeClaimsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, "Attempting to list PVCs");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, "Successfully retrieved cluster config for listing PVCs");
    let pvcs = pvc_service
        .list_persistent_volume_claims(&cluster_config, &namespace_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, count = pvcs.len(), "Successfully listed PVCs");
    Ok(HttpResponse::Ok().json(pvcs))
}

pub async fn get_pvc_details_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, pvc_name)
    pvc_service: web::Data<Arc<PersistentVolumeClaimsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, pvc_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %namespace_name, %pvc_name, "Attempting to get PVC details");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %pvc_name, "Successfully retrieved cluster config for PVC details");
    let pvc_details = pvc_service
        .get_persistent_volume_claim_details(&cluster_config, &namespace_name, &pvc_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %namespace_name, %pvc_name, "Successfully retrieved PVC details");
    Ok(HttpResponse::Ok().json(pvc_details))
}

pub async fn list_pvs_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>, // cluster_id
    pv_service: web::Data<Arc<PersistentVolumesService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, "Attempting to list PVs");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, "Successfully retrieved cluster config for listing PVs");
    let pvs = pv_service.list_persistent_volumes(&cluster_config).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, count = pvs.len(), "Successfully listed PVs");
    Ok(HttpResponse::Ok().json(pvs))
}

pub async fn get_pv_details_controller(
    claims: web::ReqData<Claims>, // Changed _claims to claims to use it in log
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, pv_name)
    pv_service: web::Data<Arc<PersistentVolumesService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, pv_name) = path.into_inner();
    debug!(target: "mayyam::controllers::kubernetes", user_id = %claims.username, %cluster_id, %pv_name, "Attempting to get PV details");
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %pv_name, "Successfully retrieved cluster config for PV details");
    let pv_details = pv_service
        .get_persistent_volume_details(&cluster_config, &pv_name)
        .await?;
    debug!(target: "mayyam::controllers::kubernetes", %cluster_id, %pv_name, "Successfully retrieved PV details");
    Ok(HttpResponse::Ok().json(pv_details))
}

pub async fn test_db_connection_controller(
    db: web::Data<Arc<DatabaseConnection>>,
) -> Result<impl Responder, AppError> {
    // Just try to access the connection to ensure it's extracted.
    // The existence of 'db' here means extraction was successful.
    info!("Successfully extracted DatabaseConnection in test_db_connection_controller.");
    // You could even try a super simple query if you have a table name handy,
    // but for now, just extracting is enough.
    // Example: let _ = db.get_ref().get_database_backend();
    Ok(HttpResponse::Ok().body("Database connection extracted successfully!"))
}
