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
    assert_eq!(
        reports[0]["remediation_workflow"]["workflow_id"],
        "ec2_resilience_safe_remediation"
    );
    assert_eq!(reports[0]["remediation_workflow"]["read_only_mode"], true);
    assert_eq!(
        reports[0]["remediation_workflow"]["rbac_permission"],
        "aws.ec2.resilience.remediation.approve"
    );
    assert_eq!(
        reports[0]["remediation_workflow"]["audit_stream"],
        "ec2_resilience_remediation_audit"
    );
    assert!(reports[0]["remediation_workflow"]["actions"].is_array());
    assert!(reports[0]["remediation_workflow"]["approval_gates"].is_array());
    let remediation_actions = reports[0]["remediation_workflow"]["actions"]
        .as_array()
        .expect("resilience remediation actions should be an array");
    assert!(remediation_actions.iter().all(|action| {
        action["dry_run"] == true
            && action["requires_approval"] == true
            && !action["approval_gate_id"].is_null()
            && action["audit_event_type"] == "ec2.resilience.remediation.dry_run_planned"
            && (action["status"] == "dry_run_pending_approval"
                || action["status"] == "blocked_missing_evidence")
            && action["rollback_note"]
                .as_str()
                .map(|note| note.contains("rollback"))
                .unwrap_or(false)
    }));
    assert_eq!(
        reports[0]["slo_policy_tracking"]["workflow_id"],
        "ec2_resilience_slo_policy"
    );
    assert_eq!(reports[0]["slo_policy_tracking"]["read_only_mode"], true);
    assert_eq!(
        reports[0]["slo_policy_tracking"]["objective"]["objective_id"],
        "ec2-resilience-score-min-95"
    );
    assert_eq!(
        reports[0]["slo_policy_tracking"]["objective"]["target_score_min"],
        95
    );
    assert_eq!(
        reports[0]["slo_policy_tracking"]["freshness_required"],
        true
    );
    assert!(matches!(
        reports[0]["slo_policy_tracking"]["objective"]["status"].as_str(),
        Some("on_track" | "at_risk" | "breached")
    ));
    assert!(matches!(
        reports[0]["slo_policy_tracking"]["objective"]["trend_direction"].as_str(),
        Some("stable" | "degrading")
    ));
    assert!(matches!(
        reports[0]["slo_policy_tracking"]["objective"]["policy_state"].as_str(),
        Some("active" | "active_with_findings" | "blocked_stale_data")
    ));
    assert!(reports[0]["slo_policy_tracking"]["objective"]["notification_targets"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["objective"]["status_history"].is_array());
    assert!(reports[0]["slo_policy_tracking"]["evidence_reason_codes"].is_array());
    assert_eq!(
        reports[0]["forecasting"]["workflow_id"],
        "ec2_resilience_forecasting"
    );
    assert_eq!(reports[0]["forecasting"]["read_only_mode"], true);
    assert_eq!(reports[0]["forecasting"]["baseline_window_days"], 30);
    assert_eq!(reports[0]["forecasting"]["forecast_horizon_days"], 30);
    assert_eq!(reports[0]["forecasting"]["confidence_level"], 75);
    assert_eq!(
        reports[0]["forecasting"]["forecast_band"]["horizon_days"],
        30
    );
    assert!(
        reports[0]["forecasting"]["forecast_band"]["expected_recovery_exposure_index"].is_number()
    );
    assert!(matches!(
        reports[0]["forecasting"]["risk_level"].as_str(),
        Some("low" | "moderate" | "high" | "blocked")
    ));
    assert!(reports[0]["forecasting"]["recovery_capacity_risk"].is_string());
    assert!(reports[0]["forecasting"]["backtesting_fixture_status"].is_string());
    assert!(reports[0]["forecasting"]["threshold_controls"].is_array());
    assert!(reports[0]["forecasting"]["what_if_inputs"].is_array());
    assert!(reports[0]["forecasting"]["blocked_by_stale_data"].is_boolean());
    assert!(reports[0]["forecasting"]["blast_radius_summary"].is_string());
    assert!(reports[0]["forecasting"]["recovery_note"]
        .as_str()
        .map(|note| note.contains("read-only") && note.contains("approval"))
        .unwrap_or(false));
    assert!(reports[0]["forecasting"]["missing_data_reason_codes"].is_array());
    assert!(reports[0]["forecasting"]["risk_drivers"].is_array());
    assert!(reports[0]["forecasting"]["evidence_reason_codes"].is_array());
    assert_eq!(
        reports[0]["reporting"]["workflow_id"],
        "ec2_resilience_reporting"
    );
    assert_eq!(reports[0]["reporting"]["read_only_mode"], true);
    assert!(matches!(
        reports[0]["reporting"]["scheduled_delivery_state"].as_str(),
        Some(
            "ready_for_schedule"
                | "ready_with_resilience_evidence_gaps"
                | "blocked_until_fresh_resilience_evidence"
        )
    ));
    assert!(reports[0]["reporting"]["portfolio_summary_ready"].is_boolean());
    assert!(reports[0]["reporting"]["workload_summary_ready"].is_boolean());
    assert!(reports[0]["reporting"]["stale_data_blocks_delivery"].is_boolean());
    assert_eq!(
        reports[0]["reporting"]["executive_summary"]["report_id"],
        "ec2-resilience-executive-summary"
    );
    assert!(reports[0]["reporting"]["executive_summary"]["top_reason_codes"].is_array());
    assert!(reports[0]["reporting"]["executive_summary"]["blast_radius_summary"].is_string());
    assert_eq!(
        reports[0]["reporting"]["incident_review"]["report_id"],
        "ec2-resilience-incident-review"
    );
    assert_eq!(reports[0]["reporting"]["incident_review"]["page"], 0);
    assert_eq!(reports[0]["reporting"]["incident_review"]["page_size"], 50);
    assert!(reports[0]["reporting"]["incident_review"]["rows"].is_array());
    assert!(reports[0]["reporting"]["missing_data_reason_codes"].is_array());
    assert!(reports[0]["reporting"]["evidence_reason_codes"].is_array());

    let resp = client
        .get(format!(
            "{}/api/aws/inventory/ec2/pillars?account_id={}&pillar=performance",
            base, account_id
        ))
        .send()
        .await
        .expect("performance pillar request failed");
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.expect("invalid JSON body");
    let reports = body["reports"].as_array().expect("reports array");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["pillar"], "performance");
    assert_eq!(
        reports[0]["assessment_scope"],
        "ec2_core_performance_telemetry_and_cpu_headroom"
    );
    assert_eq!(reports[0]["posture"]["rules_evaluated"], 3);
    assert!(matches!(
        reports[0]["posture"]["status"].as_str(),
        Some("pass" | "fail")
    ));
    let performance_rules = reports[0]["posture"]["rules"]
        .as_array()
        .expect("performance posture rules");
    assert_eq!(performance_rules.len(), 3);
    let expected_rules = [
        ("ec2-performance-inventory-freshness", "EC2_INV_STALE_DATA"),
        (
            "ec2-performance-core-telemetry-present",
            "EC2_PERF_MISSING_CORE_TELEMETRY",
        ),
        (
            "ec2-performance-cpu-headroom",
            "EC2_PERF_HIGH_CPU_TELEMETRY",
        ),
    ];
    for (rule, (expected_rule_id, expected_reason_code)) in
        performance_rules.iter().zip(expected_rules)
    {
        assert_eq!(rule["rule_id"], expected_rule_id);
        assert_eq!(rule["reason_codes"][0], expected_reason_code);
        assert!(rule["suppression_supported"].is_boolean());
        assert!(rule["assignment_supported"].is_boolean());
        assert!(rule["affected_resources"].is_array());
    }
    let triage = &reports[0]["triage_context"];
    assert_eq!(triage["workflow_id"], "ec2_performance_triage_context");
    assert_eq!(
        triage["context_builder_id"],
        "ec2-performance-deterministic-context-v1"
    );
    assert_eq!(triage["prompt_template_id"], "ec2-performance-ai-triage-v1");
    assert_eq!(triage["generation_mode"], "deterministic_no_llm");
    assert_eq!(triage["max_prompt_tokens"], 1200);
    assert_eq!(triage["provider_routing"][0], "primary_ops_llm");
    assert_eq!(triage["audit_event_type"], "ec2_ai_triage_context_built");
    assert_eq!(triage["guardrails"]["read_only_mode"], true);
    assert_eq!(triage["guardrails"]["evidence_required"], true);
    assert_eq!(triage["guardrails"]["no_llm_invocation"], true);
    assert_eq!(triage["guardrails"]["no_mutation_planning"], true);
    assert!(triage["facts"].is_array());
    assert!(triage["hypotheses"].is_array());
    assert!(triage["missing_data_questions"].is_array());
    assert!(triage["evidence_citations"].is_array());
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
