#!/bin/bash

# Secure AWS Credential Management for Tests
# Uses AWS CLI profiles instead of environment variables

set -e

echo "🔐 Secure AWS Credential Setup for Tests"
echo "========================================"
echo ""

# Function to check AWS CLI and profile
check_aws_setup() {
    local profile=${AWS_PROFILE:-default}

    echo "Checking AWS CLI configuration..."

    if ! command -v aws &> /dev/null; then
        echo "❌ AWS CLI is not installed"
        echo "Install with: pip install awscli"
        exit 1
    fi

    if ! aws sts get-caller-identity --profile "$profile" &> /dev/null; then
        echo "❌ AWS CLI profile '$profile' is not configured or invalid"
        echo ""
        echo "Configure with:"
        echo "  aws configure --profile $profile"
        echo ""
        echo "Or set a different profile:"
        echo "  export AWS_PROFILE=your-profile-name"
        exit 1
    fi

    echo "✅ AWS CLI profile '$profile' is working"
    aws sts get-caller-identity --profile "$profile" | jq -r '.Account' 2>/dev/null || echo "Account ID: $(aws sts get-caller-identity --profile "$profile" --query Account --output text)"
}

# Function to set up environment for tests
setup_test_environment() {
    local profile=${AWS_PROFILE:-default}

    echo ""
    echo "🔧 Setting up test environment..."

    # Export credentials from profile to environment (temporary)
    export AWS_ACCESS_KEY_ID=$(aws configure get aws_access_key_id --profile "$profile")
    export AWS_SECRET_ACCESS_KEY=$(aws configure get aws_secret_access_key --profile "$profile")
    export AWS_DEFAULT_REGION=$(aws configure get region --profile "$profile" || echo "us-east-1")

    # Get account ID
    export AWS_ACCOUNT_ID=$(aws sts get-caller-identity --profile "$profile" --query Account --output text)

    echo "✅ Test environment configured:"
    echo "   Profile: $profile"
    echo "   Account ID: $AWS_ACCOUNT_ID"
    echo "   Region: $AWS_DEFAULT_REGION"
    echo "   Access Key: ${AWS_ACCESS_KEY_ID:0:8}..."
}

# Function to clean up environment
cleanup_environment() {
    echo ""
    echo "🧹 Cleaning up environment variables..."
    unset AWS_ACCESS_KEY_ID
    unset AWS_SECRET_ACCESS_KEY
    unset AWS_DEFAULT_REGION
    unset AWS_ACCOUNT_ID
    echo "✅ Environment cleaned up"
}

# Function to run tests
run_secure_tests() {
    echo ""
    echo "🧪 Running Kinesis integration tests..."

    # Start Docker services
    echo "🚀 Starting Docker Compose (dev profile)..."
    docker compose --profile dev up -d

    echo "⏳ Waiting for services to be ready..."
    sleep 10

    # Change to backend directory
    cd backend

    # Run tests
    echo "Running tests with secure credentials..."
    if cargo test --test integration_tests -- --nocapture \
        test_kinesis_stream_analysis_workflow \
        test_kinesis_analysis_time_ranges \
        test_kinesis_analysis_error_handling; then

        echo ""
        echo "✅ All tests completed successfully!"
        echo ""
        echo "💡 Note: Tests used real AWS credentials from profile '${AWS_PROFILE:-default}'"
        echo "   - Kinesis streams may have been created in your AWS account"
        echo "   - Check AWS console for any test resources"
        echo "   - Clean up test resources to avoid charges"
    else
        echo ""
        echo "❌ Some tests failed"
        exit 1
    fi
}

# Main execution
trap cleanup_environment EXIT

echo "Current AWS_PROFILE: ${AWS_PROFILE:-default}"
echo ""

check_aws_setup
setup_test_environment
run_secure_tests

echo ""
echo "🎉 Secure testing completed!"
echo ""
echo "🔒 Security Notes:"
echo "  - Credentials were temporarily exported to environment"
echo "  - Environment variables are cleaned up automatically"
echo "  - No credentials were stored in files or code"
echo "  - AWS CLI profile remains your secure credential store"
