pub struct QuestionGenerator;

impl QuestionGenerator {
    pub fn get_common_questions(resource_type: &str) -> Vec<String> {
        match resource_type {
            "EC2Instance" => vec![
                "What is the CPU utilization?".to_string(),
                "How much memory is being used?".to_string(),
                "What is the network performance?".to_string(),
                "Are there any performance bottlenecks?".to_string(),
            ],
            "RdsInstance" => vec![
                "What is the database CPU usage?".to_string(),
                "How much free memory is available?".to_string(),
                "What is the storage usage trend?".to_string(),
                "Are there any slow queries?".to_string(),
            ],
            "DynamoDbTable" => vec![
                "What is the consumed read capacity?".to_string(),
                "What is the consumed write capacity?".to_string(),
                "Are there any throttled requests?".to_string(),
                "What is the storage usage?".to_string(),
            ],
            "ElasticacheCluster" => vec![
                "What is the cache hit rate?".to_string(),
                "How much memory is being used?".to_string(),
                "Are there any evictions?".to_string(),
                "What is the network bandwidth usage?".to_string(),
            ],
            "KinesisStream" => vec![
                "What is the consumer lag?".to_string(),
                "How many records are being processed?".to_string(),
                "Are there any throughput exceeded events?".to_string(),
                "Do I need to increase my shard count?".to_string(),
            ],
            _ => vec![],
        }
    }

    pub fn generate_related_questions(resource_type: &str, workflow: &str) -> Vec<String> {
        let mut questions = Self::get_common_questions(resource_type);

        // Add workflow-specific questions
        match workflow.to_lowercase().as_str() {
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
            _ => {},
        }

        questions
    }

    pub fn generate_followup_questions(resource_type: &str, original_question: &str) -> Vec<String> {
        let mut questions = Vec::new();
        let question = original_question.to_lowercase();

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