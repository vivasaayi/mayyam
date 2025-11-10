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

pub struct S3Analyzer;

impl S3Analyzer {
    pub async fn analyze_s3_bucket(
        resource: &aws_resource::Model,
        workflow: &ResourceAnalysisWorkflow,
        metrics: &cloud_watch::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut analysis = String::new();

        match workflow {
            ResourceAnalysisWorkflow::Cost => {
                analysis.push_str("# S3 Bucket Cost Analysis\n\n");

                // Add bucket info
                analysis.push_str(&format!("## Bucket Information\n"));
                analysis.push_str(&format!("- Bucket: {}\n", resource.resource_id));
                analysis.push_str(&format!("- ARN: {}\n", resource.arn));
                analysis.push_str(&format!("- Region: {}\n\n", resource.region));

                // Storage size analysis
                if let Some(size_metric) = MetricsAnalyzer::find_metric(metrics, "BucketSizeBytes")
                {
                    let (avg, _) = MetricsAnalyzer::calculate_statistics(&size_metric.datapoints);
                    let size_gb = avg / (1024.0 * 1024.0 * 1024.0);

                    analysis.push_str(&format!("## Storage Usage\n"));
                    analysis.push_str(&format!("- Total Size: {:.2} GB\n\n", size_gb));

                    // Calculate estimated costs
                    let monthly_cost_per_gb = match resource.region.as_str() {
                        "us-east-1" => 0.023, // USD per GB per month for standard storage
                        "us-west-2" => 0.023,
                        "eu-west-1" => 0.024,
                        _ => 0.025, // Average cost for other regions
                    };

                    let monthly_cost = size_gb * monthly_cost_per_gb;

                    analysis.push_str(&format!("## Cost Analysis\n"));
                    analysis.push_str(&format!(
                        "- Storage Cost per GB: ${:.3}/month\n",
                        monthly_cost_per_gb
                    ));
                    analysis.push_str(&format!(
                        "- Estimated Monthly Storage Cost: ${:.2}\n\n",
                        monthly_cost
                    ));

                    // Add cost optimization recommendations
                    analysis.push_str("## Cost Optimization Recommendations\n");

                    // Lifecycle policy recommendations
                    analysis.push_str("1. **Implement Lifecycle Policies**\n");
                    analysis.push_str(&format!("   - Move infrequently accessed data to S3 Standard-IA (${:.3}/GB/month)\n", monthly_cost_per_gb * 0.5));
                    analysis.push_str(&format!(
                        "   - Archive older data to Glacier (${:.3}/GB/month)\n",
                        monthly_cost_per_gb * 0.2
                    ));
                    analysis.push_str(&format!(
                        "   - Potential savings: up to ${:.2}/month\n\n",
                        monthly_cost * 0.7
                    ));

                    // Object expiration recommendations
                    analysis.push_str("2. **Set Up Object Expiration**\n");
                    analysis
                        .push_str("   - Automatically delete objects that are no longer needed\n");
                    analysis
                        .push_str("   - Consider setting expiration rules for temporary data\n\n");

                    // Storage class analysis
                    analysis.push_str("3. **Use S3 Storage Class Analysis**\n");
                    analysis.push_str("   - Identify optimal storage class for your data\n");
                    analysis.push_str("   - Automate transitions based on access patterns\n\n");

                    // Compression recommendations
                    analysis.push_str("4. **Compress Large Objects**\n");
                    analysis.push_str(
                        "   - Use compression for text files, logs, and other compressible data\n",
                    );
                    analysis
                        .push_str("   - Potential space savings: 40-60% for text-based files\n");
                }

                // Request analysis
                if let Some(requests_metric) = MetricsAnalyzer::find_metric(metrics, "AllRequests")
                {
                    let (avg, _) =
                        MetricsAnalyzer::calculate_statistics(&requests_metric.datapoints);
                    let requests_per_month = avg * 60.0 * 60.0 * 24.0 * 30.0;

                    // Calculate request costs (approximate)
                    let get_cost_per_1000 = 0.0004; // USD per 1000 GET requests
                    let put_cost_per_1000 = 0.005; // USD per 1000 PUT/POST/LIST requests

                    // Assume 80% GET, 20% PUT/POST/LIST
                    let get_requests = requests_per_month * 0.8;
                    let put_requests = requests_per_month * 0.2;

                    let get_cost = (get_requests / 1000.0) * get_cost_per_1000;
                    let put_cost = (put_requests / 1000.0) * put_cost_per_1000;
                    let total_request_cost = get_cost + put_cost;

                    analysis.push_str(&format!("## Request Costs\n"));
                    analysis.push_str(&format!(
                        "- Estimated Monthly Requests: {:.0}\n",
                        requests_per_month
                    ));
                    analysis.push_str(&format!("- Estimated GET Request Cost: ${:.2}\n", get_cost));
                    analysis.push_str(&format!(
                        "- Estimated PUT/POST/LIST Request Cost: ${:.2}\n",
                        put_cost
                    ));
                    analysis.push_str(&format!(
                        "- Total Request Cost: ${:.2}\n\n",
                        total_request_cost
                    ));

                    // Add request optimization recommendations
                    analysis.push_str("## Request Optimization Recommendations\n");
                    analysis.push_str("1. **Use CloudFront for Frequently Accessed Content**\n");
                    analysis.push_str("   - Reduce GET request costs and improve performance\n\n");

                    analysis.push_str("2. **Batch Small Objects**\n");
                    analysis.push_str("   - Combine small files to reduce request counts\n\n");

                    analysis.push_str("3. **Optimize List Operations**\n");
                    analysis.push_str("   - Use prefixes and delimiters to limit list results\n");
                    analysis.push_str("   - Cache list results when possible\n");
                }
            }
            ResourceAnalysisWorkflow::Storage => {
                analysis.push_str("# S3 Bucket Storage Analysis\n\n");

                // Add bucket info
                analysis.push_str(&format!("## Bucket Information\n"));
                analysis.push_str(&format!("- Bucket: {}\n", resource.resource_id));
                analysis.push_str(&format!("- ARN: {}\n", resource.arn));
                analysis.push_str(&format!("- Region: {}\n\n", resource.region));

                // Storage size analysis
                if let Some(size_metric) = MetricsAnalyzer::find_metric(metrics, "BucketSizeBytes")
                {
                    let (avg, _) = MetricsAnalyzer::calculate_statistics(&size_metric.datapoints);
                    let size_gb = avg / (1024.0 * 1024.0 * 1024.0);

                    analysis.push_str(&format!("## Storage Usage\n"));
                    analysis.push_str(&format!("- Total Size: {:.2} GB\n\n", size_gb));
                }

                // Object count analysis
                if let Some(count_metric) = MetricsAnalyzer::find_metric(metrics, "NumberOfObjects")
                {
                    let (avg, _) = MetricsAnalyzer::calculate_statistics(&count_metric.datapoints);

                    analysis.push_str(&format!("## Object Count\n"));
                    analysis.push_str(&format!("- Total Objects: {:.0}\n\n", avg));

                    // Calculate average object size if both metrics are available
                    if let Some(size_metric) =
                        MetricsAnalyzer::find_metric(metrics, "BucketSizeBytes")
                    {
                        let (size_avg, _) =
                            MetricsAnalyzer::calculate_statistics(&size_metric.datapoints);
                        if avg > 0.0 {
                            let avg_object_size = size_avg / avg;

                            analysis.push_str(&format!("## Object Size Distribution\n"));
                            analysis.push_str(&format!(
                                "- Average Object Size: {:.2} KB\n\n",
                                avg_object_size / 1024.0
                            ));

                            // Add recommendations based on average object size
                            if avg_object_size < 128.0 * 1024.0 {
                                // Less than 128KB
                                analysis.push_str("⚠️ **Small Object Size Detected**\n");
                                analysis.push_str("Consider:\n");
                                analysis
                                    .push_str("1. Batching small objects into larger archives\n");
                                analysis.push_str(
                                    "2. Using S3 Inventory to analyze object size distribution\n",
                                );
                                analysis.push_str(
                                    "3. Implementing object compression for text-based files\n\n",
                                );
                            }
                        }
                    }
                }

                // Storage class recommendations
                analysis.push_str("## Storage Class Recommendations\n");
                analysis.push_str("1. **Standard Storage**: For frequently accessed data\n");
                analysis.push_str("2. **Standard-IA**: For data accessed less than once a month\n");
                analysis.push_str("3. **Glacier**: For archival data rarely accessed\n");
                analysis.push_str(
                    "4. **Intelligent-Tiering**: For data with changing access patterns\n\n",
                );

                analysis.push_str("## Storage Optimization Recommendations\n");
                analysis.push_str("1. **Implement Lifecycle Policies**\n");
                analysis
                    .push_str("   - Automatically transition objects between storage classes\n");
                analysis.push_str("   - Set up expiration rules for temporary data\n\n");

                analysis.push_str("2. **Enable Versioning Selectively**\n");
                analysis.push_str("   - Use versioning for critical data only\n");
                analysis.push_str("   - Set up lifecycle rules to expire old versions\n\n");

                analysis.push_str("3. **Use Compression**\n");
                analysis.push_str("   - Compress text files, logs, and other compressible data\n");
                analysis.push_str("   - Consider server-side compression before upload\n\n");

                analysis.push_str("4. **Analyze Access Patterns**\n");
                analysis.push_str("   - Use S3 Analytics to identify optimal storage classes\n");
                analysis.push_str("   - Review CloudWatch metrics for access frequency\n");
            }
            _ => {
                return Err(AppError::BadRequest(format!(
                    "Unsupported workflow for S3: {:?}",
                    workflow
                )))
            }
        }

        Ok(analysis)
    }
}
