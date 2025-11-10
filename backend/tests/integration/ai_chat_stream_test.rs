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


#![cfg(feature = "integration-tests")]

use crate::integration::helpers::TestHarness;
use serde_json::json;

#[tokio::test]
async fn chat_stream_empty_messages_returns_400() {
    let harness = TestHarness::new().await;

    let payload = json!({
        "messages": [],
        "model": null,
        "temperature": 1.0
    });

    let response = harness
        .client()
        .post(&harness.build_url("/api/ai/chat/stream"))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&payload)
        .send()
        .await
        .expect("chat stream request failed");

    assert_eq!(response.status().as_u16(), 400);
}
