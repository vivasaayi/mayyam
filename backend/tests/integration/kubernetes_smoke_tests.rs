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


use crate::integration::helpers::TestHarness;

#[tokio::test]
async fn test_kubernetes_list_namespaces_empty_when_no_clusters() {
    if let Some(harness) = TestHarness::try_new().await {
        let response = harness
            .client()
            .get(&harness.build_url("/api/kubernetes/clusters/default/namespaces"))
            .header("Authorization", format!("Bearer {}", harness.auth_token()))
            .send()
            .await
            .expect("request failed");

        assert_eq!(response.status().as_u16(), 404);
    } else {
        eprintln!(
            "Skipping namespaces test: backend not healthy (likely DB down). Set up test DB or run with Docker."
        );
    }
}

#[tokio::test]
async fn test_kubernetes_health_route_exists() {
    if let Some(harness) = TestHarness::try_new().await {
        let resp = harness
            .client()
            .get(&harness.build_url("/health"))
            .send()
            .await
            .expect("health check failed");

        assert!(resp.status().is_success());
    } else {
        eprintln!("Skipping health test: backend not healthy (likely DB down). Set up test DB or run with Docker.");
    }
}
