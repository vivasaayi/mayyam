pub mod deployments_service;
pub mod stateful_sets_service;
pub mod daemon_sets;
pub mod pods; 
pub mod services_service;
pub mod nodes_service;
pub mod namespaces_service;
pub mod persistent_volume_claims_service;
pub mod persistent_volumes_service; // Added


pub mod prelude {
    pub use super::deployments_service::DeploymentsService;
    pub use super::stateful_sets_service::StatefulSetsService; 
    pub use super::daemon_sets::DaemonSetsService;
    pub use super::pods::PodService;
    pub use super::services_service::ServicesService;
    pub use super::nodes_service::NodesService;
    pub use super::namespaces_service::NamespacesService;
    pub use super::persistent_volume_claims_service::PersistentVolumeClaimsService;
    pub use super::persistent_volumes_service::PersistentVolumesService; // Added
}