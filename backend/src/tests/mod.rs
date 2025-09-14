#[cfg(test)]
mod aws_account_test;

#[cfg(test)]
mod kinesis_api_tests;

#[cfg(test)]
mod kinesis_unit_tests;

#[cfg(test)]
#[cfg(feature = "integration-tests")]
mod integration;
