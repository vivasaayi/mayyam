use crate::errors::AppError;
use crate::models::aws_account::AwsAccountDto;
use aws_config::{BehaviorVersion, Region};
use aws_credential_types::Credentials;

pub struct AwsConfigService {}

impl AwsConfigService {
    pub fn new() -> Self {
        AwsConfigService {}
    }

    // This can be moved along with the AwsAccountDto
    // Like aws_account_dto.to_sdk_config()
    pub async fn get_aws_config(
        &self,
        region: &str,
        aws_account_dto: &AwsAccountDto,
    ) -> Result<aws_config::SdkConfig, AppError> {
        let mut builder =
            aws_config::defaults(BehaviorVersion::latest()).region(Region::new(region.to_string()));

        if let Some(profile) = aws_account_dto.profile.as_ref() {
            builder = builder.profile_name(profile.as_str());
        } else if aws_account_dto.has_access_key {
            builder = builder.credentials_provider(
                Credentials::builder()
                    .access_key_id(
                        aws_account_dto
                            .access_key_id
                            .as_ref()
                            .unwrap()
                            .clone()
                            .as_str(),
                    )
                    .secret_access_key(
                        aws_account_dto
                            .secret_access_key
                            .as_ref()
                            .unwrap()
                            .clone()
                            .as_str(),
                    )
                    .build(),
            );
        }

        let config = builder.load().await;

        return Ok(config);
    }
}
