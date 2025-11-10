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
async fn aws_account_crud_flow() {
    let harness = TestHarness::new().await;

    if !harness.aws_tests_enabled() {
        eprintln!("Skipping AWS account CRUD test: set ENABLE_AWS_TESTS=1 to run");
        return;
    }

    harness.test_delay().await;

    let (access_key, secret_key, region, _) = get_aws_credentials();
    let account_identifier = get_test_account_id();

    let create_payload = json!({
        "account_id": account_identifier,
        "account_name": "Integration Test AWS Account",
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

    let created_account: serde_json::Value = create_response
        .json()
        .await
        .expect("failed to parse account creation response");
    let account_id = created_account["id"]
        .as_str()
        .expect("account id missing")
        .to_string();

    let fetched = harness
        .client()
        .get(&harness.build_url(&format!("/api/aws/accounts/{}", account_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to fetch account");

    assert_eq!(fetched.status().as_u16(), 200);
    let fetched_body: serde_json::Value = fetched
        .json()
        .await
        .expect("failed to parse fetched account");
    assert_eq!(fetched_body["account_id"], account_identifier);

    let list = harness
        .client()
        .get(&harness.build_url("/api/aws/accounts"))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to list accounts");

    assert_eq!(list.status().as_u16(), 200);
    let list_body: serde_json::Value = list
        .json()
        .await
        .expect("failed to parse list response");
    assert!(list_body.as_array().is_some(), "accounts list should be an array");

    let update_payload = json!({
        "account_name": "Integration Test AWS Account Updated",
        "default_region": region,
        "profile": "test-profile"
    });

    let update_response = harness
        .client()
        .put(&harness.build_url(&format!("/api/aws/accounts/{}", account_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .json(&update_payload)
        .send()
        .await
        .expect("failed to update account");

    assert_eq!(update_response.status().as_u16(), 200);
    let updated_body: serde_json::Value = update_response
        .json()
        .await
        .expect("failed to parse update response");
    assert_eq!(updated_body["account_name"], "Integration Test AWS Account Updated");

    let delete_response = harness
        .client()
        .delete(&harness.build_url(&format!("/api/aws/accounts/{}", account_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to delete account");

    assert_eq!(delete_response.status().as_u16(), 204);

    let verify_deleted = harness
        .client()
        .get(&harness.build_url(&format!("/api/aws/accounts/{}", account_id)))
        .header("Authorization", format!("Bearer {}", harness.auth_token()))
        .send()
        .await
        .expect("failed to verify deletion");

    assert_eq!(verify_deleted.status().as_u16(), 404);
}
