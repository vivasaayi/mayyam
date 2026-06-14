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

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MySqlSignalSnapshot {
    pub id: Uuid,
    pub collected_at: DateTime<Utc>,
    pub qps_since_start: f64,
    pub threads_connected: i32,
    pub threads_running: i32,
    pub connection_usage_pct: Option<f64>,
    pub buffer_pool_hit_ratio: Option<f64>,
    pub slow_queries: i64,
    pub findings_count: i32,
    pub high_priority_findings_count: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MySqlSignalSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MySqlSignalCategory {
    Connections,
    QueryPerformance,
    Locking,
    Memory,
    Findings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MySqlPerformanceSignal {
    pub severity: MySqlSignalSeverity,
    pub category: MySqlSignalCategory,
    pub title: String,
    pub summary: String,
    pub evidence: Vec<String>,
    pub recommendation: String,
}

#[derive(Debug, Clone)]
pub struct MySqlSignalRules {
    pub connection_pressure_pct: f64,
    pub connection_pressure_min_samples: usize,
    pub buffer_pool_drop_points: f64,
    pub buffer_pool_low_ratio: f64,
}

impl Default for MySqlSignalRules {
    fn default() -> Self {
        Self {
            connection_pressure_pct: 85.0,
            connection_pressure_min_samples: 3,
            buffer_pool_drop_points: 5.0,
            buffer_pool_low_ratio: 0.95,
        }
    }
}

pub struct MySqlSignalEvaluator;

impl MySqlSignalEvaluator {
    pub fn evaluate(
        snapshots: &[MySqlSignalSnapshot],
        rules: &MySqlSignalRules,
    ) -> Vec<MySqlPerformanceSignal> {
        if snapshots.len() < 2 {
            return Vec::new();
        }

        let mut ordered = snapshots.to_vec();
        ordered.sort_by(|a, b| b.collected_at.cmp(&a.collected_at));

        let mut signals = Vec::new();
        Self::detect_sustained_connection_pressure(&ordered, rules, &mut signals);
        Self::detect_new_high_priority_findings(&ordered, &mut signals);
        Self::detect_buffer_pool_regression(&ordered, rules, &mut signals);

        signals
    }

    fn detect_sustained_connection_pressure(
        snapshots: &[MySqlSignalSnapshot],
        rules: &MySqlSignalRules,
        signals: &mut Vec<MySqlPerformanceSignal>,
    ) {
        let window = snapshots.iter().take(5).collect::<Vec<_>>();
        let high_samples = window
            .iter()
            .filter(|snapshot| {
                snapshot
                    .connection_usage_pct
                    .map(|usage| usage >= rules.connection_pressure_pct)
                    .unwrap_or(false)
            })
            .count();

        if high_samples < rules.connection_pressure_min_samples {
            return;
        }

        let latest_usage = window
            .first()
            .and_then(|snapshot| snapshot.connection_usage_pct)
            .unwrap_or_default();
        let evidence = window
            .iter()
            .filter_map(|snapshot| {
                snapshot.connection_usage_pct.map(|usage| {
                    format!(
                        "{}: {:.1}% connection usage",
                        snapshot.collected_at.to_rfc3339(),
                        usage
                    )
                })
            })
            .collect::<Vec<_>>();

        signals.push(MySqlPerformanceSignal {
            severity: if latest_usage >= 95.0 {
                MySqlSignalSeverity::Critical
            } else {
                MySqlSignalSeverity::High
            },
            category: MySqlSignalCategory::Connections,
            title: "Connection pressure is sustained".to_string(),
            summary: format!(
                "{} of the last {} samples were at or above {:.1}% connection usage.",
                high_samples,
                window.len(),
                rules.connection_pressure_pct
            ),
            evidence,
            recommendation:
                "Inspect application connection pools, connection leaks, and long-running sessions before increasing max_connections."
                    .to_string(),
        });
    }

    fn detect_new_high_priority_findings(
        snapshots: &[MySqlSignalSnapshot],
        signals: &mut Vec<MySqlPerformanceSignal>,
    ) {
        let Some(latest) = snapshots.first() else {
            return;
        };

        if latest.high_priority_findings_count <= 0 {
            return;
        }

        let previous_high_watermark = snapshots
            .iter()
            .skip(1)
            .map(|snapshot| snapshot.high_priority_findings_count)
            .max()
            .unwrap_or(0);

        if previous_high_watermark > 0 {
            return;
        }

        signals.push(MySqlPerformanceSignal {
            severity: MySqlSignalSeverity::High,
            category: MySqlSignalCategory::Findings,
            title: "New high-priority finding appeared".to_string(),
            summary: format!(
                "The latest snapshot has {} high-priority finding(s), while earlier samples in the window had none.",
                latest.high_priority_findings_count
            ),
            evidence: vec![format!(
                "{}: {} high-priority finding(s)",
                latest.collected_at.to_rfc3339(),
                latest.high_priority_findings_count
            )],
            recommendation:
                "Open the current telemetry findings and validate the top evidence before making database changes."
                    .to_string(),
        });
    }

    fn detect_buffer_pool_regression(
        snapshots: &[MySqlSignalSnapshot],
        rules: &MySqlSignalRules,
        signals: &mut Vec<MySqlPerformanceSignal>,
    ) {
        let latest = snapshots.iter().find_map(|snapshot| {
            snapshot
                .buffer_pool_hit_ratio
                .map(|ratio| (snapshot, ratio))
        });
        let oldest = snapshots.iter().rev().find_map(|snapshot| {
            snapshot
                .buffer_pool_hit_ratio
                .map(|ratio| (snapshot, ratio))
        });

        let (Some((latest_snapshot, latest_ratio)), Some((oldest_snapshot, oldest_ratio))) =
            (latest, oldest)
        else {
            return;
        };

        let drop_points = (oldest_ratio - latest_ratio) * 100.0;
        if latest_ratio >= rules.buffer_pool_low_ratio
            || drop_points < rules.buffer_pool_drop_points
        {
            return;
        }

        signals.push(MySqlPerformanceSignal {
            severity: MySqlSignalSeverity::Medium,
            category: MySqlSignalCategory::Memory,
            title: "Buffer pool hit ratio degraded".to_string(),
            summary: format!(
                "Buffer pool hit ratio fell by {:.1} percentage points and is now {:.1}%.",
                drop_points,
                latest_ratio * 100.0
            ),
            evidence: vec![
                format!(
                    "{}: {:.1}% buffer pool hit ratio",
                    oldest_snapshot.collected_at.to_rfc3339(),
                    oldest_ratio * 100.0
                ),
                format!(
                    "{}: {:.1}% buffer pool hit ratio",
                    latest_snapshot.collected_at.to_rfc3339(),
                    latest_ratio * 100.0
                ),
            ],
            recommendation:
                "Correlate with workload changes, table scans, and working-set growth before changing innodb_buffer_pool_size."
                    .to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn snapshot(
        minutes_ago: i64,
        connection_usage_pct: Option<f64>,
        buffer_pool_hit_ratio: Option<f64>,
        high_priority_findings_count: i32,
    ) -> MySqlSignalSnapshot {
        MySqlSignalSnapshot {
            id: Uuid::new_v4(),
            collected_at: Utc::now() - Duration::minutes(minutes_ago),
            qps_since_start: 10.0,
            threads_connected: 20,
            threads_running: 2,
            connection_usage_pct,
            buffer_pool_hit_ratio,
            slow_queries: 0,
            findings_count: high_priority_findings_count,
            high_priority_findings_count,
        }
    }

    fn signal_titles(signals: &[MySqlPerformanceSignal]) -> Vec<String> {
        signals.iter().map(|signal| signal.title.clone()).collect()
    }

    #[test]
    fn detects_sustained_connection_pressure() {
        let snapshots = vec![
            snapshot(0, Some(91.0), Some(0.99), 0),
            snapshot(5, Some(88.0), Some(0.99), 0),
            snapshot(10, Some(72.0), Some(0.99), 0),
            snapshot(15, Some(87.0), Some(0.99), 0),
            snapshot(20, Some(50.0), Some(0.99), 0),
        ];

        let signals = MySqlSignalEvaluator::evaluate(&snapshots, &MySqlSignalRules::default());

        assert!(signal_titles(&signals).contains(&"Connection pressure is sustained".to_string()));
    }

    #[test]
    fn ignores_single_connection_spike() {
        let snapshots = vec![
            snapshot(0, Some(91.0), Some(0.99), 0),
            snapshot(5, Some(50.0), Some(0.99), 0),
            snapshot(10, Some(52.0), Some(0.99), 0),
            snapshot(15, Some(48.0), Some(0.99), 0),
            snapshot(20, Some(49.0), Some(0.99), 0),
        ];

        let signals = MySqlSignalEvaluator::evaluate(&snapshots, &MySqlSignalRules::default());

        assert!(!signal_titles(&signals).contains(&"Connection pressure is sustained".to_string()));
    }

    #[test]
    fn detects_new_high_priority_findings() {
        let snapshots = vec![
            snapshot(0, Some(30.0), Some(0.99), 2),
            snapshot(5, Some(30.0), Some(0.99), 0),
            snapshot(10, Some(30.0), Some(0.99), 0),
        ];

        let signals = MySqlSignalEvaluator::evaluate(&snapshots, &MySqlSignalRules::default());

        assert!(signal_titles(&signals).contains(&"New high-priority finding appeared".to_string()));
    }

    #[test]
    fn detects_buffer_pool_hit_ratio_regression() {
        let snapshots = vec![
            snapshot(0, Some(30.0), Some(0.93), 0),
            snapshot(5, Some(30.0), Some(0.96), 0),
            snapshot(10, Some(30.0), Some(0.99), 0),
        ];

        let signals = MySqlSignalEvaluator::evaluate(&snapshots, &MySqlSignalRules::default());

        assert!(signal_titles(&signals).contains(&"Buffer pool hit ratio degraded".to_string()));
    }
}
