#!/usr/bin/env bash

# Mayyam Backend Test Runner
# This script runs all tests for the Mayyam backend with different configurations

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to run tests with coverage
run_tests_with_coverage() {
    print_status "Running tests with coverage..."

    if command -v cargo-tarpaulin &> /dev/null; then
        cargo tarpaulin --ignore-tests --out Html --output-dir ./target/coverage
        print_success "Coverage report generated in ./target/coverage/tarpaulin-report.html"
    else
        print_warning "cargo-tarpaulin not found. Install with: cargo install cargo-tarpaulin"
        run_tests
    fi
}

# Function to run unit tests
run_unit_tests() {
    print_status "Running unit tests..."

    cargo test --bin mayyam -- --test-threads=1
    print_success "Unit tests completed"
}

# Function to run integration tests
run_integration_tests() {
    print_status "Running integration tests..."

    # Set up test database if needed
    export DATABASE_URL="sqlite::memory:"

    cargo test --test integration -- --test-threads=1
    print_success "Integration tests completed"
}

# Function to run all tests
run_all_tests() {
    print_status "Running all tests..."

    cargo test -- --test-threads=1
    print_success "All tests completed"
}

# Function to run tests with specific pattern
run_tests_with_pattern() {
    local pattern=$1
    print_status "Running tests matching pattern: $pattern"

    cargo test $pattern -- --test-threads=1
    print_success "Tests matching '$pattern' completed"
}

# Function to check test coverage
check_coverage() {
    print_status "Checking test coverage..."

    if command -v cargo-tarpaulin &> /dev/null; then
        cargo tarpaulin --ignore-tests --out Lcov --output-dir ./target/coverage
        print_success "Coverage data generated in ./target/coverage/lcov.info"
    else
        print_warning "cargo-tarpaulin not found. Skipping coverage check."
    fi
}

# Function to run benchmarks
run_benchmarks() {
    print_status "Running benchmarks..."

    cargo bench
    print_success "Benchmarks completed"
}

# Function to run tests in watch mode
run_tests_watch() {
    print_status "Running tests in watch mode..."

    if command -v cargo-watch &> /dev/null; then
        cargo watch -x test
    else
        print_warning "cargo-watch not found. Install with: cargo install cargo-watch"
        print_error "Cannot run tests in watch mode without cargo-watch"
        exit 1
    fi
}

# Function to generate test report
generate_test_report() {
    print_status "Generating test report..."

    # Create test results directory
    mkdir -p test-results

    # Run tests with JUnit output if available
    if command -v cargo-nextest &> /dev/null; then
        cargo nextest run --profile ci --junit path=test-results/junit.xml
        print_success "Test report generated in test-results/junit.xml"
    else
        print_warning "cargo-nextest not found. Install with: cargo install cargo-nextest"
        print_warning "Running tests without JUnit output"
        run_all_tests
    fi
}

# Function to show help
show_help() {
    echo "Mayyam Backend Test Runner"
    echo ""
    echo "Usage: $0 [COMMAND]"
    echo ""
    echo "Commands:"
    echo "  all              Run all tests"
    echo "  unit             Run unit tests only"
    echo "  integration      Run integration tests only"
    echo "  coverage         Run tests with coverage"
    echo "  check-coverage   Check test coverage without running tests"
    echo "  bench            Run benchmarks"
    echo "  watch            Run tests in watch mode"
    echo "  report           Generate test report"
    echo "  pattern PATTERN  Run tests matching pattern"
    echo "  help             Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 all"
    echo "  $0 unit"
    echo "  $0 pattern aws_account"
    echo "  $0 coverage"
}

# Main script logic
case "${1:-all}" in
    "all")
        run_all_tests
        ;;
    "unit")
        run_unit_tests
        ;;
    "integration")
        run_integration_tests
        ;;
    "coverage")
        run_tests_with_coverage
        ;;
    "check-coverage")
        check_coverage
        ;;
    "bench")
        run_benchmarks
        ;;
    "watch")
        run_tests_watch
        ;;
    "report")
        generate_test_report
        ;;
    "pattern")
        if [ -z "$2" ]; then
            print_error "Pattern not specified. Usage: $0 pattern PATTERN"
            exit 1
        fi
        run_tests_with_pattern "$2"
        ;;
    "help"|"-h"|"--help")
        show_help
        ;;
    *)
        print_error "Unknown command: $1"
        echo ""
        show_help
        exit 1
        ;;
esac

print_success "Test execution completed!"
