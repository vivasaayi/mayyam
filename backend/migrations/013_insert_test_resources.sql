-- Insert test RDS instance records
INSERT INTO aws_resources (
    id, 
    account_id, 
    profile, 
    region, 
    resource_type, 
    resource_id, 
    arn, 
    name, 
    tags, 
    resource_data, 
    created_at, 
    updated_at, 
    last_refreshed
) VALUES
(
    '11111111-1111-1111-1111-111111111111',
    '123456789012',
    'default',
    'us-east-1',
    'RdsInstance',
    'db-instance-1',
    'arn:aws:rds:us-east-1:123456789012:db:db-instance-1',
    'production-postgres',
    '{"Environment": "Production", "Department": "Engineering"}',
    '{"Engine": "postgres", "EngineVersion": "13.7", "DBInstanceClass": "db.t3.medium", "AllocatedStorage": 100, "MultiAZ": true}',
    NOW(),
    NOW(),
    NOW()
),
(
    '22222222-2222-2222-2222-222222222222',
    '123456789012',
    'default',
    'us-east-2',
    'RdsInstance',
    'db-instance-2',
    'arn:aws:rds:us-east-2:123456789012:db:db-instance-2',
    'staging-mysql',
    '{"Environment": "Staging", "Department": "QA"}',
    '{"Engine": "mysql", "EngineVersion": "8.0.28", "DBInstanceClass": "db.t3.large", "AllocatedStorage": 50, "MultiAZ": false}',
    NOW(),
    NOW(),
    NOW()
),
(
    '33333333-3333-3333-3333-333333333333',
    '123456789012',
    'default',
    'us-west-1',
    'RdsInstance',
    'db-instance-3',
    'arn:aws:rds:us-west-1:123456789012:db:db-instance-3',
    'dev-postgres',
    '{"Environment": "Development", "Department": "Engineering"}',
    '{"Engine": "postgres", "EngineVersion": "14.3", "DBInstanceClass": "db.t3.small", "AllocatedStorage": 20, "MultiAZ": false}',
    NOW(),
    NOW(),
    NOW()
);

-- Insert a couple of EC2 instance records
INSERT INTO aws_resources (
    id, 
    account_id, 
    profile, 
    region, 
    resource_type, 
    resource_id, 
    arn, 
    name, 
    tags, 
    resource_data, 
    created_at, 
    updated_at, 
    last_refreshed
) VALUES
(
    '44444444-4444-4444-4444-444444444444',
    '123456789012',
    'default',
    'us-east-1',
    'EC2Instance',
    'i-0abc123def456789',
    'arn:aws:ec2:us-east-1:123456789012:instance/i-0abc123def456789',
    'web-server-1',
    '{"Environment": "Production", "Role": "WebServer"}',
    '{"InstanceType": "t3.large", "State": {"Name": "running"}, "PrivateIpAddress": "10.0.1.123", "PublicIpAddress": "54.123.45.67"}',
    NOW(),
    NOW(),
    NOW()
),
(
    '55555555-5555-5555-5555-555555555555',
    '123456789012',
    'default',
    'us-east-1',
    'EC2Instance',
    'i-0def456789abc0123',
    'arn:aws:ec2:us-east-1:123456789012:instance/i-0def456789abc0123',
    'app-server-1',
    '{"Environment": "Production", "Role": "ApplicationServer"}',
    '{"InstanceType": "c5.2xlarge", "State": {"Name": "running"}, "PrivateIpAddress": "10.0.2.45", "PublicIpAddress": null}',
    NOW(),
    NOW(),
    NOW()
);

-- Insert S3 bucket record
INSERT INTO aws_resources (
    id, 
    account_id, 
    profile, 
    region, 
    resource_type, 
    resource_id, 
    arn, 
    name, 
    tags, 
    resource_data, 
    created_at, 
    updated_at, 
    last_refreshed
) VALUES
(
    '66666666-6666-6666-6666-666666666666',
    '123456789012',
    'default',
    'us-east-1',
    'S3Bucket',
    'my-data-bucket',
    'arn:aws:s3:::my-data-bucket',
    'my-data-bucket',
    '{"Environment": "Production", "Department": "Data"}',
    '{"CreationDate": "2023-01-15T10:00:00Z", "VersioningEnabled": true, "BucketEncryption": {"SSEAlgorithm": "AES256"}}',
    NOW(),
    NOW(),
    NOW()
);
