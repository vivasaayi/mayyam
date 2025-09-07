# AWS Credentials Management Guide
# ================================

## Overview
This guide covers secure methods to manage AWS access keys without leaking them to code.

## 🔐 Security Methods (Ordered by Preference)

### 1. AWS CLI Named Profiles (RECOMMENDED for Development)
```bash
# Configure AWS CLI with named profile
aws configure --profile dev
# Enter your access key, secret key, and region

# Use in environment
export AWS_PROFILE=dev

# Or use directly in commands
aws s3 ls --profile dev
```

### 2. IAM Roles (RECOMMENDED for Production)
- Attach IAM roles to EC2 instances, ECS tasks, EKS pods
- No credentials needed in code or environment
- Automatically rotated by AWS

### 3. Environment Variables (Current Approach - Use with Caution)
```bash
export AWS_ACCESS_KEY_ID="your-key"
export AWS_SECRET_ACCESS_KEY="your-secret"
export AWS_DEFAULT_REGION="us-east-1"
```

### 4. AWS SSO (Enterprise)
```bash
aws configure sso --profile my-sso-profile
aws sso login --profile my-sso-profile
```

### 5. Credential Files (~/.aws/credentials)
```ini
[default]
aws_access_key_id = YOUR_ACCESS_KEY
aws_secret_access_key = YOUR_SECRET_KEY

[dev]
aws_access_key_id = DEV_ACCESS_KEY
aws_secret_access_key = DEV_SECRET_KEY
```

## 🛡️ Security Best Practices

### ✅ DO:
- Use IAM roles when possible (EC2, ECS, EKS)
- Rotate access keys every 90 days
- Use least privilege principle
- Enable MFA for sensitive operations
- Store credentials in secure locations only
- Use different credentials for different environments

### ❌ DON'T:
- Hardcode credentials in source code
- Commit credentials to version control
- Share access keys between team members
- Use root account for applications
- Store credentials in plain text files accessible to others

## 🔧 Implementation Examples

### For Development (Using Profiles)
```bash
# Set up profile
aws configure --profile mayyam-dev

# Use in your test script
export AWS_PROFILE=mayyam-dev
./run_real_aws_tests.sh
```

### For CI/CD (Using Environment Variables)
```yaml
# GitHub Actions example
env:
  AWS_ACCESS_KEY_ID: ${{ secrets.AWS_ACCESS_KEY_ID }}
  AWS_SECRET_ACCESS_KEY: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
  AWS_DEFAULT_REGION: us-east-1
```

### For Production (Using IAM Roles)
```yaml
# ECS Task Definition
{
  "taskRoleArn": "arn:aws:iam::123456789012:role/MyAppRole",
  "executionRoleArn": "arn:aws:iam::123456789012:role/MyExecutionRole"
}
```

## 🧹 Cleanup and Rotation

### Rotate Access Keys
```bash
# Create new access key
aws iam create-access-key --user-name MyUser

# Update your configuration with new key
aws configure --profile dev

# Delete old access key
aws iam delete-access-key --access-key-id OLD_KEY_ID
```

### Clean Up Unused Credentials
```bash
# List all access keys
aws iam list-access-keys

# Delete unused keys
aws iam delete-access-key --access-key-id KEY_TO_DELETE
```

## 🔍 Monitoring and Auditing

### Check Current Identity
```bash
aws sts get-caller-identity
```

### Monitor Key Usage
```bash
# Check when keys were last used
aws iam list-access-keys
aws iam get-access-key-last-used --access-key-id YOUR_KEY_ID
```

## 🚨 Emergency Actions

### If Credentials Are Compromised:
1. **Immediately rotate** all affected access keys
2. **Delete** the compromised keys
3. **Review** CloudTrail logs for unauthorized access
4. **Update** all systems using the old credentials
5. **Notify** security team if applicable

## 📚 Additional Resources

- [AWS IAM Best Practices](https://docs.aws.amazon.com/IAM/latest/UserGuide/best-practices.html)
- [AWS CLI Configuration](https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-files.html)
- [IAM Roles for EC2](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/iam-roles-for-amazon-ec2.html)
- [AWS SSO User Guide](https://docs.aws.amazon.com/singlesignon/latest/userguide/what-is.html)
