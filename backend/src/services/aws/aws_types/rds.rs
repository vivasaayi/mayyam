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


use serde::{Deserialize, Serialize};

// RDS-specific types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdsInstanceInfo {
    pub db_instance_identifier: String,
    pub engine: String,
    pub engine_version: String,
    pub instance_class: String,
    pub allocated_storage: i32,
    pub endpoint: Option<RdsEndpoint>,
    pub status: String,
    pub availability_zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdsEndpoint {
    pub address: String,
    pub port: i32,
    pub hosted_zone_id: String,
}
