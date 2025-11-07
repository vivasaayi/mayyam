use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::models::cluster::{CreateKubernetesClusterRequest, UpdateKubernetesClusterRequest};
use crate::repositories::cluster::ClusterRepository; // For accessing user_id from token

#[derive(Clone)]
pub struct KubernetesClusterManagementController {
    // Renamed from ClusterManagementController
    cluster_repo: Arc<ClusterRepository>,
}

#[derive(Deserialize, Debug)]
pub struct KubernetesClusterQuery {
    // Renamed from ClusterQuery
    // No longer need type here if the base path implies kubernetes
    // If you still want to support other query params for k8s clusters, add them here
}

impl KubernetesClusterManagementController {
    // Renamed from ClusterManagementController
    pub fn new(cluster_repo: Arc<ClusterRepository>) -> Self {
        Self { cluster_repo }
    }

    pub async fn create_kubernetes_cluster(
        &self,
        claims: web::ReqData<Claims>,
        req: web::Json<CreateKubernetesClusterRequest>,
    ) -> impl Responder {
        debug!(
            "Attempting to create Kubernetes cluster with name: {} by user: {}",
            req.name, claims.sub
        ); // Use claims.sub for user identifier
        match self
            .cluster_repo
            .create_kubernetes_cluster(&req.into_inner(), &claims.sub.to_string())
            .await
        {
            // Pass claims.sub as string
            Ok(cluster) => {
                info!(
                    "Successfully created Kubernetes cluster with ID: {}",
                    cluster.id
                );
                HttpResponse::Created().json(cluster)
            }
            Err(AppError::Conflict(msg)) => {
                error!("Conflict error creating Kubernetes cluster: {}", msg);
                HttpResponse::Conflict().json(serde_json::json!({ "error": msg }))
            }
            Err(e) => {
                error!("Error creating Kubernetes cluster: {:?}", e);
                HttpResponse::InternalServerError()
                    .json(serde_json::json!({ "error": e.to_string() }))
            }
        }
    }

    // This method now specifically gets Kubernetes clusters due to the new routing
    pub async fn get_all_kubernetes_clusters(
        &self,
        _query: web::Query<KubernetesClusterQuery>,
    ) -> impl Responder {
        debug!("Fetching all Kubernetes clusters.");
        match self.cluster_repo.find_by_type("kubernetes").await {
            Ok(clusters) => {
                info!(
                    "Successfully fetched {} Kubernetes clusters",
                    clusters.len()
                );
                HttpResponse::Ok().json(clusters)
            }
            Err(e) => {
                error!("Error fetching Kubernetes clusters: {:?}", e);
                HttpResponse::InternalServerError()
                    .json(serde_json::json!({ "error": e.to_string() }))
            }
        }
    }

    // Renamed from get_cluster_by_id to be more specific if needed, or keep as is if context is clear
    pub async fn get_kubernetes_cluster_by_id(&self, path: web::Path<Uuid>) -> impl Responder {
        let cluster_id = path.into_inner();
        debug!("Fetching Kubernetes cluster with ID: {}", cluster_id);
        // Add a check to ensure the fetched cluster is indeed a Kubernetes cluster if necessary
        match self.cluster_repo.find_by_id(cluster_id).await {
            Ok(Some(cluster)) => {
                if cluster.cluster_type == "kubernetes" {
                    info!(
                        "Successfully fetched Kubernetes cluster with ID: {}",
                        cluster.id
                    );
                    HttpResponse::Ok().json(cluster)
                } else {
                    error!(
                        "Cluster with ID: {} is not a Kubernetes cluster",
                        cluster_id
                    );
                    HttpResponse::NotFound()
                        .json(serde_json::json!({ "error": "Kubernetes cluster not found" }))
                }
            }
            Ok(None) => {
                error!("Cluster not found with ID: {}", cluster_id);
                HttpResponse::NotFound().json(serde_json::json!({ "error": "Cluster not found" }))
            }
            Err(e) => {
                error!("Error fetching cluster by ID: {:?}", e);
                HttpResponse::InternalServerError()
                    .json(serde_json::json!({ "error": e.to_string() }))
            }
        }
    }

    pub async fn update_kubernetes_cluster(
        &self,
        claims: web::ReqData<Claims>, // Assuming updates should also be authenticated
        path: web::Path<Uuid>,
        req: web::Json<UpdateKubernetesClusterRequest>,
    ) -> impl Responder {
        let cluster_id = path.into_inner();
        let update_req = req.into_inner();
        debug!(
            "Attempting to update Kubernetes cluster with ID: {}",
            cluster_id
        );

        // Convert UpdateKubernetesClusterRequest to the generic serde_json::Value for the repository
        let config_value = serde_json::json!({
            "kube_config_path": update_req.kube_config_path,
            "kube_context": update_req.kube_context,
            "api_server_url": update_req.api_server_url,
            "certificate_authority_data": update_req.certificate_authority_data,
            "client_certificate_data": update_req.client_certificate_data,
            "client_key_data": update_req.client_key_data,
            "token": update_req.token,
        });

        // Note: The existing repository `update` method is generic.
        // We might need to ensure it correctly handles partial updates or specific logic for Kubernetes clusters if necessary.
        // For now, it updates name and the whole config blob.
        match self
            .cluster_repo
            .update(cluster_id, &update_req.name, config_value)
            .await
        {
            Ok(cluster) => {
                info!("Successfully updated cluster with ID: {}", cluster.id);
                HttpResponse::Ok().json(cluster)
            }
            Err(AppError::NotFound(msg)) => {
                error!("Cluster not found for update: {}", msg);
                HttpResponse::NotFound().json(serde_json::json!({ "error": msg }))
            }
            Err(AppError::Conflict(msg)) => {
                error!("Conflict error updating cluster: {}", msg);
                HttpResponse::Conflict().json(serde_json::json!({ "error": msg }))
            }
            Err(e) => {
                error!("Error updating cluster: {:?}", e);
                HttpResponse::InternalServerError()
                    .json(serde_json::json!({ "error": e.to_string() }))
            }
        }
    }

    // Renamed from delete_cluster
    pub async fn delete_kubernetes_cluster(
        &self,
        claims: web::ReqData<Claims>,
        path: web::Path<Uuid>,
    ) -> impl Responder {
        let cluster_id = path.into_inner();
        debug!(
            "User {} attempting to delete Kubernetes cluster with ID: {}",
            claims.username, cluster_id
        );
        // TODO: Ensure this only deletes Kubernetes clusters, perhaps by checking type before calling repo delete
        // Or rely on the fact that it's scoped under /kubernetes-clusters
        match self.cluster_repo.delete(cluster_id).await {
            // Consider adding a type check in repo or here
            Ok(_) => {
                info!(
                    "Successfully deleted Kubernetes cluster with ID: {}",
                    cluster_id
                );
                HttpResponse::NoContent().finish()
            }
            Err(AppError::NotFound(msg)) => {
                error!("Cluster not found for deletion: {}", msg);
                HttpResponse::NotFound().json(serde_json::json!({ "error": msg }))
            }
            Err(e) => {
                error!("Error deleting cluster: {:?}", e);
                HttpResponse::InternalServerError()
                    .json(serde_json::json!({ "error": e.to_string() }))
            }
        }
    }
}
