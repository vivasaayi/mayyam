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

// Deterministic MySQL Group Replication inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00491/00498/00519.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlGroupReplication";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_GROUP_REPLICATION_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_GROUP_REPLICATION_METRICS: &str =
    "MYSQL_GROUP_REPLICATION_COST_NO_METRICS";
pub const REASON_COST_GROUP_CAPACITY_SPEND_REVIEW: &str =
    "MYSQL_GROUP_REPLICATION_COST_CAPACITY_SPEND_REVIEW";
pub const REASON_RES_NO_GROUP_REPLICATION_METRICS: &str = "MYSQL_GROUP_REPLICATION_RES_NO_METRICS";
pub const REASON_RES_GROUP_QUORUM_RISK: &str = "MYSQL_GROUP_REPLICATION_RES_QUORUM_RISK";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str =
    "MYSQL_GROUP_REPLICATION_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_NO_GROUP_REPLICATION_METRICS: &str = "MYSQL_GROUP_REPLICATION_SEC_NO_METRICS";
pub const REASON_SEC_GROUP_REPLICATION_REVIEW: &str =
    "MYSQL_GROUP_REPLICATION_SEC_GROUP_REPLICATION_REVIEW";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_GROUP_REPLICATION_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupReplicationInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub group_replication_metric_count: usize,
    pub group_members_total: Option<i64>,
    pub online_members: Option<i64>,
    pub primary_member_present: Option<bool>,
    pub write_operations: i64,
    pub qps_since_start: f64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_group_replication_inventory(
    items: &[GroupReplicationInventoryItem],
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

pub fn group_replication_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> GroupReplicationInventoryItem {
    GroupReplicationInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        group_replication_metric_count: 0,
        group_members_total: None,
        online_members: None,
        primary_member_present: None,
        write_operations: snapshot.workload.com_insert
            + snapshot.workload.com_update
            + snapshot.workload.com_delete,
        qps_since_start: snapshot.workload.qps_since_start,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &GroupReplicationInventoryItem,
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
                "MySQL Group Replication inventory for connection {} has no owner, team, project, or cost-center metadata",
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

    if !has_group_replication_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_GROUP_REPLICATION_METRICS,
            Severity::High,
            format!(
                "MySQL Group Replication inventory for connection {} has no group replication evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "group_replication_metric_count": item.group_replication_metric_count,
                "recommendation": "Collect performance_schema replication_group_members and group status evidence before making member count, topology, or capacity-spend recommendations",
            }),
        ));
    }

    if has_group_capacity_spend_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_GROUP_CAPACITY_SPEND_REVIEW,
            Severity::Medium,
            format!(
                "MySQL Group Replication membership for connection {} needs spend review before scaling",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "group_members_total": item.group_members_total,
                "online_members": item.online_members,
                "primary_member_present": item.primary_member_present,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Review failed members, write routing, quorum margin, and replica sizing before adding nodes or increasing primary capacity spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &GroupReplicationInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_group_replication_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_GROUP_REPLICATION_METRICS,
            Severity::High,
            format!(
                "MySQL Group Replication inventory for connection {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "group_replication_metric_count": item.group_replication_metric_count,
                "recommendation": "Collect group member count, online member state, and primary-member evidence so quorum and failover readiness can be evaluated deterministically",
            }),
        ));
    }

    if has_group_quorum_risk(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_GROUP_QUORUM_RISK,
            Severity::High,
            format!(
                "MySQL Group Replication quorum or primary-member risk is present for connection {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "group_members_total": item.group_members_total,
                "online_members": item.online_members,
                "primary_member_present": item.primary_member_present,
                "recommendation": "Validate online member count, primary election state, split-brain protection, and failover eligibility before relying on this group for availability",
            }),
        ));
    }
}

fn evaluate_security(
    item: &GroupReplicationInventoryItem,
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
                "MySQL Group Replication inventory for connection {} has no recorded server version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record MySQL server version with group replication evidence so version-specific group communication and privilege guidance can be mapped deterministically",
            }),
        ));
    }

    if !has_group_replication_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_GROUP_REPLICATION_METRICS,
            Severity::High,
            format!(
                "MySQL Group Replication inventory for connection {} has no scoped evidence for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "group_replication_metric_count": item.group_replication_metric_count,
                "recommendation": "Collect scoped group replication evidence so group membership, primary state, communication settings, and incident blast radius can be reviewed without ad hoc privileged diagnostics",
            }),
        ));
    }

    if has_group_replication_security_review(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_GROUP_REPLICATION_REVIEW,
            Severity::Medium,
            format!(
                "MySQL Group Replication evidence for connection {} should be reviewed",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "group_members_total": item.group_members_total,
                "online_members": item.online_members,
                "primary_member_present": item.primary_member_present,
                "recommendation": "Review group communication allowlists, replication users, TLS posture, and external member exposure before sharing group replication evidence outside the database team",
            }),
        ));
    }
}

fn stale_finding(
    item: &GroupReplicationInventoryItem,
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
            "Inventory data for MySQL Group Replication connection {} is {} hours old (threshold {} hours)",
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
    item: &GroupReplicationInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://group-replication/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &GroupReplicationInventoryItem) -> bool {
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

fn has_group_replication_metrics(item: &GroupReplicationInventoryItem) -> bool {
    item.group_replication_metric_count > 0
}

fn required_quorum(total_members: i64) -> i64 {
    (total_members / 2) + 1
}

fn group_is_degraded(item: &GroupReplicationInventoryItem) -> bool {
    match (item.group_members_total, item.online_members) {
        (Some(total), Some(online)) if total > 0 => online < total || total < 3,
        (Some(total), None) if total > 0 => total < 3,
        _ => false,
    }
}

fn has_group_capacity_spend_pressure(item: &GroupReplicationInventoryItem) -> bool {
    has_group_replication_metrics(item)
        && group_is_degraded(item)
        && (item.write_operations >= 25_000 || item.qps_since_start >= 80.0)
}

fn has_group_quorum_risk(item: &GroupReplicationInventoryItem) -> bool {
    if !has_group_replication_metrics(item) {
        return false;
    }

    if item.primary_member_present == Some(false) {
        return true;
    }

    match (item.group_members_total, item.online_members) {
        (Some(total), Some(online)) if total > 0 => online < required_quorum(total),
        _ => false,
    }
}

fn has_group_replication_security_review(item: &GroupReplicationInventoryItem) -> bool {
    has_group_replication_metrics(item)
        && (group_is_degraded(item) || item.primary_member_present == Some(false))
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
        group_replication_metric_count: usize,
        group_members_total: Option<i64>,
        online_members: Option<i64>,
        primary_member_present: Option<bool>,
        write_operations: i64,
        qps_since_start: f64,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> GroupReplicationInventoryItem {
        GroupReplicationInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-mysql".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: server_version.map(str::to_string),
            group_replication_metric_count,
            group_members_total,
            online_members,
            primary_member_present,
            write_operations,
            qps_since_start,
            collected_at,
        }
    }

    fn healthy_item() -> GroupReplicationInventoryItem {
        item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            Some(3),
            Some(3),
            Some(true),
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
    fn cost_flags_missing_owner_missing_metrics_and_capacity_spend_review() {
        let missing_metrics = item(
            Some(""),
            Some("8.0.36"),
            0,
            None,
            None,
            None,
            0,
            0.0,
            BTreeMap::new(),
            now(),
        );
        let degraded_group = item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            Some(3),
            Some(2),
            Some(true),
            80_000,
            150.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_group_replication_inventory(
            &[missing_metrics, degraded_group],
            Pillar::Cost,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_NO_GROUP_REPLICATION_METRICS));
        assert!(codes.contains(&REASON_COST_GROUP_CAPACITY_SPEND_REVIEW));
    }

    #[test]
    fn resilience_flags_missing_metrics_and_quorum_risk() {
        let missing_metrics = item(
            Some("database-platform"),
            Some("8.0.36"),
            0,
            None,
            None,
            None,
            0,
            0.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let quorum_risk = item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            Some(3),
            Some(1),
            Some(false),
            50_000,
            100.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_group_replication_inventory(
            &[missing_metrics, quorum_risk],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NO_GROUP_REPLICATION_METRICS));
        assert!(codes.contains(&REASON_RES_GROUP_QUORUM_RISK));
    }

    #[test]
    fn security_flags_missing_version_missing_metrics_and_group_review() {
        let missing_metrics = item(
            Some("database-platform"),
            None,
            0,
            None,
            None,
            None,
            0,
            0.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let degraded_group = item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            Some(3),
            Some(2),
            Some(false),
            40_000,
            95.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_group_replication_inventory(
            &[missing_metrics, degraded_group],
            Pillar::Security,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_NO_GROUP_REPLICATION_METRICS));
        assert!(codes.contains(&REASON_SEC_GROUP_REPLICATION_REVIEW));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let stale = item(
            Some("database-platform"),
            Some("8.0.36"),
            3,
            Some(3),
            Some(3),
            Some(true),
            100,
            10.0,
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(49),
        );

        let report =
            evaluate_mysql_group_replication_inventory(&[stale], Pillar::Resilience, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_group_replication_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report =
                evaluate_mysql_group_replication_inventory(&[healthy_item()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
            assert!(report.score >= 99);
        }
    }
}
