#!/usr/bin/env bash
set -euo pipefail

echo "Stopping and removing docker compose test stack and volumes"
docker compose --profile dev --profile test down --volumes --remove-orphans || true
docker compose -f docker/docker-compose.integration.yml down --volumes --remove-orphans || true

echo "Cleaning up dangling docker volumes"
docker volume prune -f || true

echo "Cleanup complete"
