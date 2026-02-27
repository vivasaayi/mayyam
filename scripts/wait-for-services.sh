#!/usr/bin/env bash
set -euo pipefail

# Wait for service availability (host:port) with timeout
wait_for() {
  local host_port=$1
  local timeout=${2:-60}

  local host=${host_port%%:*}
  local port=${host_port##*:}

  local start=$(date +%s)
  while true; do
    if nc -z "$host" "$port" >/dev/null 2>&1; then
      echo "${host_port} is ready"
      break
    fi
    now=$(date +%s)
    if [ $((now - start)) -ge $timeout ]; then
      echo "Timed out waiting for ${host_port}" >&2
      return 1
    fi
    sleep 1
  done
}

echo "Waiting for services to become ready" 

# Postgres
wait_for "postgres:5432" 60
# MySQL
wait_for "mysql:3306" 60
# Kafka (plain port)
wait_for "kafka:29092" 60
# Localstack
wait_for "localstack:4566" 120

echo "All services are available"
