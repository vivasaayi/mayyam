use crate::errors::AppError;
use crate::models::aws_resource;
use crate::services::analytics::aws_analytics::metrics::MetricsAnalyzer;
use crate::services::analytics::aws_analytics::models::resource_workflows::ResourceAnalysisWorkflow;
use crate::services::aws::aws_types::cloud_watch;

pub struct KinesisAnalyzer;

impl KinesisAnalyzer {
    pub async fn analyze_kinesis_stream(
        resource: &aws_resource::Model,
        workflow: &ResourceAnalysisWorkflow,
        metrics: &cloud_watch::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut analysis = String::new();

        match workflow {
            ResourceAnalysisWorkflow::Performance => {
                analysis.push_str("# Kinesis Stream Performance Analysis\n\n");

                // Add stream info
                analysis.push_str(&format!("## Stream Information\n"));
                analysis.push_str(&format!("- Stream: {}\n", resource.resource_id));
                analysis.push_str(&format!("- ARN: {}\n", resource.arn));
                analysis.push_str(&format!("- Region: {}\n\n", resource.region));

                // Analyze GetRecords.IteratorAgeMilliseconds - measures how far behind the consumer is
                if let Some(iterator_age) =
                    MetricsAnalyzer::find_metric(metrics, "GetRecords.IteratorAgeMilliseconds")
                {
                    let (avg, max) =
                        MetricsAnalyzer::calculate_statistics(&iterator_age.datapoints);
                    analysis.push_str(&format!(
                        "## Consumer Lag\n- Average Iterator Age: {:.2} ms\n- Maximum Iterator Age: {:.2} ms\n\n",
                        avg, max
                    ));

                    if max > 30000.0 {
                        analysis.push_str("⚠️ **High Consumer Lag Detected**\n");
                        analysis.push_str(
                            "Your consumers are falling behind processing the stream. Consider:\n",
                        );
                        analysis.push_str("1. Increasing the number of consumer instances\n");
                        analysis.push_str("2. Optimizing consumer code for faster processing\n");
                        analysis
                            .push_str("3. Increasing the resource allocation for consumers\n\n");
                    }
                }

                // Analyze throughput
                if let Some(incoming_records) =
                    MetricsAnalyzer::find_metric(metrics, "IncomingRecords")
                {
                    let (avg, max) =
                        MetricsAnalyzer::calculate_statistics(&incoming_records.datapoints);
                    analysis.push_str(&format!(
                        "## Incoming Records\n- Average: {:.2} records/second\n- Peak: {:.2} records/second\n\n",
                        avg, max
                    ));
                }

                if let Some(incoming_bytes) = MetricsAnalyzer::find_metric(metrics, "IncomingBytes")
                {
                    let (avg, max) =
                        MetricsAnalyzer::calculate_statistics(&incoming_bytes.datapoints);
                    analysis.push_str(&format!(
                        "## Incoming Data\n- Average: {:.2} KB/s\n- Peak: {:.2} KB/s\n\n",
                        avg / 1024.0,
                        max / 1024.0
                    ));
                }

                // Add shard metrics if available
                if let Some(read_throughput) =
                    MetricsAnalyzer::find_metric(metrics, "ReadProvisionedThroughputExceeded")
                {
                    let (avg, max) =
                        MetricsAnalyzer::calculate_statistics(&read_throughput.datapoints);
                    if max > 0.0 {
                        analysis.push_str("⚠️ **Throughput Exceeded**\n");
                        analysis
                            .push_str("Your stream has experienced throttling events. Consider:\n");
                        analysis.push_str("1. Increasing the number of shards\n");
                        analysis.push_str(
                            "2. Implementing a more even distribution of partition keys\n\n",
                        );
                    } else {
                        analysis.push_str("✅ **No Throughput Exceeded Events**\n");
                        analysis.push_str(
                            "Your stream is handling the current load without throttling.\n\n",
                        );
                    }
                }

                // Add recommendations section
                analysis.push_str("## Recommendations\n");
                analysis.push_str("1. Monitor the Iterator Age metric closely to ensure consumers keep up with producers\n");
                analysis
                    .push_str("2. Implement auto-scaling for consumers based on Iterator Age\n");
                analysis
                    .push_str("3. Consider Enhanced Fan-Out for high-throughput applications\n");
                analysis.push_str("4. Review your shard count to ensure adequate capacity\n");
            }
            ResourceAnalysisWorkflow::Cost => {
                analysis.push_str("# Kinesis Stream Cost Analysis\n\n");

                // Add stream info
                analysis.push_str(&format!("## Stream Information\n"));
                analysis.push_str(&format!("- Stream: {}\n", resource.resource_id));
                analysis.push_str(&format!("- ARN: {}\n", resource.arn));
                analysis.push_str(&format!("- Region: {}\n\n", resource.region));

                // Extract number of shards if available in resource data
                let shard_count = resource
                    .resource_data
                    .get("ShardCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1);

                // Calculate estimated costs
                let hourly_shard_cost = match resource.region.as_str() {
                    "us-east-1" => 0.015, // USD per shard hour
                    "us-west-2" => 0.015,
                    "eu-west-1" => 0.017,
                    _ => 0.016, // Average cost for other regions
                };

                let monthly_shard_cost = hourly_shard_cost * 24.0 * 30.0;
                let total_monthly_cost = monthly_shard_cost * shard_count as f64;

                analysis.push_str(&format!("## Cost Analysis\n"));
                analysis.push_str(&format!("- Shard Count: {}\n", shard_count));
                analysis.push_str(&format!(
                    "- Cost per Shard per Hour: ${:.4}\n",
                    hourly_shard_cost
                ));
                analysis.push_str(&format!(
                    "- Estimated Monthly Cost: ${:.2}\n\n",
                    total_monthly_cost
                ));

                // Add PUT payload units analysis if available
                if let Some(incoming_bytes) = MetricsAnalyzer::find_metric(metrics, "IncomingBytes")
                {
                    let (avg_bytes, _) =
                        MetricsAnalyzer::calculate_statistics(&incoming_bytes.datapoints);
                    let avg_bytes_per_month = avg_bytes * 60.0 * 60.0 * 24.0 * 30.0;
                    let million_put_payload_units = avg_bytes_per_month / (1024.0 * 1024.0);

                    // Cost per million PUT payload units (25KB = 1 PUT payload unit)
                    let put_cost_per_million = 0.014; // USD per million PUT payload units
                    let put_cost = (million_put_payload_units / 25.0) * put_cost_per_million;

                    analysis.push_str(&format!("## Data Transfer Costs\n"));
                    analysis.push_str(&format!(
                        "- Estimated Monthly Data Transfer: {:.2} GB\n",
                        avg_bytes_per_month / (1024.0 * 1024.0 * 1024.0)
                    ));
                    analysis.push_str(&format!("- Estimated PUT Cost: ${:.2}\n\n", put_cost));

                    analysis.push_str(&format!("## Total Estimated Monthly Cost\n"));
                    analysis.push_str(&format!("- Shard Cost: ${:.2}\n", total_monthly_cost));
                    analysis.push_str(&format!("- PUT Cost: ${:.2}\n", put_cost));
                    analysis.push_str(&format!(
                        "- Total: ${:.2}\n\n",
                        total_monthly_cost + put_cost
                    ));
                }

                // Add cost optimization recommendations
                analysis.push_str("## Cost Optimization Recommendations\n");

                if shard_count > 1 {
                    // Check if shards are underutilized
                    if let Some(incoming_records) =
                        MetricsAnalyzer::find_metric(metrics, "IncomingRecords")
                    {
                        let (avg_records, max_records) =
                            MetricsAnalyzer::calculate_statistics(&incoming_records.datapoints);

                        // 1000 records per second per shard is a common threshold
                        let max_capacity = shard_count as f64 * 1000.0;
                        let utilization_pct = (max_records / max_capacity) * 100.0;

                        if utilization_pct < 50.0 {
                            analysis.push_str("1. **Consider Reducing Shard Count**\n");
                            analysis.push_str(&format!(
                                "   - Current peak utilization: {:.1} % of capacity\n",
                                utilization_pct
                            ));
                            analysis.push_str(&format!(
                                "   - Potential savings: ${:.2} per month by reducing shards\n\n",
                                total_monthly_cost * 0.5
                            ));
                        }
                    }
                }

                analysis.push_str("2. **Evaluate Enhanced Fan-Out Necessity**\n");
                analysis.push_str("   - Enhanced Fan-Out costs $0.015 per consumer-shard hour\n");
                analysis
                    .push_str("   - Only use for applications requiring dedicated throughput\n\n");

                analysis.push_str("3. **Consider Kinesis Analytics**\n");
                analysis.push_str("   - For real-time analytics, Kinesis Analytics may be more cost-effective than maintaining multiple consumers\n\n");

                analysis.push_str("4. **Review Data Retention Period**\n");
                analysis.push_str(
                    "   - Extended retention beyond 24 hours costs $0.02 per shard per hour\n",
                );
                analysis.push_str("   - Only extend retention if absolutely necessary\n");
            }
            _ => {
                return Err(AppError::BadRequest(
                    "Unsupported workflow type for Kinesis Stream".to_string(),
                ))
            }
        }

        Ok(analysis)
    }
}
