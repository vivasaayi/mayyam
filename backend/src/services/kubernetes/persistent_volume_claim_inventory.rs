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

// Deterministic Kubernetes PersistentVolumeClaim inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01422/01429/01450.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesPersistentVolumeClaim";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_PVC_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_PENDING_OR_LOST: &str = "K8S_PVC_RES_PENDING_OR_LOST";
pub const REASON_SEC_READ_WRITE_MANY: &str = "K8S_PVC_SEC_READ_WRITE_MANY";
pub const REASON_INV_STALE_DATA: &str = "K8S_PVC_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentVolumeClaimConditionInventoryItem {
    pub condition_type: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentVolumeClaimInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub requested_storage: BTreeMap<String, String>,
    pub capacity: BTreeMap<String, String>,
    pub access_modes: Vec<String>,
    pub storage_class_name: Option<String>,
    pub volume_mode: Option<String>,
    pub volume_name: Option<String>,
    pub phase: Option<String>,
    pub conditions: Vec<PersistentVolumeClaimConditionInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_persistent_volume_claim_inventory(
    claims: &[PersistentVolumeClaimInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for claim in claims {
        if let Some(finding) = stale_finding(claim, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(claim, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(claim, pillar, &mut findings),
            Pillar::Security => evaluate_security(claim, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: claims.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    claim: &PersistentVolumeClaimInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&claim.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&claim.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        claim,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes PersistentVolumeClaim {}/{} has no owner, team, project, or cost-center label or annotation",
            claim.namespace, claim.name
        ),
        json!({
            "cluster_id": claim.cluster_id,
            "namespace": claim.namespace,
            "name": claim.name,
            "requested_storage": claim.requested_storage,
            "storage_class_name": claim.storage_class_name,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    claim: &PersistentVolumeClaimInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let phase = claim.phase.as_deref().unwrap_or("");
    if !phase.eq_ignore_ascii_case("Pending") && !phase.eq_ignore_ascii_case("Lost") {
        return;
    }

    findings.push(finding(
        claim,
        pillar,
        REASON_RES_PENDING_OR_LOST,
        Severity::High,
        format!(
            "Kubernetes PersistentVolumeClaim {}/{} is in {} phase",
            claim.namespace,
            claim.name,
            if phase.is_empty() { "unknown" } else { phase }
        ),
        json!({
            "cluster_id": claim.cluster_id,
            "namespace": claim.namespace,
            "name": claim.name,
            "phase": claim.phase,
            "volume_name": claim.volume_name,
            "storage_class_name": claim.storage_class_name,
            "conditions": claim.conditions,
            "recommendation": "Investigate pending or lost PersistentVolumeClaims before dependent pods remain unscheduled or lose storage binding",
        }),
    ));
}

fn evaluate_security(
    claim: &PersistentVolumeClaimInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !claim
        .access_modes
        .iter()
        .any(|mode| mode.eq_ignore_ascii_case("ReadWriteMany"))
    {
        return;
    }

    findings.push(finding(
        claim,
        pillar,
        REASON_SEC_READ_WRITE_MANY,
        Severity::Medium,
        format!(
            "Kubernetes PersistentVolumeClaim {}/{} allows ReadWriteMany access",
            claim.namespace, claim.name
        ),
        json!({
            "cluster_id": claim.cluster_id,
            "namespace": claim.namespace,
            "name": claim.name,
            "access_modes": claim.access_modes,
            "volume_mode": claim.volume_mode,
            "volume_name": claim.volume_name,
            "storage_class_name": claim.storage_class_name,
            "recommendation": "Review workload and RBAC boundaries for shared writable PersistentVolumeClaims and restrict consumers where possible",
        }),
    ));
}

fn stale_finding(
    claim: &PersistentVolumeClaimInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - claim.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        claim,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes PersistentVolumeClaim {}/{} is {} hours old (threshold {} hours)",
            claim.namespace, claim.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": claim.cluster_id,
            "namespace": claim.namespace,
            "name": claim.name,
            "collected_at": claim.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    claim: &PersistentVolumeClaimInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/PersistentVolumeClaim/{}",
            claim.cluster_id, claim.namespace, claim.name
        ),
        arn: format!(
            "kubernetes://persistentvolumeclaims/{}/{}/{}",
            claim.cluster_id, claim.namespace, claim.name
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

    fn map(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn healthy_claim() -> PersistentVolumeClaimInventoryItem {
        PersistentVolumeClaimInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: "data".to_string(),
            labels: map(&[("team", "storage")]),
            annotations: BTreeMap::new(),
            requested_storage: map(&[("storage", "100Gi")]),
            capacity: map(&[("storage", "100Gi")]),
            access_modes: vec!["ReadWriteOnce".to_string()],
            storage_class_name: Some("fast".to_string()),
            volume_mode: Some("Filesystem".to_string()),
            volume_name: Some("pv-fast-1".to_string()),
            phase: Some("Bound".to_string()),
            conditions: Vec::new(),
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let mut untagged = healthy_claim();
        untagged.labels.clear();

        let report =
            evaluate_kubernetes_persistent_volume_claim_inventory(&[untagged], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_COST_OWNER_NOT_RECORDED
        );
        assert_eq!(report.findings[0].pillar, Pillar::Cost);
    }

    #[test]
    fn resilience_flags_pending_or_lost_claims() {
        let mut pending = healthy_claim();
        pending.phase = Some("Pending".to_string());
        pending.volume_name = None;

        let report = evaluate_kubernetes_persistent_volume_claim_inventory(
            &[pending],
            Pillar::Resilience,
            now(),
        );

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_RES_PENDING_OR_LOST);
    }

    #[test]
    fn security_flags_read_write_many_claims() {
        let mut shared = healthy_claim();
        shared.access_modes = vec!["ReadWriteMany".to_string()];

        let report = evaluate_kubernetes_persistent_volume_claim_inventory(
            &[shared],
            Pillar::Security,
            now(),
        );

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_SEC_READ_WRITE_MANY);
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_claim();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report =
            evaluate_kubernetes_persistent_volume_claim_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_persistent_volume_claims_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_persistent_volume_claim_inventory(
                &[healthy_claim()],
                pillar,
                now(),
            );

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}
