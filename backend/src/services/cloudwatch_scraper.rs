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


use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Cursor, Write};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use arrow::array::{Float64Array, Int32Array, StringArray, TimestampMillisecondArray};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatch;
use aws_config::BehaviorVersion;
use aws_sdk_cloudwatch::types::{Dimension, Metric, MetricDataQuery, MetricStat, ScanBy};
use aws_sdk_cloudwatch::Client as CloudWatchClient;
use aws_sdk_s3::Client as S3Client;
use aws_smithy_types::DateTime as AwsDateTime;
use aws_types::region::Region;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use cron::Schedule;
use futures::{stream, StreamExt, TryStreamExt};
use parquet::arrow::arrow_writer::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use sea_query::{Alias, ColumnDef, Expr, Query, Table};
use serde::{Deserialize, Serialize};
use tokio::time;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::utils::time_conversion::AwsDateTimeExt;

const DEFAULT_PERIOD_SECONDS: i32 = 300;
const DEFAULT_LOOKBACK_HOURS: i64 = 1;
const DEFAULT_MAX_QUERIES_PER_BATCH: usize = 100;
const MAX_REGION_CONCURRENCY: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudWatchScraperConfig {
    pub schedule: String,
    #[serde(default)]
    pub regions: Vec<String>,
    #[serde(default)]
    pub namespaces: Vec<String>,
    #[serde(default)]
    pub metric_names: Option<Vec<String>>,
    #[serde(default = "default_period_seconds")]
    pub period_seconds: i32,
    #[serde(default = "default_stat")]
    pub stat: String,
    #[serde(default = "default_lookback_hours")]
    pub lookback_hours: i64,
    #[serde(default = "default_max_queries")]
    pub max_queries_per_batch: usize,
    #[serde(default)]
    pub target: ScraperTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScraperTarget {
    S3 {
        bucket: String,
        prefix: String,
        #[serde(default)]
        format: StorageFormat,
    },
    FileSystem {
        path: String,
        #[serde(default)]
        format: StorageFormat,
    },
    Database {
        table_name: String,
    },
}

impl Default for ScraperTarget {
    fn default() -> Self {
        ScraperTarget::FileSystem {
            path: "./metrics".to_string(),
            format: StorageFormat::Parquet,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StorageFormat {
    Json,
    Parquet,
}

impl Default for StorageFormat {
    fn default() -> Self {
        StorageFormat::Parquet
    }
}

fn default_period_seconds() -> i32 {
    DEFAULT_PERIOD_SECONDS
}

fn default_stat() -> String {
    "Average".to_string()
}

fn default_lookback_hours() -> i64 {
    DEFAULT_LOOKBACK_HOURS
}

fn default_max_queries() -> usize {
    DEFAULT_MAX_QUERIES_PER_BATCH
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricRecord {
    pub timestamp: DateTime<Utc>,
    pub namespace: String,
    pub metric_name: String,
    pub dimensions: HashMap<String, String>,
    pub value: f64,
    pub stat: String,
    pub period: i32,
    pub region: String,
}

#[derive(Clone)]
pub struct CloudWatchScraper {
    config: CloudWatchScraperConfig,
    base_client: CloudWatchClient,
    s3_client: Option<S3Client>,
    database: Option<DatabaseConnection>,
}

impl CloudWatchScraper {
    pub async fn new(
        config: CloudWatchScraperConfig,
        database: Option<DatabaseConnection>,
    ) -> Result<Self> {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
        let base_client = CloudWatchClient::new(&sdk_config);
        let needs_s3 = matches!(config.target, ScraperTarget::S3 { .. });
        let s3_client = needs_s3.then(|| S3Client::new(&sdk_config));

        Ok(Self {
            config,
            base_client,
            s3_client,
            database,
        })
    }

    pub async fn spawn_scheduler(self: Arc<Self>) -> Result<()> {
        let schedule = Schedule::from_str(&self.config.schedule)
            .context("invalid cron expression for CloudWatch scraper")?;
        let runner = Arc::clone(&self);

        tokio::spawn(async move {
            let mut upcoming = schedule.upcoming(Utc);
            while let Some(next_run) = upcoming.next() {
                let now = Utc::now();
                let delay = next_run - now;
                let sleep_duration = delay.to_std().unwrap_or_else(|_| Duration::from_secs(0));
                time::sleep(sleep_duration).await;

                let task_runner = Arc::clone(&runner);
                tokio::spawn(async move {
                    if let Err(err) = task_runner.run_scrape().await {
                        error!(error = ?err, "CloudWatch scheduled scrape failed");
                    }
                });
            }
        });

        Ok(())
    }

    pub async fn run_scrape(&self) -> Result<()> {
        self.validate_config()?;
        info!("Starting CloudWatch scrape job");
        let end_time = Utc::now();
        let start_time = end_time - ChronoDuration::hours(self.config.lookback_hours);

        let regions = self.config.regions.clone();
        let concurrency = self.region_concurrency();

        let region_results = stream::iter(regions.into_iter())
            .map(|region| {
                let scraper = self;
                let start = start_time;
                let end = end_time;
                async move { scraper.scrape_region(region, start, end).await }
            })
            .buffer_unordered(concurrency)
            .try_collect::<Vec<Vec<MetricRecord>>>()
            .await?;

        let aggregated_records: Vec<MetricRecord> = region_results.into_iter().flatten().collect();

        if aggregated_records.is_empty() {
            warn!("No CloudWatch metrics were collected during this run");
            return Ok(());
        }

        self.store_records(&aggregated_records).await?;
        info!(
            count = aggregated_records.len(),
            "CloudWatch scrape job completed"
        );
        Ok(())
    }

    fn validate_config(&self) -> Result<()> {
        if self.config.regions.is_empty() {
            bail!("CloudWatch scraper requires at least one AWS region");
        }
        if self.config.namespaces.is_empty() {
            bail!("CloudWatch scraper requires at least one CloudWatch namespace");
        }
        if self.config.period_seconds <= 0 {
            bail!("CloudWatch scraper requires period_seconds to be greater than zero");
        }
        if self.config.lookback_hours <= 0 {
            bail!("CloudWatch scraper requires lookback_hours to be greater than zero");
        }
        Ok(())
    }

    fn region_concurrency(&self) -> usize {
        let configured = self.config.regions.len();
        configured.min(MAX_REGION_CONCURRENCY).max(1)
    }

    fn metric_name_filter(&self) -> Option<HashSet<String>> {
        self.config
            .metric_names
            .as_ref()
            .map(|names| names.iter().cloned().collect())
    }

    fn apply_metric_filters(
        &self,
        metrics: Vec<Metric>,
        filter: Option<&HashSet<String>>,
    ) -> Vec<Metric> {
        match filter {
            Some(names) => metrics
                .into_iter()
                .filter(|metric| {
                    metric
                        .metric_name()
                        .map(|name| names.contains(name))
                        .unwrap_or(false)
                })
                .collect(),
            None => metrics,
        }
    }

    async fn scrape_region(
        &self,
        region: String,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<MetricRecord>> {
        info!(region = %region, "Collecting CloudWatch metrics for region");
        let client = self.client_for_region(&region).await?;
        let metric_filter = self.metric_name_filter();
        let mut region_records = Vec::new();

        for namespace in &self.config.namespaces {
            let discovered_metrics = self.discover_metrics(&client, namespace).await?;
            let namespace_total = discovered_metrics.len();
            let filtered_metrics =
                self.apply_metric_filters(discovered_metrics, metric_filter.as_ref());

            if filtered_metrics.is_empty() {
                debug!(region = %region, namespace = %namespace, total = namespace_total, "No metrics selected for namespace");
                continue;
            }

            let mut namespace_records = self
                .fetch_metric_data(&client, &region, namespace, &filtered_metrics, start, end)
                .await?;

            if namespace_records.is_empty() {
                debug!(region = %region, namespace = %namespace, "No datapoints returned for namespace");
            } else {
                info!(region = %region, namespace = %namespace, count = namespace_records.len(), "Collected datapoints for namespace");
            }

            region_records.append(&mut namespace_records);
        }

        if region_records.is_empty() {
            warn!(region = %region, "No metrics collected for region");
        } else {
            info!(region = %region, count = region_records.len(), "Region collection complete");
        }

        Ok(region_records)
    }

    async fn client_for_region(&self, region: &str) -> Result<CloudWatchClient> {
        if region.is_empty() {
            return Ok(self.base_client.clone());
        }

        let region_provider = Region::new(region.to_string());
        let regional_config = aws_config::defaults(BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;
        Ok(CloudWatchClient::new(&regional_config))
    }

    async fn discover_metrics(
        &self,
        client: &CloudWatchClient,
        namespace: &str,
    ) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut request = client.list_metrics().namespace(namespace.to_string());
            if let Some(token) = next_token.as_ref() {
                request = request.next_token(token);
            }

            let response = request.send().await?;
            metrics.extend(response.metrics().iter().cloned());

            next_token = response.next_token().map(|token| token.to_string());
            if next_token.is_none() {
                break;
            }
        }

        Ok(metrics)
    }

    async fn fetch_metric_data(
        &self,
        client: &CloudWatchClient,
        region: &str,
        namespace: &str,
        metrics: &[Metric],
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<MetricRecord>> {
        if metrics.is_empty() {
            return Ok(Vec::new());
        }

        let mut descriptors = Vec::with_capacity(metrics.len());
        for (index, metric) in metrics.iter().enumerate() {
            if let Some(metric_name) = metric.metric_name() {
                let id = format!("m{}_{}", index, sanitize_id(metric_name));
                descriptors.push((id, metric.clone()));
            }
        }

        let mut records = Vec::new();
        let max_queries = self.config.max_queries_per_batch.max(1);
        let descriptor_map: HashMap<_, _> = descriptors
            .iter()
            .map(|(id, metric)| (id.clone(), metric.clone()))
            .collect();

        for chunk in descriptors.chunks(max_queries) {
            let metric_queries: Vec<MetricDataQuery> = chunk
                .iter()
                .map(|(id, metric)| {
                    MetricDataQuery::builder()
                        .id(id)
                        .metric_stat(
                            MetricStat::builder()
                                .metric(metric.clone())
                                .period(self.config.period_seconds)
                                .stat(&self.config.stat)
                                .build(),
                        )
                        .return_data(true)
                        .build()
                })
                .collect();

            let response = client
                .get_metric_data()
                .set_metric_data_queries(Some(metric_queries))
                .start_time(AwsDateTime::from_millis(start.timestamp_millis()))
                .end_time(AwsDateTime::from_millis(end.timestamp_millis()))
                .scan_by(ScanBy::TimestampAscending)
                .send()
                .await?;

            for result in response.metric_data_results() {
                let Some(id) = result.id() else {
                    continue;
                };
                let Some(metric) = descriptor_map.get(id) else {
                    debug!(query_id = %id, "Received metric data for unknown query identifier");
                    continue;
                };

                let metric_name = metric.metric_name().unwrap_or_default().to_string();
                let namespace_value = metric.namespace().unwrap_or(namespace).to_string();
                let dimension_map = dimensions_to_map(metric.dimensions());

                for (timestamp, value) in result.timestamps().iter().zip(result.values().iter()) {
                    let record = MetricRecord {
                        timestamp: timestamp.to_chrono_utc(),
                        namespace: namespace_value.clone(),
                        metric_name: metric_name.clone(),
                        dimensions: dimension_map.clone(),
                        value: *value,
                        stat: self.config.stat.clone(),
                        period: self.config.period_seconds,
                        region: region.to_string(),
                    };
                    records.push(record);
                }
            }
        }

        Ok(records)
    }

    async fn store_records(&self, records: &[MetricRecord]) -> Result<()> {
        match &self.config.target {
            ScraperTarget::S3 {
                bucket,
                prefix,
                format,
            } => self.store_to_s3(bucket, prefix, *format, records).await,
            ScraperTarget::FileSystem { path, format } => {
                self.store_to_filesystem(path, *format, records)
            }
            ScraperTarget::Database { table_name } => {
                self.store_to_database(table_name, records).await
            }
        }
    }

    async fn store_to_s3(
        &self,
        bucket: &str,
        prefix: &str,
        format: StorageFormat,
        records: &[MetricRecord],
    ) -> Result<()> {
        let client = self
            .s3_client
            .as_ref()
            .context("S3 target configured but S3 client was not initialised")?;

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let (key, body, content_type) = match format {
            StorageFormat::Json => {
                let key = format!("{}/metrics_{}.json", prefix, timestamp);
                let body = serde_json::to_vec_pretty(records)?;
                (key, body, "application/json")
            }
            StorageFormat::Parquet => {
                let key = format!("{}/metrics_{}.parquet", prefix, timestamp);
                let body = self.serialize_to_parquet(records)?;
                (key, body, "application/octet-stream")
            }
        };

        client
            .put_object()
            .bucket(bucket)
            .key(&key)
            .body(body.into())
            .content_type(content_type)
            .send()
            .await?;

        info!(bucket = %bucket, key = %key, format = ?format, "Uploaded metrics to S3");
        Ok(())
    }

    fn store_to_filesystem(
        &self,
        path: &str,
        format: StorageFormat,
        records: &[MetricRecord],
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        fs::create_dir_all(path).with_context(|| format!("failed to create directory {}", path))?;
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");

        match format {
            StorageFormat::Json => {
                let filename = format!("{}/metrics_{}.json", path, timestamp);
                let file = File::create(&filename)
                    .with_context(|| format!("failed to create file {}", filename))?;
                serde_json::to_writer_pretty(file, records)?;
                info!(file = %filename, "Stored metrics locally (JSON)");
            }
            StorageFormat::Parquet => {
                let filename = format!("{}/metrics_{}.parquet", path, timestamp);
                let buffer = self.serialize_to_parquet(records)?;
                let mut file = File::create(&filename)
                    .with_context(|| format!("failed to create file {}", filename))?;
                file.write_all(&buffer)?;
                info!(file = %filename, "Stored metrics locally (Parquet)");
            }
        }

        Ok(())
    }

    async fn store_to_database(&self, table_name: &str, records: &[MetricRecord]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        let db = self
            .database
            .as_ref()
            .context("Database target configured but no database connection supplied")?;

        self.ensure_metrics_table(db, table_name).await?;
        let backend = db.get_database_backend();
        let table_alias = Alias::new(table_name);

        for record in records {
            let dimensions_json = serde_json::to_string(&record.dimensions)?;
            let mut insert = Query::insert();
            insert.into_table(table_alias.clone()).columns([
                Alias::new("id"),
                Alias::new("timestamp"),
                Alias::new("namespace"),
                Alias::new("metric_name"),
                Alias::new("dimensions"),
                Alias::new("value"),
                Alias::new("stat"),
                Alias::new("period"),
                Alias::new("region"),
                Alias::new("created_at"),
            ]);
            insert.values_panic([
                Expr::value(record_id_value()),
                Expr::value(string_value(record.timestamp.to_rfc3339())),
                Expr::value(string_value(record.namespace.clone())),
                Expr::value(string_value(record.metric_name.clone())),
                Expr::value(string_value(dimensions_json)),
                Expr::value(sea_query::Value::Double(Some(record.value))),
                Expr::value(string_value(record.stat.clone())),
                Expr::value(sea_query::Value::Int(Some(record.period))),
                Expr::value(string_value(record.region.clone())),
                Expr::value(string_value(Utc::now().to_rfc3339())),
            ]);

            let statement = build_statement(backend, insert);
            db.execute(statement).await?;
        }

        info!(table = %table_name, count = records.len(), "Persisted metrics to database");
        Ok(())
    }

    async fn ensure_metrics_table(&self, db: &DatabaseConnection, table_name: &str) -> Result<()> {
        let backend = db.get_database_backend();
        let table_alias = Alias::new(table_name);

        let create_table = Table::create()
            .if_not_exists()
            .table(table_alias.clone())
            .col(
                ColumnDef::new(Alias::new("id"))
                    .string_len(36)
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(Alias::new("timestamp")).string().not_null())
            .col(ColumnDef::new(Alias::new("namespace")).string().not_null())
            .col(
                ColumnDef::new(Alias::new("metric_name"))
                    .string()
                    .not_null(),
            )
            .col(ColumnDef::new(Alias::new("dimensions")).string().not_null())
            .col(ColumnDef::new(Alias::new("value")).double().not_null())
            .col(ColumnDef::new(Alias::new("stat")).string().not_null())
            .col(ColumnDef::new(Alias::new("period")).integer().not_null())
            .col(ColumnDef::new(Alias::new("region")).string().not_null())
            .col(
                ColumnDef::new(Alias::new("created_at"))
                    .string()
                    .not_null()
                    .default(Expr::current_timestamp()),
            )
            .to_owned();

        let statement = backend.build(&create_table);
        db.execute(statement).await?;
        Ok(())
    }

    fn serialize_to_parquet(&self, records: &[MetricRecord]) -> Result<Vec<u8>> {
        if records.is_empty() {
            return Ok(Vec::new());
        }

        let schema = Arc::new(Schema::new(vec![
            Field::new(
                "timestamp",
                DataType::Timestamp(TimeUnit::Millisecond, None),
                false,
            ),
            Field::new("namespace", DataType::Utf8, false),
            Field::new("metric_name", DataType::Utf8, false),
            Field::new("dimensions", DataType::Utf8, false),
            Field::new("value", DataType::Float64, false),
            Field::new("stat", DataType::Utf8, false),
            Field::new("period", DataType::Int32, false),
            Field::new("region", DataType::Utf8, false),
        ]));

        let timestamps: Vec<i64> = records
            .iter()
            .map(|record| record.timestamp.timestamp_millis())
            .collect();
        let namespaces: Vec<&str> = records
            .iter()
            .map(|record| record.namespace.as_str())
            .collect();
        let metric_names: Vec<&str> = records
            .iter()
            .map(|record| record.metric_name.as_str())
            .collect();
        let dimensions: Vec<String> = records
            .iter()
            .map(|record| serde_json::to_string(&record.dimensions))
            .collect::<Result<_, _>>()?;
        let values: Vec<f64> = records.iter().map(|record| record.value).collect();
        let stats: Vec<&str> = records.iter().map(|record| record.stat.as_str()).collect();
        let periods: Vec<i32> = records.iter().map(|record| record.period).collect();
        let regions: Vec<&str> = records
            .iter()
            .map(|record| record.region.as_str())
            .collect();

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(TimestampMillisecondArray::from(timestamps))
                    as Arc<dyn arrow::array::Array>,
                Arc::new(StringArray::from(namespaces)),
                Arc::new(StringArray::from(metric_names)),
                Arc::new(StringArray::from(dimensions)),
                Arc::new(Float64Array::from(values)),
                Arc::new(StringArray::from(stats)),
                Arc::new(Int32Array::from(periods)),
                Arc::new(StringArray::from(regions)),
            ],
        )?;

        let props = WriterProperties::builder()
            .set_compression(Compression::SNAPPY)
            .build();

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = ArrowWriter::try_new(&mut cursor, schema, Some(props))?;
            writer.write(&batch)?;
            writer.close()?;
        }

        Ok(cursor.into_inner())
    }
}

fn sanitize_id(metric_name: &str) -> String {
    metric_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn dimensions_to_map(dimensions: &[Dimension]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for dimension in dimensions.iter() {
        if let (Some(name), Some(value)) = (dimension.name(), dimension.value()) {
            map.insert(name.to_string(), value.to_string());
        }
    }
    map
}

fn record_id_value() -> sea_query::Value {
    let id = Uuid::new_v4().to_string();
    sea_query::Value::String(Some(Box::new(id)))
}

fn string_value<S: Into<String>>(value: S) -> sea_query::Value {
    sea_query::Value::String(Some(Box::new(value.into())))
}

fn build_statement(backend: DatabaseBackend, query: sea_query::InsertStatement) -> Statement {
    backend.build(&query)
}
