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
use uuid::Uuid;

// Common Request/Response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSyncRequest {
    pub sync_id: Uuid,
    pub account_id: String,
    pub profile: Option<String>,
    pub region: String,
    pub resource_types: Option<Vec<String>>,
    // Authentication fields
    pub use_role: bool,
    pub role_arn: Option<String>,
    pub external_id: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSyncResponse {
    pub summary: Vec<ResourceTypeSyncSummary>,
    pub total_resources: usize,
    pub sync_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTypeSyncSummary {
    pub resource_type: String,
    pub count: usize,
    pub status: String,
    pub details: Option<String>,
}
