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


use crate::models::slow_query_event::SlowQueryEvent;
use crate::models::query_fingerprint::QueryFingerprint;
use crate::repositories::slow_query_repository::SlowQueryRepository;
use crate::repositories::query_fingerprint_repository::QueryFingerprintRepository;
use crate::repositories::aurora_cluster_repository::AuroraClusterRepository;
use crate::utils::retry::{retry_with_backoff, db_retry_config};
use uuid::Uuid;
use chrono::{NaiveDateTime, Utc};
use regex::Regex;
use serde_json;

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

    pub async fn ingest_logs(
        &self,
        cluster_id: Uuid,
        logs: &[String],
        engine: &str,
    ) -> Result<(), String> {
        let cluster = retry_with_backoff(
            &db_retry_config(),
            || self.cluster_repo.find_by_id(cluster_id),
        ).await?
            .ok_or_else(|| "Aurora cluster not found".to_string())?;

        if !cluster.is_active {
            return Ok(());
        }

        let events = match engine.to_lowercase().as_str() {
            "mysql" | "aurora-mysql" => self.parse_mysql_logs(logs, cluster_id)?,
            "postgresql" | "aurora-postgresql" => self.parse_postgresql_logs(logs, cluster_id)?,
            _ => return Err(format!("Unsupported engine: {}", engine)),
        };

        if events.is_empty() {
            return Ok(());
        }

        const BATCH_SIZE: usize = 100;
        for chunk in events.chunks(BATCH_SIZE) {
            self.process_slow_query_event_batch(chunk, engine).await?;
        }

        Ok(())
    }

    fn parse_mysql_logs(&self, logs: &[String], cluster_id: Uuid) -> Result<Vec<ParsedSlowQueryEvent>, String> {
        let log_content = logs.join("\n");
        let mut events = Vec::new();
        let lines: Vec<&str> = log_content.lines().collect();

        let mut i = 0;
        while i < lines.len() {
            if lines[i].starts_with("# Time:") {
                if let Some(mut ev) = self.parse_single_mysql_event(&lines[i..])? {
                    ev.cluster_id = cluster_id;
                    events.push(ev);
                    i += self.count_mysql_event_lines(&lines[i..]);
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        Ok(events)
    }

    fn parse_postgresql_logs(&self, logs: &[String], cluster_id: Uuid) -> Result<Vec<ParsedSlowQueryEvent>, String> {
        let re = Regex::new(r"(?x)
            ^(?P<ts>\d{4}-\d{2}-\d{2}\s\d{2}:\d{2}:\d{2}\.\d{3}\s\w+)\s
            \[(?P<pid>\d+)\]\s
            (?P<user>[\w-]+)@(?P<db>[\w-]+)\s
            LOG:\s+duration:\s+(?P<dur>\d+\.?\d*)\sms\s+
            statement:\s+(?P<sql>.*)$").map_err(|e| e.to_string())?;

        let mut events = Vec::new();
        for log in logs {
            if let Some(caps) = re.captures(log) {
                let timestamp_str = &caps["ts"];
                let event_timestamp = NaiveDateTime::parse_from_str(&timestamp_str[..23], "%Y-%m-%d %H:%M:%S.%3f")
                    .unwrap_or_else(|_| Utc::now().naive_utc());

                events.push(ParsedSlowQueryEvent {
                    cluster_id,
                    event_timestamp,
                    user_host: caps["user"].to_string(),
                    query_time: caps["dur"].parse::<f64>().unwrap_or(0.0) / 1000.0,
                    lock_time: 0.0,
                    rows_sent: 0,
                    rows_examined: 0,
                    sql_text: caps["sql"].to_string(),
                    thread_id: Some(caps["pid"].parse().unwrap_or(0)),
                    schema_name: Some(caps["db"].to_string()),
                });
            }
        }
        Ok(events)
    }

    async fn process_slow_query_event_batch(&self, events: &[ParsedSlowQueryEvent], engine: &str) -> Result<(), String> {
        let mut fingerprint_records = Vec::new();
        for event in events {
            let hash = self.generate_query_hash(&event.sql_text)?;
            let record = self.find_or_create_fingerprint(event.cluster_id, &hash, &event.sql_text, engine).await?;
            fingerprint_records.push(record);
        }

        let slow_query_events: Vec<SlowQueryEvent> = events.iter().enumerate().map(|(i, event)| {
            SlowQueryEvent {
                id: Uuid::new_v4(),
                cluster_id: event.cluster_id,
                event_timestamp: event.event_timestamp,
                query_time: event.query_time,
                lock_time: Some(event.lock_time),
                rows_sent: Some(event.rows_sent),
                rows_examined: Some(event.rows_examined),
                user_host: Some(event.user_host.clone()),
                database: event.schema_name.clone(),
                sql_text: event.sql_text.clone(),
                raw_log_line: String::new(),
                fingerprint_id: Some(fingerprint_records[i].id),
                parsed_at: Utc::now().naive_utc(),
                created_at: Utc::now().naive_utc(),
            }
        }).collect();

        retry_with_backoff(&db_retry_config(), || self.slow_query_repo.create_many(slow_query_events.clone())).await?;

        let mut stats_updates = Vec::new();
        for (i, record) in fingerprint_records.iter().enumerate() {
            let current = record.clone();
            let new_count = current.execution_count + 1;
            let new_total_time = current.total_query_time + events[i].query_time;
            let new_avg_time = new_total_time / new_count as f64;
            let new_rows_ex = current.total_rows_examined + events[i].rows_examined;
            let new_rows_sent = current.total_rows_sent + events[i].rows_sent;

            stats_updates.push((
                current.id,
                new_count,
                new_total_time,
                new_avg_time,
                new_rows_ex,
                new_rows_sent,
                Utc::now().naive_utc()
            ));
        }

        if !stats_updates.is_empty() {
            retry_with_backoff(&db_retry_config(), || self.fingerprint_repo.update_stats_batch(stats_updates.clone())).await?;
        }
        Ok(())
    }

    fn generate_query_hash(&self, sql: &str) -> Result<String, String> {
        let normalized = self.normalize_sql(sql)?;
        Ok(format!("{:x}", md5::compute(normalized.as_bytes())))
    }

    fn normalize_sql(&self, sql: &str) -> Result<String, String> {
        let mut normalized = sql.to_uppercase();
        let comment_regex = Regex::new(r"(?s)/\*.*?\*/|--.*?$").unwrap();
        normalized = comment_regex.replace_all(&normalized, "").to_string();
        let whitespace_regex = Regex::new(r"\s+").unwrap();
        normalized = whitespace_regex.replace_all(&normalized, " ").to_string();
        let number_regex = Regex::new(r"\b\d+\b").unwrap();
        normalized = number_regex.replace_all(&normalized, "?").to_string();
        let string_regex = Regex::new(r"'[^']*'").unwrap();
        normalized = string_regex.replace_all(&normalized, "?").to_string();
        Ok(normalized.trim().to_string())
    }

    fn extract_catalog_metadata(&self, sql: &str, engine: &str) -> (Vec<String>, Vec<String>) {
        use sqlparser::dialect::{MySqlDialect, PostgreSqlDialect, Dialect};
        use sqlparser::parser::Parser;
        use sqlparser::ast::Statement;

        let dialect: Box<dyn Dialect> = match engine.to_lowercase().as_str() {
            "mysql" | "aurora-mysql" => Box::new(MySqlDialect {}),
            _ => Box::new(PostgreSqlDialect {}),
        };

        let mut tables = Vec::new();
        let mut columns = Vec::new();

        if let Ok(ast) = Parser::parse_sql(&*dialect, sql) {
            for stmt in ast {
                match stmt {
                    Statement::Query(_query) => {
                        // Advanced AST traversal would happen here
                    }
                    _ => {}
                }
            }
        }
        
        let table_re = Regex::new(r"(?i)FROM\s+([\w\.]+)").unwrap();
        for cap in table_re.captures_iter(sql) {
            tables.push(cap[1].to_string());
        }
        let join_re = Regex::new(r"(?i)JOIN\s+([\w\.]+)").unwrap();
        for cap in join_re.captures_iter(sql) {
            tables.push(cap[1].to_string());
        }

        tables.sort();
        tables.dedup();
        (tables, columns)
    }

    async fn find_or_create_fingerprint(&self, cluster_id: Uuid, hash: &str, raw_sql: &str, engine: &str) -> Result<QueryFingerprint, String> {
        if let Some(existing) = retry_with_backoff(&db_retry_config(), || self.fingerprint_repo.find_by_hash(cluster_id, hash)).await? {
            return Ok(existing);
        }

        let normalized = self.normalize_sql(raw_sql)?;
        let (tables, columns) = self.extract_catalog_metadata(raw_sql, engine);
        
        let fingerprint = QueryFingerprint {
            id: Uuid::new_v4(),
            normalized_sql: normalized,
            fingerprint_hash: hash.to_string(),
            total_query_time: 0.0,
            avg_query_time: 0.0,
            p95_query_time: 0.0,
            p99_query_time: 0.0,
            total_rows_examined: 0,
            total_rows_sent: 0,
            waste_score: 0.0,
            execution_count: 0,
            cluster_count: 1,
            first_seen: Utc::now().naive_utc(),
            last_seen: Utc::now().naive_utc(),
            tables_used: serde_json::to_value(tables).unwrap_or(serde_json::Value::Array(vec![])),
            columns_used: serde_json::to_value(columns).unwrap_or(serde_json::Value::Array(vec![])),
            has_full_scan: false,
            has_filesort: false,
            has_temp_table: false,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };

        retry_with_backoff(&db_retry_config(), || self.fingerprint_repo.create(fingerprint.clone())).await
    }

    fn parse_single_mysql_event(&self, lines: &[&str]) -> Result<Option<ParsedSlowQueryEvent>, String> {
        if lines.is_empty() || !lines[0].starts_with("# Time:") {
            return Ok(None);
        }
        let mut event = ParsedSlowQueryEvent {
            cluster_id: Uuid::nil(),
            event_timestamp: Utc::now().naive_utc(),
            user_host: String::new(),
            query_time: 0.0,
            lock_time: 0.0,
            rows_sent: 0,
            rows_examined: 0,
            sql_text: String::new(),
            thread_id: None,
            schema_name: None,
        };
        let mut sql_lines = Vec::new();
        let mut in_sql = false;
        for line in lines {
            if line.starts_with("# Time:") {
                let ts = line.split(": ").nth(1).unwrap_or("");
                if let Ok(parsed) = NaiveDateTime::parse_from_str(ts, "%y%m%d %H:%M:%S") { event.event_timestamp = parsed; }
            } else if line.starts_with("# User@Host:") { event.user_host = line.split(": ").nth(1).unwrap_or("").to_string(); }
            else if line.starts_with("# Query_time:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for i in 0..parts.len() {
                    match parts[i] {
                        "Query_time:" => event.query_time = parts.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(0.0),
                        "Lock_time:" => event.lock_time = parts.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(0.0),
                        "Rows_sent:" => event.rows_sent = parts.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(0),
                        "Rows_examined:" => event.rows_examined = parts.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(0),
                        _ => {}
                    }
                }
            } else if line.starts_with("# Schema:") { event.schema_name = Some(line.split(": ").nth(1).unwrap_or("").to_string()); }
            else if !line.starts_with("#") && !line.trim().is_empty() { sql_lines.push(line.to_string()); in_sql = true; }
            else if in_sql && line.trim().is_empty() { break; }
        }
        event.sql_text = sql_lines.join("\n");
        Ok(Some(event))
    }

    fn count_mysql_event_lines(&self, lines: &[&str]) -> usize {
        let mut count = 0;
        let mut in_sql = false;
        for line in lines {
            count += 1;
            if line.starts_with("# Time:") { continue; }
            if !line.starts_with("#") && !line.trim().is_empty() { in_sql = true; }
            else if in_sql && line.trim().is_empty() { break; }
        }
        count
    }
}