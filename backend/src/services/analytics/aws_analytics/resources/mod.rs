pub mod ec2;
pub mod rds;
pub mod dynamodb;
pub mod elasticache;
pub mod kinesis;
pub mod s3;

pub use ec2::Ec2Analyzer;
pub use rds::RdsAnalyzer;
pub use dynamodb::DynamoDbAnalyzer;
pub use elasticache::ElastiCacheAnalyzer;
pub use kinesis::KinesisAnalyzer;
pub use s3::S3Analyzer;