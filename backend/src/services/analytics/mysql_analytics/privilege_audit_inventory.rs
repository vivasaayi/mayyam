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

// Deterministic privilege-audit inventory evaluator for roadmap rows
// 03-MYSQL-AI-TRIAGER-01275/01282/01303.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::analytics::mysql_analytics::mysql_telemetry::{
    MySqlPrivilegeTelemetry, MySqlTelemetrySnapshot,
};
use crate::services::aws::inventory::types::{
    score_pillar, InventoryFinding, Pillar, PillarReport, Severity, COST_ALLOCATION_TAG_KEYS,
    DEFAULT_STALE_AFTER_HOURS,
};

pub const RESOURCE_TYPE: &str = "MySqlPrivilegeAudit";
pub const REASON_COST_OWNER_NOT_RECORDED: &str = "MYSQL_PRIVILEGE_AUDIT_COST_OWNER_NOT_RECORDED";
pub const REASON_COST_NO_PRIVILEGE_EVIDENCE: &str =
    "MYSQL_PRIVILEGE_AUDIT_COST_NO_PRIVILEGE_EVIDENCE";
pub const REASON_COST_ADMIN_ACCOUNT_REVIEW: &str =
    "MYSQL_PRIVILEGE_AUDIT_COST_ADMIN_ACCOUNT_REVIEW";
pub const REASON_RES_NO_PRIVILEGE_EVIDENCE: &str =
    "MYSQL_PRIVILEGE_AUDIT_RES_NO_PRIVILEGE_EVIDENCE";
pub const REASON_RES_UNLOCKED_ADMIN_ACCOUNTS: &str =
    "MYSQL_PRIVILEGE_AUDIT_RES_UNLOCKED_ADMIN_ACCOUNTS";
pub const REASON_SEC_OWNER_NOT_RECORDED: &str = "MYSQL_PRIVILEGE_AUDIT_SEC_OWNER_NOT_RECORDED";
pub const REASON_SEC_BROAD_OR_UNENCRYPTED_PRIVILEGES: &str =
    "MYSQL_PRIVILEGE_AUDIT_SEC_BROAD_OR_UNENCRYPTED_PRIVILEGES";
pub const REASON_INV_STALE_DATA: &str = "MYSQL_PRIVILEGE_AUDIT_INV_STALE_DATA";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivilegeAccountSample {
    pub user: String,
    pub host: String,
    pub account_locked: Option<bool>,
    pub password_expired: Option<bool>,
    pub ssl_type: Option<String>,
    pub super_priv: bool,
    pub grant_priv: bool,
    pub create_user_priv: bool,
    pub file_priv: bool,
    pub process_priv: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivilegeAuditInventoryItem {
    pub connection_id: String,
    pub connection_name: String,
    pub owner: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub server_version: Option<String>,
    pub account_count: usize,
    pub administrative_account_count: usize,
    pub grantable_account_count: usize,
    pub wildcard_host_account_count: usize,
    pub unlocked_admin_account_count: usize,
    pub privileged_without_tls_count: usize,
    pub sampled_administrative_accounts: Vec<PrivilegeAccountSample>,
    pub sampled_broad_accounts: Vec<PrivilegeAccountSample>,
    pub collected_at: DateTime<Utc>,
}

pub fn evaluate_mysql_privilege_audit_inventory(
    items: &[PrivilegeAuditInventoryItem],
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

pub fn privilege_audit_item_from_telemetry(
    connection_id: &str,
    connection_name: &str,
    owner: Option<String>,
    labels: BTreeMap<String, String>,
    snapshot: &MySqlTelemetrySnapshot,
) -> PrivilegeAuditInventoryItem {
    let mut administrative_accounts = snapshot
        .privileges
        .iter()
        .filter(|account| is_administrative_account(account))
        .map(sample_from_account)
        .collect::<Vec<_>>();
    administrative_accounts.sort_by(|left, right| {
        left.user
            .cmp(&right.user)
            .then_with(|| left.host.cmp(&right.host))
    });

    let mut broad_accounts = snapshot
        .privileges
        .iter()
        .filter(|account| account.host == "%" || privileged_without_tls(account))
        .map(sample_from_account)
        .collect::<Vec<_>>();
    broad_accounts.sort_by(|left, right| {
        left.user
            .cmp(&right.user)
            .then_with(|| left.host.cmp(&right.host))
    });

    PrivilegeAuditInventoryItem {
        connection_id: connection_id.to_string(),
        connection_name: connection_name.to_string(),
        owner,
        labels,
        server_version: snapshot.server.version.clone(),
        account_count: snapshot.privileges.len(),
        administrative_account_count: administrative_accounts.len(),
        grantable_account_count: snapshot
            .privileges
            .iter()
            .filter(|account| account.grant_priv)
            .count(),
        wildcard_host_account_count: snapshot
            .privileges
            .iter()
            .filter(|account| account.host == "%")
            .count(),
        unlocked_admin_account_count: snapshot
            .privileges
            .iter()
            .filter(|account| {
                is_administrative_account(account) && account.account_locked != Some(true)
            })
            .count(),
        privileged_without_tls_count: snapshot
            .privileges
            .iter()
            .filter(|account| privileged_without_tls(account))
            .count(),
        sampled_administrative_accounts: administrative_accounts.into_iter().take(10).collect(),
        sampled_broad_accounts: broad_accounts.into_iter().take(10).collect(),
        collected_at: snapshot.collected_at,
    }
}

fn evaluate_cost(
    item: &PrivilegeAuditInventoryItem,
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
                "Privilege-audit inventory for {} has no owner, team, project, or cost-center metadata",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if item.account_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_NO_PRIVILEGE_EVIDENCE,
            Severity::High,
            format!(
                "Privilege-audit inventory for {} has no account privilege evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect mysql.user privilege evidence before estimating administrative-account review cost or access cleanup effort",
            }),
        ));
    }

    if item.administrative_account_count > 0 || item.grantable_account_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_COST_ADMIN_ACCOUNT_REVIEW,
            Severity::Medium,
            format!(
                "Privilege-audit inventory for {} has administrative accounts to review",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "administrative_account_count": item.administrative_account_count,
                "grantable_account_count": item.grantable_account_count,
                "sampled_administrative_accounts": item.sampled_administrative_accounts,
                "recommendation": "Review privileged account ownership and remove unused administrative grants before adding replicas, migrations, or managed-service controls",
            }),
        ));
    }
}

fn evaluate_resilience(
    item: &PrivilegeAuditInventoryItem,
    pillar: Pillar,
    findings: &mut Vec<InventoryFinding>,
) {
    if item.account_count == 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_NO_PRIVILEGE_EVIDENCE,
            Severity::High,
            format!(
                "Privilege-audit inventory for {} has no break-glass or service-account evidence",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "recommendation": "Collect privilege evidence so incident, restore, and failover procedures can identify accountable administrative access",
            }),
        ));
    }

    if item.unlocked_admin_account_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_RES_UNLOCKED_ADMIN_ACCOUNTS,
            Severity::Medium,
            format!(
                "Privilege-audit inventory for {} has unlocked administrative accounts",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "unlocked_admin_account_count": item.unlocked_admin_account_count,
                "sampled_administrative_accounts": item.sampled_administrative_accounts,
                "recommendation": "Map active administrative accounts to runbooks, rotation ownership, and break-glass approval before incidents",
            }),
        ));
    }
}

fn evaluate_security(
    item: &PrivilegeAuditInventoryItem,
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
                "Privilege-audit inventory for {} has no owner for access review routing",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "checked_keys": COST_ALLOCATION_TAG_KEYS,
            }),
        ));
    }

    if item.wildcard_host_account_count > 0 || item.privileged_without_tls_count > 0 {
        findings.push(finding(
            item,
            pillar,
            REASON_SEC_BROAD_OR_UNENCRYPTED_PRIVILEGES,
            Severity::High,
            format!(
                "Privilege-audit inventory for {} has broad or unencrypted privileged access",
                item.connection_name
            ),
            json!({
                "connection_id": item.connection_id,
                "wildcard_host_account_count": item.wildcard_host_account_count,
                "privileged_without_tls_count": item.privileged_without_tls_count,
                "sampled_broad_accounts": item.sampled_broad_accounts,
                "recommendation": "Restrict wildcard hosts, require TLS for privileged accounts, and route access changes through accountable owners",
            }),
        ));
    }
}

fn stale_finding(
    item: &PrivilegeAuditInventoryItem,
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
            "Inventory data for privilege-audit resource {} is {} hours old (threshold {} hours)",
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
    item: &PrivilegeAuditInventoryItem,
    pillar: Pillar,
    reason_code: &str,
    severity: Severity,
    message: String,
    evidence: Value,
) -> InventoryFinding {
    InventoryFinding {
        resource_id: item.connection_id.clone(),
        arn: format!("mysql://privilege-audit/{}", item.connection_id),
        pillar,
        reason_code: reason_code.to_string(),
        severity,
        message,
        evidence,
    }
}

fn has_owner_metadata(item: &PrivilegeAuditInventoryItem) -> bool {
    item.owner.as_deref().is_some_and(|owner| !owner.is_empty())
        || COST_ALLOCATION_TAG_KEYS
            .iter()
            .any(|key| item.labels.get(*key).is_some_and(|value| !value.is_empty()))
}

fn is_administrative_account(account: &MySqlPrivilegeTelemetry) -> bool {
    account.super_priv
        || account.grant_priv
        || account.create_user_priv
        || account.file_priv
        || account.shutdown_priv
        || account.reload_priv
}

fn privileged_without_tls(account: &MySqlPrivilegeTelemetry) -> bool {
    is_administrative_account(account)
        && account
            .ssl_type
            .as_deref()
            .is_none_or(|ssl_type| ssl_type.is_empty() || ssl_type.eq_ignore_ascii_case("ANY"))
}

fn sample_from_account(account: &MySqlPrivilegeTelemetry) -> PrivilegeAccountSample {
    PrivilegeAccountSample {
        user: account.user.clone(),
        host: account.host.clone(),
        account_locked: account.account_locked,
        password_expired: account.password_expired,
        ssl_type: account.ssl_type.clone(),
        super_priv: account.super_priv,
        grant_priv: account.grant_priv,
        create_user_priv: account.create_user_priv,
        file_priv: account.file_priv,
        process_priv: account.process_priv,
    }
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
        account_count: usize,
        admin_count: usize,
        wildcard_count: usize,
        tls_gap_count: usize,
        collected_hours_ago: i64,
    ) -> PrivilegeAuditInventoryItem {
        PrivilegeAuditInventoryItem {
            connection_id: "conn-1".to_string(),
            connection_name: "orders-db".to_string(),
            owner: owner.map(str::to_string),
            labels,
            server_version: Some("8.0.36".to_string()),
            account_count,
            administrative_account_count: admin_count,
            grantable_account_count: admin_count,
            wildcard_host_account_count: wildcard_count,
            unlocked_admin_account_count: admin_count,
            privileged_without_tls_count: tls_gap_count,
            sampled_administrative_accounts: vec![sample("admin", "%", true, Some(""))],
            sampled_broad_accounts: vec![sample("admin", "%", true, Some(""))],
            collected_at: now() - Duration::hours(collected_hours_ago),
        }
    }

    fn sample(
        user: &str,
        host: &str,
        super_priv: bool,
        ssl_type: Option<&str>,
    ) -> PrivilegeAccountSample {
        PrivilegeAccountSample {
            user: user.to_string(),
            host: host.to_string(),
            account_locked: Some(false),
            password_expired: Some(false),
            ssl_type: ssl_type.map(str::to_string),
            super_priv,
            grant_priv: false,
            create_user_priv: false,
            file_priv: false,
            process_priv: false,
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
    fn cost_flags_missing_owner_and_admin_review() {
        let target = item(None, BTreeMap::new(), 3, 1, 0, 0, 1);

        let report = evaluate_mysql_privilege_audit_inventory(&[target], Pillar::Cost, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_COST_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_COST_ADMIN_ACCOUNT_REVIEW));
    }

    #[test]
    fn cost_flags_missing_privilege_evidence() {
        let target = item(Some("db-team"), BTreeMap::new(), 0, 0, 0, 0, 1);

        let report = evaluate_mysql_privilege_audit_inventory(&[target], Pillar::Cost, now());

        assert!(reason_codes(&report).contains(&REASON_COST_NO_PRIVILEGE_EVIDENCE));
    }

    #[test]
    fn resilience_flags_unlocked_admin_accounts() {
        let target = item(Some("db-team"), BTreeMap::new(), 3, 1, 0, 0, 1);

        let report = evaluate_mysql_privilege_audit_inventory(&[target], Pillar::Resilience, now());

        assert!(reason_codes(&report).contains(&REASON_RES_UNLOCKED_ADMIN_ACCOUNTS));
    }

    #[test]
    fn security_flags_broad_or_unencrypted_privileges() {
        let target = item(None, BTreeMap::new(), 3, 1, 1, 1, 1);

        let report = evaluate_mysql_privilege_audit_inventory(&[target], Pillar::Security, now());
        let codes = reason_codes(&report);

        assert!(codes.contains(&REASON_SEC_OWNER_NOT_RECORDED));
        assert!(codes.contains(&REASON_SEC_BROAD_OR_UNENCRYPTED_PRIVILEGES));
    }

    #[test]
    fn stale_inventory_is_counted_for_any_pillar() {
        let target = item(Some("db-team"), BTreeMap::new(), 3, 0, 0, 0, 48);

        let report = evaluate_mysql_privilege_audit_inventory(&[target], Pillar::Cost, now());

        assert_eq!(report.stale_resources, 1);
        let stale = report
            .findings
            .iter()
            .find(|finding| finding.reason_code == REASON_INV_STALE_DATA)
            .expect("stale finding");
        assert_eq!(stale.evidence["age_hours"], json!(48));
    }

    #[test]
    fn healthy_privilege_audit_passes_claimed_pillars() {
        let mut labels = BTreeMap::new();
        labels.insert("cost-center".to_string(), "cc-42".to_string());
        let target = item(None, labels, 3, 0, 0, 0, 1);

        for pillar in [Pillar::Cost, Pillar::Resilience, Pillar::Security] {
            let report = evaluate_mysql_privilege_audit_inventory(
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
