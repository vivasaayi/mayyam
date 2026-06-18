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

// Deterministic cost-attribution inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-01520/01527/01548.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlCostAttribution";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_COST_ATTRIBUTION_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_CENTER_MISSING: &str = "MYSQL_COST_ATTRIBUTION_COST_CENTER_MISSING";
pub const REASON_COST_SPEND_EVIDENCE_MISSING: &str =
    "MYSQL_COST_ATTRIBUTION_SPEND_EVIDENCE_MISSING";
pub const REASON_RES_OWNER_NOT_RECORDED: &str = "MYSQL_COST_ATTRIBUTION_RES_OWNER_NOT_RECORDED";
pub const REASON_RES_RECOVERY_COST_UNASSIGNED: &str =
    "MYSQL_COST_ATTRIBUTION_RES_RECOVERY_COST_UNASSIGNED";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_COST_ATTRIBUTION_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_CHARGEBACK_NOT_AUDITABLE: &str =
    "MYSQL_COST_ATTRIBUTION_SEC_CHARGEBACK_NOT_AUDITABLE";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_COST_ATTRIBUTION_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostAttributionInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub table_count: usize,
    pub total_allocated_bytes: i64,
    pub write_operations: i64,
    pub monthly_cost_usd: Option<f64>,
    pub cost_center: Option<String>,
    pub environment: Option<String>,
    pub application: Option<String>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_cost_attribution_inventory(
    items: &[CostAttributionInventoryItem],
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

pub fn cost_attribution_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> CostAttributionInventoryItem {
    CostAttributionInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        cost_center: metadata_value(&labels, "cost-center")
            .or_else(|| metadata_value(&labels, "cost_center")),
        environment: metadata_value(&labels, "environment")
            .or_else(|| metadata_value(&labels, "env")),
        application: metadata_value(&labels, "application")
            .or_else(|| metadata_value(&labels, "app")),
        labels,
        server_version: snapshot.server.version.clone(),
        table_count: snapshot.tables.len(),
        total_allocated_bytes: snapshot
            .tables
            .iter()
            .map(|table| table.data_length + table.index_length)
            .sum(),
        write_operations: snapshot.workload.com_insert
            + snapshot.workload.com_update
            + snapshot.workload.com_delete,
        monthly_cost_usd: None,
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &CostAttributionInventoryItem,
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
                "Cost-attribution inventory for {} has no owner metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if item.cost_center.as_deref().is_none_or(str::is_empty) {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_CENTER_MISSING,
            Severity::High,
            format!(
                "Cost-attribution inventory for {} has no cost center",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "labels": item.labels,
                "recommendation": "Attach cost-center, application, and environment metadata before making savings or chargeback recommendations",
            }),
        ));
    }

    if item.monthly_cost_usd.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_SPEND_EVIDENCE_MISSING,
            Severity::Medium,
            format!(
                "Cost-attribution inventory for {} has no monthly spend evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "table_count": item.table_count,
                "total_allocated_bytes": item.total_allocated_bytes,
                "recommendation": "Join inventory to cloud or internal cost data so savings recommendations can be quantified",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &CostAttributionInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_OWNER_NOT_RECORDED,
            Severity::High,
            format!(
                "Cost-attribution inventory for {} has no accountable owner for incident spend",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Record owner metadata so failover, restore, and incident cost decisions can be routed quickly",
            }),
        ));
    }

    if item.cost_center.as_deref().is_none_or(str::is_empty)
        || item.environment.as_deref().is_none_or(str::is_empty)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_RECOVERY_COST_UNASSIGNED,
            Severity::Medium,
            format!(
                "Recovery-related cost attribution is incomplete for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "cost_center": item.cost_center,
                "environment": item.environment,
                "recommendation": "Assign cost center and environment before disaster-recovery or capacity actions create unowned spend",
            }),
        ));
    }
}

fn evaluate_security(
    item: &CostAttributionInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if !has_owner_metadata(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_OWNER_NOT_RECORDED,
            Severity::Medium,
            format!(
                "Cost-attribution inventory for {} has no owner for approval routing",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if item.application.as_deref().is_none_or(str::is_empty)
        || item.cost_center.as_deref().is_none_or(str::is_empty)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_CHARGEBACK_NOT_AUDITABLE,
            Severity::High,
            format!(
                "Cost-attribution inventory for {} is not auditable for chargeback or approval review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "application": item.application,
                "cost_center": item.cost_center,
                "recommendation": "Record application and cost-center evidence so spend exceptions and remediation approvals are auditable",
            }),
        ));
    }
}

fn stale_finding(
    item: &CostAttributionInventoryItem,
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
            "Inventory data for cost-attribution resource {} is {} hours old (threshold {} hours)",
            item.connection_name, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        json!({
            "connection_id": item.connection_id,
            "collected_at": item.collected_at,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    ))
}

fn finding(
    item: &CostAttributionInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://cost-attribution/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &CostAttributionInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
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
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-14T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn item(owner: Option<&str>, labels: &[(&str, &str)]) -> CostAttributionInventoryItem {
        let labels = labels
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<BTreeMap<_, _>>();
        CostAttributionInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            cost_center: metadata_value(&labels, "cost-center"),
            environment: metadata_value(&labels, "environment"),
            application: metadata_value(&labels, "application"),
            labels,
            server_version: Some("8.0.36".to_string()),
            table_count: 12,
            total_allocated_bytes: 1024,
            write_operations: 500,
            monthly_cost_usd: Some(120.0),
            collected_at: now() - Duration::hours(1),
        }
    }

    fn reason_codes(report: &PillarReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|finding| finding.reason_code.as_str())
            .collect()
    }

    #[test]
    fn cost_flags_missing_owner_cost_center_and_spend_evidence() {
        let mut target = item(None, &[]);
        target.monthly_cost_usd = None;

        let report = evaluate_mysql_cost_attribution_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_CENTER_MISSING));
        assert!(codes.contains(&REASON_COST_SPEND_EVIDENCE_MISSING));
    }

    #[test]
    fn resilience_flags_unassigned_recovery_cost() {
        let target = item(Some("db-team"), &[("application", "orders")]);

        let report =
            evaluate_mysql_cost_attribution_inventory(&[target], Pillar::Resilience, now());

        assert!(reason_codes(&report).contains(&REASON_RES_RECOVERY_COST_UNASSIGNED));
    }

    #[test]
    fn security_flags_missing_owner_and_unauditable_chargeback() {
        let target = item(None, &[("environment", "prod")]);

        let report = evaluate_mysql_cost_attribution_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_CHARGEBACK_NOT_AUDITABLE));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let mut target = item(
            Some("db-team"),
            &[
                ("cost-center", "cc-42"),
                ("environment", "prod"),
                ("application", "orders"),
            ],
        );
        target.collected_at = now() - Duration::hours(48);

        let report = evaluate_mysql_cost_attribution_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        assert!(reason_codes(&report).contains(&REASON_INV_STALE_DATA));
    }

    #[test]
    fn healthy_cost_attribution_passes_claimed_pillars() {
        let target = item(
            Some("db-team"),
            &[
                ("cost-center", "cc-42"),
                ("environment", "prod"),
                ("application", "orders"),
            ],
        );

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_cost_attribution_inventory(
                std::slice::from_ref(&target),
                pillar,
                now(),
            );
            assert!(
                report.findings.is_empty(),
                "unexpected findings for {:?}: {:?}",
                pillar,
                report.findings
            );
            assert_eq!(report.score, 100);
        }
    }
}
