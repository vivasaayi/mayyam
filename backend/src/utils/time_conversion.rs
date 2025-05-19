use chrono::{DateTime, Utc};
use aws_smithy_types::DateTime as AwsDateTime;

pub fn to_aws_datetime(dt: &DateTime<Utc>) -> AwsDateTime {
    AwsDateTime::from_secs(dt.timestamp())
}

pub fn from_aws_datetime(dt: &AwsDateTime) -> DateTime<Utc> {
    DateTime::from_timestamp(dt.secs(), 0).unwrap_or_else(|| Utc::now())
}
