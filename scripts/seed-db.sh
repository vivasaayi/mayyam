#!/usr/bin/env bash
set -euo pipefail

SQL_FILE=${1:-backend/test_data/seed.sql}
if [ ! -f "$SQL_FILE" ]; then
  echo "No SQL seed file found at $SQL_FILE" >&2
  exit 1
fi

echo "Seeding Postgres with $SQL_FILE"
docker compose exec -T postgres psql -U postgres -d mayyam -f - < "$SQL_FILE"

echo "Seeding MySQL with $SQL_FILE"
docker compose exec -T mysql mysql -u root -prootpassword mayyam_db < "$SQL_FILE"

echo "Database seed complete"
