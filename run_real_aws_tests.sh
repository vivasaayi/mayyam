#!/bin/bash

# Script to run Kinesis integration tests with real AWS credentials
# This removes all mocks and ensures tests run against real AWS services

set -e

echo "🔧 Setting up real AWS credentials for integration tests..."
echo ""

# Check if required environment variables are set
if [ -z "$AWS_ACCESS_KEY_ID" ]; then
    echo "❌ AWS_ACCESS_KEY_ID environment variable is not set"
    echo "   Please set it with: export AWS_ACCESS_KEY_ID='your-access-key'"
    exit 1
fi

if [ -z "$AWS_SECRET_ACCESS_KEY" ]; then
    echo "❌ AWS_SECRET_ACCESS_KEY environment variable is not set"
    echo "   Please set it with: export AWS_SECRET_ACCESS_KEY='your-secret-key'"
    exit 1
fi

if [ -z "$AWS_ACCOUNT_ID" ]; then
    echo "❌ AWS_ACCOUNT_ID environment variable is not set"
    echo "   Please set it with: export AWS_ACCOUNT_ID='your-account-id'"
    exit 1
fi

# Set default region if not set
export AWS_DEFAULT_REGION=${AWS_DEFAULT_REGION:-us-east-1}

echo "✅ AWS Credentials configured:"
echo "   Access Key ID: ${AWS_ACCESS_KEY_ID:0:8}..."
echo "   Account ID: $AWS_ACCOUNT_ID"
echo "   Region: $AWS_DEFAULT_REGION"
echo ""

echo "🚀 Starting Docker Compose (dev profile)..."
docker compose --profile dev up -d

echo "⏳ Waiting for services to be ready..."
sleep 10

echo "🧪 Running Kinesis integration tests with real AWS credentials..."
cd backend

# Run the specific Kinesis tests
cargo test --test integration_tests -- --nocapture \
    test_kinesis_stream_analysis_workflow \
    test_kinesis_analysis_time_ranges \
    test_kinesis_analysis_error_handling

echo ""
echo "✅ Tests completed! Check the output above for results."
echo ""
echo "💡 Note: These tests now use real AWS credentials and will:"
echo "   - Create actual Kinesis streams in your AWS account"
echo "   - Generate real CloudWatch metrics"
echo "   - Perform actual AWS API calls"
echo "   - Incur standard AWS charges for the resources used"
echo ""
echo "🧹 Remember to clean up any test resources created in AWS!"
