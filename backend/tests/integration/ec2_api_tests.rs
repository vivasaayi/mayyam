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
use serde_json::Value;

use crate::integration::helpers::server::base_url;

#[tokio::test]
async fn vpc_resources_list_flow() {
    // Determine whether to run AWS integration tests
    if std::env::var("ENABLE_AWS_TESTS").unwrap_or_else(|_| "false".to_string()) != "true" {
        println!("Skipping vpc_resources_list_flow because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();

    // 1. Create a dummy AWS account for testing
    let account_id = "123456789012";
    let create_payload = serde_json::json!({
        "account_id": account_id,
        "account_name": "vpc-test-account",
        "profile": "default",
        "default_region": "us-east-1",
        "use_role": false,
        "auth_type": "auto"
    });

    let res = client
        .post(&format!("{}/api/aws/accounts", base))
        .json(&create_payload)
        .send()
        .await
        .expect("Failed to post AWS account");

    assert!(res.status().is_success() || res.status().as_u16() == 409, "Account creation/check failed");

    async fn test_endpoint(
        client: Client,
        base: String,
        account_id: String,
        endpoint_path: &str,
        resource_name: &str,
    ) {
        let url = format!("{}/api/aws/accounts/{}/regions/{}/{}", base, account_id, "us-east-1", endpoint_path);
        
        let res = client
            .get(&url)
            .send()
            .await
            .unwrap_or_else(|_| panic!("Failed to send GET to {}", url));

        assert!(
            res.status().is_success(),
            "Expected 200 OK for {}, got {}",
            resource_name,
            res.status()
        );

        let res_json: Value = res
            .json()
            .await
            .unwrap_or_else(|_| panic!("invalid JSON from list {}", resource_name));

        match &res_json {
            Value::Array(arr) => {
                println!("{} listing returned {} items.", resource_name, arr.len());
            }
            Value::Object(_) => {
                // It could be a paginated response
                let items = res_json.get("resources").and_then(|v| v.as_array());
                assert!(items.is_some(), "Expected 'resources' array in {} response", resource_name);
                println!("{} paginated listing returned {} items.", resource_name, items.unwrap().len());
            }
            _ => panic!("Expected array or object from list {}", resource_name),
        }
    }

    // 2. Test fetching VPC resources
    test_endpoint(client.clone(), base.clone(), account_id.to_string(), "vpcs", "VPCs").await;
    test_endpoint(client.clone(), base.clone(), account_id.to_string(), "subnets", "Subnets").await;
    test_endpoint(client.clone(), base.clone(), account_id.to_string(), "security-groups", "Security Groups").await;
    test_endpoint(client.clone(), base.clone(), account_id.to_string(), "route-tables", "Route Tables").await;
    test_endpoint(client.clone(), base.clone(), account_id.to_string(), "internet-gateways", "Internet Gateways").await;
    test_endpoint(client.clone(), base.clone(), account_id.to_string(), "nat-gateways", "NAT Gateways").await;
    test_endpoint(client.clone(), base.clone(), account_id.to_string(), "network-acls", "Network ACLs").await;

    // 3. Clean up the test account
    let del_res = client
        .delete(&format!("{}/api/aws/accounts/{}", base, account_id))
        .send()
        .await
        .expect("Failed to delete AWS account");

    assert!(
        del_res.status().is_success() || del_res.status().as_u16() == 404,
        "Account cleanup failed"
    );
}
