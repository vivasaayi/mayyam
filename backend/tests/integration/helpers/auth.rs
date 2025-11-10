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


use reqwest::Client;

use super::server::ensure_server;

/// Log in with default admin credentials and return JWT token
pub async fn get_auth_token() -> String {
    ensure_server().await;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("failed to build reqwest client");

    let base_url =
        std::env::var("TEST_API_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    let payload = serde_json::json!({
        "username": "admin",
        "password": "admin123",
    });

    let resp = client
        .post(format!("{}/api/auth/login", base_url))
        .json(&payload)
        .send()
        .await
        .expect("login request failed");

    assert!(
        resp.status().is_success(),
        "login failed: {}",
        resp.status()
    );

    let body: serde_json::Value = resp.json().await.expect("invalid login response");
    body["token"].as_str().expect("missing token").to_string()
}
