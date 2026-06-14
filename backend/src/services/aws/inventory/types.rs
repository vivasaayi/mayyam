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

// Shared domain types for deterministic inventory pillar evaluators.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::aws_resource::Model as AwsResourceModel;

/// Well-Architected pillar covered by the inventory evaluators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pillar {
    Cost,
    Security,
    Resilience,
    Performance,
    Scalability,
    #[serde(rename = "disaster-recovery")]
    DisasterRecovery,
    #[serde(rename = "operational-excellence")]
    OperationalExcellence,
}

impl Pillar {
    pub fn as_str(&self) -> &'static str {
        match self {
            Pillar::Cost => "cost",
            Pillar::Security => "security",
            Pillar::Resilience => "resilience",
            Pillar::Performance => "performance",
            Pillar::Scalability => "scalability",
            Pillar::DisasterRecovery => "disaster-recovery",
            Pillar::OperationalExcellence => "operational-excellence",
        }
    }

    pub fn parse(s: &str) -> Option<Pillar> {
        match s.to_ascii_lowercase().as_str() {
            "cost" => Some(Pillar::Cost),
            "security" => Some(Pillar::Security),
            "resilience" => Some(Pillar::Resilience),
            "performance" => Some(Pillar::Performance),
            "scalability" => Some(Pillar::Scalability),
            "disaster-recovery" | "disaster_recovery" => Some(Pillar::DisasterRecovery),
            "operational-excellence" | "operational_excellence" => {
                Some(Pillar::OperationalExcellence)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    High,
    Medium,
    Low,
}

impl Severity {
    fn score_penalty(&self) -> i32 {
        match self {
            Severity::High => 15,
            Severity::Medium => 7,
            Severity::Low => 3,
        }
    }
}

/// Inventory rows older than this are treated as stale evidence.
pub const DEFAULT_STALE_AFTER_HOURS: i64 = 24;

/// Tag keys accepted as cost allocation ownership.
pub const COST_ALLOCATION_TAG_KEYS: &[&str] = &[
    "owner",
    "team",
    "cost-center",
    "costcenter",
    "cost_center",
    "project",
];
/// Tag keys accepted as a routable owner.
pub const OWNER_TAG_KEYS: &[&str] = &["owner", "team"];

/// A single deterministic finding with the evidence that produced it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryFinding {
    pub resource_id: String,
    pub arn: String,
    pub pillar: Pillar,
    pub reason_code: String,
    pub severity: Severity,
    pub message: String,
    pub evidence: Value,
}

/// Pillar evaluation result for one resource fleet (one account, one type).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PillarReport {
    pub pillar: Pillar,
    pub resources_evaluated: usize,
    pub stale_resources: usize,
    pub score: u8,
    pub findings: Vec<InventoryFinding>,
}

/// Deterministic score: start at 100, subtract a fixed penalty per finding
/// severity, clamp to 0.
pub fn score_pillar(findings: &[InventoryFinding]) -> u8 {
    let penalty: i32 = findings.iter().map(|f| f.severity.score_penalty()).sum();
    (100 - penalty).clamp(0, 100) as u8
}

/// Stale-data failure path shared by every evaluator. `reason_code` is the
/// service-specific stale code (e.g. `EC2_INV_STALE_DATA`).
pub fn check_stale(
    resource: &AwsResourceModel,
    pillar: Pillar,
    reason_code: &str,
    now: DateTime<Utc>,
) -> Option<InventoryFinding> {
    let age_hours = (now - resource.last_refreshed).num_hours();
    if age_hours <= DEFAULT_STALE_AFTER_HOURS {
        return None;
    }
    Some(InventoryFinding {
        resource_id: resource.resource_id.clone(),
        arn: resource.arn.clone(),
        pillar,
        reason_code: reason_code.to_string(),
        severity: Severity::Medium,
        message: format!(
            "Inventory data for {} is {} hours old (threshold {} hours); pillar evaluation may not reflect current state",
            resource.resource_id, age_hours, DEFAULT_STALE_AFTER_HOURS
        ),
        evidence: json!({
            "last_refreshed": resource.last_refreshed,
            "age_hours": age_hours,
            "stale_after_hours": DEFAULT_STALE_AFTER_HOURS,
        }),
    })
}

/// Read a string field from the normalized `resource_data` JSON object.
pub fn data_str(resource_data: &Value, key: &str) -> Option<String> {
    resource_data
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Look up a tag value. Supports both shapes that collectors persist:
/// an object map `{"Owner": "x"}` and an AWS-style array
/// `[{"Key": "Owner", "Value": "x"}]` (or lowercase `key`/`value`).
/// Tag key comparison is case-insensitive.
pub fn tag_value(tags: &Value, key: &str) -> Option<String> {
    let wanted = key.to_ascii_lowercase();
    match tags {
        Value::Object(map) => map
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == wanted)
            .and_then(|(_, v)| v.as_str().map(|s| s.to_string())),
        Value::Array(entries) => entries.iter().find_map(|entry| {
            let k = entry.get("Key").or_else(|| entry.get("key"))?.as_str()?;
            if k.to_ascii_lowercase() == wanted {
                entry
                    .get("Value")
                    .or_else(|| entry.get("value"))?
                    .as_str()
                    .map(|s| s.to_string())
            } else {
                None
            }
        }),
        _ => None,
    }
}

pub fn has_any_tag(tags: &Value, keys: &[&str]) -> bool {
    keys.iter().any(|k| tag_value(tags, k).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_value_supports_object_map_and_key_value_array_case_insensitively() {
        let map = json!({"Owner": "sre"});
        assert_eq!(tag_value(&map, "owner").as_deref(), Some("sre"));
        let array =
            json!([{"Key": "Cost-Center", "Value": "cc-1"}, {"key": "team", "value": "db"}]);
        assert_eq!(tag_value(&array, "cost-center").as_deref(), Some("cc-1"));
        assert_eq!(tag_value(&array, "TEAM").as_deref(), Some("db"));
        assert_eq!(tag_value(&array, "missing"), None);
        assert_eq!(tag_value(&json!(null), "owner"), None);
    }

    #[test]
    fn score_is_deterministic_and_clamped_at_zero() {
        let make = |severity: Severity| InventoryFinding {
            resource_id: "i-x".to_string(),
            arn: String::new(),
            pillar: Pillar::Cost,
            reason_code: "TEST".to_string(),
            severity,
            message: String::new(),
            evidence: json!({}),
        };
        assert_eq!(score_pillar(&[]), 100);
        assert_eq!(score_pillar(&[make(Severity::High)]), 85);
        assert_eq!(
            score_pillar(&[make(Severity::Medium), make(Severity::Low)]),
            90
        );
        let many: Vec<InventoryFinding> = (0..10).map(|_| make(Severity::High)).collect();
        assert_eq!(score_pillar(&many), 0);
    }
}
