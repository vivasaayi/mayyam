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

// Lambda Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaInvokeRequest {
    pub function_name: String,
    pub payload: serde_json::Value,
    pub invocation_type: Option<String>,
    pub client_context: Option<String>,
    pub qualifier: Option<String>,
}
