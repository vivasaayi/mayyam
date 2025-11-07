pub mod dynamodb;
pub mod ec2;
pub mod elasticache;
pub mod kinesis;
pub mod rds;
pub mod s3;

pub use dynamodb::DynamoDbAnalyzer;
pub use ec2::Ec2Analyzer;
pub use elasticache::ElastiCacheAnalyzer;
pub use kinesis::KinesisAnalyzer;
pub use rds::RdsAnalyzer;
pub use s3::S3Analyzer;
