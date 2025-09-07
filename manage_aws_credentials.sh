#!/bin/bash

# Secure AWS Credential Management Script
# This script demonstrates multiple secure ways to manage AWS credentials

set -e

echo "🔐 AWS Credential Management Options"
echo "===================================="
echo ""

# Function to check if AWS CLI is configured
check_aws_cli() {
    if command -v aws &> /dev/null; then
        if aws sts get-caller-identity &> /dev/null; then
            echo "✅ AWS CLI is configured and working"
            return 0
        else
            echo "⚠️  AWS CLI is installed but not configured"
            return 1
        fi
    else
        echo "❌ AWS CLI is not installed"
        return 1
    fi
}

# Option 1: AWS CLI Configuration
setup_aws_cli() {
    echo ""
    echo "1️⃣ AWS CLI Configuration (RECOMMENDED)"
    echo "--------------------------------------"

    if check_aws_cli; then
        echo "Current AWS CLI identity:"
        aws sts get-caller-identity | jq -r '.User|.Arn' 2>/dev/null || aws sts get-caller-identity
    else
        echo "To configure AWS CLI:"
        echo "  aws configure"
        echo "  # Or for specific profile:"
        echo "  aws configure --profile myprofile"
    fi
}

# Option 2: Named Profiles
setup_named_profiles() {
    echo ""
    echo "2️⃣ Named Profiles"
    echo "-----------------"

    if [ -f ~/.aws/credentials ]; then
        echo "Available profiles in ~/.aws/credentials:"
        awk '/^\[/{gsub(/\[|\]/,""); print "  - " $0}' ~/.aws/credentials
    else
        echo "No AWS credentials file found at ~/.aws/credentials"
        echo "Create it with:"
        echo "  mkdir -p ~/.aws"
        echo "  cat > ~/.aws/credentials << EOF"
        echo "  [default]"
        echo "  aws_access_key_id = YOUR_ACCESS_KEY"
        echo "  aws_secret_access_key = YOUR_SECRET_KEY"
        echo "  [dev]"
        echo "  aws_access_key_id = DEV_ACCESS_KEY"
        echo "  aws_secret_access_key = DEV_SECRET_KEY"
        echo "  EOF"
    fi
}

# Option 3: Environment Variables (Current approach)
setup_env_vars() {
    echo ""
    echo "3️⃣ Environment Variables (Current)"
    echo "----------------------------------"

    echo "Current environment variables:"
    echo "  AWS_ACCESS_KEY_ID: ${AWS_ACCESS_KEY_ID:+SET (${AWS_ACCESS_KEY_ID:0:8}...)} ${AWS_ACCESS_KEY_ID:-NOT SET}"
    echo "  AWS_SECRET_ACCESS_KEY: ${AWS_SECRET_ACCESS_KEY:+SET} ${AWS_SECRET_ACCESS_KEY:-NOT SET}"
    echo "  AWS_DEFAULT_REGION: ${AWS_DEFAULT_REGION:-NOT SET}"
    echo "  AWS_PROFILE: ${AWS_PROFILE:-NOT SET}"
}

# Option 4: IAM Roles (for EC2/ECS/EKS)
setup_iam_roles() {
    echo ""
    echo "4️⃣ IAM Roles (For AWS Services)"
    echo "-------------------------------"

    if curl -s http://169.254.169.254/latest/meta-data/iam/security-credentials/ &> /dev/null; then
        echo "✅ Running on EC2 with IAM role"
        ROLE_NAME=$(curl -s http://169.254.169.254/latest/meta-data/iam/security-credentials/)
        echo "Role: $ROLE_NAME"
    else
        echo "❌ Not running on EC2 or no IAM role attached"
        echo "For local development, use AWS CLI or environment variables"
    fi
}

# Option 5: AWS SSO
setup_aws_sso() {
    echo ""
    echo "5️⃣ AWS SSO (Enterprise)"
    echo "-----------------------"

    if aws configure sso --help &> /dev/null; then
        echo "AWS SSO is available"
        echo "To configure SSO:"
        echo "  aws configure sso"
        echo "  aws sso login --profile my-sso-profile"
    else
        echo "AWS SSO not available in this CLI version"
    fi
}

# Option 6: Credential Process
setup_credential_process() {
    echo ""
    echo "6️⃣ Credential Process (Advanced)"
    echo "--------------------------------"

    echo "For custom credential providers, use credential_process in ~/.aws/config:"
    echo "  [profile custom]"
    echo "  credential_process = /path/to/credential/script.sh"
    echo "  region = us-east-1"
}

# Security Best Practices
show_security_best_practices() {
    echo ""
    echo "🔒 Security Best Practices"
    echo "=========================="
    echo ""
    echo "✅ DO:"
    echo "  - Use IAM roles when possible"
    echo "  - Rotate access keys regularly"
    echo "  - Use least privilege principle"
    echo "  - Store credentials securely (not in code)"
    echo "  - Use MFA for sensitive operations"
    echo ""
    echo "❌ DON'T:"
    echo "  - Hardcode credentials in source code"
    echo "  - Commit credentials to version control"
    echo "  - Share access keys between team members"
    echo "  - Use root account for applications"
    echo "  - Store credentials in plain text files"
    echo ""
}

# Main execution
setup_aws_cli
setup_named_profiles
setup_env_vars
setup_iam_roles
setup_aws_sso
setup_credential_process
show_security_best_practices

echo ""
echo "🎯 Recommended Setup for Development:"
echo "====================================="
echo "1. Install AWS CLI: pip install awscli"
echo "2. Configure with: aws configure --profile dev"
echo "3. Use in scripts: export AWS_PROFILE=dev"
echo "4. For production: Use IAM roles on EC2/ECS/EKS"
echo ""
