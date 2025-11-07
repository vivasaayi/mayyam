use crate::errors::AppError;
use crate::models::aws_resource;
use crate::services::analytics::aws_analytics::metrics::MetricsAnalyzer;
use crate::services::analytics::aws_analytics::models::resource_workflows::ResourceAnalysisWorkflow;
use crate::services::aws::aws_types::cloud_watch;

pub struct Ec2Analyzer;

impl Ec2Analyzer {
    pub async fn analyze_ec2_instance(
        _resource: &aws_resource::Model,
        workflow: &ResourceAnalysisWorkflow,
        metrics: &cloud_watch::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut analysis = String::new();

        match workflow {
            ResourceAnalysisWorkflow::Performance => {
                analysis.push_str("# EC2 Instance Performance Analysis\n\n");

                // Analyze CPU metrics
                if let Some(cpu_metric) = MetricsAnalyzer::find_metric(metrics, "CPUUtilization") {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&cpu_metric.datapoints);
                    analysis.push_str(&format!(
                        "## CPU Utilization\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                        avg, max
                    ));

                    // Add recommendations based on CPU usage
                    if max > 80.0 {
                        analysis.push_str("⚠️ **High CPU Usage Detected**\n");
                        analysis.push_str("Consider:\n");
                        analysis.push_str("1. Scaling up the instance type\n");
                        analysis.push_str("2. Using auto-scaling groups\n");
                        analysis.push_str("3. Analyzing resource-intensive processes\n\n");
                    }
                }

                // Analyze memory metrics if available
                if let Some(mem_metric) = MetricsAnalyzer::find_metric(metrics, "MemoryUtilization")
                {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&mem_metric.datapoints);
                    analysis.push_str(&format!(
                        "## Memory Utilization\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                        avg, max
                    ));
                }

                // Network analysis
                MetricsAnalyzer::analyze_network_metrics(&mut analysis, metrics);
            }
            ResourceAnalysisWorkflow::Cost => {
                analysis.push_str("# EC2 Instance Cost Analysis\n\n");
                // Implement cost analysis...
                analysis.push_str("Cost analysis not yet implemented\n");
            }
            ResourceAnalysisWorkflow::FiveWhy => {
                analysis.push_str("# 5 Why Analysis for EC2 Instance\n\n");

                analysis.push_str("## What is 5 Why Analysis?\n");
                analysis.push_str("The 5 Why technique is an iterative interrogative technique used to explore the cause-and-effect relationships underlying a particular problem. The primary goal is to determine the root cause by repeating the question \"Why?\" five times.\n\n");

                analysis.push_str("## Current EC2 Instance Status\n\n");

                // Add current metrics summary
                if let Some(cpu_metric) = MetricsAnalyzer::find_metric(metrics, "CPUUtilization") {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&cpu_metric.datapoints);
                    analysis.push_str(&format!(
                        "- CPU Utilization: Avg {:.2}%, Max {:.2}%\n",
                        avg, max
                    ));

                    // Flag potential issues
                    if max > 80.0 {
                        analysis.push_str("  - ⚠️ **High CPU usage detected**\n");
                    }
                }

                if let Some(mem_metric) = MetricsAnalyzer::find_metric(metrics, "MemoryUtilization")
                {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&mem_metric.datapoints);
                    analysis.push_str(&format!(
                        "- Memory Utilization: Avg {:.2}%, Max {:.2}%\n",
                        avg, max
                    ));

                    if max > 80.0 {
                        analysis.push_str("  - ⚠️ **High memory usage detected**\n");
                    }
                }

                // Network metrics summary
                if let Some(net_in) = MetricsAnalyzer::find_metric(metrics, "NetworkIn") {
                    let (avg, _) = MetricsAnalyzer::calculate_statistics(&net_in.datapoints);
                    analysis.push_str(&format!("- Network In: Avg {:.2} bytes/sec\n", avg));
                }

                if let Some(net_out) = MetricsAnalyzer::find_metric(metrics, "NetworkOut") {
                    let (avg, _) = MetricsAnalyzer::calculate_statistics(&net_out.datapoints);
                    analysis.push_str(&format!("- Network Out: Avg {:.2} bytes/sec\n", avg));
                }

                analysis.push_str("\n## Starting the 5 Why Analysis\n\n");
                analysis.push_str(
                    "To begin your root cause analysis, select one of the following questions:\n\n",
                );

                // Suggest initial why questions based on metrics
                if let Some(cpu_metric) = MetricsAnalyzer::find_metric(metrics, "CPUUtilization") {
                    let (_, max) = MetricsAnalyzer::calculate_statistics(&cpu_metric.datapoints);
                    if max > 80.0 {
                        analysis.push_str("- Why is the CPU utilization high?\n");
                    } else if max < 20.0 {
                        analysis.push_str("- Why is the CPU utilization low?\n");
                    }
                }

                if let Some(mem_metric) = MetricsAnalyzer::find_metric(metrics, "MemoryUtilization")
                {
                    let (_, max) = MetricsAnalyzer::calculate_statistics(&mem_metric.datapoints);
                    if max > 80.0 {
                        analysis.push_str("- Why is the memory usage high?\n");
                    }
                }

                // Add generic why questions
                analysis.push_str("- Why is the instance performance inconsistent?\n");
                analysis.push_str("- Why is the network throughput fluctuating?\n");
                analysis.push_str("- Why is the disk I/O performance degraded?\n");

                analysis.push_str("\nSelect a question to continue with your 5 Why analysis. Each follow-up question will help you dig deeper into the root cause.\n");
            }
            _ => {
                return Err(AppError::BadRequest(
                    "Unsupported workflow type for EC2 instance".to_string(),
                ));
            }
        }

        Ok(analysis)
    }

    pub async fn answer_ec2_question(
        resource: &aws_resource::Model,
        question: &str,
        metrics: &cloud_watch::CloudWatchMetricsResult,
    ) -> Result<String, AppError> {
        let mut answer = String::new();

        // Convert question to lowercase for easier matching
        let question = question.to_lowercase();

        // Check if this is a "why" question (part of 5 why analysis)
        if question.starts_with("why") {
            // Handle 5 why analysis questions
            if question.contains("cpu") && question.contains("high") {
                answer.push_str("## Why is the CPU utilization high?\n\n");

                if let Some(cpu_metric) = MetricsAnalyzer::find_metric(metrics, "CPUUtilization") {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&cpu_metric.datapoints);
                    answer.push_str(&format!("The EC2 instance is experiencing high CPU utilization (Average: {:.2}%, Peak: {:.2}%).\n\n", avg, max));

                    answer.push_str("### Potential root causes:\n\n");
                    answer.push_str("1. **Application workload**: The application running on this instance may be processing a higher volume of requests than it was designed for.\n\n");
                    answer.push_str("2. **Inefficient code**: There might be inefficient algorithms or unoptimized code running on the instance.\n\n");
                    answer.push_str("3. **Insufficient resources**: The instance type may be undersized for the current workload.\n\n");
                    answer.push_str("4. **Background processes**: System processes or maintenance tasks might be consuming CPU resources.\n\n");
                    answer.push_str("5. **Malicious activity**: Unauthorized processes like crypto miners could be running on the instance.\n\n");

                    answer.push_str("### Next steps for investigation:\n\n");
                    answer.push_str("- Check the processes running on the instance using tools like `top` or CloudWatch Process Monitoring\n");
                    answer.push_str("- Review application logs for errors or unusual activity\n");
                    answer.push_str("- Analyze the timing of CPU spikes to identify patterns\n");
                    answer.push_str(
                        "- Consider enabling detailed monitoring for more granular data\n",
                    );
                }
            } else if question.contains("memory")
                && (question.contains("high") || question.contains("usage"))
            {
                answer.push_str("## Why is the memory usage high?\n\n");

                if let Some(mem_metric) = MetricsAnalyzer::find_metric(metrics, "MemoryUtilization")
                {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&mem_metric.datapoints);
                    answer.push_str(&format!("The EC2 instance is showing elevated memory usage (Average: {:.2}%, Peak: {:.2}%).\n\n", avg, max));

                    answer.push_str("### Potential root causes:\n\n");
                    answer.push_str("1. **Memory leaks**: The application might have memory leaks that gradually consume available RAM.\n\n");
                    answer.push_str("2. **Insufficient capacity**: The instance may have insufficient memory for the workload.\n\n");
                    answer.push_str("3. **Caching issues**: Excessive caching by applications or the operating system.\n\n");
                    answer.push_str("4. **Concurrent processes**: Too many processes running simultaneously.\n\n");
                    answer.push_str("5. **Inefficient memory management**: The application might not be releasing memory properly.\n\n");

                    answer.push_str("### Next steps for investigation:\n\n");
                    answer.push_str(
                        "- Use tools like `free`, `vmstat`, or CloudWatch Memory Monitoring\n",
                    );
                    answer.push_str("- Check for processes consuming large amounts of memory\n");
                    answer.push_str("- Review application memory management practices\n");
                    answer.push_str("- Consider enabling swap space if not already configured\n");
                }
            } else if question.contains("disk") || question.contains("i/o") {
                answer.push_str("## Why is the disk I/O performance degraded?\n\n");

                answer.push_str("### Potential root causes:\n\n");
                answer.push_str("1. **High I/O operations**: Too many read/write operations occurring simultaneously.\n\n");
                answer.push_str("2. **Disk type limitations**: The EBS volume type might not be appropriate for the workload.\n\n");
                answer.push_str("3. **Filesystem fragmentation**: Fragmented filesystems can lead to slower I/O operations.\n\n");
                answer.push_str("4. **Inefficient I/O patterns**: Small, random I/O operations instead of sequential ones.\n\n");
                answer.push_str("5. **Noisy neighbors**: Other instances on the same host competing for I/O resources.\n\n");

                answer.push_str("### Next steps for investigation:\n\n");
                answer.push_str(
                    "- Monitor disk I/O metrics using CloudWatch or tools like `iostat`\n",
                );
                answer.push_str("- Check for processes with high disk activity\n");
                answer.push_str(
                    "- Consider upgrading to a different EBS volume type (e.g., gp3, io2)\n",
                );
                answer.push_str("- Optimize application I/O patterns if possible\n");
            } else if question.contains("network") || question.contains("throughput") {
                answer.push_str("## Why is the network throughput fluctuating?\n\n");

                if let Some(net_in) = MetricsAnalyzer::find_metric(metrics, "NetworkIn") {
                    let (avg_in, max_in) =
                        MetricsAnalyzer::calculate_statistics(&net_in.datapoints);
                    if let Some(net_out) = MetricsAnalyzer::find_metric(metrics, "NetworkOut") {
                        let (avg_out, max_out) =
                            MetricsAnalyzer::calculate_statistics(&net_out.datapoints);
                        answer.push_str(&format!("Network traffic: Avg In: {:.2} bytes/sec, Max In: {:.2} bytes/sec, Avg Out: {:.2} bytes/sec, Max Out: {:.2} bytes/sec\n\n", 
                            avg_in, max_in, avg_out, max_out));
                    }
                }

                answer.push_str("### Potential root causes:\n\n");
                answer.push_str(
                    "1. **Network congestion**: High traffic periods or network bottlenecks.\n\n",
                );
                answer.push_str("2. **Instance type limitations**: The instance type may have limited network bandwidth.\n\n");
                answer.push_str(
                    "3. **Application behavior**: Bursty application traffic patterns.\n\n",
                );
                answer.push_str("4. **Network configuration**: Suboptimal network settings or security groups.\n\n");
                answer.push_str("5. **External dependencies**: Slow responses from external services or APIs.\n\n");

                answer.push_str("### Next steps for investigation:\n\n");
                answer.push_str("- Monitor network metrics in CloudWatch\n");
                answer.push_str("- Use tools like `netstat`, `tcpdump`, or VPC Flow Logs\n");
                answer.push_str("- Check for network-intensive processes\n");
                answer.push_str("- Consider enhanced networking if not already enabled\n");
            } else if question.contains("performance") && question.contains("inconsistent") {
                answer.push_str("## Why is the instance performance inconsistent?\n\n");

                answer.push_str("### Potential root causes:\n\n");
                answer.push_str(
                    "1. **Resource contention**: Competition for shared resources on the host.\n\n",
                );
                answer.push_str("2. **Burstable performance**: If using a burstable instance type (T2/T3), CPU credits may be depleting.\n\n");
                answer.push_str(
                    "3. **Workload variability**: Unpredictable or variable workload patterns.\n\n",
                );
                answer.push_str("4. **External dependencies**: Reliance on external services with variable performance.\n\n");
                answer.push_str("5. **Scheduled tasks**: Background jobs or maintenance tasks affecting performance.\n\n");

                answer.push_str("### Next steps for investigation:\n\n");
                answer.push_str("- Monitor CPU credit balance if using burstable instances\n");
                answer.push_str(
                    "- Check for correlation between performance issues and specific events\n",
                );
                answer.push_str("- Review scheduled tasks and cron jobs\n");
                answer.push_str("- Consider upgrading to a dedicated instance type\n");
            } else {
                // Generic why question response
                answer.push_str("## Root Cause Analysis\n\n");

                answer.push_str("To properly analyze this issue, I need to examine the metrics and configuration of your EC2 instance.\n\n");

                // Add summary of available metrics
                answer.push_str("### Current Instance Metrics:\n\n");

                if let Some(cpu_metric) = MetricsAnalyzer::find_metric(metrics, "CPUUtilization") {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&cpu_metric.datapoints);
                    answer.push_str(&format!(
                        "- CPU Utilization: Avg {:.2}%, Max {:.2}%\n",
                        avg, max
                    ));
                }

                if let Some(mem_metric) = MetricsAnalyzer::find_metric(metrics, "MemoryUtilization")
                {
                    let (avg, max) = MetricsAnalyzer::calculate_statistics(&mem_metric.datapoints);
                    answer.push_str(&format!(
                        "- Memory Utilization: Avg {:.2}%, Max {:.2}%\n",
                        avg, max
                    ));
                }

                answer.push_str("\n### General Troubleshooting Approach:\n\n");
                answer.push_str(
                    "1. **Identify symptoms**: Collect data on when and how the issue occurs\n",
                );
                answer.push_str(
                    "2. **Isolate variables**: Determine what factors correlate with the issue\n",
                );
                answer.push_str(
                    "3. **Test hypotheses**: Make controlled changes to verify root causes\n",
                );
                answer.push_str("4. **Implement solutions**: Address the underlying issues\n");
                answer.push_str(
                    "5. **Monitor results**: Verify that the solutions resolve the problem\n\n",
                );

                answer.push_str("To continue this analysis, please ask a more specific question about the instance's performance, such as questions about CPU, memory, disk I/O, or network issues.\n");
            }

            // Add instance details for context
            answer.push_str("\n### Instance Details:\n");
            answer.push_str(&format!("- Instance ID: {}\n", resource.resource_id));

            // Try to get instance type from resource_data
            if let Some(instance_type) = resource
                .resource_data
                .get("instance_type")
                .and_then(|v| v.as_str())
            {
                answer.push_str(&format!("- Instance Type: {}\n", instance_type));
            } else {
                answer.push_str("- Instance Type: Unknown\n");
            }

            answer.push_str(&format!("- Region: {}\n", resource.region));

            return Ok(answer);
        }

        // Handle standard (non-why) questions
        if question.contains("cpu") || question.contains("processor") {
            if let Some(cpu_metric) = MetricsAnalyzer::find_metric(metrics, "CPUUtilization") {
                let (avg, max) = MetricsAnalyzer::calculate_statistics(&cpu_metric.datapoints);
                answer.push_str(&format!(
                    "The EC2 instance's CPU usage:\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                    avg, max
                ));
            }
        }

        if question.contains("memory") || question.contains("ram") {
            if let Some(mem_metric) = MetricsAnalyzer::find_metric(metrics, "MemoryUtilization") {
                let (avg, max) = MetricsAnalyzer::calculate_statistics(&mem_metric.datapoints);
                answer.push_str(&format!(
                    "Memory utilization:\n- Average: {:.2}%\n- Peak: {:.2}%\n\n",
                    avg, max
                ));
            }
        }

        if answer.is_empty() {
            answer = "I apologize, but I don't have enough information to answer that specific question about the EC2 instance.".to_string();
        }

        Ok(answer)
    }
}
