use crate::models::mysql_performance_snapshot::MySQLPerformanceSnapshot;
use crate::repositories::mysql_performance_repository::MySQLPerformanceRepository;
use crate::repositories::aurora_cluster_repository::AuroraClusterRepository;
use uuid::Uuid;
use chrono::{NaiveDateTime, Duration};
use serde_json;
use std::collections::HashMap;

#[derive(Clone)]
pub struct MySQLPerformanceService {
    performance_repo: MySQLPerformanceRepository,
    cluster_repo: AuroraClusterRepository,
}

#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub connections: ConnectionMetrics,
    pub workload: WorkloadMetrics,
    pub innodb: InnoDBMetrics,
    pub replication: ReplicationMetrics,
}

#[derive(Debug, Clone)]
pub struct ConnectionMetrics {
    pub max_connections: i64,
    pub threads_connected: i64,
    pub threads_running: i64,
    pub connection_errors: HashMap<String, i64>,
}

#[derive(Debug, Clone)]
pub struct WorkloadMetrics {
    pub queries_per_second: f64,
    pub slow_queries: i64,
    pub select_commands: i64,
    pub insert_commands: i64,
    pub update_commands: i64,
    pub delete_commands: i64,
}

#[derive(Debug, Clone)]
pub struct InnoDBMetrics {
    pub buffer_pool_hit_rate: f64,
    pub buffer_pool_pages_total: i64,
    pub buffer_pool_pages_free: i64,
    pub buffer_pool_pages_dirty: i64,
    pub log_waits: i64,
    pub lock_waits: i64,
}

#[derive(Debug, Clone)]
pub struct ReplicationMetrics {
    pub slave_io_running: bool,
    pub slave_sql_running: bool,
    pub seconds_behind_master: Option<i64>,
    pub replication_errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub overall_score: f64,
    pub issues: Vec<String>,
    pub recommendations: Vec<String>,
    pub critical_issues: Vec<String>,
}

impl MySQLPerformanceService {
    pub fn new(
        performance_repo: MySQLPerformanceRepository,
        cluster_repo: AuroraClusterRepository,
    ) -> Self {
        Self {
            performance_repo,
            cluster_repo,
        }
    }

    pub async fn capture_performance_snapshot(
        &self,
        cluster_id: Uuid,
        metrics: PerformanceMetrics,
    ) -> Result<MySQLPerformanceSnapshot, String> {
        // Verify cluster exists
        let _cluster = self.cluster_repo.find_by_id(cluster_id).await?
            .ok_or_else(|| "Aurora cluster not found".to_string())?;

        let health_check = self.perform_health_check(&metrics);

        let snapshot = MySQLPerformanceSnapshot {
            id: Uuid::new_v4(),
            cluster_id,
            captured_at: chrono::Utc::now().naive_utc(),
            overall_health_score: health_check.overall_score,

            // Connection metrics
            max_connections: metrics.connections.max_connections,
            threads_connected: metrics.connections.threads_connected,
            threads_running: metrics.connections.threads_running,
            connection_errors: serde_json::to_value(metrics.connections.connection_errors)
                .map_err(|e| format!("Failed to serialize connection errors: {}", e))?,

            // Workload metrics
            queries_per_second: metrics.workload.queries_per_second,
            slow_queries: metrics.workload.slow_queries,
            select_commands: metrics.workload.select_commands,
            insert_commands: metrics.workload.insert_commands,
            update_commands: metrics.workload.update_commands,
            delete_commands: metrics.workload.delete_commands,

            // InnoDB metrics
            innodb_buffer_pool_hit_rate: metrics.innodb.buffer_pool_hit_rate,
            innodb_buffer_pool_pages_total: metrics.innodb.buffer_pool_pages_total,
            innodb_buffer_pool_pages_free: metrics.innodb.buffer_pool_pages_free,
            innodb_buffer_pool_pages_dirty: metrics.innodb.buffer_pool_pages_dirty,
            innodb_log_waits: metrics.innodb.log_waits,
            innodb_lock_waits: metrics.innodb.lock_waits,

            // Replication metrics
            slave_io_running: metrics.replication.slave_io_running,
            slave_sql_running: metrics.replication.slave_sql_running,
            seconds_behind_master: metrics.replication.seconds_behind_master,
            replication_errors: serde_json::to_value(metrics.replication.replication_errors)
                .map_err(|e| format!("Failed to serialize replication errors: {}", e))?,

            // Health check results
            top_issues: serde_json::to_value(health_check.issues)
                .map_err(|e| format!("Failed to serialize issues: {}", e))?,
            recommendations: serde_json::to_value(health_check.recommendations)
                .map_err(|e| format!("Failed to serialize recommendations: {}", e))?,
        };

        self.performance_repo.create(snapshot).await
    }

    pub async fn get_latest_health_check(&self, cluster_id: Uuid) -> Result<Option<HealthCheckResult>, String> {
        if let Some(snapshot) = self.performance_repo.find_latest_by_cluster(cluster_id).await? {
            let issues: Vec<String> = serde_json::from_value(snapshot.top_issues)
                .map_err(|e| format!("Failed to deserialize issues: {}", e))?;
            let recommendations: Vec<String> = serde_json::from_value(snapshot.recommendations)
                .map_err(|e| format!("Failed to deserialize recommendations: {}", e))?;

            Ok(Some(HealthCheckResult {
                overall_score: snapshot.overall_health_score,
                issues,
                recommendations,
                critical_issues: Vec::new(), // Would need additional logic to determine critical issues
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_performance_trends(
        &self,
        cluster_id: Uuid,
        hours: i64,
    ) -> Result<HashMap<String, Vec<(NaiveDateTime, f64)>>, String> {
        let cutoff_time = chrono::Utc::now().naive_utc() - Duration::hours(hours);
        let snapshots = self.performance_repo.find_by_time_range(
            cluster_id,
            cutoff_time,
            chrono::Utc::now().naive_utc(),
        ).await?;

        let mut trends = HashMap::new();

        let mut health_scores = Vec::new();
        let mut qps_trends = Vec::new();
        let mut connection_trends = Vec::new();
        let mut buffer_pool_hit_rates = Vec::new();

        for snapshot in snapshots {
            health_scores.push((snapshot.captured_at, snapshot.overall_health_score));
            qps_trends.push((snapshot.captured_at, snapshot.queries_per_second));
            connection_trends.push((snapshot.captured_at, snapshot.threads_connected as f64));
            buffer_pool_hit_rates.push((snapshot.captured_at, snapshot.innodb_buffer_pool_hit_rate));
        }

        trends.insert("health_score".to_string(), health_scores);
        trends.insert("queries_per_second".to_string(), qps_trends);
        trends.insert("threads_connected".to_string(), connection_trends);
        trends.insert("buffer_pool_hit_rate".to_string(), buffer_pool_hit_rates);

        Ok(trends)
    }

    pub async fn detect_performance_anomalies(&self, cluster_id: Uuid) -> Result<Vec<String>, String> {
        let mut anomalies = Vec::new();

        // Get recent snapshots (last 24 hours)
        let cutoff_time = chrono::Utc::now().naive_utc() - Duration::hours(24);
        let snapshots = self.performance_repo.find_by_time_range(
            cluster_id,
            cutoff_time,
            chrono::Utc::now().naive_utc(),
        ).await?;

        if snapshots.len() < 2 {
            return Ok(vec!["Insufficient data for anomaly detection".to_string()]);
        }

        // Check for sudden drops in health score
        let recent_snapshots: Vec<_> = snapshots.iter().rev().take(5).collect();
        if let (Some(latest), Some(previous)) = (recent_snapshots.first(), recent_snapshots.get(1)) {
            let health_drop = previous.overall_health_score - latest.overall_health_score;
            if health_drop > 0.2 { // 20% drop
                anomalies.push(format!("Health score dropped by {:.1}% in recent hours", health_drop * 100.0));
            }
        }

        // Check for high connection usage
        let avg_connections: f64 = snapshots.iter().map(|s| s.threads_connected as f64).sum::<f64>() / snapshots.len() as f64;
        let max_connections: f64 = snapshots.iter().map(|s| s.max_connections as f64).sum::<f64>() / snapshots.len() as f64;

        if avg_connections > max_connections * 0.8 {
            anomalies.push("Connection usage is consistently high (>80% of max_connections)".to_string());
        }

        // Check for low buffer pool hit rate
        let avg_hit_rate: f64 = snapshots.iter().map(|s| s.innodb_buffer_pool_hit_rate).sum::<f64>() / snapshots.len() as f64;
        if avg_hit_rate < 0.95 {
            anomalies.push(format!("Buffer pool hit rate is low: {:.1}%", avg_hit_rate * 100.0));
        }

        // Check for increasing slow queries
        let slow_query_trend: Vec<_> = snapshots.iter().map(|s| s.slow_queries).collect();
        if slow_query_trend.len() >= 3 {
            let recent_avg = slow_query_trend.iter().rev().take(3).sum::<i64>() / 3;
            let earlier_avg = slow_query_trend.iter().take(3).sum::<i64>() / 3;

            if recent_avg > earlier_avg * 2 {
                anomalies.push("Slow query count has doubled recently".to_string());
            }
        }

        if anomalies.is_empty() {
            anomalies.push("No significant performance anomalies detected".to_string());
        }

        Ok(anomalies)
    }

    fn perform_health_check(&self, metrics: &PerformanceMetrics) -> HealthCheckResult {
        let mut score = 1.0; // Start with perfect score
        let mut issues = Vec::new();
        let mut recommendations = Vec::new();
        let mut critical_issues = Vec::new();

        // Connection health checks
        let connection_usage = metrics.connections.threads_connected as f64 / metrics.connections.max_connections as f64;
        if connection_usage > 0.9 {
            score -= 0.3;
            issues.push("Connection usage is very high".to_string());
            recommendations.push("Consider increasing max_connections or optimizing connection pooling".to_string());
            critical_issues.push("High connection usage".to_string());
        } else if connection_usage > 0.7 {
            score -= 0.1;
            issues.push("Connection usage is elevated".to_string());
            recommendations.push("Monitor connection usage trends".to_string());
        }

        // Workload health checks
        if metrics.workload.queries_per_second < 10.0 {
            score -= 0.1;
            issues.push("Low query throughput detected".to_string());
            recommendations.push("Verify application is generating expected load".to_string());
        }

        if metrics.workload.slow_queries > 100 {
            score -= 0.2;
            issues.push("High number of slow queries".to_string());
            recommendations.push("Review and optimize slow queries".to_string());
            critical_issues.push("Slow queries".to_string());
        }

        // InnoDB health checks
        if metrics.innodb.buffer_pool_hit_rate < 0.95 {
            score -= 0.25;
            issues.push(format!("Buffer pool hit rate is low: {:.1}%", metrics.innodb.buffer_pool_hit_rate * 100.0));
            recommendations.push("Consider increasing innodb_buffer_pool_size".to_string());
            critical_issues.push("Low buffer pool hit rate".to_string());
        }

        if metrics.innodb.log_waits > 10 {
            score -= 0.15;
            issues.push("InnoDB log waits detected".to_string());
            recommendations.push("Consider increasing innodb_log_file_size or optimizing write patterns".to_string());
        }

        if metrics.innodb.lock_waits > 50 {
            score -= 0.2;
            issues.push("High InnoDB lock waits".to_string());
            recommendations.push("Investigate lock contention and long-running transactions".to_string());
            critical_issues.push("Lock contention".to_string());
        }

        // Replication health checks
        if !metrics.replication.slave_io_running {
            score -= 0.5;
            issues.push("Slave IO thread is not running".to_string());
            recommendations.push("Check replication configuration and network connectivity".to_string());
            critical_issues.push("Replication IO failure".to_string());
        }

        if !metrics.replication.slave_sql_running {
            score -= 0.5;
            issues.push("Slave SQL thread is not running".to_string());
            recommendations.push("Check for replication errors and resolve SQL thread issues".to_string());
            critical_issues.push("Replication SQL failure".to_string());
        }

        if let Some(seconds_behind) = metrics.replication.seconds_behind_master {
            if seconds_behind > 300 { // 5 minutes
                score -= 0.3;
                issues.push(format!("Replication lag is high: {} seconds", seconds_behind));
                recommendations.push("Investigate replication performance and network issues".to_string());
                critical_issues.push("High replication lag".to_string());
            } else if seconds_behind > 60 {
                score -= 0.1;
                issues.push(format!("Replication lag detected: {} seconds", seconds_behind));
                recommendations.push("Monitor replication lag trends".to_string());
            }
        }

        // Ensure score doesn't go below 0
        score = score.max(0.0);

        HealthCheckResult {
            overall_score: score,
            issues,
            recommendations,
            critical_issues,
        }
    }

    pub async fn get_performance_summary(&self, cluster_id: Uuid, hours: i64) -> Result<HashMap<String, serde_json::Value>, String> {
        let cutoff_time = chrono::Utc::now().naive_utc() - Duration::hours(hours);
        let snapshots = self.performance_repo.find_by_time_range(
            cluster_id,
            cutoff_time,
            chrono::Utc::now().naive_utc(),
        ).await?;

        if snapshots.is_empty() {
            return Ok(HashMap::new());
        }

        let mut summary = HashMap::new();

        // Calculate averages
        let avg_health_score = snapshots.iter().map(|s| s.overall_health_score).sum::<f64>() / snapshots.len() as f64;
        let avg_qps = snapshots.iter().map(|s| s.queries_per_second).sum::<f64>() / snapshots.len() as f64;
        let avg_connections = snapshots.iter().map(|s| s.threads_connected).sum::<i64>() / snapshots.len() as i64;
        let avg_buffer_hit_rate = snapshots.iter().map(|s| s.innodb_buffer_pool_hit_rate).sum::<f64>() / snapshots.len() as f64;

        // Get latest values
        let latest = snapshots.last().unwrap();
        let max_connections = latest.max_connections;

        summary.insert("period_hours".to_string(), serde_json::json!(hours));
        summary.insert("snapshots_count".to_string(), serde_json::json!(snapshots.len()));
        summary.insert("average_health_score".to_string(), serde_json::json!(avg_health_score));
        summary.insert("average_queries_per_second".to_string(), serde_json::json!(avg_qps));
        summary.insert("average_connections".to_string(), serde_json::json!(avg_connections));
        summary.insert("max_connections".to_string(), serde_json::json!(max_connections));
        summary.insert("average_buffer_pool_hit_rate".to_string(), serde_json::json!(avg_buffer_hit_rate));
        summary.insert("latest_health_score".to_string(), serde_json::json!(latest.overall_health_score));

        Ok(summary)
    }
}