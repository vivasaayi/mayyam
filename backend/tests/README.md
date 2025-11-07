# Mayyam Backend Testing Framework

This document describes the comprehensive testing framework implemented for the Mayyam backend, a comprehensive toolbox for DevOps and SRE engineers.

## Overview

The testing framework follows a multi-layered approach with unit tests, integration tests, and end-to-end tests to ensure code quality and reliability.

## Test Structure

```
backend/tests/
├── unit/                          # Unit tests
│   ├── controllers/               # Controller unit tests
│   │   └── aws_account_controller_test.rs
│   ├── models/                    # Model unit tests
│   │   └── aws_models_test.rs
│   ├── repositories/              # Repository unit tests
│   │   └── aws_account_repository_test.rs
│   ├── services/                  # Service unit tests
│   │   └── aws_account_service_test.rs
│   └── utils/                     # Utility unit tests
│       └── utils_test.rs
├── integration/                   # Integration tests
│   └── api_integration_test.rs
├── e2e/                          # End-to-end tests (future)
├── test_utils.rs                 # Shared test utilities
└── README.md                     # This file
```

## Test Categories

### 1. Unit Tests

Unit tests focus on testing individual components in isolation:

- **Repository Tests**: Test data access layer with mocked database connections
- **Service Tests**: Test business logic with mocked dependencies
- **Controller Tests**: Test HTTP handlers with mocked services
- **Model Tests**: Test data structures and validation
- **Utility Tests**: Test helper functions and utilities

### 2. Integration Tests

Integration tests verify component interactions:

- **API Integration Tests**: Test complete HTTP request/response cycles
- **Database Integration Tests**: Test actual database operations
- **Service Integration Tests**: Test service-to-service communication

### 3. End-to-End Tests (Future)

E2E tests will verify complete user workflows from frontend to backend.

## Testing Tools and Libraries

### Core Testing Framework
- **Rust Standard Library**: Built-in `#[test]` and `#[cfg(test)]`
- **rstest**: Parameterized tests for comprehensive coverage
- **proptest**: Property-based testing for edge cases

### Mocking and Fakes
- **mockall**: Mocking framework for traits and structs
- **fake**: Fake data generation for tests
- **wiremock**: HTTP service mocking for external APIs

### Test Database
- **SQLite**: In-memory database for fast, isolated testing
- **testcontainers**: Docker containers for integration tests

### Test Organization
- **serial_test**: Run database tests serially to avoid conflicts
- **tokio-test**: Async test utilities
- **tempfile**: Temporary file creation for tests

## Running Tests

### Using the Test Runner Script

The `run_tests.sh` script provides convenient commands for running tests:

```bash
# Run all tests
./run_tests.sh all

# Run unit tests only
./run_tests.sh unit

# Run integration tests only
./run_tests.sh integration

# Run tests with coverage
./run_tests.sh coverage

# Run tests in watch mode (requires cargo-watch)
./run_tests.sh watch

# Generate test report
./run_tests.sh report

# Run tests matching a pattern
./run_tests.sh pattern aws_account
```

### Using Cargo Directly

```bash
# Run all tests
cargo test

# Run specific test module
cargo test --test aws_account_repository_test

# Run tests with output
cargo test -- --nocapture

# Run tests in release mode
cargo test --release

# Run benchmarks
cargo bench
```

### Test Configuration

Tests can be configured using environment variables:

```bash
# Database URL for integration tests
export DATABASE_URL="sqlite::memory:"

# Enable debug logging in tests
export RUST_LOG=debug

# Test-specific configuration
export TEST_MODE=true
```

## Test Database Setup

### Unit Tests
Unit tests use SQLite in-memory database for fast, isolated testing:

```rust
use crate::common::test_utils::setup_test_database;

#[tokio::test]
async fn test_my_function() {
    let db = setup_test_database().await;
    // Test logic here
}
```

### Integration Tests
Integration tests can use:
- SQLite for fast testing
- PostgreSQL/MySQL containers via testcontainers
- Actual test databases

## Mocking Strategy

### Service Layer Mocking

```rust
use mockall::mock;

#[mock]
trait AwsAccountRepositoryTrait {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<AwsAccount>, Error>;
    async fn create(&self, account: AwsAccountCreateDto) -> Result<AwsAccount, Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_with_mock() {
        let mut mock_repo = MockAwsAccountRepositoryTrait::new();

        mock_repo
            .expect_find_by_id()
            .returning(|_| Ok(Some(test_account())));

        let service = AwsAccountService::new(Arc::new(mock_repo));
        // Test service logic
    }
}
```

### External API Mocking

```rust
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_external_api_call() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/aws/accounts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(test_accounts()))
        .mount(&mock_server)
        .await;

    // Test code that calls the external API
}
```

## Test Data Management

### Fixtures

```rust
use rstest::rstest;

#[fixture]
fn test_aws_account() -> AwsAccount {
    AwsAccount {
        id: Uuid::new_v4(),
        account_id: "123456789012".to_string(),
        account_name: "Test Account".to_string(),
        // ... other fields
    }
}

#[rstest]
fn test_account_operations(#[from(test_aws_account)] account: AwsAccount) {
    // Test with fixture data
}
```

### Fake Data Generation

```rust
use fake::{Fake, Faker};

#[rstest]
fn test_with_fake_data() {
    let account_id: String = Faker.fake();
    let account_name: String = (8..20).fake(); // Random string 8-20 chars

    // Use fake data in test
}
```

## Test Coverage

### Coverage Tools

```bash
# Install cargo-tarpaulin
cargo install cargo-tarpaulin

# Generate HTML coverage report
cargo tarpaulin --out Html --output-dir ./target/coverage

# Generate LCOV for CI/CD
cargo tarpaulin --out Lcov --output-dir ./target/coverage
```

### Coverage Goals

- **Unit Tests**: >80% coverage
- **Integration Tests**: >70% coverage
- **Critical Paths**: 100% coverage

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run tests
        run: cargo test --verbose
      - name: Generate coverage
        run: cargo tarpaulin --out Lcov --output-dir ./target/coverage
      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          file: ./target/coverage/lcov.info
```

## Best Practices

### Test Organization
1. **One test file per module**: Keep tests co-located with code
2. **Descriptive test names**: Use clear, descriptive names
3. **Arrange-Act-Assert**: Follow AAA pattern in tests
4. **Independent tests**: Each test should be independent

### Test Performance
1. **Fast unit tests**: Use in-memory databases
2. **Parallel execution**: Use `cargo test -- --test-threads=N`
3. **Selective testing**: Run only affected tests in CI
4. **Resource cleanup**: Clean up resources after tests

### Test Reliability
1. **Deterministic tests**: Avoid flaky tests
2. **Proper mocking**: Mock external dependencies
3. **Test data isolation**: Use unique test data
4. **Error handling**: Test error conditions

## Debugging Tests

### Common Issues

1. **Async test deadlocks**: Use `#[tokio::test]` properly
2. **Database connection issues**: Ensure proper cleanup
3. **Mock setup errors**: Verify mock expectations
4. **Race conditions**: Use serial_test for database tests

### Debugging Tools

```bash
# Run specific test with output
cargo test test_name -- --nocapture

# Run tests with backtrace
RUST_BACKTRACE=1 cargo test

# Debug specific test
cargo test test_name -- --nocapture --exact
```

## Future Enhancements

### Planned Improvements
1. **E2E Test Suite**: Complete end-to-end testing with Playwright
2. **Performance Testing**: Load testing and performance benchmarks
3. **Contract Testing**: API contract validation
4. **Visual Testing**: UI component testing
5. **Chaos Testing**: Fault injection testing

### Tool Integration
1. **TestRail/Jira**: Test case management
2. **Allure Reports**: Enhanced test reporting
3. **SonarQube**: Code quality integration
4. **OWASP ZAP**: Security testing integration

## Contributing

When adding new code:

1. **Write tests first**: Follow TDD principles
2. **Maintain coverage**: Ensure new code is tested
3. **Update documentation**: Keep test docs current
4. **Follow patterns**: Use established testing patterns

### Code Review Checklist
- [ ] Unit tests added for new functions
- [ ] Integration tests for new endpoints
- [ ] Test coverage maintained
- [ ] Test documentation updated
- [ ] CI/CD pipeline passes

## Support

For questions about the testing framework:

1. Check this README first
2. Review existing test examples
3. Ask in the development channel
4. Create an issue for framework improvements

---

This testing framework ensures the reliability and maintainability of the Mayyam SRE toolbox through comprehensive, automated testing at all levels.
