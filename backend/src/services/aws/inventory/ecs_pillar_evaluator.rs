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

// Deterministic ECS inventory evaluators for the cost, security, and
// resilience pillars (roadmap rows 01-AWS-CLOUD-00190/00199/00226).
//
// Evaluates both EcsCluster and EcsService rows persisted by
// ecs_control_plane (PascalCase keys: Status, RunningTasksCount,
// DesiredCount, RunningCount, ...). The collector stores empty tags for
// ECS, so tag posture is reported as an explicit data gap.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;
use crate::services::aws::inventory::types::{
    check_stale, data_str, score_pillar, InventoryFinding, Pillar, PillarReport, Severity,
};

// Reason codes are the stable contract for findings; never reuse or rename.
pub const REASON_COST_TAG_DATA_NOT_COLLECTED: &str = "ECS_COST_TAG_DATA_NOT_COLLECTED";
pub const REASON_COST_IDLE_CLUSTER: &str = "ECS_COST_IDLE_CLUSTER";
pub const REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA: &str =
    "ECS_COST_MISSING_TELEMETRY_COLLECTION_METADATA";
pub const REASON_COST_MISSING_CLOUDWATCH_TELEMETRY: &str = "ECS_COST_MISSING_CLOUDWATCH_TELEMETRY";
pub const REASON_SEC_POSTURE_DATA_NOT_COLLECTED: &str = "ECS_SEC_POSTURE_DATA_NOT_COLLECTED";
pub const REASON_RES_SERVICE_BELOW_DESIRED: &str = "ECS_RES_SERVICE_BELOW_DESIRED";
pub const REASON_RES_SINGLE_TASK_SERVICE: &str = "ECS_RES_SINGLE_TASK_SERVICE";
pub const REASON_RES_CLUSTER_NOT_ACTIVE: &str = "ECS_RES_CLUSTER_NOT_ACTIVE";
pub const REASON_INV_STALE_DATA: &str = "ECS_INV_STALE_DATA";

fn is_cluster(resource: &AwsResourceModel) -> bool {
    resource.resource_type == "EcsCluster"
}

fn data_i64(resource: &AwsResourceModel, key: &str) -> Option<i64> {
    resource.resource_data.get(key).and_then(|v| v.as_i64())
}

/// Evaluate every ECS cluster and service in the fleet for one pillar.
pub fn evaluate_ecs_fleet(
    resources: &[AwsResourceModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut findings: Vec<InventoryFinding> = Vec::new();
    let mut stale_resources = 0usize;

    for resource in resources {
        if let Some(stale) = check_stale(resource, pillar, REASON_INV_STALE_DATA, now) {
            stale_resources += 1;
            findings.push(stale);
        }
        match pillar {
            Pillar::Cost => evaluate_cost(resource, &mut findings),
            Pillar::Security => evaluate_security(resource, &mut findings),
            Pillar::Resilience => evaluate_resilience(resource, &mut findings),
            // Pillars without checks for this service yet produce no findings.
            _ => {}
        }
    }

    let score = score_pillar(&findings);
    PillarReport {
        pillar,
        resources_evaluated: resources.len(),
        stale_resources,
        score,
        findings,
    }
}

fn tags_missing(resource: &AwsResourceModel) -> bool {
    resource
        .tags
        .as_object()
        .map(|m| m.is_empty())
        .unwrap_or(true)
}

fn evaluate_cost(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if !has_any_key(
        &resource.resource_data,
        &["TelemetryCollectedAt", "TelemetryWindowMinutes"],
    ) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA.to_string(),
            severity: Severity::Medium,
            message: format!(
                "ECS telemetry collection metadata for {} {} is missing; cost evidence freshness cannot be proven",
                resource.resource_type, resource.resource_id
            ),
            evidence: json!({
                "required_metadata": ["TelemetryCollectedAt", "TelemetryWindowMinutes"],
            }),
        });
    }

    if tags_missing(resource) {
        findings.push(InventoryFinding {
            resource_id: resource.resource_id.clone(),
            arn: resource.arn.clone(),
            pillar: Pillar::Cost,
            reason_code: REASON_COST_TAG_DATA_NOT_COLLECTED.to_string(),
            severity: Severity::Medium,
            message: format!(
                "Tags for {} {} are not collected yet; cost allocation cannot be assessed",
                resource.resource_type, resource.resource_id
            ),
            evidence: json!({ "tags": resource.tags }),
        });
    }

    if is_cluster(resource) {
        if !has_any_key(
            &resource.resource_data,
            &["CpuUtilizationAverage", "MemoryUtilizationAverage"],
        ) {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_MISSING_CLOUDWATCH_TELEMETRY.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Cluster {} is missing ECS CPU and memory utilization telemetry for cost analysis",
                    resource.resource_id
                ),
                evidence: json!({
                    "cloudwatch_namespace": "AWS/ECS",
                    "required_metrics": ecs_cost_metric_names(),
                }),
            });
        }

        let instances = data_i64(resource, "RegisteredContainerInstancesCount").unwrap_or(0);
        let running = data_i64(resource, "RunningTasksCount").unwrap_or(0);
        let services = data_i64(resource, "ActiveServicesCount").unwrap_or(0);
        if instances == 0 && running == 0 && services == 0 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Cost,
                reason_code: REASON_COST_IDLE_CLUSTER.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Cluster {} has no container instances, tasks, or services; it appears abandoned",
                    resource.resource_id
                ),
                evidence: json!({
                    "RegisteredContainerInstancesCount": instances,
                    "RunningTasksCount": running,
                    "ActiveServicesCount": services,
                }),
            });
        }
    }
}

fn has_any_key(value: &Value, keys: &[&str]) -> bool {
    value
        .as_object()
        .map(|object| keys.iter().any(|key| object.contains_key(*key)))
        .unwrap_or(false)
}

fn ecs_cost_metric_names() -> Vec<&'static str> {
    vec![
        "CPUUtilization",
        "MemoryUtilization",
        "RunningTaskCount",
        "PendingTaskCount",
        "ServiceCount",
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EcsPostureStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcsEvidenceCitation {
    pub reason_code: String,
    pub resource_id: String,
    pub severity: Severity,
    pub evidence: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcsPostureRule {
    pub rule_id: &'static str,
    pub status: EcsPostureStatus,
    pub reason_codes: Vec<&'static str>,
    pub affected_resources: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcsCostPostureSummary {
    pub workflow_id: &'static str,
    pub rule_pack_id: &'static str,
    pub read_only_mode: bool,
    pub audit_event_type: &'static str,
    pub status: EcsPostureStatus,
    pub rules_evaluated: usize,
    pub rules_failed: usize,
    pub affected_resources: Vec<String>,
    pub rules: Vec<EcsPostureRule>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcsTriageGuardrails {
    pub read_only_mode: bool,
    pub evidence_required: bool,
    pub separate_facts_from_hypotheses: bool,
    pub ask_for_missing_data: bool,
    pub no_llm_invocation: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcsCostTriageContext {
    pub workflow_id: &'static str,
    pub pillar: Pillar,
    pub api_path: &'static str,
    pub context_builder_id: &'static str,
    pub prompt_template_id: &'static str,
    pub generation_mode: &'static str,
    pub audit_event_type: &'static str,
    pub guardrails: EcsTriageGuardrails,
    pub facts: Vec<String>,
    pub hypotheses: Vec<String>,
    pub missing_data_questions: Vec<String>,
    pub follow_up_questions: Vec<String>,
    pub evidence_citations: Vec<EcsEvidenceCitation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcsCostTelemetrySummary {
    pub workflow_id: &'static str,
    pub read_only_mode: bool,
    pub freshness_required: bool,
    pub telemetry_collection_required: bool,
    pub cloudwatch_namespace: &'static str,
    pub cloudwatch_dimension: &'static str,
    pub required_metrics: Vec<&'static str>,
    pub missing_data_reason_codes: Vec<String>,
    pub evidence_reason_codes: Vec<String>,
    pub stale_data_blocks_delivery: bool,
    pub telemetry_quality_score: u8,
}

pub fn ecs_cost_posture_summary(report: &PillarReport) -> EcsCostPostureSummary {
    let rules = vec![
        ecs_posture_rule(
            report,
            "ecs-cost-inventory-freshness",
            &[REASON_INV_STALE_DATA],
        ),
        ecs_posture_rule(
            report,
            "ecs-cost-telemetry-metadata-present",
            &[REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA],
        ),
        ecs_posture_rule(
            report,
            "ecs-cost-cloudwatch-utilization-present",
            &[REASON_COST_MISSING_CLOUDWATCH_TELEMETRY],
        ),
        ecs_posture_rule(
            report,
            "ecs-cost-allocation-tags-present",
            &[REASON_COST_TAG_DATA_NOT_COLLECTED],
        ),
        ecs_posture_rule(
            report,
            "ecs-cost-idle-clusters-reviewed",
            &[REASON_COST_IDLE_CLUSTER],
        ),
    ];
    let rules_failed = rules
        .iter()
        .filter(|rule| rule.status == EcsPostureStatus::Fail)
        .count();
    let affected_resources = sorted_unique_strings(
        rules
            .iter()
            .flat_map(|rule| rule.affected_resources.iter().cloned())
            .collect(),
    );

    EcsCostPostureSummary {
        workflow_id: "ecs_cost_posture",
        rule_pack_id: "ecs-cost-posture-rules-v1",
        read_only_mode: true,
        audit_event_type: "ecs_cost_posture_evaluated",
        status: if rules_failed == 0 {
            EcsPostureStatus::Pass
        } else {
            EcsPostureStatus::Fail
        },
        rules_evaluated: rules.len(),
        rules_failed,
        affected_resources,
        rules,
        recommendations: ecs_cost_recommendations(report),
    }
}

pub fn ecs_cost_triage_context(report: &PillarReport) -> EcsCostTriageContext {
    let mut facts = Vec::new();
    let mut hypotheses = Vec::new();
    let mut missing_data_questions = Vec::new();
    let mut follow_up_questions = Vec::new();
    let mut evidence_citations = Vec::new();

    for finding in &report.findings {
        facts.push(format!(
            "{} affects {} with {:?} severity",
            finding.reason_code, finding.resource_id, finding.severity
        ));
        evidence_citations.push(EcsEvidenceCitation {
            reason_code: finding.reason_code.clone(),
            resource_id: finding.resource_id.clone(),
            severity: finding.severity,
            evidence: finding.evidence.clone(),
        });

        match finding.reason_code.as_str() {
            REASON_INV_STALE_DATA => missing_data_questions.push(format!(
                "Refresh ECS inventory and cost telemetry for {} before explaining current cost posture",
                finding.resource_id
            )),
            REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA => missing_data_questions.push(
                format!(
                    "Collect ECS telemetry run metadata for {} before trusting utilization evidence",
                    finding.resource_id
                ),
            ),
            REASON_COST_MISSING_CLOUDWATCH_TELEMETRY => missing_data_questions.push(format!(
                "Collect ECS CPUUtilization, MemoryUtilization, RunningTaskCount, PendingTaskCount, and ServiceCount for {}",
                finding.resource_id
            )),
            REASON_COST_TAG_DATA_NOT_COLLECTED => missing_data_questions.push(format!(
                "Collect owner, team, project, or cost-center tags for {} so ECS cost ownership can be routed",
                finding.resource_id
            )),
            REASON_COST_IDLE_CLUSTER => hypotheses.push(format!(
                "{} appears idle; validate deployment schedule, recent tasks, and owner expectations before cleanup",
                finding.resource_id
            )),
            _ => {}
        }

        follow_up_questions.push(ecs_cost_follow_up_question(finding.reason_code.as_str()));
    }

    EcsCostTriageContext {
        workflow_id: "ecs_cost_triage_context",
        pillar: report.pillar,
        api_path: "/api/aws/inventory/ecs/pillars",
        context_builder_id: "ecs-cost-deterministic-context-v1",
        prompt_template_id: "ecs-cost-ai-triage-v1",
        generation_mode: "deterministic_no_llm",
        audit_event_type: "ecs_cost_ai_triage_context_built",
        guardrails: EcsTriageGuardrails {
            read_only_mode: true,
            evidence_required: true,
            separate_facts_from_hypotheses: true,
            ask_for_missing_data: true,
            no_llm_invocation: true,
        },
        facts,
        hypotheses,
        missing_data_questions,
        follow_up_questions: sorted_unique_strings(follow_up_questions),
        evidence_citations,
    }
}

pub fn ecs_cost_telemetry_summary(report: &PillarReport) -> EcsCostTelemetrySummary {
    let missing_data_reason_codes = sorted_unique_strings(
        report
            .findings
            .iter()
            .filter(|finding| {
                matches!(
                    finding.reason_code.as_str(),
                    REASON_INV_STALE_DATA
                        | REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA
                        | REASON_COST_MISSING_CLOUDWATCH_TELEMETRY
                )
            })
            .map(|finding| finding.reason_code.clone())
            .collect(),
    );
    let evidence_reason_codes = sorted_unique_strings(
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.clone())
            .collect(),
    );

    EcsCostTelemetrySummary {
        workflow_id: "ecs_cost_telemetry",
        read_only_mode: true,
        freshness_required: true,
        telemetry_collection_required: true,
        cloudwatch_namespace: "AWS/ECS",
        cloudwatch_dimension: "ClusterName",
        required_metrics: ecs_cost_metric_names(),
        missing_data_reason_codes,
        evidence_reason_codes,
        stale_data_blocks_delivery: report.stale_resources > 0,
        telemetry_quality_score: report.score,
    }
}

fn ecs_posture_rule(
    report: &PillarReport,
    rule_id: &'static str,
    reason_codes: &[&'static str],
) -> EcsPostureRule {
    let affected_resources = sorted_unique_strings(
        report
            .findings
            .iter()
            .filter(|finding| reason_codes.contains(&finding.reason_code.as_str()))
            .map(|finding| finding.resource_id.clone())
            .collect(),
    );

    EcsPostureRule {
        rule_id,
        status: if affected_resources.is_empty() {
            EcsPostureStatus::Pass
        } else {
            EcsPostureStatus::Fail
        },
        reason_codes: reason_codes.to_vec(),
        affected_resources,
    }
}

fn ecs_cost_follow_up_question(reason_code: &str) -> String {
    match reason_code {
        REASON_INV_STALE_DATA => {
            "Has ECS inventory and cost telemetry been refreshed in the current sync window?"
        }
        REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA => {
            "Which collector run proves ECS cost telemetry freshness and window coverage?"
        }
        REASON_COST_MISSING_CLOUDWATCH_TELEMETRY => {
            "Which ECS utilization datapoints are missing for the affected cluster?"
        }
        REASON_COST_TAG_DATA_NOT_COLLECTED => {
            "Who owns the ECS savings review when allocation tags are missing?"
        }
        REASON_COST_IDLE_CLUSTER => {
            "Is the ECS cluster intentionally empty, recently drained, or safe to retire after owner confirmation?"
        }
        _ => "What additional evidence is required before explaining this ECS cost finding?",
    }
    .to_string()
}

fn ecs_cost_recommendations(report: &PillarReport) -> Vec<String> {
    sorted_unique_strings(
        report
            .findings
            .iter()
            .map(|finding| match finding.reason_code.as_str() {
                REASON_INV_STALE_DATA => "refresh_ecs_inventory_and_cost_telemetry",
                REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA => {
                    "record_ecs_telemetry_collection_metadata"
                }
                REASON_COST_MISSING_CLOUDWATCH_TELEMETRY => {
                    "collect_ecs_cloudwatch_utilization_metrics"
                }
                REASON_COST_TAG_DATA_NOT_COLLECTED => "collect_ecs_allocation_tags",
                REASON_COST_IDLE_CLUSTER => "review_idle_ecs_cluster_for_retirement",
                _ => "review_ecs_cost_evidence",
            })
            .map(str::to_string)
            .collect(),
    )
}

fn sorted_unique_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn evaluate_security(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    // The collector gathers no IAM/network posture for ECS yet; report the
    // gap once per resource rather than scoring blind.
    findings.push(InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar: Pillar::Security,
        reason_code: REASON_SEC_POSTURE_DATA_NOT_COLLECTED.to_string(),
        severity: Severity::Medium,
        message: format!(
            "Security posture (task role, network mode, exec config) for {} {} is not collected yet",
            resource.resource_type, resource.resource_id
        ),
        evidence: json!({
            "collected_fields": resource
                .resource_data
                .as_object()
                .map(|m| m.keys().cloned().collect::<Vec<_>>()),
        }),
    });
}

fn evaluate_resilience(resource: &AwsResourceModel, findings: &mut Vec<InventoryFinding>) {
    if is_cluster(resource) {
        if let Some(status) = data_str(&resource.resource_data, "Status") {
            if status != "ACTIVE" {
                findings.push(InventoryFinding {
                    resource_id: resource.resource_id.clone(),
                    arn: resource.arn.clone(),
                    pillar: Pillar::Resilience,
                    reason_code: REASON_RES_CLUSTER_NOT_ACTIVE.to_string(),
                    severity: Severity::Medium,
                    message: format!("Cluster {} is in status '{}'", resource.resource_id, status),
                    evidence: json!({ "Status": status }),
                });
            }
        }
        return;
    }

    // Service-level checks.
    let desired = data_i64(resource, "DesiredCount");
    let running = data_i64(resource, "RunningCount");
    if let (Some(desired), Some(running)) = (desired, running) {
        if running < desired {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SERVICE_BELOW_DESIRED.to_string(),
                severity: Severity::High,
                message: format!(
                    "Service {} runs {} of {} desired tasks; capacity is degraded",
                    resource.resource_id, running, desired
                ),
                evidence: json!({ "DesiredCount": desired, "RunningCount": running }),
            });
        }
        if desired == 1 {
            findings.push(InventoryFinding {
                resource_id: resource.resource_id.clone(),
                arn: resource.arn.clone(),
                pillar: Pillar::Resilience,
                reason_code: REASON_RES_SINGLE_TASK_SERVICE.to_string(),
                severity: Severity::Medium,
                message: format!(
                    "Service {} has a desired count of 1; a single task failure causes downtime",
                    resource.resource_id
                ),
                evidence: json!({ "DesiredCount": desired }),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::Value;
    use uuid::Uuid;

    fn fixture(
        resource_type: &str,
        resource_id: &str,
        tags: Value,
        resource_data: Value,
        now: DateTime<Utc>,
    ) -> AwsResourceModel {
        let refreshed = now - Duration::hours(1);
        AwsResourceModel {
            id: Uuid::new_v4(),
            sync_id: None,
            account_id: "123456789012".to_string(),
            profile: None,
            region: "us-east-1".to_string(),
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            arn: format!("arn:aws:ecs:us-east-1:123456789012:{}", resource_id),
            name: Some(resource_id.to_string()),
            tags,
            resource_data,
            created_at: refreshed,
            updated_at: refreshed,
            last_refreshed: refreshed,
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn cost_flags_idle_cluster_and_tag_gap() {
        let r = fixture(
            "EcsCluster",
            "empty-cluster",
            json!({}),
            json!({"Status": "ACTIVE", "RegisteredContainerInstancesCount": 0, "RunningTasksCount": 0, "ActiveServicesCount": 0}),
            now(),
        );
        let report = evaluate_ecs_fleet(&[r], Pillar::Cost, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_COST_IDLE_CLUSTER));
        assert!(codes.contains(&REASON_COST_TAG_DATA_NOT_COLLECTED));
        assert!(codes.contains(&REASON_COST_MISSING_TELEMETRY_COLLECTION_METADATA));
        assert!(codes.contains(&REASON_COST_MISSING_CLOUDWATCH_TELEMETRY));
    }

    #[test]
    fn cost_passes_for_busy_tagged_cluster() {
        let r = fixture(
            "EcsCluster",
            "busy-cluster",
            json!({"team": "platform"}),
            json!({
                "Status": "ACTIVE",
                "RegisteredContainerInstancesCount": 3,
                "RunningTasksCount": 10,
                "ActiveServicesCount": 2,
                "TelemetryCollectedAt": "2026-06-10T00:00:00Z",
                "TelemetryWindowMinutes": 60,
                "CpuUtilizationAverage": 42.0,
                "MemoryUtilizationAverage": 51.0
            }),
            now(),
        );
        let report = evaluate_ecs_fleet(&[r], Pillar::Cost, now());
        assert!(
            report.findings.is_empty(),
            "unexpected: {:?}",
            report.findings
        );
    }

    #[test]
    fn cost_telemetry_posture_and_triage_explain_evidence_gaps() {
        let cluster = fixture(
            "EcsCluster",
            "idle-cluster",
            json!({}),
            json!({"Status": "ACTIVE", "RegisteredContainerInstancesCount": 0, "RunningTasksCount": 0, "ActiveServicesCount": 0}),
            now(),
        );
        let report = evaluate_ecs_fleet(&[cluster], Pillar::Cost, now());

        let posture = ecs_cost_posture_summary(&report);
        assert_eq!(posture.workflow_id, "ecs_cost_posture");
        assert_eq!(posture.rule_pack_id, "ecs-cost-posture-rules-v1");
        assert_eq!(posture.rules_evaluated, 5);
        assert_eq!(posture.status, EcsPostureStatus::Fail);
        assert!(posture
            .recommendations
            .iter()
            .any(|recommendation| recommendation == "collect_ecs_cloudwatch_utilization_metrics"));

        let triage = ecs_cost_triage_context(&report);
        assert_eq!(triage.workflow_id, "ecs_cost_triage_context");
        assert_eq!(triage.prompt_template_id, "ecs-cost-ai-triage-v1");
        assert!(triage.guardrails.read_only_mode);
        assert!(triage.guardrails.separate_facts_from_hypotheses);
        assert!(triage
            .missing_data_questions
            .iter()
            .any(|question| question.contains("ECS CPUUtilization")));

        let telemetry = ecs_cost_telemetry_summary(&report);
        assert_eq!(telemetry.workflow_id, "ecs_cost_telemetry");
        assert_eq!(telemetry.cloudwatch_namespace, "AWS/ECS");
        assert!(telemetry
            .missing_data_reason_codes
            .contains(&REASON_COST_MISSING_CLOUDWATCH_TELEMETRY.to_string()));
        assert!(telemetry.required_metrics.contains(&"RunningTaskCount"));
    }

    #[test]
    fn security_reports_posture_data_gap_per_resource() {
        let r = fixture(
            "EcsService",
            "svc-a",
            json!({"team": "platform"}),
            json!({"Status": "ACTIVE", "DesiredCount": 2, "RunningCount": 2}),
            now(),
        );
        let report = evaluate_ecs_fleet(&[r], Pillar::Security, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_SEC_POSTURE_DATA_NOT_COLLECTED]
        );
    }

    #[test]
    fn resilience_flags_degraded_and_single_task_services() {
        let degraded = fixture(
            "EcsService",
            "svc-degraded",
            json!({"team": "platform"}),
            json!({"Status": "ACTIVE", "DesiredCount": 3, "RunningCount": 1}),
            now(),
        );
        let single = fixture(
            "EcsService",
            "svc-single",
            json!({"team": "platform"}),
            json!({"Status": "ACTIVE", "DesiredCount": 1, "RunningCount": 1}),
            now(),
        );
        let report = evaluate_ecs_fleet(&[degraded, single], Pillar::Resilience, now());
        let codes: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.reason_code.as_str())
            .collect();
        assert!(codes.contains(&REASON_RES_SERVICE_BELOW_DESIRED));
        assert!(codes.contains(&REASON_RES_SINGLE_TASK_SERVICE));
        let below = report
            .findings
            .iter()
            .find(|f| f.reason_code == REASON_RES_SERVICE_BELOW_DESIRED)
            .unwrap();
        assert_eq!(below.severity, Severity::High);
    }

    #[test]
    fn resilience_flags_inactive_cluster_and_passes_healthy_service() {
        let cluster = fixture(
            "EcsCluster",
            "draining",
            json!({"team": "platform"}),
            json!({"Status": "DEPROVISIONING", "RegisteredContainerInstancesCount": 1, "RunningTasksCount": 1, "ActiveServicesCount": 1}),
            now(),
        );
        let healthy = fixture(
            "EcsService",
            "svc-ok",
            json!({"team": "platform"}),
            json!({"Status": "ACTIVE", "DesiredCount": 2, "RunningCount": 2}),
            now(),
        );
        let report = evaluate_ecs_fleet(&[cluster, healthy], Pillar::Resilience, now());
        assert_eq!(
            report
                .findings
                .iter()
                .map(|f| f.reason_code.as_str())
                .collect::<Vec<_>>(),
            vec![REASON_RES_CLUSTER_NOT_ACTIVE]
        );
    }
}
