#!/usr/bin/env bash
set -euo pipefail

echo "Starting integration tests using docker-compose.local.yml"

# Build and start testing dependencies
docker compose -f docker-compose.local.yml up --build -d postgres mysql zookeeper kafka localstack backend

echo "Waiting for services to be ready (postgres, mysql, kafka, localstack)"
chmod +x ./scripts/wait-for-services.sh
./scripts/wait-for-services.sh
# Wait for DB-specific readiness (try to run a test query)
chmod +x ./scripts/wait-for-db.sh
./scripts/wait-for-db.sh

echo "Bootstrapping Localstack resources"
chmod +x ./scripts/bootstrap-localstack.sh
./scripts/bootstrap-localstack.sh

# Seed database if test data exists
if [ -f backend/test_data/seed.sql ]; then
	chmod +x ./scripts/seed-db.sh
	./scripts/seed-db.sh backend/test_data/seed.sql
fi

echo "Running integration-tests container"
# Run the integration-tests container and *capture* its exit code.
docker compose -f docker-compose.local.yml run --rm integration-tests

RC=$?

echo "Integration tests finished with return code: $RC"

echo "Tearing down docker-compose test stack"
docker compose -f docker-compose.local.yml down --volumes --remove-orphans

echo "Collecting docker-compose logs"
docker compose -f docker-compose.local.yml logs --no-color > docker-compose.integration.log || true

# If junit results exist, print a short summary
if [ -f backend/test-results/junit.xml ]; then
	echo "Found junit report at backend/test-results/junit.xml"
	chmod +r backend/test-results/junit.xml || true
	echo "Summarizing junit results:"
	./scripts/parse-junit.sh backend/test-results/junit.xml || true
else
	echo "No junit xml found at backend/test-results/junit.xml"
fi

exit $RC
