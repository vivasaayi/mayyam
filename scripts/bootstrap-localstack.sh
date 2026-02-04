#!/usr/bin/env bash
set -euo pipefail

echo "Bootstrapping Localstack (S3/Kinesis/Dynamo/STS) for integration tests"

# Wait until localstack is ready
host=localstack
port=4566
tries=0
while ! nc -z "$host" "$port"; do
  echo "Waiting for Localstack..."
  tries=$((tries + 1))
  if [ "$tries" -gt 60 ]; then
    echo "Localstack did not become available" >&2
    exit 1
  fi
  sleep 2
done

echo "Localstack is up; creating resources using awslocal"

SEED_FILE=docker/localstack-seed.json
if [ ! -f "$SEED_FILE" ]; then
  echo "Seed file not found: $SEED_FILE" >&2
  exit 1
fi

# Ensure jq is available
if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required to parse the seed file; please install jq (apt-get install jq)" >&2
  exit 1
fi

echo "Parsing seed file: $SEED_FILE"
# Ensure jq is available
if ! docker compose exec -T localstack sh -lc "which awslocal" >/dev/null 2>&1; then
  echo "awslocal not found in localstack container" >&2
fi

# Create S3 buckets
cat "$SEED_FILE" | jq -r '.s3[]?.bucket' | while read -r bucket; do
  if [ -n "$bucket" ]; then
    docker compose exec -T localstack awslocal s3 mb s3://$bucket || true
    echo "Created S3 bucket: $bucket"
  fi
done

# Create Kinesis streams
cat "$SEED_FILE" | jq -c '.kinesis[]?' | while read -r streamObj; do
  name=$(echo "$streamObj" | jq -r '.stream_name')
  shards=$(echo "$streamObj" | jq -r '.shard_count')
  if [ "$name" != "null" ]; then
    docker compose exec -T localstack awslocal kinesis create-stream --stream-name "$name" --shard-count $shards || true
    echo "Created Kinesis stream: $name"
  fi
done

# Create DynamoDB tables
cat "$SEED_FILE" | jq -c '.dynamodb[]?' | while read -r tableObj; do
  tname=$(echo "$tableObj" | jq -r '.table_name')
  attrdefs=$(echo "$tableObj" | jq -c '.attribute_definitions')
  keys=$(echo "$tableObj" | jq -c '.key_schema')
  rcu=$(echo "$tableObj" | jq -r '.provisioned_throughput.ReadCapacityUnits')
  wcu=$(echo "$tableObj" | jq -r '.provisioned_throughput.WriteCapacityUnits')
  if [ "$tname" != "null" ]; then
    docker compose exec -T localstack awslocal dynamodb create-table --table-name "$tname" --attribute-definitions "AttributeName=id,AttributeType=S" --key-schema "AttributeName=id,KeyType=HASH" --provisioned-throughput ReadCapacityUnits=$rcu,WriteCapacityUnits=$wcu || true
    echo "Created DynamoDB table: $tname"
  fi
done

echo "Localstack resources have been created"
