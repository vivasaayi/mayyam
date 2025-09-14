use actix_web::web;
use crate::controllers::kubernetes as kube_controller;
use sea_orm::DatabaseConnection; // Ensure this is imported
use std::sync::Arc; // Ensure this is imported

// Modified signature to accept db connection
pub fn configure(cfg: &mut web::ServiceConfig, db: Arc<DatabaseConnection>) {
    // Explicitly add the db connection as app_data for this scope
    cfg.app_data(web::Data::new(db.clone()));

    let scope = web::scope("/api/kubernetes")
        .route("/test-db-connection", web::get().to(kube_controller::test_db_connection_controller))
        
        .route("/clusters", web::get().to(kube_controller::list_clusters_controller))
        .route("/clusters", web::post().to(kube_controller::create_cluster_controller))
        .route("/clusters/{cluster_id}", web::get().to(kube_controller::get_cluster_controller))
        .route("/clusters/{cluster_id}", web::put().to(kube_controller::update_cluster_controller))
        .route("/clusters/{cluster_id}", web::delete().to(kube_controller::delete_cluster_controller))
        
        .route("/clusters/{cluster_id}/namespaces", web::get().to(kube_controller::list_namespaces_controller))
    .route("/clusters/{cluster_id}/namespaces", web::post().to(kube_controller::create_namespace_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}", web::get().to(kube_controller::get_namespace_details_controller))
    .route("/clusters/{cluster_id}/namespaces/{namespace_name}", web::delete().to(kube_controller::delete_namespace_controller))

        .route("/clusters/{cluster_id}/nodes", web::get().to(kube_controller::list_nodes_controller))
        .route("/clusters/{cluster_id}/nodes/{node_name}", web::get().to(kube_controller::get_node_details_controller))

        // Route for all deployments in a cluster (new)
        .route("/clusters/{cluster_id}/deployments", web::get().to(kube_controller::list_all_deployments_controller))
        // Route for deployments in a specific namespace
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/deployments", web::get().to(kube_controller::list_deployments_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/deployments/{deployment_name}", web::get().to(kube_controller::get_deployment_details_controller))
    .route("/clusters/{cluster_id}/namespaces/{namespace_name}/deployments/{deployment_name}:scale", web::post().to(kube_controller::scale_deployment_controller))
    .route("/clusters/{cluster_id}/namespaces/{namespace_name}/deployments/{deployment_name}:restart", web::post().to(kube_controller::restart_deployment_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/deployments/{deployment_name}/pods", web::get().to(kube_controller::get_pods_for_deployment_controller))

        // Route for all stateful sets in a cluster
        .route("/clusters/{cluster_id}/statefulsets", web::get().to(kube_controller::list_all_stateful_sets_controller))
        // Route for stateful sets in a specific namespace
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/statefulsets", web::get().to(kube_controller::list_stateful_sets_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/statefulsets/{stateful_set_name}", web::get().to(kube_controller::get_stateful_set_details_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/statefulsets/{stateful_set_name}/pods", web::get().to(kube_controller::get_pods_for_stateful_set_controller))

        // Route for all services in a cluster
        .route("/clusters/{cluster_id}/services", web::get().to(kube_controller::list_all_services_controller))
        // Route for services in a specific namespace
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/services", web::get().to(kube_controller::list_services_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/services/{service_name}", web::get().to(kube_controller::get_service_details_controller))

        // Route for all daemon sets in a cluster
        .route("/clusters/{cluster_id}/daemonsets", web::get().to(kube_controller::list_all_daemon_sets_controller))
        // Route for daemon sets in a specific namespace
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/daemonsets", web::get().to(kube_controller::list_daemon_sets_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/daemonsets/{daemon_set_name}", web::get().to(kube_controller::get_daemon_set_details_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/daemonsets/{daemon_set_name}/pods", web::get().to(kube_controller::get_pods_for_daemon_set_controller))

        // Route for all PVCs in a cluster
        .route("/clusters/{cluster_id}/persistentvolumeclaims", web::get().to(kube_controller::list_all_pvcs_controller))
        // Route for PVCs in a specific namespace
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/persistentvolumeclaims", web::get().to(kube_controller::list_pvcs_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/persistentvolumeclaims/{pvc_name}", web::get().to(kube_controller::get_pvc_details_controller))
        
        .route("/clusters/{cluster_id}/persistentvolumes", web::get().to(kube_controller::list_pvs_controller))
        .route("/clusters/{cluster_id}/persistentvolumes/{pv_name}", web::get().to(kube_controller::get_pv_details_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/pods/{pod_name}", web::get().to(kube_controller::get_pod_details_controller)) // New route for specific pod details
    .route("/clusters/{cluster_id}/namespaces/{namespace_name}/pods/{pod_name}/events", web::get().to(kube_controller::get_pod_events_controller))
    .route("/clusters/{cluster_id}/namespaces/{namespace_name}/pods/{pod_name}/exec", web::post().to(kube_controller::exec_pod_command_controller));

    // ConfigMaps
    let scope = scope
        .route("/clusters/{cluster_id}/namespaces/{namespace}/configmaps", web::get().to(crate::controllers::configmaps::list_configmaps_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/configmaps/{name}", web::get().to(crate::controllers::configmaps::get_configmap_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/configmaps/{name}", web::put().to(crate::controllers::configmaps::upsert_configmap_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/configmaps/{name}", web::delete().to(crate::controllers::configmaps::delete_configmap_controller));

    // Secrets (redacted get)
    let scope = scope
        .route("/clusters/{cluster_id}/namespaces/{namespace}/secrets", web::get().to(crate::controllers::secrets::list_secrets_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/secrets/{name}", web::get().to(crate::controllers::secrets::get_secret_redacted_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/secrets/{name}", web::put().to(crate::controllers::secrets::upsert_secret_plaintext_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/secrets/{name}", web::delete().to(crate::controllers::secrets::delete_secret_controller));

    // Jobs
    let scope = scope
        .route("/clusters/{cluster_id}/namespaces/{namespace}/jobs", web::get().to(crate::controllers::jobs::list_jobs_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/jobs/{name}", web::get().to(crate::controllers::jobs::get_job_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/jobs/{name}", web::put().to(crate::controllers::jobs::upsert_job_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/jobs/{name}", web::delete().to(crate::controllers::jobs::delete_job_controller));

    // CronJobs
    let scope = scope
        .route("/clusters/{cluster_id}/namespaces/{namespace}/cronjobs", web::get().to(crate::controllers::cronjobs::list_cronjobs_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/cronjobs/{name}", web::get().to(crate::controllers::cronjobs::get_cronjob_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/cronjobs/{name}", web::put().to(crate::controllers::cronjobs::upsert_cronjob_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/cronjobs/{name}", web::delete().to(crate::controllers::cronjobs::delete_cronjob_controller));

    // Ingress
    let scope = scope
        .route("/clusters/{cluster_id}/namespaces/{namespace}/ingress", web::get().to(crate::controllers::ingress::list_ingress_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/ingress/{name}", web::get().to(crate::controllers::ingress::get_ingress_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/ingress/{name}", web::put().to(crate::controllers::ingress::upsert_ingress_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace}/ingress/{name}", web::delete().to(crate::controllers::ingress::delete_ingress_controller));

    cfg.service(scope);
}
