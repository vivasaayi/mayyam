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


use once_cell::sync::OnceCell;
use std::time::{Duration, Instant};

static BASE_URL: OnceCell<String> = OnceCell::new();

async fn wait_for_health(base_url: &str, timeout: Duration) -> bool {
    let client = reqwest::Client::new();
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(resp) = client.get(format!("{}/health", base_url)).send().await {
            if resp.status().is_success() {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    false
}

/// Ensure the test server is running and return the base URL.
pub async fn ensure_server() -> String {
    if let Some(url) = BASE_URL.get() {
        return url.clone();
    }

    let mut candidate_urls = Vec::new();

    if let Ok(url) = std::env::var("TEST_API_BASE_URL") {
        candidate_urls.push(url);
    }

    if let Some(port) = std::env::var("BACKEND_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
    {
        candidate_urls.push(format!("http://127.0.0.1:{}", port));
    }

    candidate_urls.push("http://127.0.0.1:8080".to_string());

    for url in candidate_urls {
        if wait_for_health(&url, Duration::from_secs(5)).await {
            BASE_URL.set(url.clone()).ok();
            std::env::set_var("TEST_API_BASE_URL", url.clone());
            return url;
        }
    }

    panic!(
        "Backend server not reachable. Start the Mayyam backend first or set TEST_API_BASE_URL."
    );
}

/// Try to start the server; return None if health never becomes ready within timeout.
pub async fn try_ensure_server() -> Option<String> {
    if let Some(url) = BASE_URL.get() {
        return Some(url.clone());
    }

    let mut candidate_urls = Vec::new();

    if let Ok(url) = std::env::var("TEST_API_BASE_URL") {
        candidate_urls.push(url);
    }

    if let Some(port) = std::env::var("BACKEND_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
    {
        candidate_urls.push(format!("http://127.0.0.1:{}", port));
    }

    candidate_urls.push("http://127.0.0.1:8080".to_string());

    for url in candidate_urls {
        if wait_for_health(&url, Duration::from_secs(5)).await {
            BASE_URL.set(url.clone()).ok();
            std::env::set_var("TEST_API_BASE_URL", url.clone());
            return Some(url);
        }
    }

    None
}

/// Returns the base URL. Starts the server if not yet running.
pub async fn base_url() -> String {
    if let Some(url) = BASE_URL.get() {
        url.clone()
    } else {
        ensure_server().await
    }
}
