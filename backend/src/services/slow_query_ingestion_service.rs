use crate::models::slow_query_event::SlowQueryEvent;
use crate::models::query_fingerprint::QueryFingerprint;
use crate::repositories::slow_query_repository::SlowQueryRepository;
use crate::repositories::query_fingerprint_repository::QueryFingerprintRepository;
use crate::repositories::aurora_cluster_repository::AuroraClusterRepository;
use uuid::Uuid;
use chrono::NaiveDateTime;
use std::collections::HashMap;
use regex::Regex;

#[derive(Clone)]
pub struct SlowQueryIngestionService {
    slow_query_repo: SlowQueryRepository,
    fingerprint_repo: QueryFingerprintRepository,
    cluster_repo: AuroraClusterRepository,
}

#[derive(Debug, Clone)]
pub struct ParsedSlowQueryEvent {
    pub cluster_id: Uuid,
    pub event_timestamp: NaiveDateTime,
    pub user_host: String,
    pub query_time: f64,
    pub lock_time: f64,
    pub rows_sent: i64,
    pub rows_examined: i64,
    pub sql_text: String,
    pub thread_id: Option<i64>,
    pub schema_name: Option<String>,
    pub last_errno: Option<i32>,
    pub killed: Option<i32>,
    pub bytes_received: Option<i64>,
    pub bytes_sent: Option<i64>,
    pub read_first: Option<i64>,
    pub read_last: Option<i64>,
    pub read_key: Option<i64>,
    pub read_next: Option<i64>,
    pub read_prev: Option<i64>,
    pub read_rnd: Option<i64>,
    pub read_rnd_next: Option<i64>,
    pub sort_merge_passes: Option<i64>,
    pub sort_range_count: Option<i64>,
    pub sort_rows: Option<i64>,
    pub sort_scan_count: Option<i64>,
    pub tmp_table_size: Option<i64>,
    pub tmp_tables: Option<i64>,
    pub tmp_disk_tables: Option<i64>,
    pub tmp_table_on_disk: Option<i64>,
}

impl SlowQueryIngestionService {
    pub fn new(
        slow_query_repo: SlowQueryRepository,
        fingerprint_repo: QueryFingerprintRepository,
        cluster_repo: AuroraClusterRepository,
    ) -> Self {
        Self {
            slow_query_repo,
            fingerprint_repo,
            cluster_repo,
        }
    }

    pub async fn ingest_slow_query_log(
        &self,
        cluster_id: Uuid,
        log_content: &str,
    ) -> Result<(), String> {
        // Verify cluster exists and is active
        let cluster = self.cluster_repo.find_by_id(cluster_id).await?
            .ok_or_else(|| "Aurora cluster not found".to_string())?;

        if !cluster.is_active {
            return Err("Cluster is not active".to_string());
        }

        // Parse the slow query log
        let events = self.parse_slow_query_log(log_content)?;

        // Process each event
        for event in events {
            if event.cluster_id != cluster_id {
                continue; // Skip events for different clusters
            }

            self.process_slow_query_event(event).await?;
        }

        Ok(())
    }

    pub async fn process_slow_query_event(&self, event: ParsedSlowQueryEvent) -> Result<(), String> {
        // Generate fingerprint for the query
        let fingerprint = self.generate_query_fingerprint(&event.sql_text)?;

        // Find or create fingerprint record
        let fingerprint_record = self.find_or_create_fingerprint(event.cluster_id, &fingerprint).await?;

        // Create slow query event
        let slow_query_event = SlowQueryEvent {
            id: Uuid::new_v4(),
            cluster_id: event.cluster_id,
            event_timestamp: event.event_timestamp,
            query_time: event.query_time,
            lock_time: Some(event.lock_time),
            rows_sent: Some(event.rows_sent),
            rows_examined: Some(event.rows_examined),
            user_host: Some(event.user_host),
            database: event.schema_name,
            sql_text: event.sql_text,
            raw_log_line: String::new(), // Will be populated from original log
            fingerprint_id: Some(fingerprint_record.id),
            parsed_at: chrono::Utc::now().naive_utc(),
            created_at: chrono::Utc::now().naive_utc(),
        };

        self.slow_query_repo.create(slow_query_event).await?;

        // Update fingerprint statistics
        self.update_fingerprint_statistics(fingerprint_record.id, event.query_time).await?;

        Ok(())
    }

    fn generate_query_fingerprint(&self, sql: &str) -> Result<String, String> {
        // Normalize the SQL query for fingerprinting
        let normalized = self.normalize_sql(sql)?;
        Ok(format!("{:x}", md5::compute(normalized.as_bytes())))
    }

    fn normalize_sql(&self, sql: &str) -> Result<String, String> {
        let mut normalized = sql.to_uppercase();

        // Remove comments
        let comment_regex = Regex::new(r"/\*.*?\*/|--.*?$").map_err(|e| e.to_string())?;
        normalized = comment_regex.replace_all(&normalized, "").to_string();

        // Normalize whitespace
        let whitespace_regex = Regex::new(r"\s+").map_err(|e| e.to_string())?;
        normalized = whitespace_regex.replace_all(&normalized, " ").to_string();

        // Normalize numbers
        let number_regex = Regex::new(r"\b\d+\b").map_err(|e| e.to_string())?;
        normalized = number_regex.replace_all(&normalized, "?").to_string();

        // Normalize quoted strings
        let string_regex = Regex::new(r"'[^']*'").map_err(|e| e.to_string())?;
        normalized = string_regex.replace_all(&normalized, "?").to_string();

        Ok(normalized.trim().to_string())
    }

    async fn find_or_create_fingerprint(&self, cluster_id: Uuid, query_hash: &str) -> Result<QueryFingerprint, String> {
        // Try to find existing fingerprint
        if let Some(existing) = self.fingerprint_repo.find_by_hash(cluster_id, query_hash).await? {
            return Ok(existing);
        }

        // Create new fingerprint
        let fingerprint = QueryFingerprint {
            id: Uuid::new_v4(),
            normalized_sql: String::new(), // Will be populated later
            fingerprint_hash: query_hash.to_string(),
            total_query_time: 0.0,
            avg_query_time: 0.0,
            p95_query_time: 0.0,
            p99_query_time: 0.0,
            total_rows_examined: 0,
            total_rows_sent: 0,
            execution_count: 0,
            cluster_count: 1,
            first_seen: chrono::Utc::now().naive_utc(),
            last_seen: chrono::Utc::now().naive_utc(),
            tables_used: serde_json::Value::Array(vec![]),
            columns_used: serde_json::Value::Array(vec![]),
            has_full_scan: false,
            has_filesort: false,
            has_temp_table: false,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        self.fingerprint_repo.create(fingerprint).await
    }

    async fn update_fingerprint_statistics(&self, fingerprint_id: Uuid, query_time: f64) -> Result<(), String> {
        // Get current fingerprint
        let current = self.fingerprint_repo.find_by_id(fingerprint_id).await?
            .ok_or_else(|| "Fingerprint not found".to_string())?;

        let new_count = current.execution_count + 1;
        let new_total_time = current.total_query_time + query_time;
        let new_avg_time = new_total_time / new_count as f64;

        // For now, we'll update basic statistics. P95/P99 would need more sophisticated calculation
        self.fingerprint_repo.update_statistics(
            fingerprint_id,
            new_count,
            new_total_time,
            new_avg_time,
            chrono::Utc::now().naive_utc(),
        ).await
    }

    fn parse_slow_query_log(&self, log_content: &str) -> Result<Vec<ParsedSlowQueryEvent>, String> {
        let mut events = Vec::new();
        let lines: Vec<&str> = log_content.lines().collect();

        let mut i = 0;
        while i < lines.len() {
            if lines[i].starts_with("# Time:") {
                if let Some(event) = self.parse_single_event(&lines[i..])? {
                    events.push(event);
                    // Skip the lines consumed by this event
                    i += self.count_event_lines(&lines[i..]);
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        Ok(events)
    }

    fn parse_single_event(&self, lines: &[&str]) -> Result<Option<ParsedSlowQueryEvent>, String> {
        if lines.is_empty() || !lines[0].starts_with("# Time:") {
            return Ok(None);
        }

        let mut event = ParsedSlowQueryEvent {
            cluster_id: Uuid::nil(), // Will be set by caller
            event_timestamp: chrono::Utc::now().naive_utc(),
            user_host: String::new(),
            query_time: 0.0,
            lock_time: 0.0,
            rows_sent: 0,
            rows_examined: 0,
            sql_text: String::new(),
            thread_id: None,
            schema_name: None,
            last_errno: None,
            killed: None,
            bytes_received: None,
            bytes_sent: None,
            read_first: None,
            read_last: None,
            read_key: None,
            read_next: None,
            read_prev: None,
            read_rnd: None,
            read_rnd_next: None,
            sort_merge_passes: None,
            sort_range_count: None,
            sort_rows: None,
            sort_scan_count: None,
            tmp_table_size: None,
            tmp_tables: None,
            tmp_disk_tables: None,
            tmp_table_on_disk: None,
        };

        let mut sql_lines = Vec::new();
        let mut in_sql = false;

        for line in lines {
            if line.starts_with("# Time:") {
                // Parse timestamp
                if let Some(timestamp_str) = line.split(": ").nth(1) {
                    if let Ok(timestamp) = chrono::NaiveDateTime::parse_from_str(timestamp_str, "%y%m%d %H:%M:%S") {
                        event.event_timestamp = timestamp;
                    }
                }
            } else if line.starts_with("# User@Host:") {
                event.user_host = line.split(": ").nth(1).unwrap_or("").to_string();
            } else if line.starts_with("# Query_time:") {
                self.parse_performance_metrics(line, &mut event)?;
            } else if line.starts_with("# Schema:") {
                event.schema_name = Some(line.split(": ").nth(1).unwrap_or("").to_string());
            } else if line.starts_with("SET ") {
                // Skip SET statements
                continue;
            } else if !line.starts_with("#") && !line.trim().is_empty() {
                // This is SQL
                sql_lines.push(line.to_string());
                in_sql = true;
            } else if in_sql && line.trim().is_empty() {
                // End of SQL
                break;
            }
        }

        event.sql_text = sql_lines.join("\n");
        Ok(Some(event))
    }

    fn parse_performance_metrics(&self, line: &str, event: &mut ParsedSlowQueryEvent) -> Result<(), String> {
        // Parse line like: "# Query_time: 0.000123  Lock_time: 0.000045 Rows_sent: 1  Rows_examined: 10"
        let parts: Vec<&str> = line.split_whitespace().collect();

        for i in 0..parts.len() {
            match parts[i] {
                "Query_time:" => {
                    if let Some(val) = parts.get(i + 1) {
                        event.query_time = val.parse().unwrap_or(0.0);
                    }
                }
                "Lock_time:" => {
                    if let Some(val) = parts.get(i + 1) {
                        event.lock_time = val.parse().unwrap_or(0.0);
                    }
                }
                "Rows_sent:" => {
                    if let Some(val) = parts.get(i + 1) {
                        event.rows_sent = val.parse().unwrap_or(0);
                    }
                }
                "Rows_examined:" => {
                    if let Some(val) = parts.get(i + 1) {
                        event.rows_examined = val.parse().unwrap_or(0);
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn count_event_lines(&self, lines: &[&str]) -> usize {
        let mut count = 0;
        let mut in_sql = false;

        for line in lines {
            count += 1;
            if line.starts_with("# Time:") {
                continue;
            } else if !line.starts_with("#") && !line.trim().is_empty() {
                in_sql = true;
            } else if in_sql && line.trim().is_empty() {
                break;
            }
        }

        count
    }
}