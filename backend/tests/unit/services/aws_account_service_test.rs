use std::sync::Arc;

use mayyam::errors::AppError;
use mayyam::models::aws_account::{AwsAccountCreateDto, AwsAccountDto, AwsAccountUpdateDto};
use mayyam::repositories::aws_account::AwsAccountRepository;
use mayyam::repositories::sync_run::SyncRunRepository;
use mayyam::services::aws::AwsControlPlane;
use mayyam::services::aws_account::AwsAccountService;
use rstest::*;
use serial_test::serial;

use crate::common::test_utils::{factories, get_test_db};

/// Mock AWS Control Plane for testing
#[cfg(test)]
use mockall::mock;

#[cfg(test)]
mock! {
    pub AwsControlPlaneImpl {}
    impl Clone for AwsControlPlaneImpl {
        fn clone(&self) -> Self;
    }
}

/// Test AWS Account Service
#[cfg(test)]
mod aws_account_service_tests {
    use super::*;

    /// Test creating an account successfully
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_create_account_success() {
        let test_db = get_test_db().await;
        let repo = Arc::new(AwsAccountRepository::new(Arc::new(test_db.conn().clone())));

        // Create mock AWS control plane
        let mut mock_control_plane = MockAwsControlPlaneImpl::new();
        mock_control_plane.expect_clone().returning(|| MockAwsControlPlaneImpl::new());

    let sync_repo = Arc::new(SyncRunRepository::new(Arc::new(test_db.conn().clone())));
    let service = AwsAccountService::new(repo, Arc::new(mock_control_plane), sync_repo);

        let create_dto = factories::fake_aws_account();

        let result = service.create_account(create_dto.clone()).await.unwrap();

        assert_eq!(result.account_id, create_dto.account_id);
        assert_eq!(result.account_name, create_dto.account_name);
        assert_eq!(result.default_region, create_dto.default_region);
    }

    /// Test creating account with missing credentials
    #[rstest]
    #[tokio::test]
    async fn test_create_account_missing_credentials() {
        let test_db = get_test_db().await;
        let repo = Arc::new(AwsAccountRepository::new(Arc::new(test_db.conn().clone())));

        let mut mock_control_plane = MockAwsControlPlaneImpl::new();
        mock_control_plane.expect_clone().returning(|| MockAwsControlPlaneImpl::new());

    let sync_repo = Arc::new(SyncRunRepository::new(Arc::new(test_db.conn().clone())));
    let service = AwsAccountService::new(repo, Arc::new(mock_control_plane), sync_repo);

        let create_dto = AwsAccountCreateDto {
            account_id: "123456789012".to_string(),
            account_name: "Test Account".to_string(),
            profile: None,
            default_region: "us-east-1".to_string(),
            use_role: false,
            role_arn: None,
            external_id: None,
            access_key_id: None, // Missing
            secret_access_key: None, // Missing
        };

        let result = service.create_account(create_dto).await;
        assert!(result.is_err());

        if let Err(AppError::Validation(msg)) = result {
            assert!(msg.contains("credentials"));
        }
    }

    /// Test getting an account by ID
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_get_account_by_id() {
        let test_db = get_test_db().await;
        let repo = Arc::new(AwsAccountRepository::new(Arc::new(test_db.conn().clone())));

        let mut mock_control_plane = MockAwsControlPlaneImpl::new();
        mock_control_plane.expect_clone().returning(|| MockAwsControlPlaneImpl::new());

    let sync_repo = Arc::new(SyncRunRepository::new(Arc::new(test_db.conn().clone())));
    let service = AwsAccountService::new(repo.clone(), Arc::new(mock_control_plane), sync_repo);

        // Create an account first
        let create_dto = factories::fake_aws_account();
        let created = service.create_account(create_dto).await.unwrap();

        // Now get it back
        let retrieved = service.get_account(created.id).await.unwrap();

        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.account_id, created.account_id);
        assert_eq!(retrieved.account_name, created.account_name);
    }

    /// Test getting non-existent account
    #[rstest]
    #[tokio::test]
    async fn test_get_nonexistent_account() {
        let test_db = get_test_db().await;
        let repo = Arc::new(AwsAccountRepository::new(Arc::new(test_db.conn().clone())));

        let mut mock_control_plane = MockAwsControlPlaneImpl::new();
        mock_control_plane.expect_clone().returning(|| MockAwsControlPlaneImpl::new());

    let sync_repo = Arc::new(SyncRunRepository::new(Arc::new(test_db.conn().clone())));
    let service = AwsAccountService::new(repo, Arc::new(mock_control_plane), sync_repo);

        let non_existent_id = uuid::Uuid::new_v4();
        let result = service.get_account(non_existent_id).await;

        assert!(result.is_err());
        if let Err(AppError::NotFound(msg)) = result {
            assert!(msg.contains(&non_existent_id.to_string()));
        }
    }

    /// Test listing accounts
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_list_accounts() {
        let test_db = get_test_db().await;
        let repo = Arc::new(AwsAccountRepository::new(Arc::new(test_db.conn().clone())));

        let mut mock_control_plane = MockAwsControlPlaneImpl::new();
        mock_control_plane.expect_clone().returning(|| MockAwsControlPlaneImpl::new());

    let sync_repo = Arc::new(SyncRunRepository::new(Arc::new(test_db.conn().clone())));
    let service = AwsAccountService::new(repo.clone(), Arc::new(mock_control_plane), sync_repo);

        // Create multiple accounts
        let account1 = service.create_account(factories::fake_aws_account()).await.unwrap();
        let account2 = service.create_account(factories::fake_aws_account()).await.unwrap();

        let accounts = service.list_accounts().await.unwrap();

        assert!(accounts.len() >= 2);
        let account_ids: Vec<String> = accounts.iter().map(|a| a.account_id.clone()).collect();
        assert!(account_ids.contains(&account1.account_id));
        assert!(account_ids.contains(&account2.account_id));
    }

    /// Test updating an account
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_update_account() {
        let test_db = get_test_db().await;
        let repo = Arc::new(AwsAccountRepository::new(Arc::new(test_db.conn().clone())));

        let mut mock_control_plane = MockAwsControlPlaneImpl::new();
        mock_control_plane.expect_clone().returning(|| MockAwsControlPlaneImpl::new());

    let sync_repo = Arc::new(SyncRunRepository::new(Arc::new(test_db.conn().clone())));
    let service = AwsAccountService::new(repo.clone(), Arc::new(mock_control_plane), sync_repo);

        // Create account
        let create_dto = factories::fake_aws_account();
        let created = service.create_account(create_dto).await.unwrap();

        // Update account
        let update_dto = crate::models::aws_account::AwsAccountUpdateDto {
            account_name: Some("Updated Name".to_string()),
            default_region: Some("us-west-2".to_string()),
            profile: Some("updated-profile".to_string()),
            use_role: Some(false),
            role_arn: None,
            external_id: None,
            access_key_id: Some("NEW_KEY".to_string()),
            secret_access_key: Some("NEW_SECRET".to_string()),
        };

        let updated = service.update_account(created.id, update_dto).await.unwrap();

        assert_eq!(updated.id, created.id);
        assert_eq!(updated.account_name, "Updated Name");
        assert_eq!(updated.default_region, "us-west-2");
    }

    /// Test deleting an account
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_delete_account() {
        let test_db = get_test_db().await;
        let repo = Arc::new(AwsAccountRepository::new(Arc::new(test_db.conn().clone())));

        let mut mock_control_plane = MockAwsControlPlaneImpl::new();
        mock_control_plane.expect_clone().returning(|| MockAwsControlPlaneImpl::new());

    let sync_repo = Arc::new(SyncRunRepository::new(Arc::new(test_db.conn().clone())));
    let service = AwsAccountService::new(repo.clone(), Arc::new(mock_control_plane), sync_repo);

        // Create account
        let create_dto = factories::fake_aws_account();
        let created = service.create_account(create_dto).await.unwrap();

        // Delete account
        service.delete_account(created.id).await.unwrap();

        // Verify it's deleted
        let result = service.get_account(created.id).await;
        assert!(result.is_err());
    }

    /// Test account sync functionality
    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_sync_account_resources() {
        let test_db = get_test_db().await;
        let repo = Arc::new(AwsAccountRepository::new(Arc::new(test_db.conn().clone())));

        let mut mock_control_plane = MockAwsControlPlaneImpl::new();
        mock_control_plane.expect_clone().returning(|| MockAwsControlPlaneImpl::new());
        let sync_repo = Arc::new(SyncRunRepository::new(Arc::new(test_db.conn().clone())));
        let service = AwsAccountService::new(repo.clone(), Arc::new(mock_control_plane), sync_repo);

        // Attempting to sync a non-existent account should return a not found error
        let result = service
            .sync_account_resources(uuid::Uuid::new_v4(), uuid::Uuid::new_v4())
            .await;

        assert!(matches!(result, Err(AppError::NotFound(_))));
    }
}
