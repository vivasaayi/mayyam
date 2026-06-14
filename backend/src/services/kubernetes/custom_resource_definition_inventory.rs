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

// Deterministic Kubernetes CustomResourceDefinition inventory evaluator for
// roadmap rows 02-KUBERNETES-DASHBOARD-01569/01576/01597.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesCustomResourceDefinition";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_CRD_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_NOT_ESTABLISHED: &str = "K8S_CRD_RES_NOT_ESTABLISHED";
pub const REASON_SEC_SCHEMA_NOT_ENFORCED: &str = "K8S_CRD_SEC_SCHEMA_NOT_ENFORCED";
pub const REASON_INV_STALE_DATA: &str = "K8S_CRD_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomResourceDefinitionVersionInventoryItem {
    pub name: String,
    pub served: bool,
    pub storage: bool,
    pub deprecated: bool,
    pub has_schema: bool,
    pub has_status_subresource: bool,
    pub has_scale_subresource: bool,
    pub additional_printer_columns_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomResourceDefinitionConditionInventoryItem {
    pub condition_type: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomResourceDefinitionInventoryItem {
    pub cluster_id: String,
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
    pub group: String,
    pub scope: String,
    pub kind: String,
    pub plural: String,
    pub singular: Option<String>,
    pub short_names: Vec<String>,
    pub categories: Vec<String>,
    pub preserve_unknown_fields: Option<bool>,
    pub versions: Vec<CustomResourceDefinitionVersionInventoryItem>,
    pub stored_versions: Vec<String>,
    pub conditions: Vec<CustomResourceDefinitionConditionInventoryItem>,
    pub created_at: Option<DateTime<Utc>>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_kubernetes_custom_resource_definition_inventory(
    crds: &[CustomResourceDefinitionInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for crd in crds {
        if let Some(finding) = stale_finding(crd, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(crd, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(crd, pillar, &mut findings),
            Pillar::Security => evaluate_security(crd, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: crds.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(
    crd: &CustomResourceDefinitionInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if has_any_metadata_key(&crd.labels, COST_ALLOCATION_TAG_KEYS)
        || has_any_metadata_key(&crd.annotations, COST_ALLOCATION_TAG_KEYS)
    {
        return;
    }

    findings.push(finding(
        crd,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes CustomResourceDefinition {} has no owner, team, project, or cost-center label or annotation",
            crd.name
        ),
        json!({
            "cluster_id": crd.cluster_id,
            "name": crd.name,
            "group": crd.group,
            "kind": crd.kind,
            "plural": crd.plural,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["labels", "annotations"],
        }),
    ));
}

fn evaluate_resilience(
    crd: &CustomResourceDefinitionInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if condition_is_true(crd, "Established") && condition_is_true(crd, "NamesAccepted") {
        return;
    }

    findings.push(finding(
        crd,
        pillar,
        REASON_RES_NOT_ESTABLISHED,
        Severity::High,
        format!(
            "Kubernetes CustomResourceDefinition {} is not fully established",
            crd.name
        ),
        json!({
            "cluster_id": crd.cluster_id,
            "name": crd.name,
            "group": crd.group,
            "kind": crd.kind,
            "conditions": crd.conditions,
            "stored_versions": crd.stored_versions,
            "recommendation": "Inspect CRD status conditions and API server registration before relying on custom resources for this definition",
        }),
    ));
}

fn evaluate_security(
    crd: &CustomResourceDefinitionInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    let served_versions_without_schema = crd
        .versions
        .iter()
        .filter(|version| version.served && !version.has_schema)
        .map(|version| version.name.clone())
        .collect::<Vec<_>>();
    let preserves_unknown_fields = crd.preserve_unknown_fields.unwrap_or(false);

    if served_versions_without_schema.is_empty() && !preserves_unknown_fields {
        return;
    }

    findings.push(finding(
        crd,
        pillar,
        REASON_SEC_SCHEMA_NOT_ENFORCED,
        Severity::High,
        format!(
            "Kubernetes CustomResourceDefinition {} has served versions without schema enforcement",
            crd.name
        ),
        json!({
            "cluster_id": crd.cluster_id,
            "name": crd.name,
            "group": crd.group,
            "kind": crd.kind,
            "served_versions_without_schema": served_versions_without_schema,
            "preserve_unknown_fields": crd.preserve_unknown_fields,
            "versions": crd.versions,
            "recommendation": "Define openAPIV3Schema for every served CRD version and avoid preserveUnknownFields so invalid or unexpected fields are pruned and rejected",
        }),
    ));
}

fn stale_finding(
    crd: &CustomResourceDefinitionInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - crd.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        crd,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes CustomResourceDefinition {} is {} hours old (threshold {} hours)",
            crd.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_id": crd.cluster_id,
            "name": crd.name,
            "collected_at": crd.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    crd: &CustomResourceDefinitionInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: format!("{}/CustomResourceDefinition/{}", crd.cluster_id, crd.name),
        arn: format!(
            "kubernetes://customresourcedefinitions/{}/{}",
            crd.cluster_id, crd.name
        ),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn condition_is_true(crd: &CustomResourceDefinitionInventoryItem, condition_type: &str) -> bool {
    crd.conditions.iter().any(|condition| {
        condition
            .condition_type
            .eq_ignore_ascii_case(condition_type)
            && condition.status.eq_ignore_ascii_case("True")
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

    fn healthy_crd() -> CustomResourceDefinitionInventoryItem {
        CustomResourceDefinitionInventoryItem {
            cluster_id: "cluster-a".to_string(),
            name: "widgets.example.com".to_string(),
            labels: map(&[("team", "platform")]),
            annotations: BTreeMap::new(),
            group: "example.com".to_string(),
            scope: "Namespaced".to_string(),
            kind: "Widget".to_string(),
            plural: "widgets".to_string(),
            singular: Some("widget".to_string()),
            short_names: vec!["wdg".to_string()],
            categories: vec!["all".to_string()],
            preserve_unknown_fields: Some(false),
            versions: vec![CustomResourceDefinitionVersionInventoryItem {
                name: "v1".to_string(),
                served: true,
                storage: true,
                deprecated: false,
                has_schema: true,
                has_status_subresource: true,
                has_scale_subresource: false,
                additional_printer_columns_count: 1,
            }],
            stored_versions: vec!["v1".to_string()],
            conditions: vec![
                CustomResourceDefinitionConditionInventoryItem {
                    condition_type: "Established".to_string(),
                    status: "True".to_string(),
                    reason: Some("InitialNamesAccepted".to_string()),
                    message: None,
                },
                CustomResourceDefinitionConditionInventoryItem {
                    condition_type: "NamesAccepted".to_string(),
                    status: "True".to_string(),
                    reason: Some("NoConflicts".to_string()),
                    message: None,
                },
            ],
            created_at: Some(now() - Duration::days(1)),
            collected_at: now(),
        }
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_labels() {
        let mut untagged = healthy_crd();
        untagged.labels.clear();

        let report = evaluate_kubernetes_custom_resource_definition_inventory(
            &[untagged],
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
    fn resilience_flags_crds_that_are_not_established() {
        let mut unestablished = healthy_crd();
        unestablished.conditions[0].status = "False".to_string();
        unestablished.conditions[0].reason = Some("Installing".to_string());

        let report = evaluate_kubernetes_custom_resource_definition_inventory(
            &[unestablished],
            Pillar::Resilience,
            now(),
        );

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].reason_code, REASON_RES_NOT_ESTABLISHED);
    }

    #[test]
    fn security_flags_served_versions_without_schema() {
        let mut schemaless = healthy_crd();
        schemaless.versions[0].has_schema = false;

        let report = evaluate_kubernetes_custom_resource_definition_inventory(
            &[schemaless],
            Pillar::Security,
            now(),
        );

        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].reason_code,
            REASON_SEC_SCHEMA_NOT_ENFORCED
        );
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut stale = healthy_crd();
        stale.collected_at = now() - Duration::hours(DEFAULT_STALE_AFTER_HOURS + 2);

        let report =
            evaluate_kubernetes_custom_resource_definition_inventory(&[stale], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert_eq!(report.findings[0].reason_code, REASON_INV_STALE_DATA);
    }

    #[test]
    fn healthy_crds_pass_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_kubernetes_custom_resource_definition_inventory(
                &[healthy_crd()],
                pillar,
                now(),
            );

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
        }
    }
}
