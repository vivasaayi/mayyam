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


use std::sync::Arc;
use rstest::*;
use uuid::Uuid;
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
use serial_test::serial;

use crate::models::aws_account::{Entity as AwsAccountEntity, Column as AwsAccountColumn};
use crate::repositories::aws_account::AwsAccountRepository;
use crate::errors::AppError;

use crate::test_utils::{TestDb, get_test_db};

/// Test AWS Account Repository
#[cfg(test)]
mod aws_account_repository_tests {
    use super::*;

    /// Test creating and retrieving an AWS account
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_create_and_get_account() {
        let test_db = get_test_db().await;
        let repo = AwsAccountRepository::new(Arc::new(test_db.conn().clone()));

        // Create test data
        let create_dto = factories::fake_aws_account();

        // Test creation
        let created = repo.create(create_dto.clone()).await.unwrap();
        assert_eq!(created.account_id, create_dto.account_id);
        assert_eq!(created.account_name, create_dto.account_name);

        // Test retrieval by ID
        let retrieved = repo.get_by_id(created.id).await.unwrap().unwrap();
        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.account_id, created.account_id);

        // Test retrieval by account ID
        let retrieved_by_account = repo.get_by_account_id(&created.account_id).await.unwrap().unwrap();
        assert_eq!(retrieved_by_account.id, created.id);
    }

    /// Test listing all accounts
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_list_accounts() {
        let test_db = get_test_db().await;
        let repo = AwsAccountRepository::new(Arc::new(test_db.conn().clone()));

        // Create multiple test accounts
        let account1 = factories::fake_aws_account();
        let account2 = factories::fake_aws_account();

        let created1 = repo.create(account1).await.unwrap();
        let created2 = repo.create(account2).await.unwrap();

        // Test listing all accounts
        let accounts = repo.get_all().await.unwrap();
        assert!(accounts.len() >= 2);

        // Verify our created accounts are in the list
        let account_ids: Vec<String> = accounts.iter().map(|a| a.account_id.clone()).collect();
        assert!(account_ids.contains(&created1.account_id));
        assert!(account_ids.contains(&created2.account_id));
    }

    /// Test updating an account
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_update_account() {
        let test_db = get_test_db().await;
        let repo = AwsAccountRepository::new(Arc::new(test_db.conn().clone()));

        // Create test account
        let create_dto = factories::fake_aws_account();
        let created = repo.create(create_dto).await.unwrap();

        // Update the account
        let update_dto = crate::models::aws_account::AwsAccountUpdateDto {
            account_name: Some("Updated Account Name".to_string()),
            default_region: Some("us-west-2".to_string()),
            profile: Some("updated-profile".to_string()),
            use_role: Some(false),
            role_arn: None,
            external_id: None,
            access_key_id: Some("NEW_ACCESS_KEY".to_string()),
            secret_access_key: Some("NEW_SECRET_KEY".to_string()),
        };

        let updated = repo.update(created.id, update_dto).await.unwrap();
        assert_eq!(updated.account_name, "Updated Account Name");
        assert_eq!(updated.default_region, "us-west-2");
        assert_eq!(updated.profile, Some("updated-profile".to_string()));
    }

    /// Test deleting an account
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_delete_account() {
        let test_db = get_test_db().await;
        let repo = AwsAccountRepository::new(Arc::new(test_db.conn().clone()));

        // Create test account
        let create_dto = factories::fake_aws_account();
        let created = repo.create(create_dto).await.unwrap();

        // Verify account exists
        let exists_before = repo.get_by_id(created.id).await.unwrap();
        assert!(exists_before.is_some());

        // Delete the account
        repo.delete(created.id).await.unwrap();

        // Verify account is deleted
        let exists_after = repo.get_by_id(created.id).await.unwrap();
        assert!(exists_after.is_none());
    }

    /// Test error handling for non-existent account
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_get_nonexistent_account() {
        let test_db = get_test_db().await;
        let repo = AwsAccountRepository::new(Arc::new(test_db.conn().clone()));

        let non_existent_id = Uuid::new_v4();
        let result = repo.get_by_id(non_existent_id).await.unwrap();

        assert!(result.is_none());
    }

    /// Test finding account by account ID
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_get_by_account_id() {
        let test_db = get_test_db().await;
        let repo = AwsAccountRepository::new(Arc::new(test_db.conn().clone()));

        let create_dto = factories::fake_aws_account();
        let account_id = create_dto.account_id.clone();
        let created = repo.create(create_dto).await.unwrap();

        let found = repo.get_by_account_id(&account_id).await.unwrap().unwrap();
        assert_eq!(found.id, created.id);
        assert_eq!(found.account_id, account_id);
    }

    /// Test validation - duplicate account ID
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_duplicate_account_id() {
        let test_db = get_test_db().await;
        let repo = AwsAccountRepository::new(Arc::new(test_db.conn().clone()));

        let create_dto1 = factories::fake_aws_account();
        let account_id = create_dto1.account_id.clone();

        // Create first account
        repo.create(create_dto1).await.unwrap();

        // Try to create second account with same ID
        let create_dto2 = crate::models::aws_account::AwsAccountCreateDto {
            account_id: account_id.clone(), // Same account ID
            account_name: "Different Name".to_string(),
            profile: Some("different-profile".to_string()),
            default_region: "us-west-2".to_string(),
            use_role: false,
            role_arn: None,
            external_id: None,
            access_key_id: Some("DIFFERENT_KEY".to_string()),
            secret_access_key: Some("DIFFERENT_SECRET".to_string()),
        };

        let result = repo.create(create_dto2).await;
        assert!(result.is_err());
        // This would depend on your database constraints
    }
}
