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


use crate::controllers::aws_account;
use actix_web::{web, Scope};

pub fn configure() -> Scope {
    web::scope("/accounts")
        // List all AWS accounts
        .route("", web::get().to(aws_account::list_accounts))
        // Create new AWS account
        .route("", web::post().to(aws_account::create_account))
        // Get a specific AWS account
        .route("/{id}", web::get().to(aws_account::get_account))
        // Update an AWS account
        .route("/{id}", web::put().to(aws_account::update_account))
        // Delete an AWS account
        .route("/{id}", web::delete().to(aws_account::delete_account))
        // Sync resources for an AWS account
        .route(
            "/{id}/sync",
            web::post().to(aws_account::sync_account_resources),
        )
        // Sync resources for all AWS accounts
        .route(
            "/sync",
            web::post().to(aws_account::sync_all_accounts_resources),
        )
}
