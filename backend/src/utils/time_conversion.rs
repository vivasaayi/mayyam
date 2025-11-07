use aws_smithy_types::DateTime as AwsDateTime;
use chrono::{DateTime, Utc};

/// Extension trait to add a to_chrono_utc method to AWS SDK DateTime
pub trait AwsDateTimeExt {
    fn to_chrono_utc(&self) -> DateTime<Utc>;
}

impl AwsDateTimeExt for AwsDateTime {
    fn to_chrono_utc(&self) -> DateTime<Utc> {
        from_aws_datetime(self)
    }
}

/// Convert chrono DateTime to AWS SDK DateTime
pub fn to_aws_datetime(dt: &DateTime<Utc>) -> AwsDateTime {
    let millis = dt.timestamp_millis();
    AwsDateTime::from_millis(millis)
}

/// Convert AWS SDK DateTime to chrono DateTime
pub fn from_aws_datetime(dt: &AwsDateTime) -> DateTime<Utc> {
    match dt.to_millis() {
        Ok(millis) => datetime_from_millis(millis),
        Err(_) => Utc::now(),
    }
}

/// Convert timestamp millis to chrono DateTime
pub fn timestamp_millis_to_datetime(millis: i64) -> DateTime<Utc> {
    datetime_from_millis(millis)
}

/// Convert i64 seconds to AWS SDK DateTime
pub fn seconds_to_aws_datetime(seconds: i64) -> AwsDateTime {
    AwsDateTime::from_secs(seconds)
}

/// Convert i64 millis to AWS SDK DateTime
pub fn millis_to_aws_datetime(millis: i64) -> AwsDateTime {
    AwsDateTime::from_millis(millis)
}

fn datetime_from_millis(millis: i64) -> DateTime<Utc> {
    let seconds = millis.div_euclid(1000);
    let nanos = (millis.rem_euclid(1000) * 1_000_000) as u32;
    DateTime::<Utc>::from_timestamp(seconds, nanos).unwrap_or_else(|| Utc::now())
}
