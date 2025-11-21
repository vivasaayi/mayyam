SHELL := /bin/bash
.PHONY: test backend frontend

test: backend frontend

backend:
	@echo "Running backend tests..."
	@cd backend && export DATABASE_URL=${DATABASE_URL:-sqlite::memory:} && cargo test --workspace --all-targets

frontend:
	@echo "Running frontend tests..."
	@cd frontend && npm ci && CI=true npm test -- --watchAll=false
