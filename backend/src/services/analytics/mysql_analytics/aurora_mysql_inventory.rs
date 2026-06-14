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

// Deterministic Aurora MySQL inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00540/00547/00568.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "AuroraMySql";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "AURORA_MYSQL_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_AURORA_METRICS: &str = "AURORA_MYSQL_COST_NO_METRICS";
pub const REASON_COST_CLUSTER_SPEND_REVIEW: &str = "AURORA_MYSQL_COST_CLUSTER_SPEND_REVIEW";
pub const REASON_RES_NO_AURORA_METRICS: &str = "AURORA_MYSQL_RES_NO_METRICS";
pub const REASON_RES_TOPOLOGY_GAP: &str = "AURORA_MYSQL_RES_TOPOLOGY_GAP";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str = "AURORA_MYSQL_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_NO_AURORA_METRICS: &str = "AURORA_MYSQL_SEC_NO_METRICS";
pub const REASON_SEC_AURORA_REVIEW: &str = "AURORA_MYSQL_SEC_AURORA_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "AURORA_MYSQL_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuroraMysqlInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub aurora_metric_count: usize,
    pub cluster_identifier: Option<String>,
    pub writer_endpoint_known: Option<bool>,
    pub reader_endpoint_known: Option<bool>,
    pub replica_count: Option<i64>,
    pub write_operations: i64,
    pub qps_since_start: f64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_aurora_inventory(
    items: &[AuroraMysqlInventoryItem],
    pillar: Pillar,
    now: DateTime<Utc>,
) -> PillarReport {
    let mut stale_resources = 0;
    let mut findings = Vec::new();

    for item in items {
        if let Some(finding) = stale_finding(item, pillar, now) {
            stale_resources += 1;
            findings.push(finding);
        }

        match pillar {
            Pillar::Cost => evaluate_cost(item, pillar, &mut findings),
            Pillar::Resilience => evaluate_resilience(item, pillar, &mut findings),
            Pillar::Security => evaluate_security(item, pillar, &mut findings),
            _ => {}
        }
    }

    PillarReport {
        pillar,
        resources_evaluated: items.len(),
        stale_resources,
        score: score_pillar(&findings),
        findings,
    }
}

pub fn aurora_mysql_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> AuroraMysqlInventoryItem {
    AuroraMysqlInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        aurora_metric_count: 0,
        cluster_identifier: None,
        writer_endpoint_known: None,
        reader_endpoint_known: None,
        replica_count: None,
        write_operations: snapshot.workload.com_insert
            + snapshot.workload.com_update
            + snapshot.workload.com_delete,
        qps_since_start: snapshot.workload.qps_since_start,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &AuroraMysqlInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!(
                "Aurora MySQL inventory for connection {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "owner": item.owner,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
                "checked_locations": ["owner", "labels"],
            }),
        ));
    }

    if !has_aurora_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_AURORA_METRICS,
            Severity::High,
            format!(
                "Aurora MySQL inventory for connection {} has no Aurora cluster evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "aurora_metric_count": item.aurora_metric_count,
                "recommendation": "Collect Aurora cluster identifier, writer/reader endpoint, replica count, and provider cost evidence before making capacity or storage-spend recommendations",
            }),
        ));
    }

    if has_cluster_spend_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_CLUSTER_SPEND_REVIEW,
            Severity::Medium,
            format!(
                "Aurora MySQL cluster {} needs spend review before scaling",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "cluster_identifier": item.cluster_identifier,
                "replica_count": item.replica_count,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Review Aurora replica count, writer load, reader endpoint use, storage growth, and failover requirements before adding instances or increasing spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &AuroraMysqlInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_aurora_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_AURORA_METRICS,
            Severity::High,
            format!(
                "Aurora MySQL inventory for connection {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "aurora_metric_count": item.aurora_metric_count,
                "recommendation": "Collect Aurora cluster topology, writer endpoint, reader endpoint, and replica count evidence so failover readiness can be evaluated deterministically",
            }),
        ));
    }

    if has_topology_gap(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_TOPOLOGY_GAP,
            Severity::High,
            format!(
                "Aurora MySQL topology evidence is incomplete for connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "cluster_identifier": item.cluster_identifier,
                "writer_endpoint_known": item.writer_endpoint_known,
                "reader_endpoint_known": item.reader_endpoint_known,
                "replica_count": item.replica_count,
                "recommendation": "Validate cluster identifier, writer endpoint, reader endpoint, replica count, and failover target before relying on this Aurora cluster for resilience",
            }),
        ));
    }
}

fn evaluate_security(
    item: &AuroraMysqlInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item
        .server_version
        .as_deref()
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .is_none()
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_VERSION_NOT_RECORDED,
            Severity::Medium,
            format!(
                "Aurora MySQL inventory for connection {} has no recorded engine version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record Aurora MySQL engine version with cluster evidence so version-specific patching, TLS, and parameter guidance can be mapped deterministically",
            }),
        ));
    }

    if !has_aurora_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_AURORA_METRICS,
            Severity::High,
            format!(
                "Aurora MySQL inventory for connection {} has no scoped evidence for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "aurora_metric_count": item.aurora_metric_count,
                "recommendation": "Collect scoped Aurora cluster evidence so endpoint exposure, replica topology, parameter groups, and incident blast radius can be reviewed without ad hoc privileged diagnostics",
            }),
        ));
    }

    if needs_aurora_security_review(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_AURORA_REVIEW,
            Severity::Medium,
            format!(
                "Aurora MySQL cluster evidence for connection {} should be reviewed",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "cluster_identifier": item.cluster_identifier,
                "writer_endpoint_known": item.writer_endpoint_known,
                "reader_endpoint_known": item.reader_endpoint_known,
                "replica_count": item.replica_count,
                "recommendation": "Review Aurora cluster endpoint exposure, subnet/security group posture, parameter groups, audit settings, and replica topology before sharing evidence outside the database team",
            }),
        ));
    }
}

fn stale_finding(
    item: &AuroraMysqlInventoryItem,
    pillar: Pillar,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - item.collected_at).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }

    Some(finding(
        item,
        pillar,
        REASON_INV_STALE_DATA,
        Severity::Medium,
        format!(
            "Inventory data for Aurora MySQL connection {} is {} hours old (threshold {} hours)",
            item.connection_name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "connection_id": item.connection_id,
            "connection_name": item.connection_name,
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    item: &AuroraMysqlInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://aurora-mysql/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &AuroraMysqlInventoryItem) -> bool {
    item.owner
        .as_deref()
        .map(str::trim)
        .filter(|owner| !owner.is_empty())
        .is_some()
        || has_any_metadata_key(&item.labels, COST_ALLOCATION_TAG_KEYS)
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

fn has_aurora_metrics(item: &AuroraMysqlInventoryItem) -> bool {
    item.aurora_metric_count > 0
}

fn cluster_identifier_present(item: &AuroraMysqlInventoryItem) -> bool {
    item.cluster_identifier
        .as_deref()
        .map(str::trim)
        .filter(|cluster| !cluster.is_empty())
        .is_some()
}

fn has_cluster_spend_pressure(item: &AuroraMysqlInventoryItem) -> bool {
    has_aurora_metrics(item)
        && (item.replica_count.unwrap_or(0) >= 4
            || ((item.write_operations >= 25_000 || item.qps_since_start >= 80.0)
                && item.replica_count.unwrap_or(0) >= 2))
}

fn has_topology_gap(item: &AuroraMysqlInventoryItem) -> bool {
    has_aurora_metrics(item)
        && (!cluster_identifier_present(item)
            || item.writer_endpoint_known != Some(true)
            || item.reader_endpoint_known != Some(true)
            || item.replica_count.unwrap_or(0) < 1)
}

fn needs_aurora_security_review(item: &AuroraMysqlInventoryItem) -> bool {
    has_aurora_metrics(item)
        && (!cluster_identifier_present(item)
            || item.writer_endpoint_known != Some(true)
            || item.reader_endpoint_known != Some(true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-12T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn labels(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    fn item(
        owner: Option<&str>,
        server_version: Option<&str>,
        aurora_metric_count: usize,
        cluster_identifier: Option<&str>,
        writer_endpoint_known: Option<bool>,
        reader_endpoint_known: Option<bool>,
        replica_count: Option<i64>,
        write_operations: i64,
        qps_since_start: f64,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> AuroraMysqlInventoryItem {
        AuroraMysqlInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-aurora".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: server_version.map(str::to_string),
            aurora_metric_count,
            cluster_identifier: cluster_identifier.map(str::to_string),
            writer_endpoint_known,
            reader_endpoint_known,
            replica_count,
            write_operations,
            qps_since_start,
            collected_at,
        }
    }

    fn healthy_item() -> AuroraMysqlInventoryItem {
        item(
            Some("database-platform"),
            Some("8.0.mysql_aurora.3.05.2"),
            4,
            Some("orders-prod"),
            Some(true),
            Some(true),
            Some(2),
            1_000,
            10.0,
            labels(&[("cost-center", "cc-42")]),
            now(),
        )
    }

    fn reason_codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect()
    }

    #[test]
    fn cost_flags_missing_owner_missing_metrics_and_cluster_spend_review() {
        let missing_metrics = item(
            Some(""),
            Some("8.0.mysql_aurora.3.05.2"),
            0,
            None,
            None,
            None,
            None,
            0,
            0.0,
            BTreeMap::new(),
            now(),
        );
        let oversized_cluster = item(
            Some("database-platform"),
            Some("8.0.mysql_aurora.3.05.2"),
            4,
            Some("orders-prod"),
            Some(true),
            Some(true),
            Some(5),
            80_000,
            150.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_aurora_inventory(
            &[missing_metrics, oversized_cluster],
            Pillar::Cost,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_NO_AURORA_METRICS));
        assert!(codes.contains(&REASON_COST_CLUSTER_SPEND_REVIEW));
    }

    #[test]
    fn resilience_flags_missing_metrics_and_topology_gap() {
        let missing_metrics = item(
            Some("database-platform"),
            Some("8.0.mysql_aurora.3.05.2"),
            0,
            None,
            None,
            None,
            None,
            0,
            0.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let topology_gap = item(
            Some("database-platform"),
            Some("8.0.mysql_aurora.3.05.2"),
            4,
            Some("orders-prod"),
            Some(false),
            Some(true),
            Some(0),
            50_000,
            100.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_aurora_inventory(
            &[missing_metrics, topology_gap],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NO_AURORA_METRICS));
        assert!(codes.contains(&REASON_RES_TOPOLOGY_GAP));
    }

    #[test]
    fn security_flags_missing_version_missing_metrics_and_aurora_review() {
        let missing_metrics = item(
            Some("database-platform"),
            None,
            0,
            None,
            None,
            None,
            None,
            0,
            0.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let topology_gap = item(
            Some("database-platform"),
            Some("8.0.mysql_aurora.3.05.2"),
            4,
            None,
            Some(true),
            Some(false),
            Some(2),
            40_000,
            95.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_aurora_inventory(
            &[missing_metrics, topology_gap],
            Pillar::Security,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_NO_AURORA_METRICS));
        assert!(codes.contains(&REASON_SEC_AURORA_REVIEW));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let stale = item(
            Some("database-platform"),
            Some("8.0.mysql_aurora.3.05.2"),
            4,
            Some("orders-prod"),
            Some(true),
            Some(true),
            Some(2),
            100,
            10.0,
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(49),
        );

        let report = evaluate_mysql_aurora_inventory(&[stale], Pillar::Resilience, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_aurora_mysql_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_aurora_inventory(&[healthy_item()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
            assert!(report.score >= 99);
        }
    }
}
