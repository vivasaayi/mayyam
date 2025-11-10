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


use crate::repositories::query_fingerprint_repository::QueryFingerprintRepository;
use uuid::Uuid;
use regex::Regex;
use std::collections::HashSet;

#[derive(Clone)]
pub struct QueryFingerprintingService {
    fingerprint_repo: QueryFingerprintRepository,
}

#[derive(Debug, Clone)]
pub struct FingerprintResult {
    pub hash: String,
    pub normalized_query: String,
    pub tables: Vec<String>,
    pub columns: Vec<String>,
}

impl QueryFingerprintingService {
    pub fn new(fingerprint_repo: QueryFingerprintRepository) -> Self {
        Self { fingerprint_repo }
    }

    pub async fn fingerprint_and_update_catalog(
        &self,
        fingerprint_id: Uuid,
        sql: &str,
    ) -> Result<(), String> {
        let result = self.generate_fingerprint(sql)?;

        // Update the normalized query
        self.update_normalized_query(fingerprint_id, &result.normalized_query).await?;

        // Update catalog data
        self.fingerprint_repo.update_catalog_data(
            fingerprint_id,
            result.tables,
            result.columns,
        ).await?;

        Ok(())
    }

    pub fn generate_fingerprint(&self, sql: &str) -> Result<FingerprintResult, String> {
        let normalized = self.normalize_sql_query(sql)?;
        let hash = self.generate_hash(&normalized);
        let tables = self.extract_tables(sql)?;
        let columns = self.extract_columns(sql)?;

        Ok(FingerprintResult {
            hash,
            normalized_query: normalized,
            tables,
            columns,
        })
    }

    fn normalize_sql_query(&self, sql: &str) -> Result<String, String> {
        let mut normalized = sql.to_uppercase();

        // Remove comments (both /* */ and --)
        let comment_regex = Regex::new(r"/\*.*?\*/|--.*?$").map_err(|e| e.to_string())?;
        normalized = comment_regex.replace_all(&normalized, "").to_string();

        // Normalize whitespace (multiple spaces/tabs/newlines to single space)
        let whitespace_regex = Regex::new(r"\s+").map_err(|e| e.to_string())?;
        normalized = whitespace_regex.replace_all(&normalized, " ").to_string();

        // Normalize numbers (replace with ?)
        let number_regex = Regex::new(r"\b\d+(\.\d+)?\b").map_err(|e| e.to_string())?;
        normalized = number_regex.replace_all(&normalized, "?").to_string();

        // Normalize quoted strings (replace with ?)
        let string_regex = Regex::new(r"'[^']*'").map_err(|e| e.to_string())?;
        normalized = string_regex.replace_all(&normalized, "?").to_string();

        // Normalize IN clauses with multiple values
        let in_clause_regex = Regex::new(r"\bIN\s*\(\s*\?(\s*,\s*\?)+\s*\)").map_err(|e| e.to_string())?;
        normalized = in_clause_regex.replace_all(&normalized, "IN (?)").to_string();

        // Remove extra spaces around operators
        let operator_regex = Regex::new(r"\s*([=<>!]+)\s*").map_err(|e| e.to_string())?;
        normalized = operator_regex.replace_all(&normalized, "$1").to_string();

        // Normalize LIMIT clauses
        let limit_regex = Regex::new(r"\bLIMIT\s+\?(\s*,\s*\?)?").map_err(|e| e.to_string())?;
        normalized = limit_regex.replace_all(&normalized, "LIMIT ?").to_string();

        // Normalize ORDER BY
        let order_by_regex = Regex::new(r"\bORDER\s+BY\s+[^)]+").map_err(|e| e.to_string())?;
        normalized = order_by_regex.replace_all(&normalized, "ORDER BY ...").to_string();

        // Normalize GROUP BY
        let group_by_regex = Regex::new(r"\bGROUP\s+BY\s+[^)]+").map_err(|e| e.to_string())?;
        normalized = group_by_regex.replace_all(&normalized, "GROUP BY ...").to_string();

        Ok(normalized.trim().to_string())
    }

    fn generate_hash(&self, normalized_sql: &str) -> String {
        format!("{:x}", md5::compute(normalized_sql.as_bytes()))
    }

    fn extract_tables(&self, sql: &str) -> Result<Vec<String>, String> {
        let mut tables = HashSet::new();

        // Match FROM clause tables
        let from_regex = Regex::new(r"(?i)\bFROM\s+([`\w\.-]+(?:\s+AS\s+[`\w]+)?(?:\s*,\s*[`\w\.-]+(?:\s+AS\s+[`\w]+)?)*)").map_err(|e| e.to_string())?;
        if let Some(captures) = from_regex.captures(sql) {
            if let Some(from_clause) = captures.get(1) {
                let table_list: Vec<&str> = from_clause.as_str().split(',').collect();
                for table_spec in table_list {
                    let table_name = self.extract_table_name(table_spec.trim())?;
                    if !table_name.is_empty() {
                        tables.insert(table_name);
                    }
                }
            }
        }

        // Match JOIN tables
        let join_regex = Regex::new(r"(?i)\b(?:INNER\s+|LEFT\s+|RIGHT\s+|FULL\s+)?JOIN\s+([`\w\.-]+)").map_err(|e| e.to_string())?;
        for capture in join_regex.captures_iter(sql) {
            if let Some(table_match) = capture.get(1) {
                let table_name = self.extract_table_name(table_match.as_str())?;
                if !table_name.is_empty() {
                    tables.insert(table_name);
                }
            }
        }

        // Match UPDATE tables
        let update_regex = Regex::new(r"(?i)\bUPDATE\s+([`\w\.-]+)").map_err(|e| e.to_string())?;
        if let Some(captures) = update_regex.captures(sql) {
            if let Some(table_match) = captures.get(1) {
                let table_name = self.extract_table_name(table_match.as_str())?;
                if !table_name.is_empty() {
                    tables.insert(table_name);
                }
            }
        }

        // Match INSERT INTO tables
        let insert_regex = Regex::new(r"(?i)\bINSERT\s+INTO\s+([`\w\.-]+)").map_err(|e| e.to_string())?;
        if let Some(captures) = insert_regex.captures(sql) {
            if let Some(table_match) = captures.get(1) {
                let table_name = self.extract_table_name(table_match.as_str())?;
                if !table_name.is_empty() {
                    tables.insert(table_name);
                }
            }
        }

        let mut result: Vec<String> = tables.into_iter().collect();
        result.sort();
        Ok(result)
    }

    fn extract_columns(&self, sql: &str) -> Result<Vec<String>, String> {
        let mut columns = HashSet::new();

        // Match SELECT columns
        let select_regex = Regex::new(r"(?i)\bSELECT\s+(.+?)\bFROM\b").map_err(|e| e.to_string())?;
        if let Some(captures) = select_regex.captures(sql) {
            if let Some(select_clause) = captures.get(1) {
                let column_list: Vec<&str> = select_clause.as_str().split(',').collect();
                for col_spec in column_list {
                    let col_name = self.extract_column_name(col_spec.trim())?;
                    if !col_name.is_empty() {
                        columns.insert(col_name);
                    }
                }
            }
        }

        // Match WHERE clause columns
        let where_regex = Regex::new(r"(?i)\bWHERE\s+(.+?)(?:\bGROUP\b|\bORDER\b|\bLIMIT\b|$)").map_err(|e| e.to_string())?;
        if let Some(captures) = where_regex.captures(sql) {
            if let Some(where_clause) = captures.get(1) {
                self.extract_columns_from_expression(where_clause.as_str(), &mut columns)?;
            }
        }

        // Match ORDER BY columns
        let order_regex = Regex::new(r"(?i)\bORDER\s+BY\s+(.+?)(?:\bLIMIT\b|$)").map_err(|e| e.to_string())?;
        if let Some(captures) = order_regex.captures(sql) {
            if let Some(order_clause) = captures.get(1) {
                let order_cols: Vec<&str> = order_clause.as_str().split(',').collect();
                for col_spec in order_cols {
                    let col_name = self.extract_column_name(col_spec.trim())?;
                    if !col_name.is_empty() {
                        columns.insert(col_name);
                    }
                }
            }
        }

        // Match GROUP BY columns
        let group_regex = Regex::new(r"(?i)\bGROUP\s+BY\s+(.+?)(?:\bORDER\b|\bLIMIT\b|$)").map_err(|e| e.to_string())?;
        if let Some(captures) = group_regex.captures(sql) {
            if let Some(group_clause) = captures.get(1) {
                let group_cols: Vec<&str> = group_clause.as_str().split(',').collect();
                for col_spec in group_cols {
                    let col_name = self.extract_column_name(col_spec.trim())?;
                    if !col_name.is_empty() {
                        columns.insert(col_name);
                    }
                }
            }
        }

        let mut result: Vec<String> = columns.into_iter().collect();
        result.sort();
        Ok(result)
    }

    fn extract_columns_from_expression(&self, expr: &str, columns: &mut HashSet<String>) -> Result<(), String> {
        // Simple column extraction from expressions like "column = value", "column > value", etc.
        let col_regex = Regex::new(r"([`\w\.-]+)\s*[=<>!]+\s*[^,\s]+").map_err(|e| e.to_string())?;
        for capture in col_regex.captures_iter(expr) {
            if let Some(col_match) = capture.get(1) {
                let col_name = self.extract_column_name(col_match.as_str())?;
                if !col_name.is_empty() {
                    columns.insert(col_name);
                }
            }
        }
        Ok(())
    }

    fn extract_table_name(&self, table_spec: &str) -> Result<String, String> {
        // Remove AS alias and extract table name
        let parts: Vec<&str> = table_spec.split_whitespace().collect();
        let table_part = parts[0];

        // Remove backticks if present
        let table_name = table_part.trim_matches('`');

        // Handle schema.table format - take the table part
        if let Some(dot_pos) = table_name.rfind('.') {
            Ok(table_name[dot_pos + 1..].to_string())
        } else {
            Ok(table_name.to_string())
        }
    }

    fn extract_column_name(&self, col_spec: &str) -> Result<String, String> {
        // Remove function calls, aliases, and extract column name
        let col_spec = col_spec.trim();

        // Skip if it's a literal or function call
        if col_spec.starts_with('\'') || col_spec.starts_with('"') ||
           col_spec.contains('(') || col_spec.chars().all(|c| c.is_numeric()) {
            return Ok(String::new());
        }

        // Remove AS alias
        let parts: Vec<&str> = col_spec.split_whitespace().collect();
        let mut col_part = parts[0];

        // Remove table prefix if present (table.column)
        if let Some(dot_pos) = col_part.rfind('.') {
            col_part = &col_part[dot_pos + 1..];
        }

        // Remove backticks
        let col_name = col_part.trim_matches('`');

        // Skip keywords and special cases
        let keywords = ["ASC", "DESC", "NULL", "TRUE", "FALSE"];
        if keywords.contains(&col_name.to_uppercase().as_str()) {
            return Ok(String::new());
        }

        Ok(col_name.to_string())
    }

    async fn update_normalized_query(&self, fingerprint_id: Uuid, normalized_query: &str) -> Result<(), String> {
        // This would require adding a method to the repository to update just the normalized query
        // For now, we'll skip this as it's not critical for the initial implementation
        Ok(())
    }
}