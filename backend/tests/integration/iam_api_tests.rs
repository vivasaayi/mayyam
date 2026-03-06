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

use crate::integration::helpers::{get_aws_credentials, get_test_account_id, TestHarness};
use serde_json::json;

#[tokio::test]
async fn iam_resources_list_flow() {
    let harness = TestHarness::new().await;

    if !harness.aws_tests_enabled() {
        eprintln!("Skipping IAM resources list test: set ENABLE_AWS_TESTS=1 to run");
        return;
    }

    harness.test_delay().await;

    let (access_key, secret_key, region, _) = get_aws_credentials();
    let account_identifier = get_test_account_id();

    // 1. Create an AWS account to use for testing
    let create_payload = json!({
        "account_id": account_identifier,
        "account_name": "IAM Test Account",
        "profile": "default",
        "default_region": region,
        "use_role": false,
        "access_key_id": access_key,
        "secret_access_key": secret_key
    });

    let create_response = harness
        .client()
        .post(&harness.build_url("/api/aws/accounts"))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&create_payload)
        .send()
        .await
        .expect("failed to create aws account");

    assert_eq!(create_response.status().as_u16(), 201);
    let created_account: serde_json::Value = create_response.json().await.unwrap();
    let account_db_id = created_account["id"].as_str().unwrap();

    // 2. Test IAM Users endpoint
    let users_resp = harness
        .client()
        .get(&harness.build_url(&format!("/api/aws/{}/iam/users", account_db_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to fetch iam users");
    
    assert_eq!(users_resp.status().as_u16(), 200);
    let users: serde_json::Value = users_resp.json().await.unwrap();
    assert!(users.as_array().is_some());

    // 3. Test IAM Roles endpoint
    let roles_resp = harness
        .client()
        .get(&harness.build_url(&format!("/api/aws/{}/iam/roles", account_db_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to fetch iam roles");
    
    assert_eq!(roles_resp.status().as_u16(), 200);
    let roles: serde_json::Value = roles_resp.json().await.unwrap();
    assert!(roles.as_array().is_some());

    // 4. Test IAM Policies endpoint
    let policies_resp = harness
        .client()
        .get(&harness.build_url(&format!("/api/aws/{}/iam/policies", account_db_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to fetch iam policies");
    
    assert_eq!(policies_resp.status().as_u16(), 200);
    let policies: serde_json::Value = policies_resp.json().await.unwrap();
    assert!(policies.as_array().is_some());

    // 5. Test IAM Groups endpoint
    let groups_resp = harness
        .client()
        .get(&harness.build_url(&format!("/api/aws/{}/iam/groups", account_db_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to fetch iam groups");
    
    assert_eq!(groups_resp.status().as_u16(), 200);
    let groups: serde_json::Value = groups_resp.json().await.unwrap();
    assert!(groups.as_array().is_some());

    // 6. Cleanup
    let delete_response = harness
        .client()
        .delete(&harness.build_url(&format!("/api/aws/accounts/{}", account_db_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to delete account");
    
    assert_eq!(delete_response.status().as_u16(), 204);
}
