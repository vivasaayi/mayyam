use crate::controllers::kubernetes as kube_controller;
use actix_web::web;
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
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/configmaps",
            web::get().to(crate::controllers::configmaps::list_configmaps_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/configmaps/{name}",
            web::get().to(crate::controllers::configmaps::get_configmap_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/configmaps/{name}",
            web::put().to(crate::controllers::configmaps::upsert_configmap_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/configmaps/{name}",
            web::delete().to(crate::controllers::configmaps::delete_configmap_controller),
        );

    // Secrets (redacted get)
    let scope = scope
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/secrets",
            web::get().to(crate::controllers::secrets::list_secrets_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/secrets/{name}",
            web::get().to(crate::controllers::secrets::get_secret_redacted_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/secrets/{name}",
            web::put().to(crate::controllers::secrets::upsert_secret_plaintext_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/secrets/{name}",
            web::delete().to(crate::controllers::secrets::delete_secret_controller),
        );

    // Jobs
    let scope = scope
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/jobs",
            web::get().to(crate::controllers::jobs::list_jobs_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/jobs/{name}",
            web::get().to(crate::controllers::jobs::get_job_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/jobs/{name}",
            web::put().to(crate::controllers::jobs::upsert_job_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/jobs/{name}",
            web::delete().to(crate::controllers::jobs::delete_job_controller),
        );

    // CronJobs
    let scope = scope
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/cronjobs",
            web::get().to(crate::controllers::cronjobs::list_cronjobs_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/cronjobs/{name}",
            web::get().to(crate::controllers::cronjobs::get_cronjob_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/cronjobs/{name}",
            web::put().to(crate::controllers::cronjobs::upsert_cronjob_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/cronjobs/{name}",
            web::delete().to(crate::controllers::cronjobs::delete_cronjob_controller),
        );

    // Ingress
    let scope = scope
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/ingress",
            web::get().to(crate::controllers::ingress::list_ingress_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/ingress/{name}",
            web::get().to(crate::controllers::ingress::get_ingress_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/ingress/{name}",
            web::put().to(crate::controllers::ingress::upsert_ingress_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/ingress/{name}",
            web::delete().to(crate::controllers::ingress::delete_ingress_controller),
        )
        // Aliases with canonical plural "/ingresses"
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/ingresses",
            web::get().to(crate::controllers::ingress::list_ingress_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/ingresses/{name}",
            web::get().to(crate::controllers::ingress::get_ingress_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/ingresses/{name}",
            web::put().to(crate::controllers::ingress::upsert_ingress_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/ingresses/{name}",
            web::delete().to(crate::controllers::ingress::delete_ingress_controller),
        );

    // Endpoints & EndpointSlices
    let scope = scope
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/endpoints",
            web::get().to(crate::controllers::endpoints::list_endpoints_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/endpointslices",
            web::get().to(crate::controllers::endpoints::list_endpoint_slices_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/endpoints/{name}",
            web::get().to(crate::controllers::endpoints::get_endpoints_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/endpoints/{name}",
            web::put().to(crate::controllers::endpoints::upsert_endpoints_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/endpoints/{name}",
            web::delete().to(crate::controllers::endpoints::delete_endpoints_controller),
        );

    // NetworkPolicies
    let scope = scope
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/networkpolicies",
            web::get().to(crate::controllers::network_policies::list_network_policies_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/networkpolicies/{name}",
            web::get().to(crate::controllers::network_policies::get_network_policy_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/networkpolicies/{name}",
            web::put().to(crate::controllers::network_policies::upsert_network_policy_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/networkpolicies/{name}",
            web::delete()
                .to(crate::controllers::network_policies::delete_network_policy_controller),
        );

    // HPA
    let scope = scope
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/hpa",
            web::get().to(crate::controllers::hpa::list_hpa_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/hpa/{name}",
            web::get().to(crate::controllers::hpa::get_hpa_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/hpa/{name}",
            web::put().to(crate::controllers::hpa::upsert_hpa_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/hpa/{name}",
            web::delete().to(crate::controllers::hpa::delete_hpa_controller),
        )
        // Aliases using full resource name
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/horizontalpodautoscalers",
            web::get().to(crate::controllers::hpa::list_hpa_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/horizontalpodautoscalers/{name}",
            web::get().to(crate::controllers::hpa::get_hpa_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/horizontalpodautoscalers/{name}",
            web::put().to(crate::controllers::hpa::upsert_hpa_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/horizontalpodautoscalers/{name}",
            web::delete().to(crate::controllers::hpa::delete_hpa_controller),
        );

    // PodDisruptionBudget
    let scope = scope
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/poddisruptionbudgets",
            web::get().to(crate::controllers::pdb::list_pdb_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/poddisruptionbudgets/{name}",
            web::get().to(crate::controllers::pdb::get_pdb_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/poddisruptionbudgets/{name}",
            web::put().to(crate::controllers::pdb::upsert_pdb_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/poddisruptionbudgets/{name}",
            web::delete().to(crate::controllers::pdb::delete_pdb_controller),
        );

    // ResourceQuota
    let scope = scope
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/resourcequotas",
            web::get().to(crate::controllers::resource_quotas::list_resource_quotas_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/resourcequotas/{name}",
            web::get().to(crate::controllers::resource_quotas::get_resource_quota_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/resourcequotas/{name}",
            web::put().to(crate::controllers::resource_quotas::upsert_resource_quota_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/resourcequotas/{name}",
            web::delete().to(crate::controllers::resource_quotas::delete_resource_quota_controller),
        );

    // LimitRanges
    let scope = scope
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/limitranges",
            web::get().to(crate::controllers::limit_ranges::list_limit_ranges_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/limitranges/{name}",
            web::get().to(crate::controllers::limit_ranges::get_limit_range_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/limitranges/{name}",
            web::put().to(crate::controllers::limit_ranges::upsert_limit_range_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/limitranges/{name}",
            web::delete().to(crate::controllers::limit_ranges::delete_limit_range_controller),
        );

    // ServiceAccounts
    let scope = scope
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/serviceaccounts",
            web::get().to(crate::controllers::service_accounts::list_service_accounts_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/serviceaccounts/{name}",
            web::get().to(crate::controllers::service_accounts::get_service_account_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/serviceaccounts/{name}",
            web::put().to(crate::controllers::service_accounts::upsert_service_account_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/serviceaccounts/{name}",
            web::delete()
                .to(crate::controllers::service_accounts::delete_service_account_controller),
        );

    // RBAC - namespaced
    let scope = scope
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/roles",
            web::get().to(crate::controllers::rbac::list_roles_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/roles/{name}",
            web::get().to(crate::controllers::rbac::get_role_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/roles/{name}",
            web::put().to(crate::controllers::rbac::upsert_role_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/roles/{name}",
            web::delete().to(crate::controllers::rbac::delete_role_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/rolebindings",
            web::get().to(crate::controllers::rbac::list_role_bindings_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/rolebindings/{name}",
            web::get().to(crate::controllers::rbac::get_role_binding_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/rolebindings/{name}",
            web::put().to(crate::controllers::rbac::upsert_role_binding_controller),
        )
        .route(
            "/clusters/{cluster_id}/namespaces/{namespace}/rolebindings/{name}",
            web::delete().to(crate::controllers::rbac::delete_role_binding_controller),
        );

    // RBAC - cluster scoped
    let scope = scope
        .route(
            "/clusters/{cluster_id}/clusterroles",
            web::get().to(crate::controllers::rbac::list_cluster_roles_controller),
        )
        .route(
            "/clusters/{cluster_id}/clusterroles/{name}",
            web::get().to(crate::controllers::rbac::get_cluster_role_controller),
        )
        .route(
            "/clusters/{cluster_id}/clusterroles/{name}",
            web::put().to(crate::controllers::rbac::upsert_cluster_role_controller),
        )
        .route(
            "/clusters/{cluster_id}/clusterroles/{name}",
            web::delete().to(crate::controllers::rbac::delete_cluster_role_controller),
        )
        .route(
            "/clusters/{cluster_id}/clusterrolebindings",
            web::get().to(crate::controllers::rbac::list_cluster_role_bindings_controller),
        )
        .route(
            "/clusters/{cluster_id}/clusterrolebindings/{name}",
            web::get().to(crate::controllers::rbac::get_cluster_role_binding_controller),
        )
        .route(
            "/clusters/{cluster_id}/clusterrolebindings/{name}",
            web::put().to(crate::controllers::rbac::upsert_cluster_role_binding_controller),
        )
        .route(
            "/clusters/{cluster_id}/clusterrolebindings/{name}",
            web::delete().to(crate::controllers::rbac::delete_cluster_role_binding_controller),
        );

    // AuthZ check
    let scope = scope.route(
        "/clusters/{cluster_id}/authz:can",
        web::post().to(crate::controllers::authz::authz_can_controller),
    );

    // Node ops
    let scope = scope
        .route(
            "/clusters/{cluster_id}/nodes/{node}:cordon",
            web::post().to(crate::controllers::node_ops::cordon_node_controller),
        )
        .route(
            "/clusters/{cluster_id}/nodes/{node}:uncordon",
            web::post().to(crate::controllers::node_ops::uncordon_node_controller),
        )
        .route(
            "/clusters/{cluster_id}/nodes/{node}:addTaint",
            web::post().to(crate::controllers::node_ops::add_taint_controller),
        )
        .route(
            "/clusters/{cluster_id}/nodes/{node}:removeTaint",
            web::post().to(crate::controllers::node_ops::remove_taint_controller),
        );

    cfg.service(scope);
}
