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

/// Helper function to test a resource list endpoint.
/// Verifies the endpoint returns HTTP 200 with a valid JSON array or paginated response.
async fn test_resource_endpoint(
    client: Client,
    base: String,
    account_id: String,
    endpoint_path: &str,
    resource_name: &str,
) {
    let url = format!(
        "{}/api/aws/accounts/{}/regions/{}/{}",
        base, account_id, "us-east-1", endpoint_path
    );

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
            // Handle paginated responses
            let items = res_json.get("resources").and_then(|v| v.as_array());
            assert!(
                items.is_some(),
                "Expected 'resources' array in {} response",
                resource_name
            );
            println!(
                "{} paginated listing returned {} items.",
                resource_name,
                items.unwrap().len()
            );
        }
        _ => panic!("Expected array or object from list {}", resource_name),
    }
}

/// Helper to create a test AWS account and return the account_id
async fn setup_test_account(client: &Client, base: &str, account_id: &str, name: &str) {
    let create_payload = serde_json::json!({
        "account_id": account_id,
        "account_name": name,
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

    assert!(
        res.status().is_success() || res.status().as_u16() == 409,
        "Account creation/check failed"
    );
}

/// Helper to clean up a test AWS account
async fn cleanup_test_account(client: &Client, base: &str, account_id: &str) {
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

// =============================================================================
// Batch 2: Security & Compliance API Tests
// =============================================================================

#[tokio::test]
async fn security_compliance_resources_list_flow() {
    if std::env::var("ENABLE_AWS_TESTS").unwrap_or_else(|_| "false".to_string()) != "true" {
        println!("Skipping security_compliance_resources_list_flow because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let account_id = "123456789012";

    setup_test_account(&client, &base, account_id, "security-compliance-test").await;

    // KMS Keys
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "kms-keys",
        "KMS Keys",
    )
    .await;

    // ACM Certificates
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "acm-certificates",
        "ACM Certificates",
    )
    .await;

    // CloudTrail Trails
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "cloudtrail-trails",
        "CloudTrail Trails",
    )
    .await;

    // Config Rules
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "config-rules",
        "Config Rules",
    )
    .await;

    cleanup_test_account(&client, &base, account_id).await;
}

// =============================================================================
// Batch 3: Containers & Serverless API Tests
// =============================================================================

#[tokio::test]
async fn containers_serverless_resources_list_flow() {
    if std::env::var("ENABLE_AWS_TESTS").unwrap_or_else(|_| "false".to_string()) != "true" {
        println!("Skipping containers_serverless_resources_list_flow because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let account_id = "123456789012";

    setup_test_account(&client, &base, account_id, "containers-serverless-test").await;

    // ECS Clusters
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "ecs-clusters",
        "ECS Clusters",
    )
    .await;

    // EKS Clusters
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "eks-clusters",
        "EKS Clusters",
    )
    .await;

    // App Runner Services
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "apprunner-services",
        "App Runner Services",
    )
    .await;

    // Batch Compute Environments
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "batch-compute-envs",
        "Batch Compute Environments",
    )
    .await;

    cleanup_test_account(&client, &base, account_id).await;
}

// =============================================================================
// Batch 4: Management & Monitoring API Tests
// =============================================================================

#[tokio::test]
async fn management_monitoring_resources_list_flow() {
    if std::env::var("ENABLE_AWS_TESTS").unwrap_or_else(|_| "false".to_string()) != "true" {
        println!("Skipping management_monitoring_resources_list_flow because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let account_id = "123456789012";

    setup_test_account(&client, &base, account_id, "management-monitoring-test").await;

    // CloudWatch Alarms
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "cloudwatch-alarms",
        "CloudWatch Alarms",
    )
    .await;

    // SSM Documents
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "ssm-documents",
        "SSM Documents",
    )
    .await;

    cleanup_test_account(&client, &base, account_id).await;
}

// =============================================================================
// Batch 5: Application Integration API Tests
// =============================================================================

#[tokio::test]
async fn application_integration_resources_list_flow() {
    if std::env::var("ENABLE_AWS_TESTS").unwrap_or_else(|_| "false".to_string()) != "true" {
        println!("Skipping application_integration_resources_list_flow because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let account_id = "123456789012";

    setup_test_account(&client, &base, account_id, "app-integration-test").await;

    // EventBridge Rules
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "eventbridge-rules",
        "EventBridge Rules",
    )
    .await;

    // Step Functions
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "step-functions",
        "Step Functions",
    )
    .await;

    // SES Identities
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "ses-identities",
        "SES Identities",
    )
    .await;

    cleanup_test_account(&client, &base, account_id).await;
}

// =============================================================================
// Batch 6: Analytics & Big Data API Tests
// =============================================================================

#[tokio::test]
async fn analytics_bigdata_resources_list_flow() {
    if std::env::var("ENABLE_AWS_TESTS").unwrap_or_else(|_| "false".to_string()) != "true" {
        println!("Skipping analytics_bigdata_resources_list_flow because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let account_id = "123456789012";

    setup_test_account(&client, &base, account_id, "analytics-bigdata-test").await;

    // Redshift Clusters
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "redshift-clusters",
        "Redshift Clusters",
    )
    .await;

    // EMR Clusters
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "emr-clusters",
        "EMR Clusters",
    )
    .await;

    // Athena Workgroups
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "athena-workgroups",
        "Athena Workgroups",
    )
    .await;

    // Glue Databases
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "glue-databases",
        "Glue Databases",
    )
    .await;

    cleanup_test_account(&client, &base, account_id).await;
}

// =============================================================================
// Batch 7: Edge & DR API Tests
// =============================================================================

#[tokio::test]
async fn edge_dr_resources_list_flow() {
    if std::env::var("ENABLE_AWS_TESTS").unwrap_or_else(|_| "false".to_string()) != "true" {
        println!("Skipping edge_dr_resources_list_flow because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let account_id = "123456789012";

    setup_test_account(&client, &base, account_id, "edge-dr-test").await;

    // WAF Web ACLs
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "waf-web-acls",
        "WAF Web ACLs",
    )
    .await;

    // Global Accelerators
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "global-accelerators",
        "Global Accelerators",
    )
    .await;

    // Backup Vaults
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "backup-vaults",
        "Backup Vaults",
    )
    .await;

    cleanup_test_account(&client, &base, account_id).await;
}

// =============================================================================
// Existing Resource Types Endpoint Tests (from Batch 1 wiring)
// =============================================================================

#[tokio::test]
async fn existing_resources_list_endpoints_flow() {
    if std::env::var("ENABLE_AWS_TESTS").unwrap_or_else(|_| "false".to_string()) != "true" {
        println!("Skipping existing_resources_list_endpoints_flow because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let account_id = "123456789012";

    setup_test_account(&client, &base, account_id, "existing-resources-test").await;

    // SNS Topics
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "sns-topics",
        "SNS Topics",
    )
    .await;

    // Lambda Functions
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "lambda-functions",
        "Lambda Functions",
    )
    .await;

    // OpenSearch Domains
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "opensearch-domains",
        "OpenSearch Domains",
    )
    .await;

    // SQS Queues
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "sqs-queues",
        "SQS Queues",
    )
    .await;

    // Kinesis Streams
    test_resource_endpoint(
        client.clone(),
        base.clone(),
        account_id.to_string(),
        "kinesis-streams",
        "Kinesis Streams",
    )
    .await;

    cleanup_test_account(&client, &base, account_id).await;
}

// =============================================================================
// Comprehensive All-Endpoints Smoke Test
// =============================================================================

#[tokio::test]
async fn all_aws_resource_endpoints_smoke_test() {
    if std::env::var("ENABLE_AWS_TESTS").unwrap_or_else(|_| "false".to_string()) != "true" {
        println!("Skipping all_aws_resource_endpoints_smoke_test because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let account_id = "123456789012";

    setup_test_account(&client, &base, account_id, "all-endpoints-smoke-test").await;

    // All resource endpoints in a single comprehensive smoke test
    let endpoints = vec![
        // Compute & Networking
        ("ec2-instances", "EC2 Instances"),
        ("vpcs", "VPCs"),
        ("subnets", "Subnets"),
        ("security-groups", "Security Groups"),
        ("route-tables", "Route Tables"),
        ("internet-gateways", "Internet Gateways"),
        ("nat-gateways", "NAT Gateways"),
        ("network-acls", "Network ACLs"),
        // Storage
        ("s3-buckets", "S3 Buckets"),
        ("ebs-volumes", "EBS Volumes"),
        ("ebs-snapshots", "EBS Snapshots"),
        ("efs-file-systems", "EFS File Systems"),
        // Database
        ("rds-instances", "RDS Instances"),
        ("dynamodb-tables", "DynamoDB Tables"),
        ("elasticache-clusters", "ElastiCache Clusters"),
        ("opensearch-domains", "OpenSearch Domains"),
        // Messaging & Streaming
        ("sqs-queues", "SQS Queues"),
        ("sns-topics", "SNS Topics"),
        ("kinesis-streams", "Kinesis Streams"),
        // Serverless
        ("lambda-functions", "Lambda Functions"),
        // Security & Compliance (Batch 2)
        ("kms-keys", "KMS Keys"),
        ("acm-certificates", "ACM Certificates"),
        ("cloudtrail-trails", "CloudTrail Trails"),
        ("config-rules", "Config Rules"),
        // Containers & Serverless (Batch 3)
        ("ecs-clusters", "ECS Clusters"),
        ("eks-clusters", "EKS Clusters"),
        ("apprunner-services", "App Runner Services"),
        ("batch-compute-envs", "Batch Compute Envs"),
        // Management & Monitoring (Batch 4)
        ("cloudwatch-alarms", "CloudWatch Alarms"),
        ("ssm-documents", "SSM Documents"),
        // Application Integration (Batch 5)
        ("eventbridge-rules", "EventBridge Rules"),
        ("step-functions", "Step Functions"),
        ("ses-identities", "SES Identities"),
        // Analytics & Big Data (Batch 6)
        ("redshift-clusters", "Redshift Clusters"),
        ("emr-clusters", "EMR Clusters"),
        ("athena-workgroups", "Athena Workgroups"),
        ("glue-databases", "Glue Databases"),
        // Edge & DR (Batch 7)
        ("waf-web-acls", "WAF Web ACLs"),
        ("global-accelerators", "Global Accelerators"),
        ("backup-vaults", "Backup Vaults"),
    ];

    let mut passed = 0;
    let mut failed = 0;

    for (endpoint, name) in &endpoints {
        let url = format!(
            "{}/api/aws/accounts/{}/regions/{}/{}",
            base, account_id, "us-east-1", endpoint
        );

        match client.get(&url).send().await {
            Ok(res) => {
                if res.status().is_success() {
                    println!("✅ {} ({}): OK", name, endpoint);
                    passed += 1;
                } else {
                    eprintln!("❌ {} ({}): HTTP {}", name, endpoint, res.status());
                    failed += 1;
                }
            }
            Err(e) => {
                eprintln!("❌ {} ({}): Request failed: {}", name, endpoint, e);
                failed += 1;
            }
        }
    }

    println!(
        "\n📊 Smoke test results: {} passed, {} failed out of {} endpoints",
        passed,
        failed,
        endpoints.len()
    );

    assert_eq!(failed, 0, "Some resource endpoints failed the smoke test");

    cleanup_test_account(&client, &base, account_id).await;
}
