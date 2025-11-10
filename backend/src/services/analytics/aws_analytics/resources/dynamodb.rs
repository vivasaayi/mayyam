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


use crate::errors::AppError;
use crate::models::aws_resource;
use crate::services::analytics::aws_analytics::metrics::MetricsAnalyzer;
use crate::services::analytics::aws_analytics::models::resource_workflows::ResourceAnalysisWorkflow;
use crate::services::aws::aws_types::cloud_watch;

pub struct DynamoDbAnalyzer;

impl DynamoDbAnalyzer {
    pub async fn analyze_dynamodb_table(
        resource: &aws_resource::Model,
        workflow: &ResourceAnalysisWorkflow,
        metrics: &cloud_watch::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut analysis = String::new();

        match workflow {
            ResourceAnalysisWorkflow::Performance => {
                analysis.push_str("# DynamoDB Table Performance Analysis\n\n");

                // Analyze read/write capacity
                if let Some(read_metric) =
                    MetricsAnalyzer::find_metric(metrics, "ConsumedReadCapacityUnits")
                {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&read_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Read Capacity Usage\n- Average: {:.2} RCUs\n- Peak: {:.2} RCUs\n\n",
                        avg, max
                    ));
                }

                // Add write capacity analysis
                if let Some(write_metric) =
                    MetricsAnalyzer::find_metric(metrics, "ConsumedWriteCapacityUnits")
                {
                    let (avg, max) =
                        MetricsAnalyzer::calculate_statistics(&write_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Write Capacity Usage\n- Average: {:.2} WCUs\n- Peak: {:.2} WCUs\n\n",
                        avg, max
                    ));
                }

                // Add throttling events analysis
                if let Some(read_throttle_metric) =
                    MetricsAnalyzer::find_metric(metrics, "ReadThrottleEvents")
                {
                    let (_, max) =
                        MetricsAnalyzer::calculate_statistics(&read_throttle_metric.datapoints);
                    if max > 0.0 {
                        analysis.push_str("⚠️ **Read Throttling Detected**\n");
                        analysis.push_str(
                            "Consider increasing read capacity or implementing retry logic.\n\n",
                        );
                    }
                }

                if let Some(write_throttle_metric) =
                    MetricsAnalyzer::find_metric(metrics, "WriteThrottleEvents")
                {
                    let (_, max) =
                        MetricsAnalyzer::calculate_statistics(&write_throttle_metric.datapoints);
                    if max > 0.0 {
                        analysis.push_str("⚠️ **Write Throttling Detected**\n");
                        analysis.push_str(
                            "Consider increasing write capacity or implementing retry logic.\n\n",
                        );
                    }
                }

                // Add performance recommendations
                analysis.push_str("## Performance Recommendations\n\n");
                analysis.push_str("1. **Optimize Capacity Planning**: ");
                analysis.push_str("Ensure your table's read and write capacity units are properly sized for your workload\n");
                analysis.push_str("2. **Consider On-Demand Mode**: ");
                analysis.push_str("For variable workloads, on-demand capacity mode can provide flexibility and cost optimization\n");
                analysis.push_str("3. **Monitor Throttling**: ");
                analysis.push_str("Set up CloudWatch alarms for throttling events to be notified when capacity limits are reached\n");
                analysis.push_str("4. **Implement Backoff Strategy**: ");
                analysis.push_str("Use exponential backoff in your application when handling throttling exceptions\n");
            }
            ResourceAnalysisWorkflow::Cost => {
                analysis.push_str("# DynamoDB Cost Analysis\n\n");

                // Get read/write capacity for cost estimation
                let read_avg = if let Some(read_metric) =
                    MetricsAnalyzer::find_metric(metrics, "ConsumedReadCapacityUnits")
                {
                    let (avg, _) = MetricsAnalyzer::calculate_statistics(&read_metric.datapoints);
                    avg
                } else {
                    0.0
                };

                let write_avg = if let Some(write_metric) =
                    MetricsAnalyzer::find_metric(metrics, "ConsumedWriteCapacityUnits")
                {
                    let (avg, _) = MetricsAnalyzer::calculate_statistics(&write_metric.datapoints);
                    avg
                } else {
                    0.0
                };

                // Simple cost calculation (very approximate)
                let hourly_read_cost = read_avg * 0.00013 / 100.0; // $0.00013 per RCU-hour / 100 RCUs
                let hourly_write_cost = write_avg * 0.00065 / 100.0; // $0.00065 per WCU-hour / 100 WCUs
                let monthly_cost = (hourly_read_cost + hourly_write_cost) * 24.0 * 30.0;

                analysis.push_str("## Current Usage\n\n");
                analysis.push_str(&format!("- Average Read Capacity: {:.2} RCUs\n", read_avg));
                analysis.push_str(&format!(
                    "- Average Write Capacity: {:.2} WCUs\n",
                    write_avg
                ));
                analysis.push_str(&format!(
                    "- Estimated Monthly Cost: ${:.2}\n\n",
                    monthly_cost
                ));

                analysis.push_str("## Cost Optimization Recommendations\n\n");
                analysis.push_str("1. **Review Capacity Mode**: ");
                analysis.push_str(
                    "Compare on-demand vs. provisioned capacity for your usage pattern\n",
                );
                analysis.push_str("2. **Reserved Capacity**: ");
                analysis.push_str(
                    "For stable workloads, purchase reserved capacity to save up to 50%\n",
                );
                analysis.push_str("3. **Data Lifecycle Management**: ");
                analysis.push_str(
                    "Implement TTL to automatically remove old data and reduce storage costs\n",
                );
                analysis.push_str("4. **Table Design**: ");
                analysis.push_str(
                    "Consider single-table design to reduce the number of tables needed\n",
                );
            }
            ResourceAnalysisWorkflow::FiveWhy => {
                analysis.push_str("# DynamoDB Table 5-Why Analysis\n\n");
                analysis.push_str("## Initial Problem Statement\n\n");

                // Detect potential issues based on metrics
                let mut has_issue = false;

                // Check for read throttling
                if let Some(read_throttle_metric) =
                    MetricsAnalyzer::find_metric(metrics, "ReadThrottleEvents")
                {
                    let (_, max) =
                        MetricsAnalyzer::calculate_statistics(&read_throttle_metric.datapoints);
                    if max > 0.0 {
                        has_issue = true;
                        analysis.push_str("**Problem**: The DynamoDB table is experiencing read throttling events.\n\n");

                        analysis.push_str("### Why #1: Why are there read throttling events?\n\n");
                        analysis.push_str("The table's provisioned read capacity is being exceeded by the actual request volume.\n\n");

                        analysis.push_str(
                            "### Why #2: Why is the provisioned capacity being exceeded?\n\n",
                        );
                        analysis.push_str("Either the workload has increased unexpectedly, or the capacity was not properly sized for the existing workload.\n\n");

                        analysis
                            .push_str("### Why #3: Why wasn't the capacity properly sized?\n\n");
                        analysis.push_str("Capacity planning might be based on historical patterns that don't reflect current usage, or auto-scaling isn't configured optimally.\n\n");

                        analysis.push_str("### Why #4: Why don't the scaling policies match current usage patterns?\n\n");
                        analysis.push_str("Usage patterns may have changed due to new features, increased user activity, or periodic events not accounted for in scaling policies.\n\n");

                        analysis.push_str(
                            "### Why #5: Why weren't these usage pattern changes anticipated?\n\n",
                        );
                        analysis.push_str("There may be insufficient monitoring of usage trends, lack of communication between development and operations teams about new features, or inadequate load testing before feature releases.\n\n");

                        analysis.push_str("## Root Cause\n\n");
                        analysis.push_str("The root cause appears to be a gap in the capacity planning process that doesn't account for changing usage patterns and doesn't include proactive monitoring to detect and adjust to these changes before they cause throttling.\n\n");

                        analysis.push_str("## Recommendations\n\n");
                        analysis.push_str("1. **Implement Better Monitoring**: Set up alerts for capacity utilization trends, not just threshold breaches\n");
                        analysis.push_str("2. **Improve Auto-scaling Configuration**: Adjust target utilization and scaling policies based on recent usage patterns\n");
                        analysis.push_str("3. **Consider On-demand Capacity Mode**: For highly variable workloads, this can eliminate the need for capacity planning\n");
                        analysis.push_str("4. **Establish Process Controls**: Create a pre-release check for database capacity impact for new features\n");
                        analysis.push_str("5. **Regular Capacity Reviews**: Schedule monthly reviews of capacity needs against actual usage\n");
                    }
                }

                // Check for write throttling if no read throttling was found
                if !has_issue {
                    if let Some(write_throttle_metric) =
                        MetricsAnalyzer::find_metric(metrics, "WriteThrottleEvents")
                    {
                        let (_, max) = MetricsAnalyzer::calculate_statistics(
                            &write_throttle_metric.datapoints,
                        );
                        if max > 0.0 {
                            has_issue = true;
                            analysis.push_str("**Problem**: The DynamoDB table is experiencing write throttling events.\n\n");

                            analysis
                                .push_str("### Why #1: Why are there write throttling events?\n\n");
                            analysis.push_str("The table's provisioned write capacity is being exceeded by the actual write volume.\n\n");

                            analysis.push_str(
                                "### Why #2: Why is the provisioned capacity being exceeded?\n\n",
                            );
                            analysis.push_str("There may be write spikes during certain operations, or the write patterns could be creating hot partitions.\n\n");

                            analysis.push_str(
                                "### Why #3: Why are there write spikes or hot partitions?\n\n",
                            );
                            analysis.push_str("Write spikes could be due to batch operations, while hot partitions usually result from suboptimal partition key design.\n\n");

                            analysis.push_str("### Why #4: Why isn't the table designed to handle these write patterns?\n\n");
                            analysis.push_str("The table may have been designed without a complete understanding of write access patterns, or the patterns may have changed as the application evolved.\n\n");

                            analysis.push_str("### Why #5: Why wasn't the table design updated as patterns changed?\n\n");
                            analysis.push_str("There may be insufficient monitoring of changing access patterns, reluctance to modify a live production table, or lack of procedures to regularly review and optimize table design.\n\n");

                            analysis.push_str("## Root Cause\n\n");
                            analysis.push_str("The root cause appears to be insufficient ongoing optimization of the table design and capacity allocation in response to changing write patterns, possibly exacerbated by a lack of write sharding or batching strategies.\n\n");

                            analysis.push_str("## Recommendations\n\n");
                            analysis.push_str("1. **Implement Write Sharding**: Distribute writes evenly across partition keys to avoid hot partitions\n");
                            analysis.push_str("2. **Batch Write Operations**: Group write operations to reduce the number of requests\n");
                            analysis.push_str("3. **Consider Table Redesign**: Evaluate if the current partition key design matches actual access patterns\n");
                            analysis.push_str("4. **Adjust Auto-scaling**: Configure auto-scaling to anticipate and handle write spikes\n");
                            analysis.push_str("5. **Implement Queue-based Architecture**: For high-volume writes, consider using SQS to buffer write operations\n");
                        }
                    }
                }

                // If no specific issues were found, provide a generic 5-why analysis
                if !has_issue {
                    analysis.push_str(
                        "**Problem**: The DynamoDB table may be performing suboptimally.\n\n",
                    );

                    analysis.push_str(
                        "### Why #1: Why might the table be performing suboptimally?\n\n",
                    );
                    analysis.push_str("There could be inefficient access patterns, suboptimal table design, or improper capacity allocation.\n\n");

                    analysis.push_str(
                        "### Why #2: Why would the access patterns or design be inefficient?\n\n",
                    );
                    analysis.push_str("Access patterns may not align with the table's primary key design, or expensive operations like table scans might be used instead of queries.\n\n");

                    analysis.push_str(
                        "### Why #3: Why aren't access patterns aligned with table design?\n\n",
                    );
                    analysis.push_str("The application requirements may have evolved since the initial table design, or the designers may not have anticipated all access patterns.\n\n");

                    analysis.push_str("### Why #4: Why wasn't the table design updated as requirements changed?\n\n");
                    analysis.push_str("Changing a table's primary key structure often requires data migration, which can be complex and risky in a production environment.\n\n");

                    analysis.push_str("### Why #5: Why isn't there a process for safely evolving the database schema?\n\n");
                    analysis.push_str("Organizations often lack well-defined processes for database schema evolution, or may prioritize new features over database optimization.\n\n");

                    analysis.push_str("## Root Cause\n\n");
                    analysis.push_str("The root cause appears to be a gap in the database lifecycle management process that doesn't adequately account for evolving access patterns and doesn't include regular performance reviews and optimization.\n\n");

                    analysis.push_str("## Recommendations\n\n");
                    analysis.push_str("1. **Regular Performance Reviews**: Schedule quarterly performance reviews of DynamoDB tables\n");
                    analysis.push_str("2. **Access Pattern Documentation**: Maintain documentation of all access patterns and update when new ones are added\n");
                    analysis.push_str("3. **Establish Schema Evolution Process**: Create a formal process for evaluating and implementing schema changes\n");
                    analysis.push_str("4. **Implement Performance Testing**: Include database performance in CI/CD pipelines\n");
                    analysis.push_str("5. **Consider NoSQL Design Patterns**: Evaluate single-table design and other NoSQL patterns for complex applications\n");
                }
            }
            _ => {
                return Err(AppError::BadRequest(
                    "Unsupported workflow type for DynamoDB table".to_string(),
                ))
            }
        }

        Ok(analysis)
    }

    pub async fn answer_dynamodb_question(
        resource: &aws_resource::Model,
        question: &str,
        metrics: &cloud_watch::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut answer = String::new();

        // Convert question to lowercase for easier matching
        let question = question.to_lowercase();

        // Check if this is a question about read capacity
        if question.contains("read") || question.contains("rcu") {
            if let Some(read_metric) =
                MetricsAnalyzer::find_metric(metrics, "ConsumedReadCapacityUnits")
            {
                let (avg, max) = MetricsAnalyzer::calculate_statistics(&read_metric.datapoints);
                answer.push_str(&format!(
                    "## Read Capacity Analysis\n\n- Average: {:.2} RCUs\n- Peak: {:.2} RCUs\n\n",
                    avg, max
                ));

                // Add capacity planning recommendations
                if max > avg * 2.0 {
                    answer.push_str("### Observations\n\n");
                    answer.push_str("- Your table shows significant spikes in read activity\n");
                    answer.push_str("- Peak usage is more than double the average consumption\n\n");

                    answer.push_str("### Recommendations\n\n");
                    answer.push_str("1. **Consider On-Demand Capacity Mode**: With your variable workload pattern, on-demand capacity might be more cost-effective\n");
                    answer.push_str("2. **Implement Auto-Scaling**: If using provisioned capacity, set up auto-scaling to handle these spikes\n");
                    answer.push_str("3. **Add Read Replicas**: For frequent read operations, consider adding global tables for regional replication\n");
                } else {
                    answer.push_str("### Observations\n\n");
                    answer.push_str("- Your read usage pattern appears relatively stable\n");
                    answer.push_str(
                        "- The difference between peak and average usage is not extreme\n\n",
                    );

                    answer.push_str("### Recommendations\n\n");
                    answer.push_str("1. **Optimize Provisioned Capacity**: Consider setting provisioned capacity slightly above your average usage\n");
                    answer.push_str("2. **Implement Caching**: Add DAX (DynamoDB Accelerator) for frequently accessed items\n");
                    answer.push_str("3. **Review Query Patterns**: Ensure your access patterns are optimized for your table design\n");
                }
            }
        }
        // Check if this is a question about write capacity
        else if question.contains("write") || question.contains("wcu") {
            if let Some(write_metric) =
                MetricsAnalyzer::find_metric(metrics, "ConsumedWriteCapacityUnits")
            {
                let (avg, max) = MetricsAnalyzer::calculate_statistics(&write_metric.datapoints);
                answer.push_str(&format!(
                    "## Write Capacity Analysis\n\n- Average: {:.2} WCUs\n- Peak: {:.2} WCUs\n\n",
                    avg, max
                ));

                // Add capacity planning recommendations
                if max > avg * 2.0 {
                    answer.push_str("### Observations\n\n");
                    answer.push_str("- Your table shows significant spikes in write activity\n");
                    answer.push_str("- Peak usage is more than double the average consumption\n\n");

                    answer.push_str("### Recommendations\n\n");
                    answer.push_str("1. **Consider On-Demand Capacity Mode**: With your variable workload pattern, on-demand capacity might be more cost-effective\n");
                    answer.push_str("2. **Implement Write Sharding**: Distribute writes across multiple partition keys to avoid hot partitions\n");
                    answer.push_str("3. **Use Batching**: Group multiple write operations to reduce the number of API calls\n");
                } else {
                    answer.push_str("### Observations\n\n");
                    answer.push_str("- Your write usage pattern appears relatively stable\n");
                    answer.push_str(
                        "- The difference between peak and average usage is not extreme\n\n",
                    );

                    answer.push_str("### Recommendations\n\n");
                    answer.push_str("1. **Optimize Provisioned Capacity**: Consider setting provisioned capacity slightly above your average usage\n");
                    answer.push_str("2. **Review Write Patterns**: Ensure you're not performing unnecessary updates\n");
                    answer.push_str("3. **Consider Time-To-Live (TTL)**: Automatically expire old data to reduce storage and backup costs\n");
                }
            }
        }
        // Check if this is a question about throttling
        else if question.contains("throttle") || question.contains("throttling") {
            let mut has_read_throttling = false;
            let mut has_write_throttling = false;

            if let Some(read_throttle_metric) =
                MetricsAnalyzer::find_metric(metrics, "ReadThrottleEvents")
            {
                let (avg, max) =
                    MetricsAnalyzer::calculate_statistics(&read_throttle_metric.datapoints);
                if max > 0.0 {
                    has_read_throttling = true;
                    answer.push_str("## Read Throttling Analysis\n\n");
                    answer.push_str(&format!(
                        "- Average Throttled Events: {:.2} per minute\n",
                        avg
                    ));
                    answer.push_str(&format!(
                        "- Peak Throttled Events: {:.2} per minute\n\n",
                        max
                    ));

                    answer.push_str("### Recommendations to Reduce Read Throttling\n\n");
                    answer.push_str("1. **Increase Read Capacity**: Consider increasing provisioned RCUs or switching to on-demand\n");
                    answer.push_str("2. **Implement Exponential Backoff**: Add retry logic with backoff in your application\n");
                    answer.push_str(
                        "3. **Add Caching**: Implement DAX or application-level caching\n",
                    );
                    answer.push_str("4. **Distribute Reads**: Review your access patterns to prevent hot partitions\n\n");
                }
            }

            if let Some(write_throttle_metric) =
                MetricsAnalyzer::find_metric(metrics, "WriteThrottleEvents")
            {
                let (avg, max) =
                    MetricsAnalyzer::calculate_statistics(&write_throttle_metric.datapoints);
                if max > 0.0 {
                    has_write_throttling = true;
                    answer.push_str("## Write Throttling Analysis\n\n");
                    answer.push_str(&format!(
                        "- Average Throttled Events: {:.2} per minute\n",
                        avg
                    ));
                    answer.push_str(&format!(
                        "- Peak Throttled Events: {:.2} per minute\n\n",
                        max
                    ));

                    answer.push_str("### Recommendations to Reduce Write Throttling\n\n");
                    answer.push_str("1. **Increase Write Capacity**: Consider increasing provisioned WCUs or switching to on-demand\n");
                    answer.push_str("2. **Implement Write Sharding**: Distribute writes across multiple partition keys\n");
                    answer.push_str("3. **Batch Write Operations**: Group write operations to utilize capacity more efficiently\n");
                    answer.push_str("4. **Add Queuing**: Implement SQS to buffer write operations during peak periods\n\n");
                }
            }

            if !has_read_throttling && !has_write_throttling {
                answer.push_str("## Throttling Analysis\n\n");
                answer.push_str("Good news! No throttling events were detected in the analyzed time period.\n\n");

                answer.push_str("### Preventive Recommendations\n\n");
                answer.push_str(
                    "1. **Monitor Closely**: Set up CloudWatch alarms for throttling metrics\n",
                );
                answer.push_str("2. **Implement Auto-Scaling**: Proactively adjust capacity based on usage patterns\n");
                answer.push_str("3. **Load Testing**: Perform load tests to identify potential bottlenecks before they affect production\n");
            }
        }
        // Check if this is a question about performance
        else if question.contains("performance") || question.contains("slow") {
            answer.push_str("## DynamoDB Performance Analysis\n\n");

            // Check for latency metrics
            if let Some(latency_metric) =
                MetricsAnalyzer::find_metric(metrics, "SuccessfulRequestLatency")
            {
                let (avg, max) = MetricsAnalyzer::calculate_statistics(&latency_metric.datapoints);
                answer.push_str(&format!("- Average Request Latency: {:.2} ms\n", avg));
                answer.push_str(&format!("- Peak Request Latency: {:.2} ms\n\n", max));

                if max > 100.0 {
                    // Arbitrary threshold for demonstration
                    answer.push_str("⚠️ **High latency detected in some operations**\n\n");
                }
            }

            answer.push_str("### Performance Optimization Recommendations\n\n");
            answer.push_str("1. **Review Table Design**:\n");
            answer.push_str("   - Ensure partition keys distribute workload evenly\n");
            answer.push_str("   - Add sparse indexes for query optimization\n");
            answer.push_str("   - Consider single-table design for related entities\n\n");

            answer.push_str("2. **Query Optimization**:\n");
            answer.push_str("   - Use projections to return only needed attributes\n");
            answer.push_str("   - Implement pagination for large result sets\n");
            answer.push_str("   - Avoid scans in favor of queries\n\n");

            answer.push_str("3. **Consider Infrastructure Improvements**:\n");
            answer.push_str("   - Use DAX for microsecond read latency\n");
            answer.push_str("   - Ensure your application is in the same region as the table\n");
            answer.push_str("   - For global users, consider global tables\n");
        }
        // Check if this is a question about cost
        else if question.contains("cost")
            || question.contains("pricing")
            || question.contains("expensive")
        {
            answer.push_str("## DynamoDB Cost Analysis\n\n");

            // Get read/write capacity for cost estimation
            let read_avg = if let Some(read_metric) =
                MetricsAnalyzer::find_metric(metrics, "ConsumedReadCapacityUnits")
            {
                let (avg, _) = MetricsAnalyzer::calculate_statistics(&read_metric.datapoints);
                avg
            } else {
                0.0
            };

            let write_avg = if let Some(write_metric) =
                MetricsAnalyzer::find_metric(metrics, "ConsumedWriteCapacityUnits")
            {
                let (avg, _) = MetricsAnalyzer::calculate_statistics(&write_metric.datapoints);
                avg
            } else {
                0.0
            };

            // Simple cost calculation (very approximate)
            let hourly_read_cost = read_avg * 0.00013 / 100.0; // $0.00013 per RCU-hour / 100 RCUs
            let hourly_write_cost = write_avg * 0.00065 / 100.0; // $0.00065 per WCU-hour / 100 WCUs
            let monthly_cost = (hourly_read_cost + hourly_write_cost) * 24.0 * 30.0;

            answer.push_str("### Estimated Monthly Costs\n\n");
            answer.push_str(&format!("- Average Read Capacity: {:.2} RCUs\n", read_avg));
            answer.push_str(&format!(
                "- Average Write Capacity: {:.2} WCUs\n",
                write_avg
            ));
            answer.push_str(&format!(
                "- Estimated Monthly Cost: ${:.2}\n\n",
                monthly_cost
            ));

            answer.push_str("### Cost Optimization Recommendations\n\n");
            answer.push_str("1. **Consider Capacity Mode**:\n");
            answer.push_str(
                "   - On-Demand: Best for unpredictable workloads with significant variation\n",
            );
            answer.push_str(
                "   - Provisioned: More cost-effective for stable, predictable workloads\n\n",
            );

            answer.push_str("2. **Reserved Capacity**:\n");
            answer.push_str("   - Purchase reserved capacity for long-term consistent usage\n");
            answer.push_str("   - Can save up to 50% compared to standard provisioned pricing\n\n");

            answer.push_str("3. **Data Management**:\n");
            answer.push_str("   - Implement TTL to automatically remove old data\n");
            answer.push_str("   - Use compression for large attribute values\n");
            answer.push_str("   - Store infrequently accessed attributes in S3\n\n");

            answer.push_str("4. **Table Operations**:\n");
            answer.push_str("   - Use efficient queries instead of scans\n");
            answer.push_str("   - Batch operations where possible\n");
            answer.push_str(
                "   - Consider DynamoDB Streams with Lambda for event-driven architectures\n",
            );
        }

        // If no specific question was matched, provide a general overview
        if answer.is_empty() {
            answer.push_str("## DynamoDB Table Overview\n\n");
            answer.push_str(&format!("- Table Name: {}\n", resource.resource_id));
            answer.push_str(&format!("- Region: {}\n\n", resource.region));

            answer.push_str("I can answer specific questions about your DynamoDB table's:\n\n");
            answer.push_str("- Read and write capacity usage\n");
            answer.push_str("- Performance optimization\n");
            answer.push_str("- Throttling events and remediation\n");
            answer.push_str("- Cost analysis and optimization\n");
            answer.push_str("- Table design best practices\n\n");

            answer.push_str("Try asking a more specific question about one of these areas.");
        }

        Ok(answer)
    }
}
