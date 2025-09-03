#[cfg(test)]
#[cfg(feature = "integration-tests")]
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::error::KafkaError;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::Message;
use std::time::Duration;
use tokio::time::timeout;

pub struct KafkaTestHelper {
    bootstrap_servers: String,
    admin_client: AdminClient<DefaultClientContext>,
    producer: FutureProducer,
    consumer: StreamConsumer,
}

impl KafkaTestHelper {
    /// Create a new KafkaTestHelper with default configuration
    pub async fn new() -> Self {
        Self::new_with_config("localhost:9092".to_string(), Duration::from_secs(30))
    }

    /// Create a new KafkaTestHelper with custom bootstrap servers and timeout
    pub fn new_with_config(bootstrap_servers: String, timeout: Duration) -> Self {
        // Create admin client with error handling
        let admin_client_result: Result<AdminClient<DefaultClientContext>, KafkaError> = ClientConfig::new()
            .set("bootstrap.servers", &bootstrap_servers)
            .set("request.timeout.ms", timeout.as_millis().to_string())
            .create();

        let admin_client = match admin_client_result {
            Ok(client) => client,
            Err(e) => {
                println!("Warning: Failed to create admin client: {:?}", e);
                // Create a dummy client that will fail on use - this allows the test to proceed
                // and properly test error scenarios
                ClientConfig::new()
                    .set("bootstrap.servers", &bootstrap_servers)
                    .create()
                    .expect("Failed to create admin client")
            }
        };

        // Create producer with error handling
        let producer_result: Result<FutureProducer, KafkaError> = ClientConfig::new()
            .set("bootstrap.servers", &bootstrap_servers)
            .set("message.timeout.ms", "5000")
            .create();

        let producer = match producer_result {
            Ok(prod) => prod,
            Err(e) => {
                println!("Warning: Failed to create producer: {:?}", e);
                ClientConfig::new()
                    .set("bootstrap.servers", &bootstrap_servers)
                    .set("message.timeout.ms", "5000")
                    .create()
                    .expect("Failed to create producer")
            }
        };

        // Create consumer with error handling
        let consumer_result: Result<StreamConsumer, KafkaError> = ClientConfig::new()
            .set("bootstrap.servers", &bootstrap_servers)
            .set("group.id", "test-group")
            .set("auto.offset.reset", "earliest")
            .create();

        let consumer = match consumer_result {
            Ok(cons) => cons,
            Err(e) => {
                println!("Warning: Failed to create consumer: {:?}", e);
                ClientConfig::new()
                    .set("bootstrap.servers", &bootstrap_servers)
                    .set("group.id", "test-group")
                    .set("auto.offset.reset", "earliest")
                    .create()
                    .expect("Failed to create consumer")
            }
        };

        Self {
            bootstrap_servers,
            admin_client,
            producer,
            consumer,
        }
    }

    /// Test basic connectivity to Kafka cluster
    pub async fn test_connection(&self) -> Result<(), KafkaError> {
        match self.consumer.fetch_metadata(None, Duration::from_secs(5)) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub async fn create_topic(&self, topic_name: &str, partitions: i32, replication_factor: i32) -> Result<(), Box<dyn std::error::Error>> {
        let topic = NewTopic::new(topic_name, partitions, TopicReplication::Fixed(replication_factor));

        let opts = AdminOptions::new().request_timeout(Some(Duration::from_secs(30)));

        match timeout(Duration::from_secs(30), self.admin_client.create_topics(&[topic], &opts)).await {
            Ok(result) => {
                match result {
                    Ok(_) => {
                        println!("Topic '{}' created successfully", topic_name);
                        Ok(())
                    }
                    Err(e) => Err(Box::new(e)),
                }
            }
            Err(_) => Err("Timeout creating topic".into()),
        }
    }

    pub async fn delete_topic(&self, topic_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let opts = AdminOptions::new().request_timeout(Some(Duration::from_secs(30)));

        match timeout(Duration::from_secs(30), self.admin_client.delete_topics(&[topic_name], &opts)).await {
            Ok(result) => {
                match result {
                    Ok(_) => {
                        println!("Topic '{}' deleted successfully", topic_name);
                        Ok(())
                    }
                    Err(e) => Err(Box::new(e)),
                }
            }
            Err(_) => Err("Timeout deleting topic".into()),
        }
    }

    pub async fn produce_message(&self, topic: &str, key: Option<&str>, payload: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut record = FutureRecord::to(topic).payload(payload);

        if let Some(k) = key {
            record = record.key(k);
        }

        match timeout(Duration::from_secs(10), self.producer.send(record, Duration::from_secs(0))).await {
            Ok(result) => {
                match result {
                    Ok(_) => {
                        println!("Message produced to topic '{}'", topic);
                        Ok(())
                    }
                    Err((e, _)) => Err(Box::new(e)),
                }
            }
            Err(_) => Err("Timeout producing message".into()),
        }
    }

    pub async fn consume_messages(&self, topic: &str, expected_count: usize, timeout_duration: Duration) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.consumer.subscribe(&[topic])?;

        let mut messages = Vec::new();
        let start_time = std::time::Instant::now();

        while messages.len() < expected_count && start_time.elapsed() < timeout_duration {
            match timeout(Duration::from_millis(100), self.consumer.recv()).await {
                Ok(result) => {
                    match result {
                        Ok(msg) => {
                            if let Some(payload) = msg.payload() {
                                if let Ok(text) = std::str::from_utf8(payload) {
                                    messages.push(text.to_string());
                                    println!("Consumed message: {}", text);
                                }
                            }
                        }
                        Err(e) => {
                            println!("Error receiving message: {:?}", e);
                            return Err(Box::new(e));
                        }
                    }
                }
                Err(_) => {
                    // Timeout on recv, continue loop
                }
            }
        }

        Ok(messages)
    }

    pub async fn wait_for_topic(&self, topic_name: &str, timeout_duration: Duration) -> Result<(), Box<dyn std::error::Error>> {
        let start_time = std::time::Instant::now();

        while start_time.elapsed() < timeout_duration {
            let metadata = self.consumer.fetch_metadata(None, Duration::from_secs(1))?;

            if metadata.topics().iter().any(|t| t.name() == topic_name) {
                return Ok(());
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Err(format!("Topic '{}' not found within timeout", topic_name).into())
    }

    pub async fn get_topic_count(&self) -> Result<usize, Box<dyn std::error::Error>> {
        let metadata = self.consumer.fetch_metadata(None, Duration::from_secs(5))?;
        Ok(metadata.topics().len())
    }
}

impl Drop for KafkaTestHelper {
    fn drop(&mut self) {
        // Cleanup will happen automatically when the test ends
    }
}
