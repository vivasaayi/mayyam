#!/usr/bin/env bash
set -euo pipefail

# Wait until Postgres and MySQL are ready by attempting a simple SQL query
# When run in CI, it will use `docker compose exec` to run the check in container.

DB_WAIT_TIMEOUT=${1:-60}

echo "Waiting for Postgres to accept connections..."
if command -v psql >/dev/null 2>&1; then
  echo "Using local psql to check Postgres"
  export PGPASSWORD=${PGPASSWORD:-postgres}
  attempts=0
  until psql -h ${PG_HOST:-localhost} -U ${PG_USER:-postgres} -d ${PG_DATABASE:-mayyam} -c 'SELECT 1;' >/dev/null 2>&1; do
    attempts=$((attempts + 1))
    if [ $attempts -ge $DB_WAIT_TIMEOUT ]; then
      echo "Timed out waiting for Postgres" >&2; exit 1
    fi
    sleep 1
  done
else
  echo "psql not found, using docker compose to run Postgres check"
  attempts=0
  until docker compose exec -T postgres psql -U postgres -d mayyam -c 'SELECT 1;' >/dev/null 2>&1; do
    attempts=$((attempts + 1))
    if [ $attempts -ge $DB_WAIT_TIMEOUT ]; then
      echo "Timed out waiting for Postgres (docker compose)" >&2; exit 1
    fi
    sleep 1
  done
fi

echo "Postgres is ready"

echo "Waiting for MySQL to accept connections..."
if command -v mysql >/dev/null 2>&1; then
  echo "Using local mysql client to check MySQL"
  attempts=0
  until mysql -h ${MYSQL_HOST:-localhost} -u ${MYSQL_USER:-root} -p${MYSQL_PASSWORD:-rootpassword} -e 'SELECT 1;' >/dev/null 2>&1; do
    attempts=$((attempts + 1))
    if [ $attempts -ge $DB_WAIT_TIMEOUT ]; then
      echo "Timed out waiting for MySQL" >&2; exit 1
    fi
    sleep 1
  done
else
  echo "mysql client not found, using docker compose to run MySQL check"
  attempts=0
  until docker compose exec -T mysql mysql -u root -prootpassword -e 'SELECT 1;' >/dev/null 2>&1; do
    attempts=$((attempts + 1))
    if [ $attempts -ge $DB_WAIT_TIMEOUT ]; then
      echo "Timed out waiting for MySQL (docker compose)" >&2; exit 1
    fi
    sleep 1
  done
fi

echo "MySQL is ready"
