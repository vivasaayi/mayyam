use actix_web::{web, HttpResponse, Responder};
use crate::errors::AppError;
use crate::middleware::auth::Claims; // Assuming you have auth middleware
use crate::models::cluster::{KubernetesClusterConfig, CreateKubernetesClusterRequest, Model as ClusterModel};
use crate::services::kubernetes::prelude::*;
use std::sync::Arc;
use sea_orm::{DatabaseConnection, EntityTrait, ColumnTrait, QueryFilter, ActiveModelTrait, Set};
use uuid::Uuid;

// Helper function to get cluster config (you'll need to implement this based on your DB structure)
// This is a simplified example. You'd typically fetch this from a database.
async fn get_cluster_config_by_id(db: &DatabaseConnection, cluster_id_str: &str) -> Result<KubernetesClusterConfig, AppError> {
    let cluster_id = Uuid::parse_str(cluster_id_str).map_err(|_| AppError::BadRequest("Invalid cluster ID format".to_string()))?;
    
    let cluster_model = crate::models::cluster::Entity::find_by_id(cluster_id)
        .one(db)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("Cluster with ID {} not found", cluster_id)))?;

    if cluster_model.cluster_type != "kubernetes" {
        return Err(AppError::BadRequest("Cluster is not a Kubernetes cluster".to_string()));
    }

    serde_json::from_value(cluster_model.config).map_err(|e| AppError::Internal(format!("Failed to parse cluster config: {}", e)))
}

// === Cluster Management Controllers ===
pub async fn list_clusters_controller(
    _claims: web::ReqData<Claims>, 
    db: web::Data<Arc<DatabaseConnection>>
) -> Result<impl Responder, AppError> {
    let clusters = crate::models::cluster::Entity::find()
        .filter(crate::models::cluster::Column::ClusterType.eq("kubernetes"))
        .all(db.get_ref().as_ref())
        .await
        .map_err(AppError::Database)?;
    Ok(HttpResponse::Ok().json(clusters))
}

pub async fn create_cluster_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    req: web::Json<CreateKubernetesClusterRequest>,
    user_id: web::ReqData<Uuid> // Assuming user_id is extracted from claims by auth middleware
) -> Result<impl Responder, AppError> {
    let new_cluster_info = req.into_inner();
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
        config: Set(serde_json::to_value(cluster_config).map_err(|e| AppError::Internal(format!("Failed to serialize cluster config: {}", e)))?),
        created_by: Set(user_id.into_inner()),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
        ..Default::default()
    };

    let saved_cluster = new_cluster.insert(db.get_ref().as_ref()).await.map_err(AppError::Database)?;
    Ok(HttpResponse::Created().json(saved_cluster))
}

pub async fn get_cluster_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String> 
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    let cluster_model = crate::models::cluster::Entity::find_by_id(Uuid::parse_str(&cluster_id).map_err(|_| AppError::BadRequest("Invalid cluster ID".to_string()))?)
        .one(db.get_ref().as_ref())
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("Cluster with ID {} not found", cluster_id)))?;
    Ok(HttpResponse::Ok().json(cluster_model))
}

pub async fn update_cluster_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>,
    req: web::Json<CreateKubernetesClusterRequest> // Using same request for update simplicity
) -> Result<impl Responder, AppError> {
    let cluster_id_str = path.into_inner();
    let cluster_id = Uuid::parse_str(&cluster_id_str).map_err(|_| AppError::BadRequest("Invalid cluster ID format".to_string()))?;
    let update_data = req.into_inner();

    let mut active_cluster: crate::models::cluster::ActiveModel = crate::models::cluster::Entity::find_by_id(cluster_id)
        .one(db.get_ref().as_ref())
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("Cluster with ID {} not found for update", cluster_id)))?
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
    active_cluster.config = Set(serde_json::to_value(cluster_config).map_err(|e| AppError::Internal(format!("Failed to serialize cluster config: {}", e)))?);
    active_cluster.updated_at = Set(chrono::Utc::now());

    let updated_cluster = active_cluster.update(db.get_ref().as_ref()).await.map_err(AppError::Database)?;
    Ok(HttpResponse::Ok().json(updated_cluster))
}

pub async fn delete_cluster_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>
) -> Result<impl Responder, AppError> {
    let cluster_id_str = path.into_inner();
    let cluster_id = Uuid::parse_str(&cluster_id_str).map_err(|_| AppError::BadRequest("Invalid cluster ID format".to_string()))?;

    let delete_result = crate::models::cluster::Entity::delete_by_id(cluster_id)
        .exec(db.get_ref().as_ref())
        .await
        .map_err(AppError::Database)?;

    if delete_result.rows_affected == 0 {
        return Err(AppError::NotFound(format!("Cluster with ID {} not found for deletion", cluster_id)));
    }
    Ok(HttpResponse::Ok().json(serde_json::json!({ "message": "Cluster deleted successfully" })))
}


// === Kubernetes Resource Controllers ===

pub async fn list_namespaces_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>, // cluster_id
    namespaces_service: web::Data<Arc<NamespacesService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let namespaces = namespaces_service.list_namespaces(&cluster_config).await?;
    Ok(HttpResponse::Ok().json(namespaces))
}

pub async fn get_namespace_details_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    namespaces_service: web::Data<Arc<NamespacesService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let namespace_details = namespaces_service.get_namespace_details(&cluster_config, &namespace_name).await?;
    Ok(HttpResponse::Ok().json(namespace_details))
}

pub async fn list_nodes_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>, // cluster_id
    nodes_service: web::Data<Arc<NodesService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let nodes = nodes_service.list_nodes(&cluster_config).await?;
    Ok(HttpResponse::Ok().json(nodes))
}

pub async fn get_node_details_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, node_name)
    nodes_service: web::Data<Arc<NodesService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, node_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let node_details = nodes_service.get_node_details(&cluster_config, &node_name).await?;
    Ok(HttpResponse::Ok().json(node_details))
}

pub async fn list_pods_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let pods = pod_service.list_pods(&cluster_config, &namespace_name).await?;
    Ok(HttpResponse::Ok().json(pods))
}

pub async fn get_pod_details_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, pod_name)
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, pod_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let pod_details = pod_service.get_pod_details(&cluster_config, &namespace_name, &pod_name).await?;
    Ok(HttpResponse::Ok().json(pod_details))
}

pub async fn get_pod_logs_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, pod_name)
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, pod_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let logs = pod_service.get_pod_logs(&cluster_config, &namespace_name, &pod_name).await?;
    Ok(HttpResponse::Ok().content_type("text/plain").body(logs))
}

pub async fn list_services_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    services_service: web::Data<Arc<ServicesService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let services = services_service.list_services(&cluster_config, &namespace_name).await?;
    Ok(HttpResponse::Ok().json(services))
}

pub async fn get_service_details_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, service_name)
    services_service: web::Data<Arc<ServicesService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, service_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let service_details = services_service.get_service_details(&cluster_config, &namespace_name, &service_name).await?;
    Ok(HttpResponse::Ok().json(service_details))
}

pub async fn list_deployments_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    deployments_service: web::Data<Arc<DeploymentsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let deployments = deployments_service.list_deployments(&cluster_config, &namespace_name).await?;
    Ok(HttpResponse::Ok().json(deployments))
}

pub async fn get_deployment_details_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, deployment_name)
    deployments_service: web::Data<Arc<DeploymentsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, deployment_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let deployment_details = deployments_service.get_deployment_details(&cluster_config, &namespace_name, &deployment_name).await?;
    Ok(HttpResponse::Ok().json(deployment_details))
}

pub async fn get_pods_for_deployment_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, deployment_name)
    // Assuming PodService can list pods by label selector, or DeploymentsService has a method for it.
    // For now, let's assume DeploymentsService handles this logic.
    deployments_service: web::Data<Arc<DeploymentsService>>,
    // pod_service: web::Data<Arc<PodService>>, // Alternatively, use PodService if it supports label selection
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, deployment_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    
    // The `delete_all_pods_for_deployment` in `DeploymentsService` has logic to find pods by label selector.
    // We need a similar method that *lists* them instead of deleting.
    // Let's add a `get_pods_for_deployment` to `DeploymentsService` for this.
    // For now, this will be a placeholder response.
    // let pods = deployments_service.get_pods_for_deployment(&cluster_config, &namespace_name, &deployment_name).await?;
    // Ok(HttpResponse::Ok().json(pods))
    
    // Placeholder until `get_pods_for_deployment` is implemented in DeploymentsService
    let pod_service = web::Data::new(Arc::new(PodService::new())); // Temporary instance
    let pods = pod_service.list_pods(&cluster_config, &namespace_name).await?; // This lists ALL pods in namespace
    Ok(HttpResponse::Ok().json(pods))
}


pub async fn list_stateful_sets_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    stateful_sets_service: web::Data<Arc<StatefulSetsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let stateful_sets = stateful_sets_service.list_stateful_sets(&cluster_config, &namespace_name).await?;
    Ok(HttpResponse::Ok().json(stateful_sets))
}

pub async fn get_stateful_set_details_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, stateful_set_name)
    stateful_sets_service: web::Data<Arc<StatefulSetsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, stateful_set_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let details = stateful_sets_service.get_stateful_set_details(&cluster_config, &namespace_name, &stateful_set_name).await?;
    Ok(HttpResponse::Ok().json(details))
}

pub async fn get_pods_for_stateful_set_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, stateful_set_name)
    stateful_sets_service: web::Data<Arc<StatefulSetsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, stateful_set_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let pods = stateful_sets_service.get_pods_for_stateful_set(&cluster_config, &namespace_name, &stateful_set_name).await?;
    Ok(HttpResponse::Ok().json(pods))
}

pub async fn list_daemon_sets_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    daemon_sets_service: web::Data<Arc<DaemonSetsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let daemon_sets = daemon_sets_service.list_daemon_sets(&cluster_config, &namespace_name).await?;
    Ok(HttpResponse::Ok().json(daemon_sets))
}

pub async fn get_daemon_set_details_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, daemon_set_name)
    daemon_sets_service: web::Data<Arc<DaemonSetsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, daemon_set_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let details = daemon_sets_service.get_daemon_set_details(&cluster_config, &namespace_name, &daemon_set_name).await?;
    Ok(HttpResponse::Ok().json(details))
}

pub async fn get_pods_for_daemon_set_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, daemon_set_name)
    // Similar to Deployments, DaemonSetService needs a method to get pods by its selector.
    // daemon_sets_service: web::Data<Arc<DaemonSetsService>>,
    // For now, placeholder:
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, _daemon_set_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    // let pods = daemon_sets_service.get_pods_for_daemon_set(&cluster_config, &namespace_name, &daemon_set_name).await?;
    // Ok(HttpResponse::Ok().json(pods))
    // Placeholder until `get_pods_for_daemon_set` is implemented in DaemonSetsService
    let pods = pod_service.list_pods(&cluster_config, &namespace_name).await?; // This lists ALL pods in namespace
    Ok(HttpResponse::Ok().json(pods))
}

pub async fn list_pvcs_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, namespace_name)
    pvc_service: web::Data<Arc<PersistentVolumeClaimsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let pvcs = pvc_service.list_persistent_volume_claims(&cluster_config, &namespace_name).await?;
    Ok(HttpResponse::Ok().json(pvcs))
}

pub async fn get_pvc_details_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, pvc_name)
    pvc_service: web::Data<Arc<PersistentVolumeClaimsService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, pvc_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let pvc_details = pvc_service.get_persistent_volume_claim_details(&cluster_config, &namespace_name, &pvc_name).await?;
    Ok(HttpResponse::Ok().json(pvc_details))
}

pub async fn list_pvs_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>, // cluster_id
    pv_service: web::Data<Arc<PersistentVolumesService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let pvs = pv_service.list_persistent_volumes(&cluster_config).await?;
    Ok(HttpResponse::Ok().json(pvs))
}

pub async fn get_pv_details_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>, // (cluster_id, pv_name)
    pv_service: web::Data<Arc<PersistentVolumesService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, pv_name) = path.into_inner();
    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let pv_details = pv_service.get_persistent_volume_details(&cluster_config, &pv_name).await?;
    Ok(HttpResponse::Ok().json(pv_details))
}
