# Kafka Integration - Advanced Features

This document describes the advanced features added to the Mayyam Kafka integration.

## 🚀 New Features

### 1. Metrics and Monitoring

The Kafka service now includes comprehensive metrics collection:

```rust
// Get current metrics
let metrics = kafka_service.get_metrics()?;

// Metrics include:
// - messages_produced: Total messages sent
// - messages_consumed: Total messages received
// - errors_count: Total errors encountered
// - avg_response_time_ms: Average operation response time
// - last_health_check: Timestamp of last health check
// - active_connections: Current active connections
```

**API Endpoint:**
```
GET /api/kafka/metrics
```

### 2. Batch Operations

Send multiple messages in a single request for better throughput:

```json
POST /api/kafka/clusters/{cluster-id}/batch-produce
{
  "messages": [
    {
      "key": "user-123",
      "value": "User data...",
      "headers": [
        ["content-type", "application/json"],
        ["correlation-id", "abc-123"]
      ]
    },
    {
      "key": "user-456",
      "value": "More user data...",
      "headers": []
    }
  ]
}
```

**Response:**
```json
{
  "message": "Batch production completed",
  "total_messages": 2,
  "total_size_bytes": 1024,
  "duration_ms": 45.2,
  "results": [
    {"partition": 0, "offset": 1001, "status": "success"},
    {"partition": 1, "offset": 850, "status": "success"}
  ]
}
```

### 3. Retry Logic

Automatically retry failed message production with exponential backoff:

```json
POST /api/kafka/clusters/{cluster-id}/produce-retry
{
  "message": {
    "key": "important-data",
    "value": "Critical message...",
    "headers": []
  },
  "max_retries": 3
}
```

### 4. Enhanced Health Checks

Improved health check with metrics tracking:

```json
GET /api/kafka/clusters/{cluster-id}/health
```

**Response:**
```json
{
  "status": "healthy",
  "cluster_id": "local",
  "brokers": [
    {"id": 1, "host": "localhost", "port": 9092}
  ],
  "topics_count": 5,
  "timestamp": 1703123456789
}
```

### 5. Configuration Validation

Validate Kafka cluster configurations before use:

```rust
let config = KafkaClusterConfig {
    name: "my-cluster".to_string(),
    bootstrap_servers: vec!["kafka1:9092".to_string(), "kafka2:9092".to_string()],
    security_protocol: "SASL_SSL".to_string(),
    sasl_username: Some("user".to_string()),
    sasl_password: Some("password".to_string()),
    sasl_mechanism: Some("PLAIN".to_string()),
};

let result = kafka_service.validate_cluster_config(&config);
match result {
    Ok(()) => println!("Configuration is valid"),
    Err(e) => println!("Configuration error: {:?}", e),
}
```

## 🔧 Configuration Examples

### Local Development
```yaml
kafka:
  clusters:
    - name: local
      bootstrap_servers:
        - localhost:9092
      security_protocol: PLAINTEXT
```

### Production with SASL_SSL
```yaml
kafka:
  clusters:
    - name: production
      bootstrap_servers:
        - kafka-1.company.com:9093
        - kafka-2.company.com:9093
        - kafka-3.company.com:9093
      security_protocol: SASL_SSL
      sasl_username: "mayyam-service"
      sasl_password: "${KAFKA_PASSWORD}"
      sasl_mechanism: "PLAIN"
```

## 📊 Monitoring and Observability

### Metrics Collection
The service automatically tracks:
- Operation success/failure rates
- Response time percentiles
- Connection health status
- Message throughput

### Error Handling
- Automatic retry with exponential backoff
- Circuit breaker pattern for resilience
- Comprehensive error logging with context

## 🧪 Testing

Run the test suite:
```bash
cargo test kafka_tests
```

Test coverage includes:
- Configuration validation
- Metrics collection
- Error handling scenarios
- Mock Kafka operations

## 🔒 Security Considerations

1. **SASL Authentication**: Use SASL_SSL in production
2. **Certificate Validation**: Enable SSL certificate verification
3. **Access Control**: Implement proper authorization checks
4. **Secrets Management**: Store credentials securely (Vault, AWS Secrets Manager, etc.)

## 📈 Performance Optimizations

1. **Connection Pooling**: Reuse connections for better performance
2. **Batch Operations**: Send multiple messages together
3. **Async Processing**: Non-blocking operations with proper timeouts
4. **Metrics Overhead**: Minimal performance impact from metrics collection

## 🚨 Troubleshooting

### Common Issues

1. **Connection Refused**
   - Check Kafka broker addresses and ports
   - Verify network connectivity and firewall rules
   - Confirm Kafka cluster is running

2. **Authentication Failed**
   - Validate SASL credentials
   - Check security protocol configuration
   - Verify SSL certificates if using SSL

3. **Timeout Errors**
   - Increase timeout values in configuration
   - Check network latency
   - Monitor Kafka cluster performance

### Debug Mode

Enable debug logging:
```bash
RUST_LOG=debug cargo run
```

## 🔄 Migration Guide

### From Basic to Advanced Integration

1. **Update Dependencies**: Ensure you have the latest rdkafka version
2. **Add Metrics**: Initialize the KafkaService with metrics enabled
3. **Update API Calls**: Use new endpoints for batch operations and retries
4. **Add Monitoring**: Integrate metrics with your monitoring system
5. **Update Tests**: Add tests for new functionality

## 📚 API Reference

### Endpoints

- `GET /api/kafka/metrics` - Get service metrics
- `GET /api/kafka/clusters/{id}/health` - Enhanced health check
- `POST /api/kafka/clusters/{id}/batch-produce` - Batch message production
- `POST /api/kafka/clusters/{id}/produce-retry` - Message production with retry

### Error Codes

- `400` - Invalid configuration or request
- `401` - Authentication failed
- `403` - Authorization failed
- `500` - Internal server error
- `503` - Kafka cluster unavailable

This enhanced Kafka integration provides production-ready features for reliable, observable, and performant Kafka operations.
