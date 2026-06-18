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

// Deterministic TLS configuration inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-01324/01331/01352.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::MySqlTelemetrySnapshot;
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlTlsConfiguration";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_TLS_CONFIG_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_TLS_EVIDENCE_MISSING: &str = "MYSQL_TLS_CONFIG_COST_EVIDENCE_MISSING";
pub const REASON_COST_MIXED_TLS_USAGE: &str = "MYSQL_TLS_CONFIG_COST_MIXED_TLS_USAGE";
pub const REASON_RES_TLS_EVIDENCE_MISSING: &str = "MYSQL_TLS_CONFIG_RES_EVIDENCE_MISSING";
pub const REASON_RES_SECURE_TRANSPORT_DISABLED: &str =
    "MYSQL_TLS_CONFIG_RES_SECURE_TRANSPORT_DISABLED";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_TLS_CONFIG_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_TLS_NOT_ENFORCED: &str = "MYSQL_TLS_CONFIG_SEC_TLS_NOT_ENFORCED";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_TLS_CONFIG_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfigurationInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub have_ssl: Option<String>,
    pub require_secure_transport: Option<String>,
    pub ssl_accepts: i64,
    pub ssl_finished_accepts: i64,
    pub ssl_accept_pct: Option<f64>,
    pub account_count: usize,
    pub accounts_without_ssl_requirement: usize,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_tls_configuration_inventory(
    items: &[TlsConfigurationInventoryItem],
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

pub fn tls_configuration_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> TlsConfigurationInventoryItem {
    TlsConfigurationInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        have_ssl: snapshot.server.have_ssl.clone(),
        require_secure_transport: snapshot.server.require_secure_transport.clone(),
        ssl_accepts: snapshot.workload.ssl_accepts,
        ssl_finished_accepts: snapshot.workload.ssl_finished_accepts,
        ssl_accept_pct: snapshot.workload.ssl_accept_pct,
        account_count: snapshot.privileges.len(),
        accounts_without_ssl_requirement: snapshot
            .privileges
            .iter()
            .filter(|account| {
                account.ssl_type.as_deref().is_none_or(|ssl_type| {
                    ssl_type.is_empty() || ssl_type.eq_ignore_ascii_case("ANY")
                })
            })
            .count(),
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &TlsConfigurationInventoryItem,
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
                "TLS configuration inventory for {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if item.have_ssl.is_none() && item.require_secure_transport.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_TLS_EVIDENCE_MISSING,
            Severity::High,
            format!(
                "TLS configuration inventory for {} has no TLS variable evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect have_ssl and require_secure_transport variables before planning TLS rollout cost or client remediation",
            }),
        ));
    }

    if item.ssl_finished_accepts > 0
        && item
            .ssl_accept_pct
            .is_some_and(|ssl_accept_pct| ssl_accept_pct < 95.0)
    {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_MIXED_TLS_USAGE,
            Severity::Medium,
            format!(
                "TLS configuration inventory for {} shows mixed TLS session usage",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "ssl_accepts": item.ssl_accepts,
                "ssl_finished_accepts": item.ssl_finished_accepts,
                "ssl_accept_pct": item.ssl_accept_pct,
                "recommendation": "Estimate client remediation before enforcing TLS so rollout cost is visible",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &TlsConfigurationInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.have_ssl.is_none() && item.require_secure_transport.is_none() {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_TLS_EVIDENCE_MISSING,
            Severity::High,
            format!(
                "TLS configuration inventory for {} has no transport-security evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect TLS variables and SSL session counters before failover or client migration planning",
            }),
        ));
    }

    if !is_secure_transport_enforced(item) {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_SECURE_TRANSPORT_DISABLED,
            Severity::Medium,
            format!(
                "TLS secure transport is not enforced for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "have_ssl": item.have_ssl,
                "require_secure_transport": item.require_secure_transport,
                "accounts_without_ssl_requirement": item.accounts_without_ssl_requirement,
                "recommendation": "Validate client TLS readiness before restore, failover, or migration workflows depend on encrypted transport",
            }),
        ));
    }
}

fn evaluate_security(
    item: &TlsConfigurationInventoryItem,
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
                "TLS configuration inventory for {} has no owner for transport-security review routing",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if !is_secure_transport_enforced(item) || item.accounts_without_ssl_requirement > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_TLS_NOT_ENFORCED,
            Severity::High,
            format!(
                "TLS configuration is not fully enforced for {}",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "have_ssl": item.have_ssl,
                "require_secure_transport": item.require_secure_transport,
                "account_count": item.account_count,
                "accounts_without_ssl_requirement": item.accounts_without_ssl_requirement,
                "recommendation": "Enable require_secure_transport and require SSL/X509 for privileged and application accounts",
            }),
        ));
    }
}

fn stale_finding(
    item: &TlsConfigurationInventoryItem,
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
            "Inventory data for TLS configuration resource {} is {} hours old (threshold {} hours)",
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
    item: &TlsConfigurationInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://tls-configuration/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &TlsConfigurationInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
}

fn is_secure_transport_enforced(item: &TlsConfigurationInventoryItem) -> bool {
    item.have_ssl
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("YES") || value.eq_ignore_ascii_case("ON"))
        && item
            .require_secure_transport
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("ON") || value == "1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-10T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn item(
        owner: Option<&str>,
        labels: BTreeMap<String, String>,
        have_ssl: Option<&str>,
        require_secure_transport: Option<&str>,
        accounts_without_ssl_requirement: usize,
        collected_hours_ago: i64,
    ) -> TlsConfigurationInventoryItem {
        TlsConfigurationInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: Some("8.0.36".to_string()),
            have_ssl: have_ssl.map(str::to_string),
            require_secure_transport: require_secure_transport.map(str::to_string),
            ssl_accepts: 20,
            ssl_finished_accepts: 100,
            ssl_accept_pct: Some(20.0),
            account_count: 3,
            accounts_without_ssl_requirement,
            collected_at: now() - Duration::hours(collected_hours_ago),
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
    fn cost_flags_missing_owner_and_mixed_tls_usage() {
        let target = item(None, BTreeMap::new(), Some("YES"), Some("ON"), 0, 1);

        let report = evaluate_mysql_tls_configuration_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_MIXED_TLS_USAGE));
    }

    #[test]
    fn cost_flags_missing_tls_evidence() {
        let target = item(Some("db-team"), BTreeMap::new(), None, None, 0, 1);

        let report = evaluate_mysql_tls_configuration_inventory(&[target], Pillar::Cost, now());

        assert!(reason_codes(&report).contains(&REASON_COST_TLS_EVIDENCE_MISSING));
    }

    #[test]
    fn resilience_flags_secure_transport_disabled() {
        let target = item(
            Some("db-team"),
            BTreeMap::new(),
            Some("YES"),
            Some("OFF"),
            0,
            1,
        );

        let report =
            evaluate_mysql_tls_configuration_inventory(&[target], Pillar::Resilience, now());

        assert!(reason_codes(&report).contains(&REASON_RES_SECURE_TRANSPORT_DISABLED));
    }

    #[test]
    fn security_flags_unenforced_tls_and_missing_owner() {
        let target = item(None, BTreeMap::new(), Some("YES"), Some("OFF"), 2, 1);

        let report = evaluate_mysql_tls_configuration_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_TLS_NOT_ENFORCED));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(
            Some("db-team"),
            BTreeMap::new(),
            Some("YES"),
            Some("ON"),
            0,
            48,
        );

        let report = evaluate_mysql_tls_configuration_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        let stale = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_INV_STALE_DATA)
            .expect("stale finding");
        assert_eq!(stale.evidence["age_hours"], json!(48));
    }

    #[test]
    fn healthy_tls_configuration_passes_claimed_pillars() {
        let mut labels = BTreeMap::new();
        labels.insert("cost-center".to_string(), "cc-42".to_string());
        let mut target = item(None, labels, Some("YES"), Some("ON"), 0, 1);
        target.ssl_accepts = 100;
        target.ssl_finished_accepts = 100;
        target.ssl_accept_pct = Some(100.0);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_tls_configuration_inventory(
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
