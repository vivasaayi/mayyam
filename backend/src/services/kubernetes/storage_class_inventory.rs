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

// Deterministic Kubernetes StorageClass inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-01471/01478/01499.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesStorageClass";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_SC_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_IMMEDIATE_BINDING: &str = "K8S_SC_RES_IMMEDIATE_BINDING";
pub const REASON_SEC_ENCRYPTION_NOT_DECLARED: &str = "K8S_SC_SEC_ENCRYPTION_NOT_DECLARED";
pub const REASON_INV_STALE_DATA: &str = "K8S_SC_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageClassInventoryItem {
    pub cluster_id: String,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub provisioner: String,
    pub parameters: BTreeMap<String, String>,
    pub reclaim_policy: Option<String>,
    pub volume_binding_mode: Option<String>,
    pub allow_volume_expansion: Option<bool>,
    pub mount_options: Vec<String>,
    pub allowed_topologies_count: usize,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_storage_class_inventory(
    storage_classes: &[StorageClassInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for storage_class in storage_classes {
        if let Some(finding) = stale_finding(storage_class, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(storage_class, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(storage_class, pillar, &mut findings),
            Pillar::Security => evaluate_security(storage_class, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: storage_classes.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    storage_class: &StorageClassInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&storage_class.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&storage_class.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        storage_class,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes StorageClass {} has no owner, team, project, or cost-center label or annotation",
            storage_class.name
        ),
        json!({
            "cluster_id": storage_class.cluster_id,
            "name": storage_class.name,
            "provisioner": storage_class.provisioner,
            "reclaim_policy": storage_class.reclaim_policy,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    storage_class: &StorageClassInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let binding_mode = storage_class.volume_binding_mode.as_deref().unwrap_or("");
    if !binding_mode.eq_ignore_ascii_case("Immediate") {
        return;
    }

    findings.push(finding(
        storage_class,
        pillar,
        REASON_RES_IMMEDIATE_BINDING,
        Severity::High,
        format!(
            "Kubernetes StorageClass {} binds volumes immediately before pod scheduling constraints are known",
            storage_class.name
        ),
        json!({
            "cluster_id": storage_class.cluster_id,
            "name": storage_class.name,
            "provisioner": storage_class.provisioner,
            "volume_binding_mode": storage_class.volume_binding_mode,
            "reclaim_policy": storage_class.reclaim_policy,
            "allowed_topologies_count": storage_class.allowed_topologies_count,
            "recommendation": "Prefer WaitForFirstConsumer for zonal or topology-constrained storage classes so volumes are provisioned after pod placement is known",
        }),
    ));
}

fn evaluate_security(
    storage_class: &StorageClassInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if declares_encryption(&storage_class.parameters) {
        return;
    }

    let parameter_keys = storage_class
        .parameters
        .keys()
        .cloned()
        .collect::<Vec<String>>();
    findings.push(finding(
        storage_class,
        pillar,
        REASON_SEC_ENCRYPTION_NOT_DECLARED,
        Severity::Medium,
        format!(
            "Kubernetes StorageClass {} does not declare an encryption or KMS parameter",
            storage_class.name
        ),
        json!({
            "cluster_id": storage_class.cluster_id,
            "name": storage_class.name,
            "provisioner": storage_class.provisioner,
            "parameter_keys": parameter_keys,
            "recommendation": "Declare provider-supported encryption or KMS parameters for dynamically provisioned volumes and verify the backing storage policy",
        }),
    ));
}

fn stale_finding(
    storage_class: &StorageClassInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - storage_class.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        storage_class,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes StorageClass {} is {} hours old (threshold {} hours)",
            storage_class.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": storage_class.cluster_id,
            "name": storage_class.name,
            "collected_at": storage_class.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    storage_class: &StorageClassInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/StorageClass/{}",
            storage_class.cluster_id, storage_class.name
        ),
        arn: format!(
            "kubernetes://storageclasses/{}/{}",
            storage_class.cluster_id, storage_class.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn declares_encryption(parameters: &BTreeMap<String, String>) -> bool {
    parameters.iter().any(|(key, value)| {
        let normalized_key = key.to_ascii_lowercase();
        let normalized_value = value.trim().to_ascii_lowercase();
        if normalized_value.is_empty() {
            return false;
        }

        if normalized_key.contains("kms") {
            return true;
        }

        normalized_key.contains("encrypt")
            && matches!(
                normalized_value.as_str(),
                "true" | "enabled" | "yes" | "1" | "on"
            )
    })
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

    fn healthy_storage_class() -> StorageClassInventoryItem {
        StorageClassInventoryItem {
            cluster_id: "cluster-a".to_string(),
            name: "fast-encrypted".to_string(),
            labels: map(&[("team", "storage")]),
            annotations: BTreeMap::new(),
            provisioner: "ebs.csi.aws.com".to_string(),
            parameters: map(&[("type", "gp3"), ("encrypted", "true")]),
            reclaim_policy: Some("Delete".to_string()),
            volume_binding_mode: Some("WaitForFirstConsumer".to_string()),
            allow_volume_expansion: Some(true),
            mount_options: vec!["discard".to_string()],
            allowed_topologies_count: 1,
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let mut untagged = healthy_storage_class();
        untagged.labels.clear();

        let report = evaluate_kubernetes_storage_class_inventory(&[untagged], Pillar::Cost, now());

        assert_eq!(report.resources_evaluated, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_COST_OWNER_NOT_RECORDED
        );
        assert_eq!(report.findings[0].pillar, Pillar::Cost);
    }

    #[test]
    fn resilience_flags_immediate_binding_storage_classes() {
        let mut immediate = healthy_storage_class();
        immediate.volume_binding_mode = Some("Immediate".to_string());
        immediate.allowed_topologies_count = 0;

        let report =
            evaluate_kubernetes_storage_class_inventory(&[immediate], Pillar::Resilience, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_RES_IMMEDIATE_BINDING);
    }

    #[test]
    fn security_flags_storage_classes_without_declared_encryption() {
        let mut unencrypted = healthy_storage_class();
        unencrypted.parameters.remove("encrypted");

        let report =
            evaluate_kubernetes_storage_class_inventory(&[unencrypted], Pillar::Security, now());

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_SEC_ENCRYPTION_NOT_DECLARED
        );
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_storage_class();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_storage_class_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_storage_classes_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_storage_class_inventory(
                &[healthy_storage_class()],
                pillar,
                now(),
            );

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}
