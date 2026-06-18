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

// API contract tests for /api/aws/inventory/ec2/pillars.

use reqwest::Client;
use serde_json::Value;

use crate::integration::helpers::server::base_url;

fn aws_tests_enabled() -> bool {
    std::env::var("ENABLE_AWS_TESTS").unwrap_or_else(|_| "false".to_string()) == "true"
}

#[tokio::test]
async fn ec2_pillar_reports_contract() {
    if !aws_tests_enabled() {
        println!("Skipping ec2_pillar_reports_contract because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let account_id = "123456789012";

    // Happy path: all supported EC2 pillar reports with freshness metadata.
    let resp = client
        .get(format!(
            "{}/api/aws/inventory/ec2/pillars?account_id={}",
            base, account_id
        ))
        .send()
        .await
        .expect("pillar report request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    assert_eq!(body["account_id"], account_id);
    assert_eq!(body["resource_type"], "EC2Instance");
    assert!(body["evaluated_at"].is_string());
    assert!(body["stale_after_hours"].is_number());
    assert!(body["resources_evaluated"].is_number());
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 7);
    for report in reports {
        assert!(report["pillar"].is_string());
        assert!(report["score"].is_number());
        assert!(report["findings"].is_array());
    }

    // Single pillar selection.
    let resp = client
        .get(format!(
            "{}/api/aws/inventory/ec2/pillars?account_id={}&pillar=cost",
            base, account_id
        ))
        .send()
        .await
        .expect("cost pillar request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "cost");
    assert_eq!(reports[0]["posture"]["rules_evaluated"], 4);
    assert!(reports[0]["posture"]["rules"].is_array());
    assert!(reports[0]["triage_context"]["facts"].is_array());
    assert!(reports[0]["triage_context"]["hypotheses"].is_array());
    assert!(reports[0]["triage_context"]["missing_data_questions"].is_array());
    assert_eq!(
        reports[0]["remediation_workflow"]["workflow_id"],
        "ec2_cost_safe_remediation"
    );
    assert_eq!(reports[0]["remediation_workflow"]["read_only_mode"], true);
    assert_eq!(
        reports[0]["remediation_workflow"]["rbac_permission"],
        "aws.ec2.cost.remediation.approve"
    );
    assert!(reports[0]["remediation_workflow"]["actions"].is_array());
    assert!(reports[0]["remediation_workflow"]["approval_gates"].is_array());
    assert_eq!(
        reports[0]["slo_policy_tracking"]["workflow_id"],
        "ec2_cost_slo_policy"
    );
    assert_eq!(reports[0]["slo_policy_tracking"]["read_only_mode"], true);
    assert_eq!(
        reports[0]["slo_policy_tracking"]["objective"]["objective_id"],
        "ec2-cost-score-min-90"
    );
    assert!(reports[0]["slo_policy_tracking"]["objective"]["status_history"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["evidence_reason_codes"].is_array());
    assert_eq!(
        reports[0]["forecasting"]["workflow_id"],
        "ec2_cost_forecasting"
    );
    assert_eq!(reports[0]["forecasting"]["read_only_mode"], true);
    assert_eq!(reports[0]["forecasting"]["baseline_window_days"], 30);
    assert_eq!(reports[0]["forecasting"]["forecast_horizon_days"], 30);
    assert!(reports[0]["forecasting"]["forecast_band"]["expected_monthly_cost_index"].is_number());
    assert!(reports[0]["forecasting"]["risk_drivers"].is_array());
    assert!(reports[0]["forecasting"]["missing_data_reason_codes"].is_array());
    assert_eq!(reports[0]["reporting"]["workflow_id"], "ec2_cost_reporting");
    assert_eq!(reports[0]["reporting"]["read_only_mode"], true);
    assert_eq!(
        reports[0]["reporting"]["executive_summary"]["report_id"],
        "ec2-cost-executive-summary"
    );
    assert!(reports[0]["reporting"]["executive_summary"]["top_reason_codes"].is_array());
    assert_eq!(
        reports[0]["reporting"]["engineering_backlog"]["report_id"],
        "ec2-cost-engineering-backlog"
    );
    assert_eq!(reports[0]["reporting"]["engineering_backlog"]["page"], 0);
    assert_eq!(
        reports[0]["reporting"]["engineering_backlog"]["page_size"],
        50
    );
    assert!(reports[0]["reporting"]["engineering_backlog"]["rows"].is_array());
    assert!(reports[0]["reporting"]["evidence_reason_codes"].is_array());

    let resp = client
        .get(format!(
            "{}/api/aws/inventory/ec2/pillars?account_id={}&pillar=resilience",
            base, account_id
        ))
        .send()
        .await
        .expect("resilience pillar request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "resilience");
    assert_eq!(
        reports[0]["assessment_scope"],
        "ec2_instance_placement_and_status_checks"
    );
    assert_eq!(reports[0]["posture"]["rules_evaluated"], 4);
    let resilience_rules = reports[0]["posture"]["rules"]
        .as_array()
        .expect("resilience posture rules");
    assert_eq!(resilience_rules.len(), 4);
    let expected_rules = [
        (
            "ec2-resilience-availability-zone-recorded",
            "EC2_RES_MISSING_AVAILABILITY_ZONE",
        ),
        (
            "ec2-resilience-multi-az-placement",
            "EC2_RES_SINGLE_AZ_CONCENTRATION",
        ),
        (
            "ec2-resilience-status-check-telemetry",
            "EC2_RES_MISSING_STATUS_TELEMETRY",
        ),
        (
            "ec2-resilience-status-check-health",
            "EC2_RES_STATUS_CHECK_FAILURE_TELEMETRY",
        ),
    ];
    for (rule, (expected_rule_id, expected_reason_code)) in
        resilience_rules.iter().zip(expected_rules)
    {
        assert_eq!(rule["rule_id"], expected_rule_id);
        assert_eq!(rule["reason_codes"][0], expected_reason_code);
    }
    assert!(reports[0]["posture"]["affected_resources"].is_array());
    assert_eq!(
        reports[0]["triage_context"]["workflow_id"],
        "ec2_resilience_triage_context"
    );
    assert_eq!(
        reports[0]["triage_context"]["context_builder_id"],
        "ec2-resilience-deterministic-context-v1"
    );
    assert_eq!(
        reports[0]["triage_context"]["generation_mode"],
        "deterministic_no_llm"
    );
    assert_eq!(
        reports[0]["triage_context"]["guardrails"]["read_only_mode"],
        true
    );
    assert_eq!(
        reports[0]["triage_context"]["guardrails"]["evidence_required"],
        true
    );
    assert_eq!(
        reports[0]["triage_context"]["guardrails"]["no_llm_invocation"],
        true
    );
    assert_eq!(
        reports[0]["triage_context"]["guardrails"]["no_mutation_planning"],
        true
    );
    assert!(reports[0]["triage_context"]["facts"].is_array());
    assert!(reports[0]["triage_context"]["hypotheses"].is_array());
    assert!(reports[0]["triage_context"]["missing_data_questions"].is_array());
    assert!(reports[0]["triage_context"]["evidence_citations"].is_array());
    assert_eq!(
        reports[0]["agentic_investigation"]["workflow_id"],
        "ec2_resilience_agentic_investigation"
    );
    assert_eq!(
        reports[0]["agentic_investigation"]["default_tool_mode"],
        "read_only"
    );
    assert_eq!(reports[0]["agentic_investigation"]["replay_required"], true);
    assert!(reports[0]["agentic_investigation"]["steps"].is_array());
    assert!(reports[0]["agentic_investigation"]["approval_gates"].is_array());
    assert!(reports[0]["agentic_investigation"]["evidence_citations"].is_array());
    let investigation_steps = reports[0]["agentic_investigation"]["steps"]
        .as_array()
        .expect("resilience investigation steps should be an array");
    assert!(investigation_steps.iter().all(|step| {
        let tool_name = step["tool_name"].as_str().unwrap_or_default();
        tool_name.starts_with("ec2.")
            && !tool_name.contains("execute")
            && !tool_name.contains("run_instances")
            && !tool_name.contains("terminate")
    }));
    assert!(investigation_steps.iter().all(|step| {
        step["tool_mode"] == "read_only"
            || (step["tool_mode"] == "approval_required"
                && step["tool_name"] == "ec2.resilience.prepare_approval_plan")
    }));
    if let Some(final_step) = investigation_steps.last() {
        if final_step["tool_mode"] == "approval_required" {
            assert_eq!(
                final_step["reason_code"],
                "EC2_RESILIENCE_APPROVAL_PLAN_REQUIRED"
            );
            assert_eq!(
                final_step["evidence"]["assessment_scope"],
                "ec2_instance_placement_and_status_checks"
            );
        }
    }
}

#[tokio::test]
async fn lambda_pillar_reports_contract() {
    if !aws_tests_enabled() {
        println!("Skipping lambda_pillar_reports_contract because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let resp = client
        .get(format!(
            "{}/api/aws/inventory/lambda/pillars?account_id=123456789012",
            base
        ))
        .send()
        .await
        .expect("lambda pillar report request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    assert_eq!(body["resource_type"], "LambdaFunction");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
}

#[tokio::test]
async fn s3_pillar_reports_contract() {
    if !aws_tests_enabled() {
        println!("Skipping s3_pillar_reports_contract because ENABLE_AWS_TESTS is not true");
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let resp = client
        .get(format!(
            "{}/api/aws/inventory/s3/pillars?account_id=123456789012",
            base
        ))
        .send()
        .await
        .expect("s3 pillar report request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    assert_eq!(body["resource_type"], "S3Bucket");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 3);
}

#[tokio::test]
async fn storage_and_database_pillar_reports_contract() {
    if !aws_tests_enabled() {
        println!(
            "Skipping storage_and_database_pillar_reports_contract because ENABLE_AWS_TESTS is not true"
        );
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    for (path, resource_type, expected_reports) in [
        ("rds", "RdsInstance", 3),
        ("ebs", "EbsVolume", 3),
        ("efs", "EfsFileSystem", 3),
        ("ecs", "EcsClusterAndService", 3),
        ("eks", "EksCluster", 3),
        ("dynamodb", "DynamoDbTable", 3),
        ("sqs", "SqsQueue", 3),
        ("sns", "SnsTopic", 3),
        ("kinesis", "KinesisStream", 3),
        ("elasticache", "ElasticacheCluster", 3),
        ("opensearch", "OpenSearchDomain", 3),
        ("vpc", "Vpc", 3),
        ("iam", "IamUserRolePolicyAndGroup", 3),
        ("cloudfront", "CloudFrontDistribution", 3),
        ("elb", "AlbNlbAndElb", 3),
        ("apigateway", "ApiGatewayRestApiStageAndMethod", 3),
        ("cloudwatch", "CloudWatchAlarmAndDashboard", 3),
        ("appsync", "AppSyncApi", 3),
        ("glacier", "GlacierArchive", 3),
        ("storagegateway", "StorageGateway", 3),
        ("kinesisanalytics", "KinesisAnalyticsApp", 3),
        ("subnet", "Subnet", 3),
        ("securitygroup", "SecurityGroup", 3),
        ("natgateway", "NatGateway", 3),
        ("internetgateway", "InternetGateway", 3),
        ("routetable", "RouteTable", 3),
        ("networkacl", "NetworkAcl", 3),
        ("fargate", "FargateProfile", 3),
        ("kms", "KmsKey", 3),
        ("acm", "AcmCertificate", 3),
        ("cloudtrail", "CloudTrailTrail", 3),
        ("config", "ConfigRule", 7),
        ("eventbridge", "EventBridgeRule", 7),
        ("stepfunctions", "StepFunction", 7),
        ("apprunner", "AppRunnerService", 3),
        ("athena", "AthenaWorkgroup", 3),
        ("ssm", "SsmDocument", 3),
        ("backup", "BackupVault", 3),
        ("batch", "BatchComputeEnv", 3),
        ("emr", "EmrCluster", 3),
        ("globalaccelerator", "GlobalAccelerator", 3),
        ("glue", "GlueDatabase", 3),
        ("redshift", "RedshiftCluster", 3),
        ("waf", "WafWebAcl", 3),
        ("autoscaling", "AutoScalingGroup", 3),
        ("cloudwatchmetrics", "CloudWatchMetric", 3),
        ("cloudwatchlogs", "CloudWatchLogGroup", 3),
        ("route53", "Route53HostedZone", 3),
        ("transitgateway", "TransitGateway", 3),
        ("secretsmanager", "SecretsManagerSecret", 3),
        ("aurora", "AuroraCluster", 3),
        ("msk", "MskCluster", 3),
        ("guardduty", "GuardDutyDetector", 3),
        ("securityhub", "SecurityHubHub", 3),
        ("inspector", "InspectorAccountCoverage", 3),
        ("macie", "MacieAccount", 3),
        ("organizations", "OrganizationsOrganization", 3),
        ("controltower", "ControlTowerLandingZone", 3),
        ("servicecatalog", "ServiceCatalogPortfolio", 3),
        ("trustedadvisor", "TrustedAdvisorAccount", 3),
        ("computeoptimizer", "ComputeOptimizerAccount", 3),
        ("health", "HealthAccount", 3),
        ("resiliencehub", "ResilienceHubAccount", 3),
        ("documentdb", "DocumentDbCluster", 3),
        ("neptune", "NeptuneCluster", 3),
        ("memorydb", "MemoryDbCluster", 3),
        ("elasticbeanstalk", "ElasticBeanstalkEnvironment", 3),
        ("datasync", "DataSyncTask", 3),
        ("fsx", "FsxFileSystem", 3),
        ("timestream", "TimestreamTable", 3),
        ("firehose", "FirehoseDeliveryStream", 3),
        ("lakeformation", "LakeFormationDataLake", 3),
        ("lightsail", "LightsailResource", 3),
        ("quicksight", "QuickSightAsset", 3),
        ("dms", "DmsResource", 3),
        ("mgn", "MgnResource", 3),
        ("drs", "DrsResource", 3),
        ("bedrock", "BedrockResource", 3),
        ("sagemaker", "SageMakerResource", 3),
        ("textract", "TextractResource", 3),
        ("comprehend", "ComprehendResource", 3),
        ("amazonmq", "AmazonMqBroker", 3),
        ("privatelink", "VpcEndpoint", 3),
        ("shield", "ShieldProtection", 3),
    ] {
        let resp = client
            .get(format!(
                "{}/api/aws/inventory/{}/pillars?account_id=123456789012",
                base, path
            ))
            .send()
            .await
            .unwrap_or_else(|e| panic!("{} pillar report request failed: {}", path, e));
        assert_eq!(resp.status(), 200, "endpoint {}", path);
        let body: Value = resp.json().await.expect("invalid JSON body");
        assert_eq!(body["resource_type"], resource_type);
        assert_eq!(
            body["reports"].as_array().expect("reports").len(),
            expected_reports,
            "endpoint {}",
            path
        );
    }
}

#[tokio::test]
async fn ec2_pillar_reports_rejects_unknown_pillar() {
    if !aws_tests_enabled() {
        println!(
            "Skipping ec2_pillar_reports_rejects_unknown_pillar because ENABLE_AWS_TESTS is not true"
        );
        return;
    }

    let base = base_url().await;
    let client = Client::new();
    let resp = client
        .get(format!(
            "{}/api/aws/inventory/ec2/pillars?account_id=123456789012&pillar=bogus",
            base
        ))
        .send()
        .await
        .expect("bad pillar request failed");
    assert_eq!(resp.status(), 400);
}
