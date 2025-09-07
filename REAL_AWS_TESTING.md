# Real AWS Integration Testing

This document explains how to run integration tests against real AWS services instead of using mocks.

## 🔧 Changes Made

### Removed Mock Elements

1. **Fake AWS Credentials**: Removed hardcoded `AKIAIOSFODNN7EXAMPLE` and `wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY`
2. **Hardcoded Account IDs**: Replaced fake account IDs (`999456789012`, `123456789012`) with environment variables
3. **Hardcoded Regions**: Updated to use `AWS_DEFAULT_REGION` environment variable
4. **Hardcoded Profiles**: Changed from `test-profile` to `default`

### Updated Test Functions

- `test_create_aws_account_api()` - Now uses real AWS credentials
- `test_get_aws_account_by_id_api()` - Uses real credentials
- `test_update_aws_account_api()` - Uses real credentials
- `test_delete_aws_account_api()` - Uses real credentials
- `test_kinesis_stream_analysis_workflow()` - Uses real AWS account/region
- `test_kinesis_analysis_time_ranges()` - Uses real AWS account/region
- `test_kinesis_analysis_error_handling()` - Uses real AWS account/region

## 🚀 How to Run Tests with Real AWS

### Prerequisites

1. **AWS Account**: You need an active AWS account
2. **AWS CLI**: Install and configure AWS CLI
3. **Permissions**: Your AWS credentials must have permissions for:
   - Kinesis (CreateStream, PutRecord, DescribeStream, DeleteStream)
   - CloudWatch (GetMetricData, ListMetrics)
   - IAM (if using roles)

### Step 1: Set Environment Variables

```bash
export AWS_ACCESS_KEY_ID="your-access-key-id"
export AWS_SECRET_ACCESS_KEY="your-secret-access-key"
export AWS_ACCOUNT_ID="your-12-digit-account-id"
export AWS_DEFAULT_REGION="us-east-1"  # or your preferred region
```

### Step 2: Run Tests

Use the provided script:

```bash
./run_real_aws_tests.sh
```

Or run manually:

```bash
# Start services
docker compose --profile dev up -d

# Wait for services to start
sleep 10

# Run tests
cd backend
cargo test --test integration_tests -- --nocapture \
    test_kinesis_stream_analysis_workflow \
    test_kinesis_analysis_time_ranges \
    test_kinesis_analysis_error_handling
```

## ⚠️ Important Warnings

### AWS Charges
- **Real AWS Resources**: Tests create actual Kinesis streams
- **CloudWatch Metrics**: Tests generate real metrics data
- **API Calls**: All AWS API calls incur standard charges
- **Data Transfer**: Stream operations may incur data transfer costs

### Resource Management
- **Manual Cleanup**: You must manually delete test resources
- **Stream Names**: Tests use predictable names (e.g., `test-kinesis-low-usage`)
- **Region Awareness**: Resources are created in your specified region

### Security Considerations
- **Credentials Exposure**: Never commit real AWS credentials
- **Least Privilege**: Use IAM credentials with minimal required permissions
- **Test Account**: Consider using a separate AWS account for testing

## 🧹 Cleanup Instructions

After running tests, clean up resources:

```bash
# List streams
aws kinesis list-streams --region us-east-1

# Delete test streams
aws kinesis delete-stream --stream-name test-kinesis-low-usage --region us-east-1
aws kinesis delete-stream --stream-name test-kinesis-medium-usage --region us-east-1
aws kinesis delete-stream --stream-name test-kinesis-high-usage --region us-east-1

# Verify deletion
aws kinesis list-streams --region us-east-1
```

## 🔍 What the Tests Do

### Kinesis Stream Analysis Workflow
1. Creates 3 streams with different usage patterns
2. Inserts records (5, 50, 200 records respectively)
3. Waits for CloudWatch metrics to be available
4. Tests performance and cost analysis
5. Cleans up streams

### Time Range Analysis
1. Tests analysis with different time ranges (1 hour, 6 hours, 1 day, 7 days)
2. Validates time-based filtering works correctly

### Error Handling
1. Tests behavior with non-existent streams
2. Validates proper error responses
3. Ensures graceful failure handling

## 📊 Expected Results

With real AWS credentials, you should see:
- ✅ Streams actually created in AWS console
- ✅ Real CloudWatch metrics generated
- ✅ Actual API responses (not mocked)
- ✅ Proper error handling for real AWS errors

## 🔄 Fallback to Mock Testing

If you want to run tests without real AWS:

```bash
# Use fake credentials (tests will pass but won't create real resources)
export AWS_ACCESS_KEY_ID="AKIAIOSFODNN7EXAMPLE"
export AWS_SECRET_ACCESS_KEY="wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
export AWS_ACCOUNT_ID="123456789012"

# Run tests (they will pass but use fake credentials)
cargo test --test integration_tests
```

## 🐛 Troubleshooting

### Common Issues

1. **Credentials Not Set**: Ensure all required environment variables are set
2. **Permissions Error**: Check IAM permissions for your credentials
3. **Region Mismatch**: Ensure AWS_DEFAULT_REGION matches your account's region
4. **Rate Limiting**: AWS API rate limits may cause test failures
5. **Resource Limits**: Check AWS service limits (e.g., Kinesis stream limits)

### Debug Tips

```bash
# Check AWS credentials
aws sts get-caller-identity

# Check Kinesis permissions
aws kinesis list-streams

# Check CloudWatch permissions
aws cloudwatch list-metrics --namespace AWS/Kinesis
```

## 📝 Environment Variables Reference

| Variable | Required | Description | Example |
|----------|----------|-------------|---------|
| `AWS_ACCESS_KEY_ID` | Yes | Your AWS access key | `AKIAIOSFODNN7EXAMPLE` |
| `AWS_SECRET_ACCESS_KEY` | Yes | Your AWS secret key | `wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY` |
| `AWS_ACCOUNT_ID` | Yes | Your 12-digit AWS account ID | `123456789012` |
| `AWS_DEFAULT_REGION` | No | AWS region (defaults to us-east-1) | `us-west-2` |
| `TEST_AWS_ACCOUNT_ID` | No | Test account ID (defaults to 123456789012) | `999999999999` |
