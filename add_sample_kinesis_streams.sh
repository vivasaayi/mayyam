#!/bin/bash

# Demo script to add sample Kinesis streams for testing bulk analysis
# This script inserts sample Kinesis stream data into the database

echo "Adding sample Kinesis streams for bulk analysis demo..."

# Check if docker-compose is running
if ! docker-compose --profile dev ps | grep -q "backend-dev.*Up"; then
    echo "Error: Backend service is not running. Please start with: docker-compose --profile dev up"
    exit 1
fi

# Wait for backend to be ready
echo "Waiting for backend to be ready..."
until curl -s http://localhost:8080/health > /dev/null 2>&1; do
    echo "Backend not ready yet, waiting..."
    sleep 2
done

echo "Backend is ready!"

# SQL script to insert sample Kinesis streams
SAMPLE_STREAMS_SQL="
INSERT INTO aws_resources (
    id,
    resource_id, 
    name, 
    resource_type, 
    arn, 
    region, 
    account_id, 
    profile,
    resource_data, 
    tags,
    created_at, 
    updated_at,
    last_refreshed
) VALUES 
(
    gen_random_uuid(),
    'user-activity-stream',
    'User Activity Stream',
    'KinesisStream',
    'arn:aws:kinesis:us-east-1:123456789012:stream/user-activity-stream',
    'us-east-1',
    '123456789012',
    'default',
    '{\"ShardCount\": 2, \"RetentionPeriod\": 24, \"StreamStatus\": \"ACTIVE\"}',
    '{\"Environment\": \"production\", \"Application\": \"user-tracking\", \"Owner\": \"analytics-team\"}',
    NOW(),
    NOW(),
    NOW()
),
(
    gen_random_uuid(),
    'payment-events-stream',
    'Payment Events Stream',
    'KinesisStream',
    'arn:aws:kinesis:us-west-2:123456789012:stream/payment-events-stream',
    'us-west-2',
    '123456789012',
    'default',
    '{\"ShardCount\": 4, \"RetentionPeriod\": 168, \"StreamStatus\": \"ACTIVE\"}',
    '{\"Environment\": \"production\", \"Application\": \"payments\", \"Owner\": \"fintech-team\"}',
    NOW(),
    NOW(),
    NOW()
),
(
    gen_random_uuid(),
    'log-aggregation-stream',
    'Log Aggregation Stream',
    'KinesisStream',
    'arn:aws:kinesis:eu-west-1:123456789012:stream/log-aggregation-stream',
    'eu-west-1',
    '123456789012',
    'default',
    '{\"ShardCount\": 8, \"RetentionPeriod\": 24, \"StreamStatus\": \"ACTIVE\"}',
    '{\"Environment\": \"production\", \"Application\": \"logging\", \"Owner\": \"platform-team\"}',
    NOW(),
    NOW(),
    NOW()
),
(
    gen_random_uuid(),
    'ml-training-stream',
    'ML Training Data Stream',
    'KinesisStream',
    'arn:aws:kinesis:us-east-1:123456789012:stream/ml-training-stream',
    'us-east-1',
    '123456789012',
    'default',
    '{\"ShardCount\": 1, \"RetentionPeriod\": 24, \"StreamStatus\": \"ACTIVE\"}',
    '{\"Environment\": \"development\", \"Application\": \"machine-learning\", \"Owner\": \"data-science-team\"}',
    NOW(),
    NOW(),
    NOW()
),
(
    gen_random_uuid(),
    'iot-sensor-stream',
    'IoT Sensor Data Stream',
    'KinesisStream',
    'arn:aws:kinesis:ap-southeast-1:123456789012:stream/iot-sensor-stream',
    'ap-southeast-1',
    '123456789012',
    'default',
    '{\"ShardCount\": 6, \"RetentionPeriod\": 72, \"StreamStatus\": \"ACTIVE\"}',
    '{\"Environment\": \"production\", \"Application\": \"iot\", \"Owner\": \"iot-team\"}',
    NOW(),
    NOW(),
    NOW()
)
ON CONFLICT (arn) DO NOTHING;
"

# Execute the SQL script via docker
echo "Inserting sample Kinesis streams..."
docker-compose --profile dev exec -T postgres psql -U postgres -d mayyam -c "$SAMPLE_STREAMS_SQL"

if [ $? -eq 0 ]; then
    echo "✅ Successfully added sample Kinesis streams!"
    echo ""
    echo "Sample streams added:"
    echo "1. User Activity Stream (us-east-1) - 2 shards"
    echo "2. Payment Events Stream (us-west-2) - 4 shards"
    echo "3. Log Aggregation Stream (eu-west-1) - 8 shards"
    echo "4. ML Training Data Stream (us-east-1) - 1 shard"
    echo "5. IoT Sensor Data Stream (ap-southeast-1) - 6 shards"
    echo ""
    echo "🎯 Now you can test the bulk analysis feature at: http://localhost:3000/kinesis-analysis"
    echo "   Click 'Analyze All Streams' to run analysis on all 5 streams!"
else
    echo "❌ Failed to add sample streams. Check the database connection."
    exit 1
fi
