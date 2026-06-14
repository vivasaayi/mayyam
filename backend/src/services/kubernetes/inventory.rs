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

// Deterministic Kubernetes cluster inventory evaluator for roadmap rows
// 02-KUBERNETES-DASHBOARD-00001/00008/00029.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::models::cluster::Model as ClusterModel;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "KubernetesCluster";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "K8S_CLUSTER_COST_OWNER_NOT_RECORDED";
pub const REASON_RES_NOT_CONNECTED: &str = "K8S_CLUSTER_RES_NOT_CONNECTED";
pub const REASON_RES_UNREADY_STATUS: &str = "K8S_CLUSTER_RES_UNREADY_STATUS";
pub const REASON_SEC_INLINE_CREDENTIALS: &str = "K8S_CLUSTER_SEC_INLINE_CREDENTIALS";
pub const REASON_INV_STALE_DATA: &str = "K8S_CLUSTER_INV_STALE_DATA";

const READY_STATUSES: &[&str] = &["connected", "healthy", "ready", "active"];
const INLINE_CREDENTIAL_KEYS: &[&str] = &["token", "client_key_data", "client_certificate_data"];

pub fn evaluate_kubernetes_cluster_fleet(
    clusters: &[ClusterModel],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut resources_evaluated = 0;
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for cluster in clusters
        .iter()
        .filter(|cluster| cluster.cluster_type.eq_ignore_ascii_case("kubernetes"))
    {
        resources_evaluated += 1;

        if let Some(finding) = stale_finding(cluster, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(cluster, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(cluster, pillar, &mut findings),
            Pillar::Security => evaluate_security(cluster, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated,
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

fn evaluate_cost(cluster: &ClusterModel, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    if has_cost_allocation_metadata(&cluster.config) {
        return;
    }

    findings.push(finding(
        cluster,
        pillar,
        REASON_COST_OWNER_NOT_RECORDED,
        Severity::Medium,
        format!(
            "Kubernetes cluster {} has no owner, team, project, or cost-center metadata in config, labels, or tags",
            cluster.name
        ),
        json!({
            "cluster_name": cluster.name,
            "checked_keys": COST_ALLOCATION_TAG_KEYS,
            "checked_locations": ["config", "labels", "tags"],
        }),
    ));
}

fn evaluate_resilience(
    cluster: &ClusterModel,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if cluster.last_connected_at.is_none() {
        findings.push(finding(
            cluster,
            pillar,
            REASON_RES_NOT_CONNECTED,
            Severity::High,
            format!(
                "Kubernetes cluster {} has never completed a successful connection",
                cluster.name
            ),
            json!({
                "cluster_name": cluster.name,
                "last_connected_at": cluster.last_connected_at,
            }),
        ));
    }

    let ready = cluster
        .status
        .as_deref()
        .map(is_ready_status)
        .unwrap_or(false);
    if !ready {
        findings.push(finding(
            cluster,
            pillar,
            REASON_RES_UNREADY_STATUS,
            Severity::Medium,
            format!(
                "Kubernetes cluster {} status is not in a ready state",
                cluster.name
            ),
            json!({
                "cluster_name": cluster.name,
                "status": cluster.status,
                "ready_statuses": READY_STATUSES,
            }),
        ));
    }
}

fn evaluate_security(cluster: &ClusterModel, pillar: Pillar, findings: &mut Vec<InventoryFinding>) {
    let inline_fields = INLINE_CREDENTIAL_KEYS
        .iter()
        .copied()
        .filter(|key| non_empty_config_string(&cluster.config, key))
        .collect::<Vec<_>>();

    if inline_fields.is_empty() {
        return;
    }

    findings.push(finding(
        cluster,
        pillar,
        REASON_SEC_INLINE_CREDENTIALS,
        Severity::High,
        format!(
            "Kubernetes cluster {} stores inline credential material in its persisted config",
            cluster.name
        ),
        json!({
            "cluster_name": cluster.name,
            "inline_credential_fields": inline_fields,
        }),
    ));
}

fn stale_finding(
    cluster: &ClusterModel,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - cluster.updated_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        cluster,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Kubernetes cluster {} is {} hours old (threshold {} hours)",
            cluster.name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "cluster_name": cluster.name,
            "updated_at": cluster.updated_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    cluster: &ClusterModel,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: cluster.id.to_string(),
        arn: String::new(),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_cost_allocation_metadata(config: &Value) -> bool {
    config_has_any_key(config, COST_ALLOCATION_TAG_KEYS)
        || nested_object_has_any_key(config, "labels", COST_ALLOCATION_TAG_KEYS)
        || nested_object_has_any_key(config, "tags", COST_ALLOCATION_TAG_KEYS)
}

fn config_has_any_key(config: &Value, wanted_keys: &[&str]) -> bool {
    let Some(map) = config.as_object() else {
        return false;
    };

    wanted_keys.iter().any(|wanted| {
        map.iter()
            .any(|(key, value)| key.eq_ignore_ascii_case(wanted) && value_is_present(value))
    })
}

fn nested_object_has_any_key(config: &Value, nested_key: &str, wanted_keys: &[&str]) -> bool {
    let Some(map) = config.as_object() else {
        return false;
    };
    let Some((_, nested)) = map
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(nested_key))
    else {
        return false;
    };

    config_has_any_key(nested, wanted_keys)
}

fn value_is_present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        Value::Bool(_) | Value::Number(_) => true,
    }
}

fn non_empty_config_string(config: &Value, wanted_key: &str) -> bool {
    let Some(map) = config.as_object() else {
        return false;
    };

    map.iter().any(|(key, value)| {
        key.eq_ignore_ascii_case(wanted_key)
            && value
                .as_str()
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
    })
}

fn is_ready_status(status: &str) -> bool {
    READY_STATUSES
        .iter()
        .any(|ready_status| status.eq_ignore_ascii_case(ready_status))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use serde_json::{json, Value};
    use uuid::Uuid;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn fixture(
        name: &str,
        cluster_type: &str,
        config: Value,
        status: Option<&str>,
        last_connected_at: Option<DateTime<Utc>>,
        updated_at: DateTime<Utc>,
    ) -> ClusterModel {
        ClusterModel {
            id: Uuid::new_v4(),
            name: name.to_string(),
            cluster_type: cluster_type.to_string(),
            config,
            created_by: Uuid::new_v4(),
            created_at: updated_at,
            updated_at,
            last_connected_at,
            status: status.map(str::to_string),
        }
    }

    fn healthy_config() -> Value {
        json!({
            "api_server_url": "https://cluster.example",
            "kube_context": "prod",
            "labels": {
                "owner": "platform",
                "cost-center": "cc-42"
            }
        })
    }

    fn reason_codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect()
    }

    #[test]
    fn cost_flags_missing_owner_and_cost_allocation_metadata() {
        let cluster = fixture(
            "prod",
            "kubernetes",
            json!({"api_server_url": "https://cluster.example"}),
            Some("connected"),
            Some(now() - Duration::hours(1)),
            now() - Duration::hours(1),
        );

        let report = evaluate_kubernetes_cluster_fleet(&[cluster], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 1);
        assert!(reason_codes(&report).contains(&REASON_COST_OWNER_NOT_RECORDED));
    }

    #[test]
    fn resilience_flags_never_connected_and_unready_status() {
        let cluster = fixture(
            "new-cluster",
            "kubernetes",
            healthy_config(),
            Some("new"),
            None,
            now() - Duration::hours(1),
        );

        let report = evaluate_kubernetes_cluster_fleet(&[cluster], Pillar::Resilience, now());
        let codes = reason_codes(&report);
        assert!(codes.contains(&REASON_RES_NOT_CONNECTED));
        assert!(codes.contains(&REASON_RES_UNREADY_STATUS));
    }

    #[test]
    fn security_flags_inline_cluster_credentials() {
        let mut config = healthy_config();
        config["token"] = json!("plain-token");
        config["client_key_data"] = json!("plain-key");
        let cluster = fixture(
            "credentialed",
            "kubernetes",
            config,
            Some("connected"),
            Some(now() - Duration::hours(1)),
            now() - Duration::hours(1),
        );

        let report = evaluate_kubernetes_cluster_fleet(&[cluster], Pillar::Security, now());
        assert!(reason_codes(&report).contains(&REASON_SEC_INLINE_CREDENTIALS));
    }

    #[test]
    fn healthy_cluster_passes_claimed_pillars() {
        let cluster = fixture(
            "healthy",
            "kubernetes",
            healthy_config(),
            Some("connected"),
            Some(now() - Duration::hours(1)),
            now() - Duration::hours(1),
        );

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_kubernetes_cluster_fleet(std::slice::from_ref(&cluster), pillar, now());
            assert!(
                report.findings.is_empty(),
                "unexpected for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }

    #[test]
    fn non_kubernetes_clusters_are_skipped() {
        let cluster = fixture(
            "kafka",
            "kafka",
            json!({}),
            Some("connected"),
            Some(now() - Duration::hours(1)),
            now() - Duration::hours(1),
        );

        let report = evaluate_kubernetes_cluster_fleet(&[cluster], Pillar::Cost, now());
        assert_eq!(report.resources_evaluated, 0);
        assert!(report.findings.is_empty());
    }
}
