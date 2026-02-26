#!/usr/bin/env bash
set -e

echo "Starting Dev Integration Tests Framework..."
docker-compose -p mayyam-integration -f docker-compose.dev.yml up --build --abort-on-container-exit --exit-code-from integration-tests
# Keeping containers to let us read logs if needed
echo "Done!"
