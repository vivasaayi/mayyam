#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
MODE=${1:-}
COMMAND=${2:-up}
ENV_FILE="$ROOT_DIR/.env.distributable"
CONFIG_FILE="$ROOT_DIR/config.distributable.yml"

usage() {
  cat <<'EOF'
Usage:
  ./scripts/bootstrap.sh local up
  ./scripts/bootstrap.sh local down
  ./scripts/bootstrap.sh local logs
  ./scripts/bootstrap.sh local test
  ./scripts/bootstrap.sh distributable up
  ./scripts/bootstrap.sh distributable down
  ./scripts/bootstrap.sh distributable logs

Modes:
  local          Run full local stack for development and integration testing.
  distributable  Run packaged Mayyam with an internal app DB and external target services.
EOF
}

detect_compose() {
  if docker compose version >/dev/null 2>&1; then
    echo "docker compose"
  elif command -v docker-compose >/dev/null 2>&1; then
    echo "docker-compose"
  else
    echo "Docker Compose is required" >&2
    exit 1
  fi
}

generate_distributable_config() {
  if [[ ! -f "$ENV_FILE" ]]; then
    cp "$ROOT_DIR/.env.distributable.example" "$ENV_FILE"
    echo "Created $ENV_FILE from template. Fill in your real service endpoints, then rerun." >&2
    exit 1
  fi

  set -a
  source "$ENV_FILE"
  set +a

  cat > "$CONFIG_FILE" <<EOF
database:
  postgres:
    - name: app
      host: app-db
      port: 5432
      username: ${APP_DB_USER:-mayyam_app}
      password: ${APP_DB_PASSWORD:-mayyam_app_password}
      database: ${APP_DB_NAME:-mayyam}
      ssl_mode: disable
EOF

  if [[ -n "${TARGET_POSTGRES_HOST:-}" ]]; then
    cat >> "$CONFIG_FILE" <<EOF
    - name: ${TARGET_POSTGRES_NAME:-target-postgres}
      host: ${TARGET_POSTGRES_HOST}
      port: ${TARGET_POSTGRES_PORT:-5432}
      username: ${TARGET_POSTGRES_USERNAME:-postgres}
      password: ${TARGET_POSTGRES_PASSWORD:-}
      database: ${TARGET_POSTGRES_DATABASE:-postgres}
      ssl_mode: disable
EOF
  fi

  cat >> "$CONFIG_FILE" <<EOF
  mysql:
EOF

  if [[ -n "${TARGET_MYSQL_HOST:-}" ]]; then
    cat >> "$CONFIG_FILE" <<EOF
    - name: ${TARGET_MYSQL_NAME:-target-mysql}
      host: ${TARGET_MYSQL_HOST}
      port: ${TARGET_MYSQL_PORT:-3306}
      username: ${TARGET_MYSQL_USERNAME:-root}
      password: ${TARGET_MYSQL_PASSWORD:-}
      database: ${TARGET_MYSQL_DATABASE:-mysql}
EOF
  else
    cat >> "$CONFIG_FILE" <<EOF
    []
EOF
  fi

  cat >> "$CONFIG_FILE" <<EOF
  redis: []
  opensearch: []

kafka:
  clusters:
EOF

  if [[ -n "${TARGET_KAFKA_BOOTSTRAP_SERVERS:-}" ]]; then
    cat >> "$CONFIG_FILE" <<EOF
    - name: ${TARGET_KAFKA_NAME:-target-kafka}
      bootstrap_servers:
EOF
    IFS=',' read -r -a kafka_servers <<< "${TARGET_KAFKA_BOOTSTRAP_SERVERS}"
    for server in "${kafka_servers[@]}"; do
      echo "        - ${server}" >> "$CONFIG_FILE"
    done
    cat >> "$CONFIG_FILE" <<EOF
      security_protocol: ${TARGET_KAFKA_SECURITY_PROTOCOL:-PLAINTEXT}
      sasl_username: ${TARGET_KAFKA_SASL_USERNAME:-}
      sasl_password: ${TARGET_KAFKA_SASL_PASSWORD:-}
      sasl_mechanism: ${TARGET_KAFKA_SASL_MECHANISM:-}
EOF
  else
    cat >> "$CONFIG_FILE" <<EOF
    []
EOF
  fi

  cat >> "$CONFIG_FILE" <<EOF

cloud:
  aws:
EOF

  if [[ -n "${AWS_ACCESS_KEY_ID:-}" || -n "${AWS_SECRET_ACCESS_KEY:-}" || -n "${TARGET_AWS_ROLE_ARN:-}" ]]; then
    cat >> "$CONFIG_FILE" <<EOF
    - name: ${TARGET_AWS_NAME:-default}
      access_key_id: ${AWS_ACCESS_KEY_ID:-}
      secret_access_key: ${AWS_SECRET_ACCESS_KEY:-}
      region: ${AWS_REGION:-us-east-1}
      role_arn: ${TARGET_AWS_ROLE_ARN:-}
      profile: ${AWS_PROFILE:-}
EOF
  else
    cat >> "$CONFIG_FILE" <<EOF
    []
EOF
  fi

  cat >> "$CONFIG_FILE" <<EOF
  azure: []
EOF
}

run_local() {
  local compose_cmd=$1
  case "$COMMAND" in
    up)
      (cd "$ROOT_DIR" && $compose_cmd -f docker-compose.local.yml up -d --build)
      ;;
    down)
      (cd "$ROOT_DIR" && $compose_cmd -f docker-compose.local.yml down --volumes --remove-orphans)
      ;;
    logs)
      (cd "$ROOT_DIR" && $compose_cmd -f docker-compose.local.yml logs -f)
      ;;
    test)
      (cd "$ROOT_DIR" && $compose_cmd -f docker-compose.local.yml up --build --abort-on-container-exit --exit-code-from integration-tests integration-tests)
      ;;
    *)
      usage
      exit 1
      ;;
  esac
}

run_distributable() {
  local compose_cmd=$1
  generate_distributable_config

  case "$COMMAND" in
    up)
      (cd "$ROOT_DIR" && $compose_cmd --env-file .env.distributable -f docker-compose.distributable.yml up -d)
      ;;
    down)
      (cd "$ROOT_DIR" && $compose_cmd --env-file .env.distributable -f docker-compose.distributable.yml down --remove-orphans)
      ;;
    logs)
      (cd "$ROOT_DIR" && $compose_cmd --env-file .env.distributable -f docker-compose.distributable.yml logs -f)
      ;;
    *)
      usage
      exit 1
      ;;
  esac
}

main() {
  if [[ -z "$MODE" ]]; then
    usage
    exit 1
  fi

  local compose_cmd
  compose_cmd=$(detect_compose)

  case "$MODE" in
    local)
      run_local "$compose_cmd"
      ;;
    distributable)
      run_distributable "$compose_cmd"
      ;;
    *)
      usage
      exit 1
      ;;
  esac
}

main "$@"
