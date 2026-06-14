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
use crate::middleware::auth::Claims; // Assuming you have auth middleware
use crate::models::cluster::{CreateKubernetesClusterRequest, KubernetesClusterConfig};
use crate::services::aws::inventory::types::{Pillar, DEFAULT_STALE_AFTER_HOURS};
use crate::services::kubernetes::admission_webhook_inventory::{
    evaluate_kubernetes_admission_webhook_inventory,
    RESOURCE_TYPE as ADMISSION_WEBHOOK_RESOURCE_TYPE,
};
use crate::services::kubernetes::admission_webhooks_service::AdmissionWebhooksService;
use crate::services::kubernetes::cluster_role_binding_inventory::{
    evaluate_kubernetes_cluster_role_binding_inventory,
    RESOURCE_TYPE as CLUSTER_ROLE_BINDING_RESOURCE_TYPE,
};
use crate::services::kubernetes::cluster_role_inventory::{
    evaluate_kubernetes_cluster_role_inventory, RESOURCE_TYPE as CLUSTER_ROLE_RESOURCE_TYPE,
};
use crate::services::kubernetes::configmap_inventory::{
    evaluate_kubernetes_configmap_inventory, RESOURCE_TYPE as CONFIGMAP_RESOURCE_TYPE,
};
use crate::services::kubernetes::configmaps_service::ConfigMapsService;
use crate::services::kubernetes::cronjob_inventory::{
    evaluate_kubernetes_cronjob_inventory, RESOURCE_TYPE as CRONJOB_RESOURCE_TYPE,
};
use crate::services::kubernetes::custom_resource_definition_inventory::{
    evaluate_kubernetes_custom_resource_definition_inventory,
    RESOURCE_TYPE as CUSTOM_RESOURCE_DEFINITION_RESOURCE_TYPE,
};
use crate::services::kubernetes::custom_resource_inventory::{
    evaluate_kubernetes_custom_resource_inventory, RESOURCE_TYPE as CUSTOM_RESOURCE_RESOURCE_TYPE,
};
use crate::services::kubernetes::daemon_set_inventory::{
    evaluate_kubernetes_daemonset_inventory, RESOURCE_TYPE as DAEMONSET_RESOURCE_TYPE,
};
use crate::services::kubernetes::deployment_inventory::{
    evaluate_kubernetes_deployment_inventory, RESOURCE_TYPE as DEPLOYMENT_RESOURCE_TYPE,
};
use crate::services::kubernetes::endpoint_slice_inventory::{
    evaluate_kubernetes_endpoint_slice_inventory, RESOURCE_TYPE as ENDPOINT_SLICE_RESOURCE_TYPE,
};
use crate::services::kubernetes::endpoints_inventory::{
    evaluate_kubernetes_endpoints_inventory, RESOURCE_TYPE as ENDPOINTS_RESOURCE_TYPE,
};
use crate::services::kubernetes::event_inventory::{
    evaluate_kubernetes_event_inventory, RESOURCE_TYPE as EVENT_RESOURCE_TYPE,
};
use crate::services::kubernetes::gateway_api_inventory::{
    evaluate_kubernetes_gateway_api_inventory, RESOURCE_TYPE as GATEWAY_API_RESOURCE_TYPE,
};
use crate::services::kubernetes::hpa_inventory::{
    evaluate_kubernetes_hpa_inventory, RESOURCE_TYPE as HPA_RESOURCE_TYPE,
};
use crate::services::kubernetes::ingress_inventory::{
    evaluate_kubernetes_ingress_inventory, RESOURCE_TYPE as INGRESS_RESOURCE_TYPE,
};
use crate::services::kubernetes::inventory::{
    evaluate_kubernetes_cluster_fleet, RESOURCE_TYPE as CLUSTER_RESOURCE_TYPE,
};
use crate::services::kubernetes::job_inventory::{
    evaluate_kubernetes_job_inventory, RESOURCE_TYPE as JOB_RESOURCE_TYPE,
};
use crate::services::kubernetes::limit_range_inventory::{
    evaluate_kubernetes_limit_range_inventory, RESOURCE_TYPE as LIMIT_RANGE_RESOURCE_TYPE,
};
use crate::services::kubernetes::namespace_inventory::{
    evaluate_kubernetes_namespace_inventory, RESOURCE_TYPE as NAMESPACE_RESOURCE_TYPE,
};
use crate::services::kubernetes::network_policy_inventory::{
    evaluate_kubernetes_network_policy_inventory, RESOURCE_TYPE as NETWORK_POLICY_RESOURCE_TYPE,
};
use crate::services::kubernetes::node_drains_inventory::{
    evaluate_kubernetes_node_drains_inventory, RESOURCE_TYPE as NODE_DRAIN_RESOURCE_TYPE,
};
use crate::services::kubernetes::node_drains_service::NodeDrainsService;
use crate::services::kubernetes::node_inventory::{
    evaluate_kubernetes_node_inventory, RESOURCE_TYPE as NODE_RESOURCE_TYPE,
};
use crate::services::kubernetes::node_taints_inventory::{
    evaluate_kubernetes_node_taints_inventory, RESOURCE_TYPE as NODE_TAINT_RESOURCE_TYPE,
};
use crate::services::kubernetes::node_taints_service::NodeTaintsService;
use crate::services::kubernetes::pdb_inventory::{
    evaluate_kubernetes_pdb_inventory, RESOURCE_TYPE as PDB_RESOURCE_TYPE,
};
use crate::services::kubernetes::persistent_volume_claim_inventory::{
    evaluate_kubernetes_persistent_volume_claim_inventory,
    RESOURCE_TYPE as PERSISTENT_VOLUME_CLAIM_RESOURCE_TYPE,
};
use crate::services::kubernetes::persistent_volume_inventory::{
    evaluate_kubernetes_persistent_volume_inventory,
    RESOURCE_TYPE as PERSISTENT_VOLUME_RESOURCE_TYPE,
};
use crate::services::kubernetes::pod_exec_inventory::{
    evaluate_kubernetes_pod_exec_inventory, RESOURCE_TYPE as POD_EXEC_RESOURCE_TYPE,
};
use crate::services::kubernetes::pod_inventory::{
    evaluate_kubernetes_pod_inventory, RESOURCE_TYPE as POD_RESOURCE_TYPE,
};
use crate::services::kubernetes::pod_log_inventory::{
    evaluate_kubernetes_pod_log_inventory, RESOURCE_TYPE as POD_LOG_RESOURCE_TYPE,
};
use crate::services::kubernetes::pod_security_standards_inventory::{
    evaluate_kubernetes_pod_security_standards_inventory,
    RESOURCE_TYPE as POD_SECURITY_STANDARDS_RESOURCE_TYPE,
};
use crate::services::kubernetes::pod_security_standards_service::PodSecurityStandardsService;
use crate::services::kubernetes::prelude::*;
use crate::services::kubernetes::replica_set_inventory::{
    evaluate_kubernetes_replicaset_inventory, RESOURCE_TYPE as REPLICASET_RESOURCE_TYPE,
};
use crate::services::kubernetes::replica_sets_service::ReplicaSetsService;
use crate::services::kubernetes::resource_quota_inventory::{
    evaluate_kubernetes_resource_quota_inventory, RESOURCE_TYPE as RESOURCE_QUOTA_RESOURCE_TYPE,
};
use crate::services::kubernetes::role_binding_inventory::{
    evaluate_kubernetes_role_binding_inventory, RESOURCE_TYPE as ROLE_BINDING_RESOURCE_TYPE,
};
use crate::services::kubernetes::role_inventory::{
    evaluate_kubernetes_role_inventory, RESOURCE_TYPE as ROLE_RESOURCE_TYPE,
};
use crate::services::kubernetes::secret_inventory::{
    evaluate_kubernetes_secret_inventory, RESOURCE_TYPE as SECRET_RESOURCE_TYPE,
};
use crate::services::kubernetes::secrets_service::SecretsService;
use crate::services::kubernetes::service_account_inventory::{
    evaluate_kubernetes_service_account_inventory, RESOURCE_TYPE as SERVICE_ACCOUNT_RESOURCE_TYPE,
};
use crate::services::kubernetes::service_accounts_service::ServiceAccountsService;
use crate::services::kubernetes::service_inventory::{
    evaluate_kubernetes_service_inventory, RESOURCE_TYPE as SERVICE_RESOURCE_TYPE,
};
use crate::services::kubernetes::stateful_set_inventory::{
    evaluate_kubernetes_statefulset_inventory, RESOURCE_TYPE as STATEFULSET_RESOURCE_TYPE,
};
use crate::services::kubernetes::storage_class_inventory::{
    evaluate_kubernetes_storage_class_inventory, RESOURCE_TYPE as STORAGE_CLASS_RESOURCE_TYPE,
};
use crate::services::kubernetes::volume_snapshot_inventory::{
    evaluate_kubernetes_volume_snapshot_inventory, RESOURCE_TYPE as VOLUME_SNAPSHOT_RESOURCE_TYPE,
};
use crate::services::kubernetes::vpa_inventory::{
    evaluate_kubernetes_vpa_inventory, RESOURCE_TYPE as VPA_RESOURCE_TYPE,
};
use actix_web::{web, HttpResponse, Responder};
use actix_web_lab::sse;
use chrono::Utc;
use futures::StreamExt;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};
use uuid::Uuid;

const KUBERNETES_INVENTORY_PILLARS: &[Pillar] =
    &[Pillar::Cost, Pillar::Resilience, Pillar::Security];

#[derive(Debug, Deserialize)]
pub struct KubernetesInventoryQuery {
    pub pillar: Option<String>,
    pub cluster_id: Option<String>,
    pub namespace: Option<String>,
}

fn parse_kubernetes_inventory_pillars(raw: &Option<String>) -> Result<Vec<Pillar>, AppError> {
    let supported_names = || {
        KUBERNETES_INVENTORY_PILLARS
            .iter()
            .map(|pillar| pillar.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };

    match raw {
        Some(raw) => {
            let pillar = Pillar::parse(raw).ok_or_else(|| {
                AppError::BadRequest(format!(
                    "Unknown pillar '{}'; expected one of: {}",
                    raw,
                    supported_names()
                ))
            })?;
            if !KUBERNETES_INVENTORY_PILLARS.contains(&pillar) {
                return Err(AppError::BadRequest(format!(
                    "Pillar '{}' is not supported for Kubernetes cluster inventory yet; expected one of: {}",
                    raw,
                    supported_names()
                )));
            }
            Ok(vec![pillar])
        }
        None => Ok(KUBERNETES_INVENTORY_PILLARS.to_vec()),
    }
}

// Helper function to get cluster config (you'll need to implement this based on your DB structure)
// This is a simplified example. You'd typically fetch this from a database.
pub async fn get_cluster_config_by_id(
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

pub async fn get_cluster_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes cluster inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let clusters = crate::models::cluster::Entity::find()
        .filter(crate::models::cluster::Column::ClusterType.eq("kubernetes"))
        .all(db.get_ref().as_ref())
        .await
        .map_err(AppError::Database)?;

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_cluster_fleet(&clusters, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = clusters.iter().map(|cluster| cluster.updated_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": CLUSTER_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "resources_evaluated": clusters.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_namespace_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    namespaces_service: web::Data<Arc<NamespacesService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes namespace inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        namespaces_service
            .list_namespace_inventory(&cluster_config, cluster_id)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_namespace_inventory(&namespace_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = namespace_items
        .iter()
        .map(|namespace| namespace.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": NAMESPACE_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "resources_evaluated": namespace_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_node_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    nodes_service: web::Data<Arc<NodesService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes node inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let node_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        nodes_service
            .list_node_inventory(&cluster_config, cluster_id)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_node_inventory(&node_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = node_items.iter().map(|node| node.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": NODE_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "resources_evaluated": node_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_node_taint_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    node_taints_service: web::Data<Arc<NodeTaintsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes Node Taints inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let taint_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        node_taints_service
            .list_inventory(&cluster_config, cluster_id)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_node_taints_inventory(&taint_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = taint_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": NODE_TAINT_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "resources_evaluated": taint_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_node_drain_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    node_drains_service: web::Data<Arc<NodeDrainsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes Node Drains inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let drain_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        node_drains_service
            .list_inventory(&cluster_config, cluster_id)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_node_drains_inventory(&drain_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = drain_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": NODE_DRAIN_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "resources_evaluated": drain_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_pod_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes pod inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let pod_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        pod_service
            .list_pod_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_pod_inventory(&pod_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = pod_items.iter().map(|pod| pod.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": POD_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": pod_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_pod_log_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes Pod Logs inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let log_targets = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        pod_service
            .list_pod_log_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_pod_log_inventory(&log_targets, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = log_targets
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": POD_LOG_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": log_targets.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_pod_exec_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes Pod Exec inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let exec_targets = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        pod_service
            .list_pod_exec_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_pod_exec_inventory(&exec_targets, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = exec_targets
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": POD_EXEC_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": exec_targets.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_deployment_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    deployments_service: web::Data<Arc<DeploymentsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes deployment inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let deployment_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        deployments_service
            .list_deployment_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_deployment_inventory(&deployment_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = deployment_items
        .iter()
        .map(|deployment| deployment.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": DEPLOYMENT_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": deployment_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_replicaset_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    replica_sets_service: web::Data<Arc<ReplicaSetsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes ReplicaSet inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let replicaset_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        replica_sets_service
            .list_replicaset_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_replicaset_inventory(&replicaset_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = replicaset_items
        .iter()
        .map(|replicaset| replicaset.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": REPLICASET_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": replicaset_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_statefulset_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    stateful_sets_service: web::Data<Arc<StatefulSetsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes StatefulSet inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let statefulset_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        stateful_sets_service
            .list_statefulset_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_statefulset_inventory(&statefulset_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = statefulset_items
        .iter()
        .map(|statefulset| statefulset.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": STATEFULSET_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": statefulset_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_daemonset_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    daemon_sets_service: web::Data<Arc<DaemonSetsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes DaemonSet inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let daemonset_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        daemon_sets_service
            .list_daemonset_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_daemonset_inventory(&daemonset_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = daemonset_items
        .iter()
        .map(|daemonset| daemonset.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": DAEMONSET_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": daemonset_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_job_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    jobs_service: web::Data<Arc<JobsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes Job inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let job_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        jobs_service
            .list_job_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_job_inventory(&job_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = job_items.iter().map(|job| job.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": JOB_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": job_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_cronjob_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    cronjobs_service: web::Data<Arc<CronJobsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes CronJob inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let cronjob_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        cronjobs_service
            .list_cronjob_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_cronjob_inventory(&cronjob_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = cronjob_items
        .iter()
        .map(|cronjob| cronjob.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": CRONJOB_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": cronjob_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_service_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    services_service: web::Data<Arc<ServicesService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes Service inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let service_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        services_service
            .list_service_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_service_inventory(&service_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = service_items
        .iter()
        .map(|service| service.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": SERVICE_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": service_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_ingress_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    ingress_service: web::Data<Arc<IngressService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes Ingress inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let ingress_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        ingress_service
            .list_ingress_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_ingress_inventory(&ingress_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = ingress_items
        .iter()
        .map(|ingress| ingress.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": INGRESS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": ingress_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_gateway_api_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    gateway_api_service: web::Data<Arc<GatewayApiService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes Gateway API inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let gateway_api_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        gateway_api_service
            .list_gateway_api_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_gateway_api_inventory(&gateway_api_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = gateway_api_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": GATEWAY_API_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": gateway_api_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_endpoints_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    endpoints_service: web::Data<Arc<EndpointsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes Endpoints inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let endpoint_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        endpoints_service
            .list_endpoints_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_endpoints_inventory(&endpoint_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = endpoint_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": ENDPOINTS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": endpoint_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_endpoint_slice_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    endpoints_service: web::Data<Arc<EndpointsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes EndpointSlice inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let endpoint_slice_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        endpoints_service
            .list_endpoint_slice_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| {
            evaluate_kubernetes_endpoint_slice_inventory(&endpoint_slice_items, *pillar, now)
        })
        .collect::<Vec<_>>();
    let oldest_refresh = endpoint_slice_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": ENDPOINT_SLICE_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": endpoint_slice_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_configmap_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    configmaps_service: web::Data<Arc<ConfigMapsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes ConfigMap inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let configmap_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        configmaps_service
            .list_configmap_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_configmap_inventory(&configmap_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = configmap_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": CONFIGMAP_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": configmap_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_secret_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    secrets_service: web::Data<Arc<SecretsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes Secret inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let secret_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        secrets_service
            .list_secret_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_secret_inventory(&secret_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = secret_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": SECRET_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": secret_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_service_account_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    service_accounts_service: web::Data<Arc<ServiceAccountsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes ServiceAccount inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let service_account_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        service_accounts_service
            .list_service_account_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| {
            evaluate_kubernetes_service_account_inventory(&service_account_items, *pillar, now)
        })
        .collect::<Vec<_>>();
    let oldest_refresh = service_account_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": SERVICE_ACCOUNT_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": service_account_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_role_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    rbac_service: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes Role inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let role_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        rbac_service
            .list_role_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_role_inventory(&role_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = role_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": ROLE_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": role_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_role_binding_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    rbac_service: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes RoleBinding inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let role_binding_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        rbac_service
            .list_role_binding_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_role_binding_inventory(&role_binding_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = role_binding_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": ROLE_BINDING_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": role_binding_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_cluster_role_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    rbac_service: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes ClusterRole inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let cluster_role_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        rbac_service
            .list_cluster_role_inventory(&cluster_config, cluster_id)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_cluster_role_inventory(&cluster_role_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = cluster_role_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": CLUSTER_ROLE_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "resources_evaluated": cluster_role_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_cluster_role_binding_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    rbac_service: web::Data<Arc<RbacService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes ClusterRoleBinding inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let cluster_role_binding_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        rbac_service
            .list_cluster_role_binding_inventory(&cluster_config, cluster_id)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| {
            evaluate_kubernetes_cluster_role_binding_inventory(
                &cluster_role_binding_items,
                *pillar,
                now,
            )
        })
        .collect::<Vec<_>>();
    let oldest_refresh = cluster_role_binding_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": CLUSTER_ROLE_BINDING_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "resources_evaluated": cluster_role_binding_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_network_policy_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    network_policies_service: web::Data<Arc<NetworkPoliciesService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes NetworkPolicy inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let network_policy_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        network_policies_service
            .list_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| {
            evaluate_kubernetes_network_policy_inventory(&network_policy_items, *pillar, now)
        })
        .collect::<Vec<_>>();
    let oldest_refresh = network_policy_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": NETWORK_POLICY_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": network_policy_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_hpa_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    hpa_service: web::Data<Arc<HorizontalPodAutoscalerService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes HorizontalPodAutoscaler inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let hpa_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        hpa_service
            .list_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_hpa_inventory(&hpa_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = hpa_items.iter().map(|resource| resource.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": HPA_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": hpa_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_vpa_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    vpa_service: web::Data<Arc<VerticalPodAutoscalerService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes VerticalPodAutoscaler inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let vpa_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        vpa_service
            .list_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_vpa_inventory(&vpa_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = vpa_items.iter().map(|resource| resource.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": VPA_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": vpa_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_pdb_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    pdb_service: web::Data<Arc<PodDisruptionBudgetsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes PodDisruptionBudget inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let pdb_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        pdb_service
            .list_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_pdb_inventory(&pdb_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = pdb_items.iter().map(|resource| resource.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": PDB_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": pdb_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_resource_quota_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    resource_quota_service: web::Data<Arc<ResourceQuotasService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes ResourceQuota inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let quota_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        resource_quota_service
            .list_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_resource_quota_inventory(&quota_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = quota_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": RESOURCE_QUOTA_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": quota_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_limit_range_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    limit_ranges_service: web::Data<Arc<LimitRangesService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes LimitRange inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let limit_range_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        limit_ranges_service
            .list_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_limit_range_inventory(&limit_range_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = limit_range_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": LIMIT_RANGE_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": limit_range_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_persistent_volume_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    pv_service: web::Data<Arc<PersistentVolumesService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes PersistentVolume inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let pv_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        pv_service
            .list_inventory(&cluster_config, cluster_id)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_persistent_volume_inventory(&pv_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = pv_items.iter().map(|resource| resource.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": PERSISTENT_VOLUME_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "resources_evaluated": pv_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_persistent_volume_claim_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    pvc_service: web::Data<Arc<PersistentVolumeClaimsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes PersistentVolumeClaim inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let pvc_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        pvc_service
            .list_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| {
            evaluate_kubernetes_persistent_volume_claim_inventory(&pvc_items, *pillar, now)
        })
        .collect::<Vec<_>>();
    let oldest_refresh = pvc_items.iter().map(|resource| resource.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": PERSISTENT_VOLUME_CLAIM_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": pvc_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_storage_class_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    storage_classes_service: web::Data<Arc<StorageClassesService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes StorageClass inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let storage_class_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        storage_classes_service
            .list_inventory(&cluster_config, cluster_id)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| {
            evaluate_kubernetes_storage_class_inventory(&storage_class_items, *pillar, now)
        })
        .collect::<Vec<_>>();
    let oldest_refresh = storage_class_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": STORAGE_CLASS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "resources_evaluated": storage_class_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_volume_snapshot_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    volume_snapshots_service: web::Data<Arc<VolumeSnapshotsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes VolumeSnapshot inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let snapshot_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        volume_snapshots_service
            .list_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_volume_snapshot_inventory(&snapshot_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = snapshot_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": VOLUME_SNAPSHOT_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": snapshot_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_custom_resource_definition_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    crds_service: web::Data<Arc<CrdsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes CustomResourceDefinition inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let crd_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        crds_service
            .list_inventory(&cluster_config, cluster_id)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| {
            evaluate_kubernetes_custom_resource_definition_inventory(&crd_items, *pillar, now)
        })
        .collect::<Vec<_>>();
    let oldest_refresh = crd_items.iter().map(|resource| resource.collected_at).min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": CUSTOM_RESOURCE_DEFINITION_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "resources_evaluated": crd_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_admission_webhook_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    admission_webhooks_service: web::Data<Arc<AdmissionWebhooksService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes Admission Webhook inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let webhook_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        admission_webhooks_service
            .list_inventory(&cluster_config, cluster_id)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_admission_webhook_inventory(&webhook_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = webhook_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": ADMISSION_WEBHOOK_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "resources_evaluated": webhook_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_pod_security_standards_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    pod_security_standards_service: web::Data<Arc<PodSecurityStandardsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes Pod Security Standards inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let standard_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        pod_security_standards_service
            .list_inventory(&cluster_config, cluster_id)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| {
            evaluate_kubernetes_pod_security_standards_inventory(&standard_items, *pillar, now)
        })
        .collect::<Vec<_>>();
    let oldest_refresh = standard_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": POD_SECURITY_STANDARDS_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "resources_evaluated": standard_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_custom_resource_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    crds_service: web::Data<Arc<CrdsService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes CustomResource inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let custom_resource_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        crds_service
            .list_custom_resource_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| {
            evaluate_kubernetes_custom_resource_inventory(&custom_resource_items, *pillar, now)
        })
        .collect::<Vec<_>>();
    let oldest_refresh = custom_resource_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": CUSTOM_RESOURCE_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": custom_resource_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
}

pub async fn get_event_inventory_pillar_reports_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    query: web::Query<KubernetesInventoryQuery>,
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, AppError> {
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        ?query,
        "Kubernetes Event inventory pillar report request"
    );

    let pillars = parse_kubernetes_inventory_pillars(&query.pillar)?;
    let cluster_id = query
        .cluster_id
        .as_deref()
        .map(str::trim)
        .filter(|cluster_id| !cluster_id.is_empty());
    let namespace = query
        .namespace
        .as_deref()
        .map(str::trim)
        .filter(|namespace| !namespace.is_empty());
    let event_items = if let Some(cluster_id) = cluster_id {
        let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), cluster_id).await?;
        pod_service
            .list_event_inventory(&cluster_config, cluster_id, namespace)
            .await?
    } else {
        Vec::new()
    };

    let now = Utc::now();
    let reports = pillars
        .iter()
        .map(|pillar| evaluate_kubernetes_event_inventory(&event_items, *pillar, now))
        .collect::<Vec<_>>();
    let oldest_refresh = event_items
        .iter()
        .map(|resource| resource.collected_at)
        .min();

    Ok(HttpResponse::Ok().json(json!({
        "resource_type": EVENT_RESOURCE_TYPE,
        "evaluated_at": now,
        "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        "cluster_id": query.cluster_id,
        "namespace": query.namespace,
        "resources_evaluated": event_items.len(),
        "oldest_refresh": oldest_refresh,
        "reports": reports,
    })))
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
pub struct PodLogsQuery {
    pub container: Option<String>,
    #[serde(default)]
    pub previous: bool,
    pub tail_lines: Option<i64>,
}

pub async fn get_pod_logs_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>, // (cluster_id, namespace_name, pod_name)
    query: web::Query<PodLogsQuery>,
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, AppError> {
    let (cluster_id, namespace_name, pod_name) = path.into_inner();
    let query = query.into_inner();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        %cluster_id,
        %namespace_name,
        %pod_name,
        container = ?query.container,
        previous = query.previous,
        tail_lines = ?query.tail_lines,
        "Fetching pod logs"
    );

    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let tail_lines = query.tail_lines.map(|v| v.clamp(1, 5000));
    let logs = pod_service
        .get_pod_logs(
            &cluster_config,
            &namespace_name,
            &pod_name,
            query.container.as_deref(),
            query.previous,
            tail_lines,
        )
        .await?;

    Ok(HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body(logs))
}

pub async fn stream_pod_logs_controller(
    _claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String, String)>,
    query: web::Query<PodLogsQuery>,
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, actix_web::Error> {
    let (cluster_id, namespace, pod_name) = path.into_inner();
    let query_params = query.into_inner();
    let container_name = query_params.container.as_deref();
    let previous = query_params.previous;
    let tail_lines = query_params.tail_lines;

    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let log_stream = pod_service
        .stream_pod_logs(
            &cluster_config,
            &namespace,
            &pod_name,
            container_name,
            previous,
            tail_lines,
        )
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let sse_stream = log_stream.map(|chunk_result| -> Result<sse::Event, actix_web::Error> {
        match chunk_result {
            Ok(bytes) => {
                let string_chunk = String::from_utf8_lossy(&bytes);
                Ok(sse::Event::Data(sse::Data::new(string_chunk.to_string())))
            }
            Err(e) => Ok(sse::Event::Data(
                sse::Data::new(format!("ERROR: {}", e)).event("error"),
            )),
        }
    });

    Ok(sse::Sse::from_stream(sse_stream).with_keep_alive(Duration::from_secs(10)))
}

pub async fn watch_pods_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, actix_web::Error> {
    let (cluster_id, namespace) = path.into_inner();

    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let watch_stream = pod_service
        .watch_pods(&cluster_config, &namespace)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let sse_stream = watch_stream.map(|event_result| -> Result<sse::Event, actix_web::Error> {
        match event_result {
            Ok(event) => {
                let json = match event {
                    kube::runtime::watcher::Event::Applied(obj) => {
                        serde_json::json!({"type": "Applied", "object": obj})
                    }
                    kube::runtime::watcher::Event::Deleted(obj) => {
                        serde_json::json!({"type": "Deleted", "object": obj})
                    }
                    kube::runtime::watcher::Event::Restarted(objs) => {
                        serde_json::json!({"type": "Restarted", "objects": objs})
                    }
                };
                let json_string = serde_json::to_string(&json).unwrap_or_default();
                Ok(sse::Event::Data(sse::Data::new(json_string)))
            }
            Err(e) => Ok(sse::Event::Data(
                sse::Data::new(format!("ERROR: {}", e)).event("error"),
            )),
        }
    });

    Ok(sse::Sse::from_stream(sse_stream).with_keep_alive(Duration::from_secs(10)))
}

pub async fn watch_events_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<(String, String)>,
    pod_service: web::Data<Arc<PodService>>,
) -> Result<impl Responder, actix_web::Error> {
    let (cluster_id, namespace) = path.into_inner();

    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let watch_stream = pod_service
        .watch_events(&cluster_config, &namespace)
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let sse_stream = watch_stream.map(|event_result| -> Result<sse::Event, actix_web::Error> {
        match event_result {
            Ok(event) => {
                let json = match event {
                    kube::runtime::watcher::Event::Applied(obj) => {
                        serde_json::json!({"type": "Applied", "object": obj})
                    }
                    kube::runtime::watcher::Event::Deleted(obj) => {
                        serde_json::json!({"type": "Deleted", "object": obj})
                    }
                    kube::runtime::watcher::Event::Restarted(objs) => {
                        serde_json::json!({"type": "Restarted", "objects": objs})
                    }
                };
                let json_string = serde_json::to_string(&json).unwrap_or_default();
                Ok(sse::Event::Data(sse::Data::new(json_string)))
            }
            Err(e) => Ok(sse::Event::Data(
                sse::Data::new(format!("ERROR: {}", e)).event("error"),
            )),
        }
    });

    Ok(sse::Sse::from_stream(sse_stream).with_keep_alive(Duration::from_secs(10)))
}

#[derive(Deserialize)]
pub struct MetricsQuery {
    pub namespace: Option<String>,
}

pub async fn get_cluster_metrics_controller(
    claims: web::ReqData<Claims>,
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<String>,
    query: web::Query<MetricsQuery>,
    metrics_service: web::Data<Arc<MetricsService>>,
) -> Result<impl Responder, AppError> {
    let cluster_id = path.into_inner();
    let query = query.into_inner();
    let namespace_ref = query.namespace.as_deref();
    debug!(
        target: "mayyam::controllers::kubernetes",
        user_id = %claims.username,
        %cluster_id,
        namespace = ?namespace_ref,
        "Collecting cluster metrics"
    );

    let cluster_config = get_cluster_config_by_id(db.get_ref().as_ref(), &cluster_id).await?;
    let overview = metrics_service
        .get_cluster_metrics(&cluster_config, namespace_ref)
        .await?;
    Ok(HttpResponse::Ok().json(overview))
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
