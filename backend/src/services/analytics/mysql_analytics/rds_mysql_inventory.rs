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

// Deterministic RDS MySQL inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-00589/00596/00617.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "RdsMySql";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "RDS_MYSQL_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_RDS_METRICS: &str = "RDS_MYSQL_COST_NO_RDS_METRICS";
pub const REASON_COST_CAPACITY_SPEND_REVIEW: &str = "RDS_MYSQL_COST_CAPACITY_SPEND_REVIEW";
pub const REASON_RES_NO_RDS_METRICS: &str = "RDS_MYSQL_RES_NO_RDS_METRICS";
pub const REASON_RES_SINGLE_AZ: &str = "RDS_MYSQL_RES_SINGLE_AZ";
pub const REASON_RES_BACKUP_RETENTION_LOW: &str = "RDS_MYSQL_RES_BACKUP_RETENTION_LOW";
pub const REASON_SEC_VERSION_NOT_RECORDED: &str = "RDS_MYSQL_SEC_VERSION_NOT_RECORDED";
pub const REASON_SEC_NO_RDS_METRICS: &str = "RDS_MYSQL_SEC_NO_RDS_METRICS";
pub const REASON_SEC_PUBLICLY_ACCESSIBLE: &str = "RDS_MYSQL_SEC_PUBLICLY_ACCESSIBLE";
pub const REASON_INV_STALE_DATA: &str = "RDS_MYSQL_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdsMysqlInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub rds_metric_count: usize,
    pub db_instance_identifier: Option<String>,
    pub instance_class: Option<String>,
    pub storage_type: Option<String>,
    pub allocated_storage_gib: Option<i64>,
    pub multi_az_enabled: Option<bool>,
    pub backup_retention_days: Option<i64>,
    pub publicly_accessible: Option<bool>,
    pub deletion_protection_enabled: Option<bool>,
    pub write_operations: i64,
    pub qps_since_start: f64,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_rds_inventory(
    items: &[RdsMysqlInventoryItem],
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

pub fn rds_mysql_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> RdsMysqlInventoryItem {
    RdsMysqlInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        rds_metric_count: 0,
        db_instance_identifier: None,
        instance_class: None,
        storage_type: None,
        allocated_storage_gib: None,
        multi_az_enabled: None,
        backup_retention_days: None,
        publicly_accessible: None,
        deletion_protection_enabled: None,
        write_operations: snapshot.workload.com_insert
            + snapshot.workload.com_update
            + snapshot.workload.com_delete,
        qps_since_start: snapshot.workload.qps_since_start,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &RdsMysqlInventoryItem,
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
                "RDS MySQL inventory for connection {} has no owner, team, project, or cost-center metadata",
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

    if !has_rds_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_RDS_METRICS,
            Severity::High,
            format!(
                "RDS MySQL inventory for connection {} has no RDS instance evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "rds_metric_count": item.rds_metric_count,
                "recommendation": "Collect RDS DB instance identifier, instance class, storage type, allocated storage, and cost tags before making rightsizing or storage-spend recommendations",
            }),
        ));
    }

    if has_capacity_spend_pressure(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_CAPACITY_SPEND_REVIEW,
            Severity::Medium,
            format!(
                "RDS MySQL instance {} needs capacity spend review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "db_instance_identifier": item.db_instance_identifier,
                "instance_class": item.instance_class,
                "storage_type": item.storage_type,
                "allocated_storage_gib": item.allocated_storage_gib,
                "write_operations": item.write_operations,
                "qps_since_start": item.qps_since_start,
                "recommendation": "Review RDS instance class, storage tier, allocated storage, write load, and reserved capacity before increasing spend",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &RdsMysqlInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_rds_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_RDS_METRICS,
            Severity::High,
            format!(
                "RDS MySQL inventory for connection {} has no resilience evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "rds_metric_count": item.rds_metric_count,
                "recommendation": "Collect RDS Multi-AZ, backup retention, deletion protection, storage, and instance identity evidence before evaluating failover readiness",
            }),
        ));
    }

    if has_rds_metrics(item) && item.multi_az_enabled != Some(true) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SINGLE_AZ,
            Severity::High,
            format!(
                "RDS MySQL instance {} is not proven to be Multi-AZ",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "db_instance_identifier": item.db_instance_identifier,
                "multi_az_enabled": item.multi_az_enabled,
                "recommendation": "Confirm Multi-AZ or document an accepted single-AZ recovery posture before treating the instance as resilient",
            }),
        ));
    }

    if has_rds_metrics(item) && item.backup_retention_days.unwrap_or(0) < 7 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_BACKUP_RETENTION_LOW,
            Severity::Medium,
            format!(
                "RDS MySQL instance {} has low or unknown backup retention",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "db_instance_identifier": item.db_instance_identifier,
                "backup_retention_days": item.backup_retention_days,
                "minimum_expected_days": 7,
                "recommendation": "Set backup retention to at least seven days or document the recovery requirement and alternate backup path",
            }),
        ));
    }
}

fn evaluate_security(
    item: &RdsMysqlInventoryItem,
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
                "RDS MySQL inventory for connection {} has no recorded engine version",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "server_version": item.server_version,
                "recommendation": "Record RDS MySQL engine version with provider evidence so patching, TLS, and parameter guidance can be mapped deterministically",
            }),
        ));
    }

    if !has_rds_metrics(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_NO_RDS_METRICS,
            Severity::High,
            format!(
                "RDS MySQL inventory for connection {} has no scoped provider evidence for security review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "rds_metric_count": item.rds_metric_count,
                "recommendation": "Collect scoped RDS evidence for public accessibility, subnet/security group posture, deletion protection, parameter groups, and audit settings before security triage",
            }),
        ));
    }

    if has_rds_metrics(item) && item.publicly_accessible == Some(true) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_PUBLICLY_ACCESSIBLE,
            Severity::High,
            format!(
                "RDS MySQL instance {} is publicly accessible",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "connection_name": item.connection_name,
                "db_instance_identifier": item.db_instance_identifier,
                "publicly_accessible": item.publicly_accessible,
                "recommendation": "Review public accessibility, security groups, subnet placement, and approved exception evidence before exposing this instance outside private networks",
            }),
        ));
    }
}

fn stale_finding(
    item: &RdsMysqlInventoryItem,
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
            "Inventory data for RDS MySQL connection {} is {} hours old (threshold {} hours)",
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
    item: &RdsMysqlInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://rds-mysql/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &RdsMysqlInventoryItem) -> bool {
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

fn has_rds_metrics(item: &RdsMysqlInventoryItem) -> bool {
    item.rds_metric_count > 0
}

fn has_capacity_spend_pressure(item: &RdsMysqlInventoryItem) -> bool {
    has_rds_metrics(item)
        && (item.allocated_storage_gib.unwrap_or(0) >= 1_024
            || is_large_instance_class(item.instance_class.as_deref())
            || is_provisioned_iops_storage(item.storage_type.as_deref())
            || ((item.write_operations >= 25_000 || item.qps_since_start >= 80.0)
                && item.allocated_storage_gib.unwrap_or(0) >= 512))
}

fn is_large_instance_class(instance_class: Option<&str>) -> bool {
    let Some(instance_class) = instance_class else {
        return false;
    };
    [
        ".4xlarge",
        ".8xlarge",
        ".9xlarge",
        ".10xlarge",
        ".12xlarge",
        ".16xlarge",
        ".18xlarge",
        ".24xlarge",
        ".32xlarge",
        ".48xlarge",
    ]
    .iter()
    .any(|suffix| instance_class.ends_with(suffix))
}

fn is_provisioned_iops_storage(storage_type: Option<&str>) -> bool {
    storage_type
        .map(|storage_type| {
            matches!(
                storage_type.trim().to_ascii_lowercase().as_str(),
                "io1" | "io2"
            )
        })
        .unwrap_or(false)
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
        rds_metric_count: usize,
        db_instance_identifier: Option<&str>,
        instance_class: Option<&str>,
        storage_type: Option<&str>,
        allocated_storage_gib: Option<i64>,
        multi_az_enabled: Option<bool>,
        backup_retention_days: Option<i64>,
        publicly_accessible: Option<bool>,
        deletion_protection_enabled: Option<bool>,
        write_operations: i64,
        qps_since_start: f64,
        labels: BTreeMap<String, String>,
        collected_at: DateTime<Utc>,
    ) -> RdsMysqlInventoryItem {
        RdsMysqlInventoryItem {
            connection_id: "mysql-1".to_string(),
            connection_name: "orders-rds".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: server_version.map(str::to_string),
            rds_metric_count,
            db_instance_identifier: db_instance_identifier.map(str::to_string),
            instance_class: instance_class.map(str::to_string),
            storage_type: storage_type.map(str::to_string),
            allocated_storage_gib,
            multi_az_enabled,
            backup_retention_days,
            publicly_accessible,
            deletion_protection_enabled,
            write_operations,
            qps_since_start,
            collected_at,
        }
    }

    fn healthy_item() -> RdsMysqlInventoryItem {
        item(
            Some("database-platform"),
            Some("8.0.36"),
            5,
            Some("orders-prod"),
            Some("db.r6g.large"),
            Some("gp3"),
            Some(256),
            Some(true),
            Some(14),
            Some(false),
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
            None,
            None,
            None,
            None,
            None,
            0,
            0.0,
            BTreeMap::new(),
            now(),
        );
        let large_instance = item(
            Some("database-platform"),
            Some("8.0.36"),
            5,
            Some("orders-prod"),
            Some("db.r6g.8xlarge"),
            Some("io1"),
            Some(2_048),
            Some(true),
            Some(14),
            Some(false),
            Some(true),
            100_000,
            180.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report =
            evaluate_mysql_rds_inventory(&[missing_metrics, large_instance], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_NO_RDS_METRICS));
        assert!(codes.contains(&REASON_COST_CAPACITY_SPEND_REVIEW));
    }

    #[test]
    fn resilience_flags_missing_metrics_single_az_and_low_backup_retention() {
        let missing_metrics = item(
            Some("database-platform"),
            Some("8.0.36"),
            0,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            0,
            0.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let weak_resilience = item(
            Some("database-platform"),
            Some("8.0.36"),
            5,
            Some("orders-prod"),
            Some("db.r6g.large"),
            Some("gp3"),
            Some(256),
            Some(false),
            Some(1),
            Some(false),
            Some(false),
            25_000,
            75.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_rds_inventory(
            &[missing_metrics, weak_resilience],
            Pillar::Resilience,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_RES_NO_RDS_METRICS));
        assert!(codes.contains(&REASON_RES_SINGLE_AZ));
        assert!(codes.contains(&REASON_RES_BACKUP_RETENTION_LOW));
    }

    #[test]
    fn security_flags_missing_version_missing_metrics_and_public_exposure() {
        let missing_metrics = item(
            Some("database-platform"),
            None,
            0,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            0,
            0.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );
        let public_instance = item(
            Some("database-platform"),
            Some("8.0.36"),
            5,
            Some("orders-prod"),
            Some("db.r6g.large"),
            Some("gp3"),
            Some(256),
            Some(true),
            Some(14),
            Some(true),
            Some(true),
            5_000,
            20.0,
            labels(&[("owner", "database-platform")]),
            now(),
        );

        let report = evaluate_mysql_rds_inventory(
            &[missing_metrics, public_instance],
            Pillar::Security,
            now(),
        );
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_VERSION_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_NO_RDS_METRICS));
        assert!(codes.contains(&REASON_SEC_PUBLICLY_ACCESSIBLE));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let stale = item(
            Some("database-platform"),
            Some("8.0.36"),
            5,
            Some("orders-prod"),
            Some("db.r6g.large"),
            Some("gp3"),
            Some(256),
            Some(true),
            Some(14),
            Some(false),
            Some(true),
            100,
            10.0,
            labels(&[("owner", "database-platform")]),
            now() - Duration::hours(49),
        );

        let report = evaluate_mysql_rds_inventory(&[stale], Pillar::Resilience, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_rds_mysql_passes_claimed_pillars() {
        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_rds_inventory(&[healthy_item()], pillar, now());

            assert_eq!(report.resources_evaluated, 1);
            assert_eq!(report.stale_resources, 0);
            assert!(report.findings.is_empty());
            assert!(report.score >= 99);
        }
    }
}
