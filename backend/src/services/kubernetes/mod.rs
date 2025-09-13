pub mod deployments_service;
pub mod stateful_sets_service;
pub mod daemon_sets;
pub mod pod; // Changed from pod_service
pub mod services_service;
pub mod nodes_service;
pub mod namespaces_service;
pub mod persistent_volume_claims_service;
pub mod persistent_volumes_service;
pub mod client;
pub mod configmaps_service;
pub mod secrets_service;


pub mod prelude {
    pub use super::deployments_service::DeploymentsService;
    pub use super::stateful_sets_service::StatefulSetsService; 
    pub use super::daemon_sets::DaemonSetsService;
    pub use super::pod::PodService; // Changed from pod_service
    pub use super::services_service::ServicesService;
    pub use super::nodes_service::NodesService;
    pub use super::namespaces_service::NamespacesService;
    pub use super::persistent_volume_claims_service::PersistentVolumeClaimsService;
    pub use super::persistent_volumes_service::PersistentVolumesService;
}