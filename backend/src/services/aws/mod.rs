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


// Root module for AWS services
#[path = "aws_client_factory.rs"]
mod client_factory;
mod control_plane;
mod data_plane;
mod service;

// Re-export service structs
pub use aws_data_plane::cost_explorer::AwsCostService;
pub use control_plane::{AwsControlPlane, AwsControlPlaneTrait};
pub use data_plane::AwsDataPlane;
pub use service::AwsService;
mod aws_config_service;
pub mod aws_control_plane;
pub mod aws_data_plane;
pub mod aws_types;
