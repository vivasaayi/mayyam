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

// Deterministic Kubernetes VolumeSnapshot inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01520/01527/01548.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesVolumeSnapshot";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_VS_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_NOT_READY: &str = "K8S_VS_RES_NOT_READY";
pub const REASON_SEC_CLASS_NOT_SET: &str = "K8S_VS_SEC_CLASS_NOT_SET";
pub const REASON_INV_STALE_DATA: &str = "K8S_VS_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeSnapshotInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub api_version: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub snapshot_class_name: Option<String>,
    pub source_persistent_volume_claim_name: Option<String>,
    pub source_volume_snapshot_content_name: Option<String>,
    pub bound_volume_snapshot_content_name: Option<String>,
    pub ready_to_use: Option<bool>,
    pub restore_size: Option<String>,
    pub error_message: Option<String>,
    pub error_time: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_volume_snapshot_inventory(
    snapshots: &[VolumeSnapshotInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for snapshot in snapshots {
        if let Some(finding) = stale_finding(snapshot, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(snapshot, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(snapshot, pillar, &mut findings),
            Pillar::Security => evaluate_security(snapshot, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: snapshots.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    snapshot: &VolumeSnapshotInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&snapshot.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&snapshot.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        snapshot,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes VolumeSnapshot {}/{} has no owner, team, project, or cost-center label or annotation",
            snapshot.namespace, snapshot.name
        ),
        json!({
            "cluster_id": snapshot.cluster_id,
            "namespace": snapshot.namespace,
            "name": snapshot.name,
            "snapshot_class_name": snapshot.snapshot_class_name,
            "restore_size": snapshot.restore_size,
            "source_persistent_volume_claim_name": snapshot.source_persistent_volume_claim_name,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    snapshot: &VolumeSnapshotInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let has_error = snapshot
        .error_message
        .as_deref()
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .is_some();
    if snapshot.ready_to_use == Some(true) && !has_error {
        return;
    }

    findings.push(finding(
        snapshot,
        pillar,
        REASON_RES_NOT_READY,
        Severity::High,
        format!(
            "Kubernetes VolumeSnapshot {}/{} is not ready or has a controller error",
            snapshot.namespace, snapshot.name
        ),
        json!({
            "cluster_id": snapshot.cluster_id,
            "namespace": snapshot.namespace,
            "name": snapshot.name,
            "ready_to_use": snapshot.ready_to_use,
            "error_message": snapshot.error_message,
            "error_time": snapshot.error_time,
            "bound_volume_snapshot_content_name": snapshot.bound_volume_snapshot_content_name,
            "source_persistent_volume_claim_name": snapshot.source_persistent_volume_claim_name,
            "source_volume_snapshot_content_name": snapshot.source_volume_snapshot_content_name,
            "recommendation": "Inspect the snapshot controller events and VolumeSnapshotContent binding before relying on this snapshot for restore workflows",
        }),
    ));
}

fn evaluate_security(
    snapshot: &VolumeSnapshotInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if snapshot
        .snapshot_class_name
        .as_deref()
        .map(str::trim)
        .filter(|class_name| !class_name.is_empty())
        .is_some()
    {
        return;
    }

    findings.push(finding(
        snapshot,
        pillar,
        REASON_SEC_CLASS_NOT_SET,
        Severity::Medium,
        format!(
            "Kubernetes VolumeSnapshot {}/{} does not set an explicit VolumeSnapshotClass",
            snapshot.namespace, snapshot.name
        ),
        json!({
            "cluster_id": snapshot.cluster_id,
            "namespace": snapshot.namespace,
            "name": snapshot.name,
            "source_persistent_volume_claim_name": snapshot.source_persistent_volume_claim_name,
            "source_volume_snapshot_content_name": snapshot.source_volume_snapshot_content_name,
            "recommendation": "Set spec.volumeSnapshotClassName so driver, retention, and policy expectations are explicit for the snapshot",
        }),
    ));
}

fn stale_finding(
    snapshot: &VolumeSnapshotInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - snapshot.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        snapshot,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes VolumeSnapshot {}/{} is {} hours old (threshold {} hours)",
            snapshot.namespace, snapshot.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": snapshot.cluster_id,
            "namespace": snapshot.namespace,
            "name": snapshot.name,
            "collected_at": snapshot.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    snapshot: &VolumeSnapshotInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/VolumeSnapshot/{}",
            snapshot.cluster_id, snapshot.namespace, snapshot.name
        ),
        arn: format!(
            "kubernetes://volumesnapshots/{}/{}/{}",
            snapshot.cluster_id, snapshot.namespace, snapshot.name
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

    fn healthy_snapshot() -> VolumeSnapshotInventoryItem {
        VolumeSnapshotInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: "data-snapshot".to_string(),
            api_version: "snapshot.storage.k8s.io/v1".to_string(),
            labels: map(&[("team", "storage")]),
            annotations: BTreeMap::new(),
            snapshot_class_name: Some("csi-gp3-snapshots".to_string()),
            source_persistent_volume_claim_name: Some("data".to_string()),
            source_volume_snapshot_content_name: None,
            bound_volume_snapshot_content_name: Some("snapcontent-abc".to_string()),
            ready_to_use: Some(true),
            restore_size: Some("100Gi".to_string()),
            error_message: None,
            error_time: None,
            created_at: Some(now() - Duration::days(1)),
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let mut untagged = healthy_snapshot();
        untagged.labels.clear();

        let report =
            evaluate_kubernetes_volume_snapshot_inventory(&[untagged], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_COST_OWNER_NOT_RECORDED
        );
        assert_eq!(report.findings[0].pillar, Pillar::Cost);
    }

    #[test]
    fn resilience_flags_snapshots_that_are_not_ready_or_have_errors() {
        let mut blocked = healthy_snapshot();
        blocked.ready_to_use = Some(false);
        blocked.error_message = Some("snapshot controller timeout".to_string());

        let report =
            evaluate_kubernetes_volume_snapshot_inventory(&[blocked], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_RES_NOT_READY);
    }

    #[test]
    fn security_flags_snapshots_without_explicit_snapshot_class() {
        let mut implicit_class = healthy_snapshot();
        implicit_class.snapshot_class_name = None;

        let report = evaluate_kubernetes_volume_snapshot_inventory(
            &[implicit_class],
            Pillar::Security,
            now(),
        );

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_SEC_CLASS_NOT_SET);
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_snapshot();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_volume_snapshot_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_snapshots_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_volume_snapshot_inventory(&[healthy_snapshot()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}
