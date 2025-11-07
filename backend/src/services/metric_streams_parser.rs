use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use arrow::array::{Float64Array, Int32Array, StringArray, TimestampMillisecondArray};
use arrow::record_batch::RecordBatch;
use aws_config::BehaviorVersion;
use aws_sdk_s3::Client as S3Client;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use tracing::{info, warn};

use super::cloudwatch_scraper::StorageFormat;
use crate::utils::time_conversion::timestamp_millis_to_datetime;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParsedMetricRecord {
    pub timestamp: DateTime<Utc>,
    pub namespace: String,
    pub metric_name: String,
    pub dimensions: HashMap<String, String>,
    pub value: f64,
    pub stat: String,
    pub period: i32,
    pub region: String,
}

pub struct MetricStreamsParser {
    s3_client: S3Client,
}

impl MetricStreamsParser {
    pub async fn new() -> Result<Self> {
        let shared_config = aws_config::defaults(BehaviorVersion::latest()).load().await;
        let s3_client = S3Client::new(&shared_config);
        Ok(Self { s3_client })
    }

    pub async fn parse_from_s3(&self, bucket: &str, key: &str) -> Result<Vec<ParsedMetricRecord>> {
        let response = self
            .s3_client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .with_context(|| format!("failed to load S3 object s3://{}/{}", bucket, key))?;

        let data = response
            .body
            .collect()
            .await
            .context("failed to read S3 object body")?
            .into_bytes();

        let format = infer_format_from_key(key);
        self.parse_bytes(&data, format)
    }

    pub fn parse_bytes(
        &self,
        data: &[u8],
        format: StorageFormat,
    ) -> Result<Vec<ParsedMetricRecord>> {
        match format {
            StorageFormat::Json => self.parse_json(data),
            StorageFormat::Parquet => self.parse_parquet(data),
        }
    }

    fn parse_json(&self, data: &[u8]) -> Result<Vec<ParsedMetricRecord>> {
        let records: Vec<ParsedMetricRecord> = serde_json::from_slice(data)
            .context("failed to deserialize metric stream JSON payload")?;
        Ok(records)
    }

    fn parse_parquet(&self, data: &[u8]) -> Result<Vec<ParsedMetricRecord>> {
        let reader = ParquetRecordBatchReaderBuilder::try_new(Bytes::from(data.to_vec()))?
            .build()
            .context("failed to construct Parquet reader")?;

        let mut records = Vec::new();
        for batch in reader {
            let batch = batch.context("failed to read Parquet record batch")?;
            records.extend(self.parse_batch(&batch)?);
        }

        Ok(records)
    }

    fn parse_batch(&self, batch: &RecordBatch) -> Result<Vec<ParsedMetricRecord>> {
        if batch.num_columns() < 8 {
            return Err(anyhow!(
                "expected 8 columns in metric batch but found {}",
                batch.num_columns()
            ));
        }

        let timestamp_col = batch
            .column(0)
            .as_any()
            .downcast_ref::<TimestampMillisecondArray>()
            .context("timestamp column is not TimestampMillisecondArray")?;
        let namespace_col = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .context("namespace column is not StringArray")?;
        let metric_name_col = batch
            .column(2)
            .as_any()
            .downcast_ref::<StringArray>()
            .context("metric_name column is not StringArray")?;
        let dimensions_col = batch
            .column(3)
            .as_any()
            .downcast_ref::<StringArray>()
            .context("dimensions column is not StringArray")?;
        let value_col = batch
            .column(4)
            .as_any()
            .downcast_ref::<Float64Array>()
            .context("value column is not Float64Array")?;
        let stat_col = batch
            .column(5)
            .as_any()
            .downcast_ref::<StringArray>()
            .context("stat column is not StringArray")?;
        let period_col = batch
            .column(6)
            .as_any()
            .downcast_ref::<Int32Array>()
            .context("period column is not Int32Array")?;
        let region_col = batch
            .column(7)
            .as_any()
            .downcast_ref::<StringArray>()
            .context("region column is not StringArray")?;

        let mut records = Vec::with_capacity(batch.num_rows());
        for row in 0..batch.num_rows() {
            let timestamp_millis = timestamp_col.value(row);
            let timestamp = timestamp_millis_to_datetime(timestamp_millis);

            let dimensions_map: HashMap<String, String> = dimensions_col
                .value(row)
                .parse::<Value>()
                .ok()
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or_default();

            let record = ParsedMetricRecord {
                timestamp,
                namespace: namespace_col.value(row).to_string(),
                metric_name: metric_name_col.value(row).to_string(),
                dimensions: dimensions_map,
                value: value_col.value(row),
                stat: stat_col.value(row).to_string(),
                period: period_col.value(row),
                region: region_col.value(row).to_string(),
            };
            records.push(record);
        }

        Ok(records)
    }

    pub async fn process_records(&self, records: &[ParsedMetricRecord]) -> Result<()> {
        info!(count = records.len(), "Processed metric stream records");
        Ok(())
    }
}

fn infer_format_from_key(key: &str) -> StorageFormat {
    if key.ends_with(".parquet") {
        StorageFormat::Parquet
    } else if key.ends_with(".json") {
        StorageFormat::Json
    } else {
        warn!(%key, "Unknown metric stream extension, defaulting to Parquet");
        StorageFormat::Parquet
    }
}
