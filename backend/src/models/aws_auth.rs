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


// Helper struct to pass authentication information when loading AWS SDK config
use crate::services::aws::aws_types::resource_sync::ResourceSyncRequest;

pub struct AccountAuthInfo {
    pub use_role: bool,
    pub role_arn: Option<String>,
    pub external_id: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
}

impl From<&ResourceSyncRequest> for AccountAuthInfo {
    fn from(req: &ResourceSyncRequest) -> Self {
        Self {
            use_role: req.use_role,
            role_arn: req.role_arn.clone(),
            external_id: req.external_id.clone(),
            access_key_id: req.access_key_id.clone(),
            secret_access_key: req.secret_access_key.clone(),
        }
    }
}
