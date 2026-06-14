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

// Deterministic Kubernetes HorizontalPodAutoscaler inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01128/01135/01156.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesHorizontalPodAutoscaler";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_HPA_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_SCALE_RANGE_NOT_EXPANDABLE: &str = "K8S_HPA_RES_SCALE_RANGE_NOT_EXPANDABLE";
pub const REASON_SEC_EXTERNAL_METRIC_SOURCE: &str = "K8S_HPA_SEC_EXTERNAL_METRIC_SOURCE";
pub const REASON_INV_STALE_DATA: &str = "K8S_HPA_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HpaOwnerReferenceInventoryItem {
    pub kind: Option<String>,
    pub name: String,
    pub controller: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HpaMetricInventoryItem {
    pub metric_type: String,
    pub name: Option<String>,
    pub target_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HpaInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub target_api_version: String,
    pub target_kind: String,
    pub target_name: String,
    pub min_replicas: Option<i32>,
    pub max_replicas: i32,
    pub current_replicas: Option<i32>,
    pub desired_replicas: Option<i32>,
    pub metrics: Vec<HpaMetricInventoryItem>,
    pub behavior_configured: bool,
    pub owner_references: Vec<HpaOwnerReferenceInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_hpa_inventory(
    hpas: &[HpaInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for hpa in hpas {
        if let Some(finding) = stale_finding(hpa, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(hpa, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(hpa, pillar, &mut findings),
            Pillar::Security => evaluate_security(hpa, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: hpas.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(hpa: &HpaInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if has_any_metadata_key(&hpa.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&hpa.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        hpa,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes HorizontalPodAutoscaler {}/{} has no owner, team, project, or cost-center label or annotation",
            hpa.namespace, hpa.name
        ),
        json!({
            "cluster_id": hpa.cluster_id,
            "namespace": hpa.namespace,
            "name": hpa.name,
            "target_kind": hpa.target_kind,
            "target_name": hpa.target_name,
            "min_replicas": hpa.min_replicas,
            "max_replicas": hpa.max_replicas,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    hpa: &HpaInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let min_replicas = hpa.min_replicas.unwrap_or(1);
    if min_replicas >= hpa.max_replicas {
        findings.push(finding(
            hpa,
            pillar,
            REASON_RES_SCALE_RANGE_NOT_EXPANDABLE,
            Severity::High,
            format!(
                "Kubernetes HorizontalPodAutoscaler {}/{} cannot scale out because minReplicas {} is greater than or equal to maxReplicas {}",
                hpa.namespace, hpa.name, min_replicas, hpa.max_replicas
            ),
            json!({
                "cluster_id": hpa.cluster_id,
                "namespace": hpa.namespace,
                "name": hpa.name,
                "target_api_version": hpa.target_api_version,
                "target_kind": hpa.target_kind,
                "target_name": hpa.target_name,
                "min_replicas": hpa.min_replicas,
                "max_replicas": hpa.max_replicas,
                "current_replicas": hpa.current_replicas,
                "desired_replicas": hpa.desired_replicas,
                "recommendation": "Set maxReplicas above minReplicas so the HPA can add capacity during demand spikes",
            }),
        ));
    }
}

fn evaluate_security(hpa: &HpaInventoryItem, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    let external_metrics = hpa
        .metrics
        .iter()
        .filter(|metric| {
            metric.metric_type.eq_ignore_ascii_case("External")
                || metric.metric_type.eq_ignore_ascii_case("Object")
        })
        .cloned()
        .collect::<Vec<_>>();
    if !external_metrics.is_empty() {
        findings.push(finding(
            hpa,
            pillar,
            REASON_SEC_EXTERNAL_METRIC_SOURCE,
            Severity::Medium,
            format!(
                "Kubernetes HorizontalPodAutoscaler {}/{} scales from external or object metric sources",
                hpa.namespace, hpa.name
            ),
            json!({
                "cluster_id": hpa.cluster_id,
                "namespace": hpa.namespace,
                "name": hpa.name,
                "target_kind": hpa.target_kind,
                "target_name": hpa.target_name,
                "matching_metrics": external_metrics,
                "recommendation": "Verify metric adapter RBAC, metric source ownership, and alerting for external or object metric driven scaling",
            }),
        ));
    }
}

fn stale_finding(
    hpa: &HpaInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - hpa.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        hpa,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes HorizontalPodAutoscaler {}/{} is {} hours old (threshold {} hours)",
            hpa.namespace, hpa.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": hpa.cluster_id,
            "namespace": hpa.namespace,
            "name": hpa.name,
            "collected_at": hpa.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    hpa: &HpaInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/HorizontalPodAutoscaler/{}",
            hpa.cluster_id, hpa.namespace, hpa.name
        ),
        arn: format!(
            "kubernetes://horizontalpodautoscalers/{}/{}/{}",
            hpa.cluster_id, hpa.namespace, hpa.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_any_metadata_key(metadata: &BTreeMap<String, String>, wanted_keys: &[&str]) -> bool {
    wanted_keys
        .iter()
        .any(|wanted| metadata_value(metadata, wanted).is_some())
}

fn metadata_value(metadata: &BTreeMap<String, String>, wanted_key: &str) -> Option<String> {
    metadata
        .iter()
        .find(|(key, value)| key.eq_ignore_ascii_case(wanted_key) && !value.trim().is_empty())
        .map(|(_, value)| value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::aws::inventory::types::{Pillar, DEFAULT_STALE_AFTER_HOURS};
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn labels(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn resource_metric(name: &str) -> HpaMetricInventoryItem {
        HpaMetricInventoryItem {
            metric_type: "Resource".to_string(),
            name: Some(name.to_string()),
            target_type: Some("Utilization".to_string()),
        }
    }

    fn external_metric(name: &str) -> HpaMetricInventoryItem {
        HpaMetricInventoryItem {
            metric_type: "External".to_string(),
            name: Some(name.to_string()),
            target_type: Some("AverageValue".to_string()),
        }
    }

    fn hpa(name: &str, metadata_labels: BTreeMap<String, String>) -> HpaInventoryItem {
        HpaInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: name.to_string(),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            target_api_version: "apps/v1".to_string(),
            target_kind: "Deployment".to_string(),
            target_name: "checkout".to_string(),
            min_replicas: Some(2),
            max_replicas: 10,
            current_replicas: Some(3),
            desired_replicas: Some(4),
            metrics: vec![resource_metric("cpu")],
            behavior_configured: true,
            owner_references: Vec::new(),
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    fn healthy_hpa() -> HpaInventoryItem {
        hpa("checkout-autoscaler", labels(&[("team", "payments")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_hpa_inventory(
            &[hpa("untagged", BTreeMap::new())],
            Pillar::Cost,
            now(),
        );

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_COST_OWNER_NOT_RECORDED
        );
        assert_eq!(report.findings[0].pillar, Pillar::Cost);
    }

    #[test]
    fn resilience_flags_scale_ranges_that_cannot_expand() {
        let mut fixed = healthy_hpa();
        fixed.min_replicas = Some(3);
        fixed.max_replicas = 3;

        let report = evaluate_kubernetes_hpa_inventory(&[fixed], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_RES_SCALE_RANGE_NOT_EXPANDABLE
        );
    }

    #[test]
    fn security_flags_external_or_object_metrics() {
        let mut risky = healthy_hpa();
        risky.metrics = vec![external_metric("queue_depth")];

        let report = evaluate_kubernetes_hpa_inventory(&[risky], Pillar::Security, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_SEC_EXTERNAL_METRIC_SOURCE
        );
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_hpa();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_hpa_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_hpas_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_hpa_inventory(&[healthy_hpa()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}
