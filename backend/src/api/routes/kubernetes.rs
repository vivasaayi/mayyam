use actix_web::{web, HttpResponse};
use crate::controllers::kubernetes as kube_controller;

pub fn configure(cfg: &mut web::ServiceConfig) {
    let scope = web::scope("/api/kubernetes")
        .route("/clusters", web::get().to(kube_controller::list_clusters_controller))
        .route("/clusters", web::post().to(kube_controller::create_cluster_controller))
        .route("/clusters/{cluster_id}", web::get().to(kube_controller::get_cluster_controller))
        .route("/clusters/{cluster_id}", web::put().to(kube_controller::update_cluster_controller))
        .route("/clusters/{cluster_id}", web::delete().to(kube_controller::delete_cluster_controller))
        
        .route("/clusters/{cluster_id}/namespaces", web::get().to(kube_controller::list_namespaces_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}", web::get().to(kube_controller::get_namespace_details_controller))

        .route("/clusters/{cluster_id}/nodes", web::get().to(kube_controller::list_nodes_controller))
        .route("/clusters/{cluster_id}/nodes/{node_name}", web::get().to(kube_controller::get_node_details_controller))

        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/pods", web::get().to(kube_controller::list_pods_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/pods/{pod_name}", web::get().to(kube_controller::get_pod_details_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/pods/{pod_name}/logs", web::get().to(kube_controller::get_pod_logs_controller))
        
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/services", web::get().to(kube_controller::list_services_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/services/{service_name}", web::get().to(kube_controller::get_service_details_controller))

        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/deployments", web::get().to(kube_controller::list_deployments_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/deployments/{deployment_name}", web::get().to(kube_controller::get_deployment_details_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/deployments/{deployment_name}/pods", web::get().to(kube_controller::get_pods_for_deployment_controller))


        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/statefulsets", web::get().to(kube_controller::list_stateful_sets_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/statefulsets/{stateful_set_name}", web::get().to(kube_controller::get_stateful_set_details_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/statefulsets/{stateful_set_name}/pods", web::get().to(kube_controller::get_pods_for_stateful_set_controller))

        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/daemonsets", web::get().to(kube_controller::list_daemon_sets_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/daemonsets/{daemon_set_name}", web::get().to(kube_controller::get_daemon_set_details_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/daemonsets/{daemon_set_name}/pods", web::get().to(kube_controller::get_pods_for_daemon_set_controller))

        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/persistentvolumeclaims", web::get().to(kube_controller::list_pvcs_controller))
        .route("/clusters/{cluster_id}/namespaces/{namespace_name}/persistentvolumeclaims/{pvc_name}", web::get().to(kube_controller::get_pvc_details_controller))
        
        .route("/clusters/{cluster_id}/persistentvolumes", web::get().to(kube_controller::list_pvs_controller))
        .route("/clusters/{cluster_id}/persistentvolumes/{pv_name}", web::get().to(kube_controller::get_pv_details_controller));

    cfg.service(scope);
}
