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

macro_rules! test_route_mount {
    ($name:ident, $path:expr) => {
        #[tokio::test]
        async fn $name() {
            let harness = TestHarness::new().await;
            harness.test_delay().await;
            
            let res = harness
                .client()
                .get(&harness.build_url($path))
                .header("Authorization", format!("Bearer {}", harness.auth_token()))
                .send()
                .await
                .expect("failed to execute request");

            // We only care that the route is mounted and handled by the application logic,
            // so we assert it does not return 404 Not Found.
            assert_ne!(res.status().as_u16(), 404, "Route {} returned 404 Not Found", $path);
        }
    };
}

// Check some basic routes from our API modules to ensure they are mounted
test_route_mount!(test_auth_mount, "/api/v1/auth/me"); // Auth endpoint check
test_route_mount!(test_budget_mount, "/api/v1/budget");
test_route_mount!(test_chaos_mount, "/api/v1/chaos/experiments");
test_route_mount!(test_cloud_mount, "/api/v1/cloud/resources");
test_route_mount!(test_cluster_management_mount, "/api/v1/clusters");
test_route_mount!(test_data_source_mount, "/api/v1/data_sources");
test_route_mount!(test_database_mount, "/api/v1/database/connections");
test_route_mount!(test_explain_plan_mount, "/api/v1/explain-plan");
test_route_mount!(test_graphql_mount, "/api/v1/graphql");
test_route_mount!(test_llm_provider_mount, "/api/v1/llm-providers");
test_route_mount!(test_metrics_mount, "/api/v1/metrics");
test_route_mount!(test_prompt_template_mount, "/api/v1/prompt-templates");
test_route_mount!(test_query_fingerprint_mount, "/api/v1/query-fingerprint");
test_route_mount!(test_query_template_mount, "/api/v1/query-template");
test_route_mount!(test_slow_query_mount, "/api/v1/slow-query");
test_route_mount!(test_sync_run_mount, "/api/v1/sync-run");
test_route_mount!(test_aws_analytics_mount, "/api/v1/aws-analytics");
test_route_mount!(test_cloud_analytics_mount, "/api/v1/cloud-analytics");
test_route_mount!(test_ai_mount, "/api/ai/analyze/rds/123/perf");
test_route_mount!(test_llm_analytics_mount, "/api/v1/llm-analytics/history");
test_route_mount!(test_unified_llm_mount, "/api/llm/providers");
test_route_mount!(test_kubernetes_cluster_management_mount, "/api/kubernetes-clusters");
