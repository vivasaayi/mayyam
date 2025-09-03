use rstest::*;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

use crate::utils::aws::{validate_aws_account_id, validate_aws_region, extract_account_id_from_arn};
use crate::utils::jwt::{create_jwt_token, validate_jwt_token};
use crate::utils::encryption::{encrypt_data, decrypt_data};
use crate::utils::validation::{validate_email, validate_uuid};

/// Test AWS utility functions
#[cfg(test)]
mod aws_utils_tests {
    use super::*;

    /// Test AWS account ID validation
    #[rstest]
    #[case("123456789012", true)]
    #[case("12345678901", false)]  // Too short
    #[case("1234567890123", false)] // Too long
    #[case("12345678901a", false)] // Contains letter
    #[case("000000000000", true)] // Valid zeros
    fn test_validate_aws_account_id(#[case] account_id: &str, #[case] expected: bool) {
        assert_eq!(validate_aws_account_id(account_id), expected);
    }

    /// Test AWS region validation
    #[rstest]
    #[case("us-east-1", true)]
    #[case("eu-west-1", true)]
    #[case("ap-southeast-1", true)]
    #[case("invalid-region", false)]
    #[case("", false)]
    #[case("us-east", false)]
    fn test_validate_aws_region(#[case] region: &str, #[case] expected: bool) {
        assert_eq!(validate_aws_region(region), expected);
    }

    /// Test ARN account ID extraction
    #[rstest]
    #[case("arn:aws:iam::123456789012:user/username", Some("123456789012"))]
    #[case("arn:aws:s3:::my-bucket", None)] // No account ID in S3 bucket ARN
    #[case("arn:aws:ec2:us-east-1:123456789012:instance/i-123", Some("123456789012"))]
    #[case("invalid-arn", None)]
    fn test_extract_account_id_from_arn(#[case] arn: &str, #[case] expected: Option<&str>) {
        assert_eq!(extract_account_id_from_arn(arn), expected.map(|s| s.to_string()));
    }
}

/// Test JWT utility functions
#[cfg(test)]
mod jwt_utils_tests {
    use super::*;

    /// Test JWT token creation and validation
    #[rstest]
    fn test_jwt_token_creation_and_validation() {
        let claims = HashMap::from([
            ("user_id".to_string(), "123".to_string()),
            ("role".to_string(), "admin".to_string()),
        ]);

        // Create token
        let token = create_jwt_token(claims.clone()).unwrap();

        // Validate token
        let decoded_claims = validate_jwt_token(&token).unwrap();

        assert_eq!(decoded_claims.get("user_id"), Some(&"123".to_string()));
        assert_eq!(decoded_claims.get("role"), Some(&"admin".to_string()));
    }

    /// Test JWT with expiration
    #[rstest]
    fn test_jwt_token_expiration() {
        let mut claims = HashMap::from([
            ("user_id".to_string(), "123".to_string()),
        ]);

        // Set expiration to past time
        let past_time = Utc::now().timestamp() - 3600; // 1 hour ago
        claims.insert("exp".to_string(), past_time.to_string());

        let token = create_jwt_token(claims).unwrap();

        // This should fail due to expiration
        let result = validate_jwt_token(&token);
        assert!(result.is_err());
    }

    /// Test invalid JWT token
    #[rstest]
    fn test_invalid_jwt_token() {
        let invalid_token = "invalid.jwt.token";

        let result = validate_jwt_token(invalid_token);
        assert!(result.is_err());
    }
}

/// Test encryption utility functions
#[cfg(test)]
mod encryption_utils_tests {
    use super::*;

    /// Test data encryption and decryption
    #[rstest]
    fn test_encrypt_decrypt_data() {
        let original_data = "sensitive information";
        let key = "test-encryption-key-32-chars-long";

        // Encrypt data
        let encrypted = encrypt_data(original_data, key).unwrap();

        // Decrypt data
        let decrypted = decrypt_data(&encrypted, key).unwrap();

        assert_eq!(decrypted, original_data);
        assert_ne!(encrypted, original_data); // Ensure data was actually encrypted
    }

    /// Test encryption with different keys
    #[rstest]
    fn test_encrypt_with_different_keys() {
        let data = "test data";
        let key1 = "key1-32-chars-long-for-encryption";
        let key2 = "key2-32-chars-long-for-encryption";

        let encrypted1 = encrypt_data(data, key1).unwrap();
        let encrypted2 = encrypt_data(data, key2).unwrap();

        // Same data with different keys should produce different encrypted results
        assert_ne!(encrypted1, encrypted2);

        // But should decrypt correctly with respective keys
        let decrypted1 = decrypt_data(&encrypted1, key1).unwrap();
        let decrypted2 = decrypt_data(&encrypted2, key2).unwrap();

        assert_eq!(decrypted1, data);
        assert_eq!(decrypted2, data);
    }

    /// Test decryption with wrong key
    #[rstest]
    fn test_decrypt_with_wrong_key() {
        let data = "test data";
        let correct_key = "correct-key-32-chars-long-123";
        let wrong_key = "wrong-key-32-chars-long-456";

        let encrypted = encrypt_data(data, correct_key).unwrap();

        // Decryption with wrong key should fail
        let result = decrypt_data(&encrypted, wrong_key);
        assert!(result.is_err());
    }
}

/// Test validation utility functions
#[cfg(test)]
mod validation_utils_tests {
    use super::*;

    /// Test email validation
    #[rstest]
    #[case("user@example.com", true)]
    #[case("test.email+tag@domain.co.uk", true)]
    #[case("user@subdomain.domain.com", true)]
    #[case("invalid-email", false)]
    #[case("@example.com", false)]
    #[case("user@", false)]
    #[case("user@.com", false)]
    fn test_validate_email(#[case] email: &str, #[case] expected: bool) {
        assert_eq!(validate_email(email), expected);
    }

    /// Test UUID validation
    #[rstest]
    #[case("550e8400-e29b-41d4-a716-446655440000", true)]
    #[case("6ba7b810-9dad-11d1-80b4-00c04fd430c8", true)]
    #[case("invalid-uuid", false)]
    #[case("550e8400-e29b-41d4-a716", false)] // Too short
    #[case("550e8400-e29b-41d4-a716-446655440000-extra", false)] // Too long
    fn test_validate_uuid(#[case] uuid_str: &str, #[case] expected: bool) {
        assert_eq!(validate_uuid(uuid_str), expected);
    }
}

/// Test common utility functions
#[cfg(test)]
mod common_utils_tests {
    use super::*;
    use crate::utils::common::{format_timestamp, generate_random_string, hash_string};

    /// Test timestamp formatting
    #[rstest]
    fn test_format_timestamp() {
        let dt = DateTime::parse_from_rfc3339("2024-01-01T12:00:00Z").unwrap().with_timezone(&Utc);

        let formatted = format_timestamp(dt);
        assert!(formatted.contains("2024"));
        assert!(formatted.contains("12:00:00"));
    }

    /// Test random string generation
    #[rstest]
    fn test_generate_random_string() {
        let length = 16;
        let str1 = generate_random_string(length);
        let str2 = generate_random_string(length);

        assert_eq!(str1.len(), length);
        assert_eq!(str2.len(), length);
        assert_ne!(str1, str2); // Should be different

        // Should contain only alphanumeric characters
        assert!(str1.chars().all(|c| c.is_alphanumeric()));
        assert!(str2.chars().all(|c| c.is_alphanumeric()));
    }

    /// Test string hashing
    #[rstest]
    fn test_hash_string() {
        let input = "test input";
        let hash1 = hash_string(input);
        let hash2 = hash_string(input);

        assert_eq!(hash1, hash2); // Same input should produce same hash
        assert_eq!(hash1.len(), 64); // SHA-256 produces 64 character hex string

        // Different input should produce different hash
        let different_hash = hash_string("different input");
        assert_ne!(hash1, different_hash);
    }
}
