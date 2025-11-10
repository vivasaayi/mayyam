// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


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
