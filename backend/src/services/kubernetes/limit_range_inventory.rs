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

// Deterministic Kubernetes LimitRange inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01324/01331/01352.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesLimitRange";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_LIMIT_RANGE_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_CONTAINER_DEFAULTS_NOT_SET: &str =
    "K8S_LIMIT_RANGE_RES_CONTAINER_DEFAULTS_NOT_SET";
pub const REASON_SEC_CONTAINER_MAX_LIMITS_NOT_SET: &str =
    "K8S_LIMIT_RANGE_SEC_CONTAINER_MAX_LIMITS_NOT_SET";
pub const REASON_INV_STALE_DATA: &str = "K8S_LIMIT_RANGE_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitRangeItemInventoryItem {
    pub item_type: String,
    pub default: BTreeMap<String, String>,
    pub default_request: BTreeMap<String, String>,
    pub max: BTreeMap<String, String>,
    pub max_limit_request_ratio: BTreeMap<String, String>,
    pub min: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitRangeInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub limits: Vec<LimitRangeItemInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_limit_range_inventory(
    limit_ranges: &[LimitRangeInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for limit_range in limit_ranges {
        if let Some(finding) = stale_finding(limit_range, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(limit_range, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(limit_range, pillar, &mut findings),
            Pillar::Security => evaluate_security(limit_range, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: limit_ranges.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    limit_range: &LimitRangeInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&limit_range.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&limit_range.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        limit_range,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes LimitRange {}/{} has no owner, team, project, or cost-center label or annotation",
            limit_range.namespace, limit_range.name
        ),
        json!({
            "cluster_id": limit_range.cluster_id,
            "namespace": limit_range.namespace,
            "name": limit_range.name,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    limit_range: &LimitRangeInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let missing_defaults = limit_range
        .limits
        .iter()
        .filter(|item| item.item_type.eq_ignore_ascii_case("Container"))
        .filter_map(|item| {
            let mut missing = Vec::new();
            for resource in ["cpu", "memory"] {
                if !has_resource(&item.default, resource)
                    || !has_resource(&item.default_request, resource)
                {
                    missing.push(resource);
                }
            }
            if missing.is_empty() {
                None
            } else {
                Some(json!({
                    "item_type": item.item_type,
                    "missing_resources": missing,
                    "default": item.default,
                    "default_request": item.default_request,
                }))
            }
        })
        .collect::<Vec<_>>();

    if missing_defaults.is_empty() {
        return;
    }

    findings.push(finding(
        limit_range,
        pillar,
        REASON_RES_CONTAINER_DEFAULTS_NOT_SET,
        Severity::High,
        format!(
            "Kubernetes LimitRange {}/{} does not set complete container CPU and memory defaults",
            limit_range.namespace, limit_range.name
        ),
        json!({
            "cluster_id": limit_range.cluster_id,
            "namespace": limit_range.namespace,
            "name": limit_range.name,
            "missing_defaults": missing_defaults,
            "recommendation": "Set both default and defaultRequest for container CPU and memory so unsized pods receive predictable QoS and scheduling behavior",
        }),
    ));
}

fn evaluate_security(
    limit_range: &LimitRangeInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let missing_max_limits = limit_range
        .limits
        .iter()
        .filter(|item| item.item_type.eq_ignore_ascii_case("Container"))
        .filter_map(|item| {
            let missing = ["cpu", "memory"]
                .iter()
                .filter(|resource| !has_resource(&item.max, resource))
                .copied()
                .collect::<Vec<_>>();
            if missing.is_empty() {
                None
            } else {
                Some(json!({
                    "item_type": item.item_type,
                    "missing_resources": missing,
                    "max": item.max,
                }))
            }
        })
        .collect::<Vec<_>>();

    if missing_max_limits.is_empty() {
        return;
    }

    findings.push(finding(
        limit_range,
        pillar,
        REASON_SEC_CONTAINER_MAX_LIMITS_NOT_SET,
        Severity::Medium,
        format!(
            "Kubernetes LimitRange {}/{} does not cap all container CPU and memory usage",
            limit_range.namespace, limit_range.name
        ),
        json!({
            "cluster_id": limit_range.cluster_id,
            "namespace": limit_range.namespace,
            "name": limit_range.name,
            "missing_max_limits": missing_max_limits,
            "recommendation": "Set max CPU and memory limits for containers to reduce unbounded resource consumption risk",
        }),
    ));
}

fn stale_finding(
    limit_range: &LimitRangeInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - limit_range.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        limit_range,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes LimitRange {}/{} is {} hours old (threshold {} hours)",
            limit_range.namespace, limit_range.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": limit_range.cluster_id,
            "namespace": limit_range.namespace,
            "name": limit_range.name,
            "collected_at": limit_range.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    limit_range: &LimitRangeInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/LimitRange/{}",
            limit_range.cluster_id, limit_range.namespace, limit_range.name
        ),
        arn: format!(
            "kubernetes://limitranges/{}/{}/{}",
            limit_range.cluster_id, limit_range.namespace, limit_range.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_resource(values: &BTreeMap<String, String>, resource: &str) -> bool {
    values
        .iter()
        .any(|(key, value)| key.eq_ignore_ascii_case(resource) && !value.trim().is_empty())
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

    fn map(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn healthy_limit_range() -> LimitRangeInventoryItem {
        LimitRangeInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: "apps-limits".to_string(),
            labels: map(&[("team", "platform")]),
            annotations: BTreeMap::new(),
            limits: vec![LimitRangeItemInventoryItem {
                item_type: "Container".to_string(),
                default: map(&[("cpu", "500m"), ("memory", "512Mi")]),
                default_request: map(&[("cpu", "100m"), ("memory", "128Mi")]),
                max: map(&[("cpu", "2"), ("memory", "2Gi")]),
                max_limit_request_ratio: BTreeMap::new(),
                min: BTreeMap::new(),
            }],
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let mut untagged = healthy_limit_range();
        untagged.labels.clear();

        let report = evaluate_kubernetes_limit_range_inventory(&[untagged], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_COST_OWNER_NOT_RECORDED
        );
        assert_eq!(report.findings[0].pillar, Pillar::Cost);
    }

    #[test]
    fn resilience_flags_container_items_without_cpu_or_memory_defaults() {
        let mut missing_defaults = healthy_limit_range();
        missing_defaults.limits[0].default.clear();
        missing_defaults.limits[0].default_request.remove("memory");

        let report = evaluate_kubernetes_limit_range_inventory(
            &[missing_defaults],
            Pillar::Resilience,
            now(),
        );

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_RES_CONTAINER_DEFAULTS_NOT_SET
        );
    }

    #[test]
    fn security_flags_container_items_without_cpu_or_memory_max_limits() {
        let mut missing_max = healthy_limit_range();
        missing_max.limits[0].max.remove("cpu");

        let report =
            evaluate_kubernetes_limit_range_inventory(&[missing_max], Pillar::Security, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_SEC_CONTAINER_MAX_LIMITS_NOT_SET
        );
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_limit_range();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_limit_range_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_limit_ranges_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_limit_range_inventory(&[healthy_limit_range()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}
