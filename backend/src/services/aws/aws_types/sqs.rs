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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqsSendMessageRequest {
    pub queue_url: String,
    pub message_body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqsReceiveMessageRequest {
    pub queue_url: String,
    pub max_number_of_messages: Option<i32>,
    pub visibility_timeout: Option<i32>,
    pub wait_time_seconds: Option<i32>,
}
