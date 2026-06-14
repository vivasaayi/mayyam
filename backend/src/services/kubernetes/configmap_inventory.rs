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

// Deterministic Kubernetes ConfigMap inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00736/00743/00764.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesConfigMap";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_CONFIGMAP_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_MUTABLE_DATA: &str = "K8S_CONFIGMAP_RES_MUTABLE_DATA";
pub const REASON_RES_EMPTY_DATA: &str = "K8S_CONFIGMAP_RES_EMPTY_DATA";
pub const REASON_SEC_SENSITIVE_KEY_NAME: &str = "K8S_CONFIGMAP_SEC_SENSITIVE_KEY_NAME";
pub const REASON_INV_STALE_DATA: &str = "K8S_CONFIGMAP_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMapOwnerReferenceInventoryItem {
    pub kind: Option<String>,
    pub name: String,
    pub controller: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMapInventoryItem {
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub data_keys: Vec<String>,
    pub binary_data_keys: Vec<String>,
    pub total_data_bytes: usize,
    pub immutable: bool,
    pub owner_references: Vec<ConfigMapOwnerReferenceInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_configmap_inventory(
    configmaps: &[ConfigMapInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for configmap in configmaps {
        if let Some(finding) = stale_finding(configmap, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(configmap, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(configmap, pillar, &mut findings),
            Pillar::Security => evaluate_security(configmap, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: configmaps.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    configmap: &ConfigMapInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&configmap.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&configmap.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        configmap,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes ConfigMap {}/{} has no owner, team, project, or cost-center label or annotation",
            configmap.namespace, configmap.name
        ),
        json!({
            "cluster_id": configmap.cluster_id,
            "namespace": configmap.namespace,
            "name": configmap.name,
            "data_keys": configmap.data_keys,
            "binary_data_keys": configmap.binary_data_keys,
            "total_data_bytes": configmap.total_data_bytes,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    configmap: &ConfigMapInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !configmap.immutable {
        findings.push(finding(
            configmap,
            pillar,
            REASON_RES_MUTABLE_DATA,
            Severity::Medium,
            format!(
                "Kubernetes ConfigMap {}/{} is mutable and can drift without a rollout boundary",
                configmap.namespace, configmap.name
            ),
            json!({
                "cluster_id": configmap.cluster_id,
                "namespace": configmap.namespace,
                "name": configmap.name,
                "immutable": configmap.immutable,
                "owner_references": configmap.owner_references,
            }),
        ));
    }

    if configmap.data_keys.is_empty() && configmap.binary_data_keys.is_empty() {
        findings.push(finding(
            configmap,
            pillar,
            REASON_RES_EMPTY_DATA,
            Severity::Medium,
            format!(
                "Kubernetes ConfigMap {}/{} has no data or binaryData keys",
                configmap.namespace, configmap.name
            ),
            json!({
                "cluster_id": configmap.cluster_id,
                "namespace": configmap.namespace,
                "name": configmap.name,
                "total_data_bytes": configmap.total_data_bytes,
            }),
        ));
    }
}

fn evaluate_security(
    configmap: &ConfigMapInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let sensitive_keys = configmap
        .data_keys
        .iter()
        .chain(configmap.binary_data_keys.iter())
        .filter(|key| is_secret_like_key(key))
        .cloned()
        .collect::<Vec<_>>();

    if !sensitive_keys.is_empty() {
        findings.push(finding(
            configmap,
            pillar,
            REASON_SEC_SENSITIVE_KEY_NAME,
            Severity::High,
            format!(
                "Kubernetes ConfigMap {}/{} contains secret-like data key names",
                configmap.namespace, configmap.name
            ),
            json!({
                "cluster_id": configmap.cluster_id,
                "namespace": configmap.namespace,
                "name": configmap.name,
                "sensitive_keys": sensitive_keys,
                "total_data_bytes": configmap.total_data_bytes,
                "recommendation": "Move secret material to Kubernetes Secret or an external secret manager",
            }),
        ));
    }
}

fn stale_finding(
    configmap: &ConfigMapInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - configmap.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        configmap,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes ConfigMap {}/{} is {} hours old (threshold {} hours)",
            configmap.namespace, configmap.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": configmap.cluster_id,
            "namespace": configmap.namespace,
            "name": configmap.name,
            "collected_at": configmap.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    configmap: &ConfigMapInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!(
            "{}/{}/ConfigMap/{}",
            configmap.cluster_id, configmap.namespace, configmap.name
        ),
        arn: format!(
            "kubernetes://configmaps/{}/{}/{}",
            configmap.cluster_id, configmap.namespace, configmap.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn is_secret_like_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    [
        "password",
        "passwd",
        "secret",
        "token",
        "credential",
        "private_key",
        "private-key",
        "apikey",
        "api_key",
        "api-key",
        "client_secret",
        "client-secret",
        "tls.key",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
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

    fn owner(kind: &str, name: &str) -> ConfigMapOwnerReferenceInventoryItem {
        ConfigMapOwnerReferenceInventoryItem {
            kind: Some(kind.to_string()),
            name: name.to_string(),
            controller: Some(true),
        }
    }

    fn configmap(name: &str, metadata_labels: BTreeMap<String, String>) -> ConfigMapInventoryItem {
        ConfigMapInventoryItem {
            cluster_id: "cluster-a".to_string(),
            namespace: "apps".to_string(),
            name: name.to_string(),
            labels: metadata_labels,
            annotations: BTreeMap::new(),
            data_keys: vec!["app.properties".to_string()],
            binary_data_keys: Vec::new(),
            total_data_bytes: 128,
            immutable: true,
            owner_references: vec![owner("Deployment", "checkout")],
            created_at: Some(now() - Duration::days(3)),
            collected_at: now(),
        }
    }

    fn healthy_configmap() -> ConfigMapInventoryItem {
        configmap("checkout-config", labels(&[("team", "payments")]))
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let report = evaluate_kubernetes_configmap_inventory(
            &[configmap("untagged", BTreeMap::new())],
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
    fn resilience_flags_mutable_and_empty_configmaps() {
        let mut risky = healthy_configmap();
        risky.immutable = false;
        risky.data_keys = Vec::new();
        risky.binary_data_keys = Vec::new();
        risky.total_data_bytes = 0;

        let report = evaluate_kubernetes_configmap_inventory(&[risky], Pillar::Resilience, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_RES_MUTABLE_DATA));
        assert!(reason_codes.contains(&REASON_RES_EMPTY_DATA));
    }

    #[test]
    fn security_flags_secret_like_keys_in_configmap_data() {
        let mut exposed = healthy_configmap();
        exposed.data_keys = vec!["database-password".to_string(), "api-token".to_string()];
        exposed.total_data_bytes = 96;

        let report = evaluate_kubernetes_configmap_inventory(&[exposed], Pillar::Security, now());
        let reason_codes = report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect::<Vec<_>>();

        assert!(reason_codes.contains(&REASON_SEC_SENSITIVE_KEY_NAME));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_configmap();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report = evaluate_kubernetes_configmap_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_configmaps_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_configmap_inventory(&[healthy_configmap()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}
