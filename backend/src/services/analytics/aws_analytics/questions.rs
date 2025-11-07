pub struct QuestionGenerator;

impl QuestionGenerator {
    pub fn get_five_why_questions(resource_type: &str) -> Vec<String> {
        match resource_type {
            "EC2Instance" => vec![
                "Why is the CPU utilization high?".to_string(),
                "Why is the memory usage increasing?".to_string(),
                "Why is the disk I/O performance degraded?".to_string(),
                "Why is the network throughput fluctuating?".to_string(),
                "Why is the instance performance inconsistent?".to_string(),
            ],
            "RdsInstance" => vec![
                "Why is the database response time slow?".to_string(),
                "Why are there connection timeouts?".to_string(),
                "Why is the database CPU utilization high?".to_string(),
                "Why is the storage space running out?".to_string(),
                "Why are queries taking longer than expected?".to_string(),
            ],
            "DynamoDbTable" => vec![
                "Why are there throttled requests?".to_string(),
                "Why is the read/write capacity insufficient?".to_string(),
                "Why is the table performance degrading?".to_string(),
                "Why are there hot partitions?".to_string(),
                "Why is the cost higher than expected?".to_string(),
            ],
            "S3Bucket" => vec![
                "Why is the storage cost increasing?".to_string(),
                "Why are there access denied errors?".to_string(),
                "Why is the data transfer cost high?".to_string(),
                "Why are objects not transitioning to cheaper storage classes?".to_string(),
                "Why is the bucket size growing rapidly?".to_string(),
            ],
            "KinesisStream" => vec![
                "Why is there a high producer throttling?".to_string(),
                "Why is there a consumer lag?".to_string(),
                "Why is the stream throughput inconsistent?".to_string(),
                "Why are some shards more utilized than others?".to_string(),
                "Why is the stream processing delayed?".to_string(),
            ],
            _ => vec![
                "Why is this resource not performing as expected?".to_string(),
                "Why are there unexpected errors or failures?".to_string(),
                "Why is the resource utilization higher than normal?".to_string(),
                "Why is the cost increasing for this resource?".to_string(),
                "Why are there operational issues with this resource?".to_string(),
            ],
        }
    }

    pub fn get_common_questions(resource_type: &str) -> Vec<String> {
        match resource_type {
            "EC2Instance" => vec![
                "Tell me about the CPU performance".to_string(),
                "Tell me about the Memory usage".to_string(),
                "Tell me about the Disk performance".to_string(),
                "Tell me about the Network performance".to_string(),
                "Tell me about overall system health".to_string(),
            ],
            "RdsInstance" => vec![
                "Tell me about the Database CPU performance".to_string(),
                "Tell me about the Database Memory usage".to_string(),
                "Tell me about the Database Storage performance".to_string(),
                "Tell me about the Database Query performance".to_string(),
                "Tell me about the Database Connection status".to_string(),
            ],
            "DynamoDbTable" => vec![
                "Tell me about the Read Capacity performance".to_string(),
                "Tell me about the Write Capacity performance".to_string(),
                "Tell me about the Throttling status".to_string(),
                "Tell me about the Storage usage".to_string(),
                "Tell me about the Table scaling".to_string(),
            ],
            "ElasticacheCluster" => vec![
                "Tell me about the Cache performance".to_string(),
                "Tell me about the Memory usage".to_string(),
                "Tell me about the Eviction status".to_string(),
                "Tell me about the Network performance".to_string(),
                "Tell me about the Connection status".to_string(),
            ],
            "KinesisStream" => vec![
                "Tell me about the Stream processing performance".to_string(),
                "Tell me about the Consumer lag".to_string(),
                "Tell me about the Throughput status".to_string(),
                "Tell me about the Shard utilization".to_string(),
                "Tell me about the Stream scaling".to_string(),
            ],
            _ => vec![],
        }
    }

    pub fn generate_related_questions(resource_type: &str, workflow: &str) -> Vec<String> {
        let workflow_lower = workflow.to_lowercase();

        // For five-why analysis, return specific why questions
        if workflow_lower == "five-why" || workflow_lower == "5why" || workflow_lower == "5-why" {
            return Self::get_five_why_questions(resource_type);
        }

        let mut questions = Self::get_common_questions(resource_type);

        // Add workflow-specific questions
        match workflow_lower.as_str() {
            "performance" => questions.extend(vec![
                "What are the peak usage times?".to_string(),
                "Are there any performance bottlenecks?".to_string(),
                "How does the current performance compare to last week?".to_string(),
            ]),
            "cost" => questions.extend(vec![
                "What is the monthly cost trend?".to_string(),
                "Are there any cost optimization opportunities?".to_string(),
                "How does the cost compare to similar resources?".to_string(),
            ]),
            _ => {}
        }

        questions
    }

    pub fn generate_followup_questions(
        resource_type: &str,
        original_question: &str,
    ) -> Vec<String> {
        let mut questions = Vec::new();
        let question = original_question.to_lowercase();

        // Check if this is a "why" question (part of 5 why analysis)
        if question.starts_with("why") {
            // Generate deeper "why" questions based on the resource type and original question
            match resource_type {
                "EC2Instance" => {
                    if question.contains("cpu") {
                        questions
                            .push("Why are specific processes consuming high CPU?".to_string());
                        questions.push("Why is the CPU scheduling inefficient?".to_string());
                        questions.push(
                            "Why hasn't auto-scaling triggered to handle the load?".to_string(),
                        );
                    } else if question.contains("memory") {
                        questions.push("Why is memory not being released properly?".to_string());
                        questions
                            .push("Why are there memory leaks in the application?".to_string());
                        questions.push("Why is the memory allocation inefficient?".to_string());
                    } else if question.contains("disk") || question.contains("i/o") {
                        questions.push("Why is the disk I/O pattern suboptimal?".to_string());
                        questions.push(
                            "Why is the storage configuration not meeting requirements?"
                                .to_string(),
                        );
                        questions.push(
                            "Why are there too many small read/write operations?".to_string(),
                        );
                    } else {
                        // Generic follow-up why questions for EC2
                        questions.push(
                            "Why is the instance size not appropriate for the workload?"
                                .to_string(),
                        );
                        questions.push(
                            "Why is the application architecture causing performance issues?"
                                .to_string(),
                        );
                        questions.push(
                            "Why are the resource allocation decisions suboptimal?".to_string(),
                        );
                    }
                }
                "RdsInstance" => {
                    if question.contains("slow") || question.contains("response") {
                        questions.push("Why are the database queries not optimized?".to_string());
                        questions.push("Why is the database schema inefficient?".to_string());
                        questions.push("Why are indexes not properly utilized?".to_string());
                    } else if question.contains("connection") {
                        questions.push(
                            "Why is the connection pool configuration inadequate?".to_string(),
                        );
                        questions
                            .push("Why are connections not being released properly?".to_string());
                        questions.push(
                            "Why is the network path to the database experiencing issues?"
                                .to_string(),
                        );
                    } else {
                        // Generic follow-up why questions for RDS
                        questions.push(
                            "Why is the database instance undersized for the workload?".to_string(),
                        );
                        questions
                            .push("Why is the database configuration not optimized?".to_string());
                        questions.push(
                            "Why is the database maintenance not scheduled appropriately?"
                                .to_string(),
                        );
                    }
                }
                _ => {
                    // Generic deeper why questions for any resource
                    questions.push(
                        "Why is the underlying infrastructure causing this issue?".to_string(),
                    );
                    questions.push(
                        "Why is the configuration not aligned with best practices?".to_string(),
                    );
                    questions.push("Why is the resource not scaling to meet demand?".to_string());
                    questions.push(
                        "Why are operational procedures not addressing this issue?".to_string(),
                    );
                }
            }

            // Return only the why questions for 5 why analysis
            return questions;
        }

        // Standard follow-up questions for non-why questions
        if question.contains("cpu") || question.contains("performance") {
            questions.extend(vec![
                "What is the memory usage like?".to_string(),
                "Are there any correlated metrics?".to_string(),
                "What time periods show the highest usage?".to_string(),
            ]);
        }

        if question.contains("memory") || question.contains("ram") {
            questions.extend(vec![
                "Is there any swap usage?".to_string(),
                "What processes are using the most memory?".to_string(),
                "Has memory usage been increasing over time?".to_string(),
            ]);
        }

        // Add resource-specific follow-up questions
        questions.extend(Self::get_common_questions(resource_type));

        questions
    }
}
