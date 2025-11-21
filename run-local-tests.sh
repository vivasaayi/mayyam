#!/usr/bin/env bash
set -euo pipefail

# run-local-tests.sh — Run both backend (Rust) and frontend (JS) unit tests locally.
# Usage: ./run-local-tests.sh [--skip-backend] [--skip-frontend]

skip_backend=false
skip_frontend=false

while [[ $# -gt 0 ]]; do
  case $1 in
    --skip-backend) skip_backend=true; shift ;;
    --skip-frontend) skip_frontend=true; shift ;;
    -h|--help)
      echo "Usage: $0 [--skip-backend] [--skip-frontend]"
      exit 0
      ;;
    *) echo "Unknown arg: $1"; exit 1 ;;
  esac
done

echo "=== run-local-tests.sh — starting unit tests ==="

if [[ "$skip_backend" != true ]]; then
  echo "\n--- Backend: Running Rust unit tests ---"
  if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo is not installed — please install Rust toolchain (https://rustup.rs/)" >&2
    exit 2
  fi

  pushd backend >/dev/null
  # Run unit tests in CI-friendly environment; use in-memory sqlite by default if not set.
  export DATABASE_URL=${DATABASE_URL:-"sqlite::memory:"}
  export RUST_LOG=${RUST_LOG:-"info"}
  # Avoid using real AWS credentials for unit tests — enforce empty values to prevent accidental usage
  export AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID:-""}
  export AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY:-""}

  echo "Running cargo test (backend)..."
  cargo test --workspace --all-targets
  popd >/dev/null
  echo "--- Backend tests complete ---"
fi

if [[ "$skip_frontend" != true ]]; then
  echo "\n--- Frontend: Running JS unit tests ---"
  if ! command -v node >/dev/null 2>&1; then
    echo "Node.js is not installed — please install Node.js (https://nodejs.org/)" >&2
    exit 3
  fi

  pushd frontend >/dev/null
  if [[ ! -d node_modules ]]; then
    echo "node_modules not found — installing dependencies with npm ci"
    npm ci
  fi

  # Use CI=true to force non-interactive test runs
  echo "Running npm test (frontend)..."
  CI=true npm test -- --watchAll=false
  popd >/dev/null
  echo "--- Frontend tests complete ---"
fi

echo "=== All tests finished successfully ==="
