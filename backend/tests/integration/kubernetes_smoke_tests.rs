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

use crate::integration::helpers::TestHarness;
use actix_web::{dev::Service as _, http::StatusCode, test, web, App, HttpMessage};
use mayyam::controllers::kubernetes::{
    get_admission_webhook_inventory_pillar_reports_controller,
    get_cluster_role_binding_inventory_pillar_reports_controller,
    get_cluster_role_inventory_pillar_reports_controller,
    get_configmap_inventory_pillar_reports_controller,
    get_cronjob_inventory_pillar_reports_controller,
    get_custom_resource_definition_inventory_pillar_reports_controller,
    get_custom_resource_inventory_pillar_reports_controller,
    get_daemonset_inventory_pillar_reports_controller,
    get_deployment_inventory_pillar_reports_controller,
    get_endpoint_slice_inventory_pillar_reports_controller,
    get_endpoints_inventory_pillar_reports_controller,
    get_event_inventory_pillar_reports_controller,
    get_gateway_api_inventory_pillar_reports_controller,
    get_hpa_inventory_pillar_reports_controller, get_ingress_inventory_pillar_reports_controller,
    get_job_inventory_pillar_reports_controller,
    get_limit_range_inventory_pillar_reports_controller,
    get_network_policy_inventory_pillar_reports_controller,
    get_node_drain_inventory_pillar_reports_controller,
    get_node_inventory_pillar_reports_controller,
    get_node_taint_inventory_pillar_reports_controller,
    get_pdb_inventory_pillar_reports_controller,
    get_persistent_volume_claim_inventory_pillar_reports_controller,
    get_persistent_volume_inventory_pillar_reports_controller,
    get_pod_exec_inventory_pillar_reports_controller, get_pod_inventory_pillar_reports_controller,
    get_pod_log_inventory_pillar_reports_controller,
    get_pod_security_standards_inventory_pillar_reports_controller,
    get_replicaset_inventory_pillar_reports_controller,
    get_resource_quota_inventory_pillar_reports_controller,
    get_role_binding_inventory_pillar_reports_controller,
    get_role_inventory_pillar_reports_controller, get_secret_inventory_pillar_reports_controller,
    get_service_account_inventory_pillar_reports_controller,
    get_service_inventory_pillar_reports_controller,
    get_statefulset_inventory_pillar_reports_controller,
    get_storage_class_inventory_pillar_reports_controller,
    get_volume_snapshot_inventory_pillar_reports_controller,
    get_vpa_inventory_pillar_reports_controller,
};
use mayyam::middleware::auth::Claims;
use mayyam::services::kubernetes::admission_webhooks_service::AdmissionWebhooksService;
use mayyam::services::kubernetes::configmaps_service::ConfigMapsService;
use mayyam::services::kubernetes::crds_service::CrdsService;
use mayyam::services::kubernetes::cronjobs_service::CronJobsService;
use mayyam::services::kubernetes::daemon_sets::DaemonSetsService;
use mayyam::services::kubernetes::deployments_service::DeploymentsService;
use mayyam::services::kubernetes::endpoints_service::EndpointsService;
use mayyam::services::kubernetes::gateway_api_service::GatewayApiService;
use mayyam::services::kubernetes::hpa_service::HorizontalPodAutoscalerService;
use mayyam::services::kubernetes::ingress_service::IngressService;
use mayyam::services::kubernetes::jobs_service::JobsService;
use mayyam::services::kubernetes::limit_ranges_service::LimitRangesService;
use mayyam::services::kubernetes::network_policies_service::NetworkPoliciesService;
use mayyam::services::kubernetes::node_drains_service::NodeDrainsService;
use mayyam::services::kubernetes::node_taints_service::NodeTaintsService;
use mayyam::services::kubernetes::nodes_service::NodesService;
use mayyam::services::kubernetes::pdb_service::PodDisruptionBudgetsService;
use mayyam::services::kubernetes::persistent_volume_claims_service::PersistentVolumeClaimsService;
use mayyam::services::kubernetes::persistent_volumes_service::PersistentVolumesService;
use mayyam::services::kubernetes::pod::PodService;
use mayyam::services::kubernetes::pod_security_standards_service::PodSecurityStandardsService;
use mayyam::services::kubernetes::rbac_service::RbacService;
use mayyam::services::kubernetes::replica_sets_service::ReplicaSetsService;
use mayyam::services::kubernetes::resource_quotas_service::ResourceQuotasService;
use mayyam::services::kubernetes::secrets_service::SecretsService;
use mayyam::services::kubernetes::service_accounts_service::ServiceAccountsService;
use mayyam::services::kubernetes::services_service::ServicesService;
use mayyam::services::kubernetes::stateful_sets_service::StatefulSetsService;
use mayyam::services::kubernetes::storage_classes_service::StorageClassesService;
use mayyam::services::kubernetes::volume_snapshots_service::VolumeSnapshotsService;
use mayyam::services::kubernetes::vpa_service::VerticalPodAutoscalerService;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;

#[tokio::test]
async fn kubernetes_cluster_inventory_pillar_reports_contract() {
    if let Some(harness) = TestHarness::try_new().await {
        let response = harness
            .client()
            .get(&harness.build_url("/api/kubernetes/inventory/clusters/pillars"))
            .header("Authorization", format!("Bearer {}", harness.auth_token()))
            .send()
            .await
            .expect("pillar report request failed");

        assert_eq!(response.status().as_u16(), 200);
        let body: Value = response.json().await.expect("invalid JSON body");
        assert_eq!(body["resource_type"], "KubernetesCluster");
        assert!(body["evaluated_at"].is_string());
        assert!(body["stale_after_hours"].is_number());
        assert!(body["resources_evaluated"].is_number());
        let reports = body["reports"].as_array().expect("reports array");
        assert_eq!(reports.len(), 3);
        for report in reports {
            assert!(report["pillar"].is_string());
            assert!(report["score"].is_number());
            assert!(report["findings"].is_array());
        }

        let response = harness
            .client()
            .get(&harness.build_url("/api/kubernetes/inventory/clusters/pillars?pillar=cost"))
            .header("Authorization", format!("Bearer {}", harness.auth_token()))
            .send()
            .await
            .expect("single pillar report request failed");
        assert_eq!(response.status().as_u16(), 200);
        let body: Value = response.json().await.expect("invalid JSON body");
        let reports = body["reports"].as_array().expect("reports array");
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0]["pillar"], "cost");

        let response = harness
            .client()
            .get(&harness.build_url("/api/kubernetes/inventory/clusters/pillars?pillar=bogus"))
            .header("Authorization", format!("Bearer {}", harness.auth_token()))
            .send()
            .await
            .expect("bad pillar request failed");
        assert_eq!(response.status().as_u16(), 400);
    } else {
        eprintln!("Skipping Kubernetes pillar contract: backend not healthy (likely DB down).");
    }
}

#[tokio::test]
async fn kubernetes_namespace_inventory_pillar_reports_contract() {
    if let Some(harness) = TestHarness::try_new().await {
        let response = harness
            .client()
            .get(&harness.build_url("/api/kubernetes/inventory/namespaces/pillars"))
            .header("Authorization", format!("Bearer {}", harness.auth_token()))
            .send()
            .await
            .expect("namespace pillar report request failed");

        assert_eq!(response.status().as_u16(), 200);
        let body: Value = response.json().await.expect("invalid JSON body");
        assert_eq!(body["resource_type"], "KubernetesNamespace");
        assert!(body["evaluated_at"].is_string());
        assert!(body["stale_after_hours"].is_number());
        assert!(body["resources_evaluated"].is_number());
        let reports = body["reports"].as_array().expect("reports array");
        assert_eq!(reports.len(), 3);
        for report in reports {
            assert!(report["pillar"].is_string());
            assert!(report["score"].is_number());
            assert!(report["findings"].is_array());
        }

        let response = harness
            .client()
            .get(&harness.build_url("/api/kubernetes/inventory/namespaces/pillars?pillar=security"))
            .header("Authorization", format!("Bearer {}", harness.auth_token()))
            .send()
            .await
            .expect("single namespace pillar report request failed");
        assert_eq!(response.status().as_u16(), 200);
        let body: Value = response.json().await.expect("invalid JSON body");
        let reports = body["reports"].as_array().expect("reports array");
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0]["pillar"], "security");

        let response = harness
            .client()
            .get(&harness.build_url("/api/kubernetes/inventory/namespaces/pillars?pillar=bogus"))
            .header("Authorization", format!("Bearer {}", harness.auth_token()))
            .send()
            .await
            .expect("bad namespace pillar request failed");
        assert_eq!(response.status().as_u16(), 400);
    } else {
        eprintln!(
            "Skipping Kubernetes namespace pillar contract: backend not healthy (likely DB down)."
        );
    }
}

#[tokio::test]
async fn kubernetes_node_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let nodes_service = Arc::new(NodesService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(nodes_service))
            .route(
                "/api/kubernetes/inventory/nodes/pillars",
                web::get().to(get_node_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/nodes/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesNode");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/nodes/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/nodes/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_node_taint_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let node_taints_service = Arc::new(NodeTaintsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(node_taints_service))
            .route(
                "/api/kubernetes/inventory/node-taints/pillars",
                web::get().to(get_node_taint_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/node-taints/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesNodeTaint");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/node-taints/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/node-taints/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_node_drain_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let node_drains_service = Arc::new(NodeDrainsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(node_drains_service))
            .route(
                "/api/kubernetes/inventory/node-drains/pillars",
                web::get().to(get_node_drain_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/node-drains/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesNodeDrain");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/node-drains/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/node-drains/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_pod_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let pod_service = Arc::new(PodService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(pod_service))
            .route(
                "/api/kubernetes/inventory/pods/pillars",
                web::get().to(get_pod_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pods/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesPod");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pods/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pods/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_pod_log_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let pod_service = Arc::new(PodService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(pod_service))
            .route(
                "/api/kubernetes/inventory/pod-logs/pillars",
                web::get().to(get_pod_log_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pod-logs/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesPodLog");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pod-logs/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pod-logs/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_pod_exec_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let pod_service = Arc::new(PodService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(pod_service))
            .route(
                "/api/kubernetes/inventory/pod-exec/pillars",
                web::get().to(get_pod_exec_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pod-exec/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesPodExec");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pod-exec/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pod-exec/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_deployment_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let deployments_service = Arc::new(DeploymentsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(deployments_service))
            .route(
                "/api/kubernetes/inventory/deployments/pillars",
                web::get().to(get_deployment_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/deployments/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesDeployment");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/deployments/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/deployments/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_replicaset_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let replica_sets_service = Arc::new(ReplicaSetsService);
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(replica_sets_service))
            .route(
                "/api/kubernetes/inventory/replicasets/pillars",
                web::get().to(get_replicaset_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/replicasets/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesReplicaSet");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/replicasets/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/replicasets/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_statefulset_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let stateful_sets_service = Arc::new(StatefulSetsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(stateful_sets_service))
            .route(
                "/api/kubernetes/inventory/statefulsets/pillars",
                web::get().to(get_statefulset_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/statefulsets/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesStatefulSet");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/statefulsets/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/statefulsets/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_daemonset_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let daemon_sets_service = Arc::new(DaemonSetsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(daemon_sets_service))
            .route(
                "/api/kubernetes/inventory/daemonsets/pillars",
                web::get().to(get_daemonset_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/daemonsets/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesDaemonSet");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/daemonsets/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/daemonsets/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_job_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let jobs_service = Arc::new(JobsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(jobs_service))
            .route(
                "/api/kubernetes/inventory/jobs/pillars",
                web::get().to(get_job_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/jobs/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesJob");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/jobs/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/jobs/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_cronjob_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let cronjobs_service = Arc::new(CronJobsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(cronjobs_service))
            .route(
                "/api/kubernetes/inventory/cronjobs/pillars",
                web::get().to(get_cronjob_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/cronjobs/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesCronJob");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/cronjobs/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/cronjobs/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_service_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let services_service = Arc::new(ServicesService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(services_service))
            .route(
                "/api/kubernetes/inventory/services/pillars",
                web::get().to(get_service_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/services/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesService");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/services/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/services/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_ingress_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let ingress_service = Arc::new(IngressService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(ingress_service))
            .route(
                "/api/kubernetes/inventory/ingresses/pillars",
                web::get().to(get_ingress_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/ingresses/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesIngress");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/ingresses/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/ingresses/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_gateway_api_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let gateway_api_service = Arc::new(GatewayApiService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(gateway_api_service))
            .route(
                "/api/kubernetes/inventory/gateway-api/pillars",
                web::get().to(get_gateway_api_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/gateway-api/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesGatewayApi");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/gateway-api/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/gateway-api/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_endpoints_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let endpoints_service = Arc::new(EndpointsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(endpoints_service))
            .route(
                "/api/kubernetes/inventory/endpoints/pillars",
                web::get().to(get_endpoints_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/endpoints/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesEndpoints");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/endpoints/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/endpoints/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_endpoint_slice_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let endpoints_service = Arc::new(EndpointsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(endpoints_service))
            .route(
                "/api/kubernetes/inventory/endpointslices/pillars",
                web::get().to(get_endpoint_slice_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/endpointslices/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesEndpointSlice");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/endpointslices/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/endpointslices/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_configmap_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let configmaps_service = Arc::new(ConfigMapsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(configmaps_service))
            .route(
                "/api/kubernetes/inventory/configmaps/pillars",
                web::get().to(get_configmap_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/configmaps/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesConfigMap");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/configmaps/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/configmaps/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_secret_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let secrets_service = Arc::new(SecretsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(secrets_service))
            .route(
                "/api/kubernetes/inventory/secrets/pillars",
                web::get().to(get_secret_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/secrets/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesSecret");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/secrets/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/secrets/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_service_account_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let service_accounts_service = Arc::new(ServiceAccountsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(service_accounts_service))
            .route(
                "/api/kubernetes/inventory/serviceaccounts/pillars",
                web::get().to(get_service_account_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/serviceaccounts/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesServiceAccount");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/serviceaccounts/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/serviceaccounts/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_role_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let rbac_service = Arc::new(RbacService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(rbac_service))
            .route(
                "/api/kubernetes/inventory/roles/pillars",
                web::get().to(get_role_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/roles/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesRole");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/roles/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/roles/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_role_binding_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let rbac_service = Arc::new(RbacService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(rbac_service))
            .route(
                "/api/kubernetes/inventory/rolebindings/pillars",
                web::get().to(get_role_binding_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/rolebindings/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesRoleBinding");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/rolebindings/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/rolebindings/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_cluster_role_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let rbac_service = Arc::new(RbacService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(rbac_service))
            .route(
                "/api/kubernetes/inventory/clusterroles/pillars",
                web::get().to(get_cluster_role_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/clusterroles/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesClusterRole");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/clusterroles/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/clusterroles/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_cluster_role_binding_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let rbac_service = Arc::new(RbacService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(rbac_service))
            .route(
                "/api/kubernetes/inventory/clusterrolebindings/pillars",
                web::get().to(get_cluster_role_binding_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/clusterrolebindings/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesClusterRoleBinding");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/clusterrolebindings/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/clusterrolebindings/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_network_policy_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let network_policies_service = Arc::new(NetworkPoliciesService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(network_policies_service))
            .route(
                "/api/kubernetes/inventory/networkpolicies/pillars",
                web::get().to(get_network_policy_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/networkpolicies/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesNetworkPolicy");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/networkpolicies/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/networkpolicies/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_hpa_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let hpa_service = Arc::new(HorizontalPodAutoscalerService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(hpa_service))
            .route(
                "/api/kubernetes/inventory/hpa/pillars",
                web::get().to(get_hpa_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/hpa/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesHorizontalPodAutoscaler");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/hpa/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/hpa/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_vpa_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let vpa_service = Arc::new(VerticalPodAutoscalerService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(vpa_service))
            .route(
                "/api/kubernetes/inventory/vpa/pillars",
                web::get().to(get_vpa_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/vpa/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesVerticalPodAutoscaler");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/vpa/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/vpa/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_pdb_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let pdb_service = Arc::new(PodDisruptionBudgetsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(pdb_service))
            .route(
                "/api/kubernetes/inventory/pdb/pillars",
                web::get().to(get_pdb_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pdb/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesPodDisruptionBudget");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pdb/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pdb/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_resource_quota_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let resource_quota_service = Arc::new(ResourceQuotasService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(resource_quota_service))
            .route(
                "/api/kubernetes/inventory/resourcequotas/pillars",
                web::get().to(get_resource_quota_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/resourcequotas/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesResourceQuota");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/resourcequotas/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/resourcequotas/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_limit_range_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let limit_ranges_service = Arc::new(LimitRangesService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(limit_ranges_service))
            .route(
                "/api/kubernetes/inventory/limitranges/pillars",
                web::get().to(get_limit_range_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/limitranges/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesLimitRange");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/limitranges/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/limitranges/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_persistent_volume_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let pv_service = Arc::new(PersistentVolumesService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(pv_service))
            .route(
                "/api/kubernetes/inventory/persistentvolumes/pillars",
                web::get().to(get_persistent_volume_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/persistentvolumes/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesPersistentVolume");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/persistentvolumes/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/persistentvolumes/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_persistent_volume_claim_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let pvc_service = Arc::new(PersistentVolumeClaimsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(pvc_service))
            .route(
                "/api/kubernetes/inventory/persistentvolumeclaims/pillars",
                web::get().to(get_persistent_volume_claim_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/persistentvolumeclaims/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesPersistentVolumeClaim");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/persistentvolumeclaims/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/persistentvolumeclaims/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_storage_class_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let storage_classes_service = Arc::new(StorageClassesService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(storage_classes_service))
            .route(
                "/api/kubernetes/inventory/storageclasses/pillars",
                web::get().to(get_storage_class_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/storageclasses/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesStorageClass");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/storageclasses/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/storageclasses/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_volume_snapshot_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let volume_snapshots_service = Arc::new(VolumeSnapshotsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(volume_snapshots_service))
            .route(
                "/api/kubernetes/inventory/volumesnapshots/pillars",
                web::get().to(get_volume_snapshot_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/volumesnapshots/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesVolumeSnapshot");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/volumesnapshots/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/volumesnapshots/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_custom_resource_definition_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let crds_service = Arc::new(CrdsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(crds_service))
            .route(
                "/api/kubernetes/inventory/customresourcedefinitions/pillars",
                web::get().to(get_custom_resource_definition_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/customresourcedefinitions/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesCustomResourceDefinition");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/customresourcedefinitions/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/customresourcedefinitions/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_admission_webhook_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let admission_webhooks_service = Arc::new(AdmissionWebhooksService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(admission_webhooks_service))
            .route(
                "/api/kubernetes/inventory/admission-webhooks/pillars",
                web::get().to(get_admission_webhook_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/admission-webhooks/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesAdmissionWebhook");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/admission-webhooks/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/admission-webhooks/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_pod_security_standards_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let pod_security_standards_service = Arc::new(PodSecurityStandardsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(pod_security_standards_service))
            .route(
                "/api/kubernetes/inventory/pod-security-standards/pillars",
                web::get().to(get_pod_security_standards_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pod-security-standards/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesPodSecurityStandard");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pod-security-standards/pillars?pillar=security")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "security");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/pod-security-standards/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_custom_resource_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let crds_service = Arc::new(CrdsService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(crds_service))
            .route(
                "/api/kubernetes/inventory/customresources/pillars",
                web::get().to(get_custom_resource_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/customresources/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesCustomResource");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/customresources/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/customresources/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn kubernetes_event_inventory_pillar_reports_contract() {
    let claims = Claims {
        sub: "test-user".to_string(),
        username: "test-user".to_string(),
        email: None,
        roles: vec!["admin".to_string()],
        exp: i64::MAX,
        iat: 0,
    };
    let db = Arc::new(DatabaseConnection::default());
    let pod_service = Arc::new(PodService::new());
    let app = test::init_service(
        App::new()
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(claims.clone());
                srv.call(req)
            })
            .app_data(web::Data::new(db))
            .app_data(web::Data::new(pod_service))
            .route(
                "/api/kubernetes/inventory/events/pillars",
                web::get().to(get_event_inventory_pillar_reports_controller),
            ),
    )
    .await;

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/events/pillars")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["resource_type"], "KubernetesEvent");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/events/pillars?pillar=resilience")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");

    let request = test::TestRequest::get()
        .uri("/api/kubernetes/inventory/events/pillars?pillar=bogus")
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_kubernetes_list_namespaces_empty_when_no_clusters() {
    if let Some(harness) = TestHarness::try_new().await {
        let response = harness
            .client()
            .get(&harness.build_url("/api/kubernetes/clusters/default/namespaces"))
            .header("Authorization", format!("Bearer {}", harness.auth_token()))
            .send()
            .await
            .expect("request failed");

        assert_eq!(response.status().as_u16(), 404);
    } else {
        eprintln!(
            "Skipping namespaces test: backend not healthy (likely DB down). Set up test DB or run with Docker."
        );
    }
}

#[tokio::test]
async fn test_kubernetes_health_route_exists() {
    if let Some(harness) = TestHarness::try_new().await {
        let resp = harness
            .client()
            .get(&harness.build_url("/health"))
            .send()
            .await
            .expect("health check failed");

        assert!(resp.status().is_success());
    } else {
        eprintln!("Skipping health test: backend not healthy (likely DB down). Set up test DB or run with Docker.");
    }
}
