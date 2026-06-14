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

pub mod admission_webhook_inventory;
pub mod admission_webhooks_service;
pub mod client;
pub mod cluster_role_binding_inventory;
pub mod cluster_role_inventory;
pub mod configmap_inventory;
pub mod configmaps_service;
pub mod cronjob_inventory;
pub mod custom_resource_definition_inventory;
pub mod custom_resource_inventory;
pub mod daemon_set_inventory;
pub mod daemon_sets;
pub mod deployment_inventory;
pub mod deployments_service;
pub mod metrics_service;
pub mod namespace_inventory;
pub mod namespaces_service;
pub mod node_drains_inventory;
pub mod node_drains_service;
pub mod node_inventory;
pub mod node_taints_service;
pub mod nodes_service;
pub mod persistent_volume_claims_service;
pub mod persistent_volumes_service;
pub mod pod; // Changed from pod_service
pub mod pod_exec_inventory;
pub mod pod_inventory;
pub mod pod_log_inventory;
pub mod pod_security_standards_inventory;
pub mod pod_security_standards_service;
pub mod replica_set_inventory;
pub mod role_binding_inventory;
pub mod role_inventory;
pub mod secret_inventory;
pub mod secrets_service;
pub mod service_account_inventory;
pub mod service_inventory;
pub mod services_service;
pub mod stateful_set_inventory;
pub mod stateful_sets_service;

// Phase 2 services
pub mod authz_service;
pub mod crds_service;
pub mod cronjobs_service;
pub mod endpoint_slice_inventory;
pub mod endpoints_inventory;
pub mod endpoints_service;
pub mod event_inventory;
pub mod gateway_api_inventory;
pub mod gateway_api_service;
pub mod hpa_inventory;
pub mod hpa_service;
pub mod ingress_inventory;
pub mod ingress_service;
pub mod inventory;
pub mod job_inventory;
pub mod jobs_service;
pub mod limit_range_inventory;
pub mod limit_ranges_service;
pub mod network_policies_service;
pub mod network_policy_inventory;
pub mod node_taints_inventory;
pub mod nodes_ops_service;
pub mod pdb_inventory;
pub mod pdb_service;
pub mod persistent_volume_claim_inventory;
pub mod persistent_volume_inventory;
pub mod rbac_service;
pub mod replica_sets_service;
pub mod resource_quota_inventory;
pub mod resource_quotas_service;
pub mod service_accounts_service;
pub mod storage_class_inventory;
pub mod storage_classes_service;
pub mod volume_snapshot_inventory;
pub mod volume_snapshots_service;
pub mod vpa_inventory;
pub mod vpa_service;

pub mod prelude {
    pub use super::authz_service::AuthorizationService;
    pub use super::crds_service::CrdsService;
    pub use super::cronjobs_service::CronJobsService;
    pub use super::daemon_sets::DaemonSetsService;
    pub use super::deployments_service::DeploymentsService;
    pub use super::endpoints_service::EndpointsService;
    pub use super::gateway_api_service::GatewayApiService;
    pub use super::hpa_service::HorizontalPodAutoscalerService;
    pub use super::ingress_service::IngressService;
    pub use super::jobs_service::JobsService;
    pub use super::limit_ranges_service::LimitRangesService;
    pub use super::metrics_service::MetricsService;
    pub use super::namespaces_service::NamespacesService;
    pub use super::network_policies_service::NetworkPoliciesService;
    pub use super::nodes_ops_service::NodeOpsService;
    pub use super::nodes_service::NodesService;
    pub use super::pdb_service::PodDisruptionBudgetsService;
    pub use super::persistent_volume_claims_service::PersistentVolumeClaimsService;
    pub use super::persistent_volumes_service::PersistentVolumesService;
    pub use super::pod::PodService; // Changed from pod_service
    pub use super::rbac_service::RbacService;
    pub use super::resource_quotas_service::ResourceQuotasService;
    pub use super::service_accounts_service::ServiceAccountsService;
    pub use super::services_service::ServicesService;
    pub use super::stateful_sets_service::StatefulSetsService;
    pub use super::storage_classes_service::StorageClassesService;
    pub use super::volume_snapshots_service::VolumeSnapshotsService;
    pub use super::vpa_service::VerticalPodAutoscalerService;
}
