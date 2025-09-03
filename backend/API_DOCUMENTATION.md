# Mayyam Kafka Backup API Documentation

## Overview

The Mayyam Kafka Backup API provides comprehensive backup, restore, migration, and monitoring capabilities for Kafka clusters. This API is designed for SRE teams to handle disaster recovery, data migration, and operational monitoring scenarios.

## Base URL
```
http://localhost:8080/api/kafka
```

## Authentication
All endpoints require JWT authentication. Include the JWT token in the Authorization header:
```
Authorization: Bearer <jwt-token>
```

## Endpoints

### 1. Backup Topic Messages

**Endpoint:** `POST /api/kafka/clusters/{cluster-id}/backup`

**Description:** Creates a compressed backup of messages from a Kafka topic to the filesystem.

**Request Body:**
```json
{
  "topic": "events",
  "compression": "gzip",
  "max_messages": 1000000,
  "start_offset": "earliest",
  "end_offset": "latest",
  "partitions": [0, 1, 2],
  "include_headers": true,
  "include_keys": true,
  "rate_limit_per_second": 10000
}
```

**Parameters:**
- `topic` (string, required): Name of the topic to backup
- `compression` (string, optional): Compression type - "none", "gzip", "snappy", "lz4" (default: "gzip")
- `max_messages` (number, optional): Maximum number of messages to backup (default: unlimited)
- `start_offset` (string, optional): Starting offset - "earliest", "latest", or specific offset (default: "earliest")
- `end_offset` (string, optional): Ending offset - "latest" or specific offset (default: "latest")
- `partitions` (array, optional): Specific partitions to backup (default: all partitions)
- `include_headers` (boolean, optional): Include message headers in backup (default: true)
- `include_keys` (boolean, optional): Include message keys in backup (default: true)
- `rate_limit_per_second` (number, optional): Rate limit for backup operations (default: 10000)

**Response:**
```json
{
  "backup_id": "backup-1234567890",
  "topic": "events",
  "cluster_id": "prod-cluster",
  "compression": "gzip",
  "total_messages": 50000,
  "total_size_bytes": 10485760,
  "compressed_size_bytes": 3145728,
  "compression_ratio": 0.3,
  "duration_ms": 45230,
  "partitions": [
    {
      "partition": 0,
      "messages_count": 16666,
      "start_offset": 1000,
      "end_offset": 17666
    }
  ],
  "created_at": "2025-09-02T10:30:00Z",
  "status": "completed"
}
```

**Error Responses:**
- `400 Bad Request`: Invalid request parameters
- `404 Not Found`: Topic or cluster not found
- `500 Internal Server Error`: Backup operation failed

### 2. Restore Topic Messages

**Endpoint:** `POST /api/kafka/clusters/{cluster-id}/restore`

**Description:** Restores messages from a backup to a Kafka topic.

**Request Body:**
```json
{
  "backup_id": "backup-1234567890",
  "target_topic": "events-restored",
  "target_partitions": 3,
  "replication_factor": 2,
  "preserve_offsets": false,
  "rate_limit_per_second": 5000,
  "validate_checksum": true
}
```

**Parameters:**
- `backup_id` (string, required): ID of the backup to restore
- `target_topic` (string, required): Name of the target topic
- `target_partitions` (number, optional): Number of partitions for target topic (default: same as source)
- `replication_factor` (number, optional): Replication factor for target topic (default: same as source)
- `preserve_offsets` (boolean, optional): Preserve original offsets (default: false)
- `rate_limit_per_second` (number, optional): Rate limit for restore operations (default: 5000)
- `validate_checksum` (boolean, optional): Validate backup integrity before restore (default: true)

**Response:**
```json
{
  "backup_id": "backup-1234567890",
  "target_topic": "events-restored",
  "cluster_id": "prod-cluster",
  "total_messages": 50000,
  "total_size_bytes": 10485760,
  "duration_ms": 38750,
  "messages_restored": 50000,
  "partitions_created": [
    {
      "partition": 0,
      "messages_count": 16666,
      "start_offset": 0,
      "end_offset": 16666
    }
  ],
  "status": "completed",
  "checksum_validated": true
}
```

### 3. Migrate Topic Messages

**Endpoint:** `POST /api/kafka/migrate`

**Description:** Migrates messages between topics, potentially across different clusters.

**Request Body:**
```json
{
  "source_cluster": "prod-cluster",
  "source_topic": "events",
  "target_cluster": "dr-cluster",
  "target_topic": "events-dr",
  "max_messages": 100000,
  "transformation": {
    "filter_expression": "value.contains('important')",
    "header_additions": [
      ["migrated-from", "prod-cluster"],
      ["migration-timestamp", "2025-09-02T10:30:00Z"]
    ]
  },
  "rate_limit_per_second": 8000
}
```

**Parameters:**
- `source_cluster` (string, required): Source cluster ID
- `source_topic` (string, required): Source topic name
- `target_cluster` (string, required): Target cluster ID
- `target_topic` (string, required): Target topic name
- `max_messages` (number, optional): Maximum messages to migrate
- `transformation` (object, optional): Message transformation rules
- `rate_limit_per_second` (number, optional): Rate limit for migration

**Response:**
```json
{
  "migration_id": "migration-1234567890",
  "source_cluster": "prod-cluster",
  "source_topic": "events",
  "target_cluster": "dr-cluster",
  "target_topic": "events-dr",
  "total_messages": 75000,
  "messages_migrated": 75000,
  "duration_ms": 89200,
  "status": "completed",
  "transformation_applied": true
}
```

### 4. Monitor Queue Drain

**Endpoint:** `POST /api/kafka/clusters/{cluster-id}/drain`

**Description:** Monitors consumer group lag until queue is drained or timeout is reached.

**Request Body:**
```json
{
  "group_id": "consumer-group-1",
  "topics": ["events", "logs"],
  "max_lag": 100,
  "timeout_seconds": 300,
  "check_interval_ms": 5000,
  "strategy": "lag_based"
}
```

**Parameters:**
- `group_id` (string, required): Consumer group ID to monitor
- `topics` (array, required): Topics to monitor
- `max_lag` (number, optional): Maximum allowed lag (default: 0)
- `timeout_seconds` (number, optional): Maximum wait time in seconds (default: 300)
- `check_interval_ms` (number, optional): Check interval in milliseconds (default: 5000)
- `strategy` (string, optional): Drain strategy - "lag_based" or "time_based" (default: "lag_based")

**Response:**
```json
{
  "group_id": "consumer-group-1",
  "status": "drained",
  "total_lag": 0,
  "duration_ms": 45200,
  "partitions_checked": 6,
  "last_check_time": "2025-09-02T10:35:00Z",
  "partition_details": [
    {
      "topic": "events",
      "partition": 0,
      "current_offset": 10000,
      "latest_offset": 10000,
      "lag": 0
    }
  ]
}
```

## Data Types

### CompressionType
```typescript
type CompressionType = "none" | "gzip" | "snappy" | "lz4";
```

### Offset
```typescript
type Offset = "earliest" | "latest" | number;
```

### MessageTransformation
```typescript
interface MessageTransformation {
  filter_expression?: string;
  header_additions?: [string, string][];
  key_transformation?: string;
}
```

## Error Handling

All endpoints return standard HTTP status codes:

- `200 OK`: Success
- `400 Bad Request`: Invalid request parameters
- `401 Unauthorized`: Authentication required
- `403 Forbidden`: Insufficient permissions
- `404 Not Found`: Resource not found
- `409 Conflict`: Resource conflict (e.g., topic already exists)
- `500 Internal Server Error`: Server error

Error response format:
```json
{
  "error": "ErrorType",
  "message": "Human-readable error message",
  "details": {
    "field": "specific field that caused the error",
    "value": "invalid value provided"
  }
}
```

## Rate Limiting

- Backup operations: 10 concurrent operations per cluster
- Restore operations: 5 concurrent operations per cluster
- Migration operations: 3 concurrent operations globally
- API calls: 100 requests per minute per user

## Monitoring

### Metrics Endpoints

**Get Backup Metrics:** `GET /api/kafka/metrics`

**Response:**
```json
{
  "active_backups": 2,
  "completed_backups_today": 15,
  "total_backup_size_gb": 125.5,
  "average_backup_duration_ms": 45230,
  "compression_savings_percent": 70.2,
  "error_rate_percent": 0.5
}
```

## Best Practices

### Backup Operations
1. **Schedule regular backups** during low-traffic periods
2. **Use compression** to reduce storage costs (gzip recommended)
3. **Validate backups** after creation
4. **Monitor backup metrics** for performance issues
5. **Test restore procedures** regularly

### Migration Operations
1. **Plan migrations** during maintenance windows
2. **Use rate limiting** to avoid overwhelming target clusters
3. **Validate data integrity** after migration
4. **Monitor consumer lag** during migration
5. **Have rollback plans** ready

### Monitoring
1. **Set up alerts** for backup failures
2. **Monitor storage usage** and plan capacity
3. **Track performance metrics** over time
4. **Regular health checks** of backup systems
5. **Log analysis** for troubleshooting

## Configuration

Backup system configuration is managed through the application config:

```yaml
backup:
  base_path: "/opt/mayyam/backups"
  compression: "gzip"
  max_concurrent_backups: 3
  retention_days: 30
  max_backup_size_gb: 100
  rate_limit_per_second: 10000
```

## Troubleshooting

### Common Issues

1. **Backup fails with timeout**
   - Increase timeout values
   - Check network connectivity
   - Reduce rate limits

2. **Restore fails with checksum error**
   - Validate backup integrity first
   - Check storage system health
   - Verify backup file permissions

3. **Migration slow performance**
   - Adjust rate limiting
   - Check cluster capacity
   - Monitor network bandwidth

4. **High memory usage**
   - Reduce batch sizes
   - Use streaming for large datasets
   - Monitor system resources

### Support

For issues or questions:
1. Check application logs
2. Review metrics and monitoring
3. Contact SRE team
4. Review this documentation

---

**Last Updated:** September 2, 2025
**Version:** 1.0.0</content>
<parameter name="filePath">/Users/rajanpanneerselvam/work/mayyam-gamma/backend/API_DOCUMENTATION.md
