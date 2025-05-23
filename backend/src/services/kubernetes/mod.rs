pub mod deployments_service;
pub mod stateful_sets_service;
pub mod daemon_sets;
pub mod pods; 

pub mod prelude {
    pub use super::deployments_service::DeploymentsService;
    pub use super::stateful_sets_service::StatefulSetsService;
    pub use super::daemon_sets::DaemonSetsService;
    pub use super::pods::PodService;
}