pub mod client;
pub mod configmaps_service;
pub mod daemon_sets;
pub mod deployments_service;
pub mod metrics_service;
pub mod namespaces_service;
pub mod nodes_service;
pub mod persistent_volume_claims_service;
pub mod persistent_volumes_service;
pub mod pod; // Changed from pod_service
pub mod secrets_service;
pub mod services_service;
pub mod stateful_sets_service;

// Phase 2 services
pub mod authz_service;
pub mod cronjobs_service;
pub mod endpoints_service;
pub mod hpa_service;
pub mod ingress_service;
pub mod jobs_service;
pub mod limit_ranges_service;
pub mod network_policies_service;
pub mod nodes_ops_service;
pub mod pdb_service;
pub mod rbac_service;
pub mod resource_quotas_service;
pub mod service_accounts_service;

pub mod prelude {
    pub use super::authz_service::AuthorizationService;
    pub use super::cronjobs_service::CronJobsService;
    pub use super::daemon_sets::DaemonSetsService;
    pub use super::deployments_service::DeploymentsService;
    pub use super::endpoints_service::EndpointsService;
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
}
