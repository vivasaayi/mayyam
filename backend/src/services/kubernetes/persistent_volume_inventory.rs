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

// Deterministic Kubernetes PersistentVolume inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01373/01380/01401.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesPersistentVolume";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_PV_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_FAILED_OR_RELEASED: &str = "K8S_PV_RES_FAILED_OR_RELEASED";
pub const REASON_SEC_HOST_PATH_VOLUME: &str = "K8S_PV_SEC_HOST_PATH_VOLUME";
pub const REASON_INV_STALE_DATA: &str = "K8S_PV_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentVolumeClaimRefInventoryItem {
    pub namespace: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentVolumeInventoryItem {
    pub cluster_id: String,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub capacity: BTreeMap<String, String>,
    pub access_modes: Vec<String>,
    pub reclaim_policy: Option<String>,
    pub phase: Option<String>,
    pub reason: Option<String>,
    pub claim_ref: Option<PersistentVolumeClaimRefInventoryItem>,
    pub storage_class_name: Option<String>,
    pub volume_mode: Option<String>,
    pub source_types: Vec<String>,
    pub csi_driver: Option<String>,
    pub csi_volume_handle_present: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_persistent_volume_inventory(
    volumes: &[PersistentVolumeInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for volume in volumes {
        if let Some(finding) = stale_finding(volume, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(volume, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(volume, pillar, &mut findings),
            Pillar::Security => evaluate_security(volume, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: volumes.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    volume: &PersistentVolumeInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&volume.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&volume.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        volume,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes PersistentVolume {} has no owner, team, project, or cost-center label or annotation",
            volume.name
        ),
        json!({
            "cluster_id": volume.cluster_id,
            "name": volume.name,
            "capacity": volume.capacity,
            "storage_class_name": volume.storage_class_name,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    volume: &PersistentVolumeInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let phase = volume.phase.as_deref().unwrap_or("");
    if !phase.eq_ignore_ascii_case("Failed") && !phase.eq_ignore_ascii_case("Released") {
        return;
    }

    findings.push(finding(
        volume,
        pillar,
        REASON_RES_FAILED_OR_RELEASED,
        Severity::High,
        format!(
            "Kubernetes PersistentVolume {} is in {} phase",
            volume.name,
            if phase.is_empty() { "unknown" } else { phase }
        ),
        json!({
            "cluster_id": volume.cluster_id,
            "name": volume.name,
            "phase": volume.phase,
            "reason": volume.reason,
            "reclaim_policy": volume.reclaim_policy,
            "claim_ref": volume.claim_ref,
            "recommendation": "Investigate released or failed PersistentVolumes before reusing storage; verify reclaim policy, claim binding, and underlying storage health",
        }),
    ));
}

fn evaluate_security(
    volume: &PersistentVolumeInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !volume
        .source_types
        .iter()
        .any(|source_type| source_type.eq_ignore_ascii_case("hostPath"))
    {
        return;
    }

    findings.push(finding(
        volume,
        pillar,
        REASON_SEC_HOST_PATH_VOLUME,
        Severity::High,
        format!(
            "Kubernetes PersistentVolume {} uses hostPath storage",
            volume.name
        ),
        json!({
            "cluster_id": volume.cluster_id,
            "name": volume.name,
            "source_types": volume.source_types,
            "storage_class_name": volume.storage_class_name,
            "volume_mode": volume.volume_mode,
            "recommendation": "Avoid hostPath PersistentVolumes outside tightly controlled single-node or local development environments",
        }),
    ));
}

fn stale_finding(
    volume: &PersistentVolumeInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - volume.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        volume,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes PersistentVolume {} is {} hours old (threshold {} hours)",
            volume.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": volume.cluster_id,
            "name": volume.name,
            "collected_at": volume.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    volume: &PersistentVolumeInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!("{}/PersistentVolume/{}", volume.cluster_id, volume.name),
        arn: format!(
            "kubernetes://persistentvolumes/{}/{}",
            volume.cluster_id, volume.name
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

    fn healthy_volume() -> PersistentVolumeInventoryItem {
        PersistentVolumeInventoryItem {
            cluster_id: "cluster-a".to_string(),
            name: "pv-fast-1".to_string(),
            labels: map(&[("team", "storage")]),
            annotations: BTreeMap::new(),
            capacity: map(&[("storage", "100Gi")]),
            access_modes: vec!["ReadWriteOnce".to_string()],
            reclaim_policy: Some("Retain".to_string()),
            phase: Some("Bound".to_string()),
            reason: None,
            claim_ref: Some(PersistentVolumeClaimRefInventoryItem {
                namespace: Some("apps".to_string()),
                name: Some("data".to_string()),
            }),
            storage_class_name: Some("fast".to_string()),
            volume_mode: Some("Filesystem".to_string()),
            source_types: vec!["csi".to_string()],
            csi_driver: Some("ebs.csi.aws.com".to_string()),
            csi_volume_handle_present: true,
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let mut untagged = healthy_volume();
        untagged.labels.clear();

        let report =
            evaluate_kubernetes_persistent_volume_inventory(&[untagged], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_COST_OWNER_NOT_RECORDED
        );
        assert_eq!(report.findings[0].pillar, Pillar::Cost);
    }

    #[test]
    fn resilience_flags_failed_or_released_volumes() {
        let mut released = healthy_volume();
        released.phase = Some("Released".to_string());
        released.claim_ref = None;

        let report =
            evaluate_kubernetes_persistent_volume_inventory(&[released], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_RES_FAILED_OR_RELEASED
        );
    }

    #[test]
    fn security_flags_host_path_persistent_volumes() {
        let mut host_path = healthy_volume();
        host_path.source_types = vec!["hostPath".to_string()];
        host_path.csi_driver = None;
        host_path.csi_volume_handle_present = false;

        let report =
            evaluate_kubernetes_persistent_volume_inventory(&[host_path], Pillar::Security, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_SEC_HOST_PATH_VOLUME);
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_volume();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_persistent_volume_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_persistent_volumes_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_persistent_volume_inventory(&[healthy_volume()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}
