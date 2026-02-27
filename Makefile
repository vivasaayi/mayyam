SHELL := /bin/bash
.PHONY: test backend frontend
integration:
	@echo "Running integration tests via docker-compose"
	@scripts/run-integration-tests.sh

integration-ci:
	@echo "Running integration tests and fail on junit failures"
	@./scripts/run-integration-tests.sh; RC=$$?; \
	if [ -f backend/test-results/junit.xml ]; then ./scripts/parse-junit.sh backend/test-results/junit.xml || exit 1; fi; \
	exit $$RC

cleanup:
	@echo "Cleaning volumes and stopping services"
	@./scripts/cleanup-test-volumes.sh

seed:
	@echo "Seeding test DBs"
	@./scripts/seed-db.sh backend/test_data/seed.sql

test: backend frontend

backend:
	@echo "Running backend tests..."
	@cd backend && export DATABASE_URL=${DATABASE_URL:-sqlite::memory:} && cargo test --workspace --all-targets

frontend:
	@echo "Running frontend tests..."
	@cd frontend && npm ci && CI=true npm test -- --watchAll=false
