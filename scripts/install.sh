#!/bin/bash

# Mayyam Remote Bootstrapper
# This script installs and starts Mayyam in distributable mode.
# Usage: curl -sSL https://raw.githubusercontent.com/sumitharajan/mayyam/main/scripts/install.sh | bash

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}==========================================${NC}"
echo -e "${BLUE}        Mayyam Installation Script        ${NC}"
echo -e "${BLUE}==========================================${NC}"

# 1. Check Prerequisites
echo -e "\n${YELLOW}Checking prerequisites...${NC}"

if ! command -v docker &> /dev/null; then
    echo -e "${RED}Error: Docker is not installed.${NC}"
    echo "Please install Docker from https://docs.docker.com/get-docker/"
    exit 1
fi

if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
    echo -e "${RED}Error: Docker Compose is not installed.${NC}"
    echo "Please install Docker Compose from https://docs.docker.com/compose/install/"
    exit 1
fi

COMPOSE_CMD="docker-compose"
if docker compose version &> /dev/null; then
    COMPOSE_CMD="docker compose"
fi

echo -e "${GREEN}✓ Docker is installed${NC}"

# 2. Setup Directory
INSTALL_DIR="${HOME}/mayyam-deployment"
echo -e "\n${YELLOW}Setting up Mayyam in ${INSTALL_DIR}...${NC}"

mkdir -p "$INSTALL_DIR"
cd "$INSTALL_DIR"

# 3. Download configurations
echo -e "\n${YELLOW}Downloading configuration files...${NC}"

REPO_RAW_URL="https://raw.githubusercontent.com/sumitharajan/mayyam/main"

curl -sSL -o docker-compose.distributable.yml "${REPO_RAW_URL}/docker-compose.distributable.yml" || {
    echo -e "${RED}Failed to download docker-compose.distributable.yml${NC}"
    exit 1
}

# 4. Generate defaults for app DB and external connections
if [ ! -f .env.distributable ]; then
    echo -e "\n${YELLOW}Generating .env.distributable...${NC}"

    if command -v openssl &> /dev/null; then
        APP_DB_PASSWORD=$(openssl rand -hex 16)
    else
        APP_DB_PASSWORD="mayyam_app_$(date +%s)"
    fi

    cat > .env.distributable << EOL
MAYYAM_IMAGE=ghcr.io/sumitharajan/mayyam:latest
APP_DB_NAME=mayyam
APP_DB_USER=mayyam_app
APP_DB_PASSWORD=${APP_DB_PASSWORD}
TARGET_POSTGRES_NAME=target-postgres
TARGET_POSTGRES_HOST=
TARGET_POSTGRES_PORT=5432
TARGET_POSTGRES_USERNAME=
TARGET_POSTGRES_PASSWORD=
TARGET_POSTGRES_DATABASE=
TARGET_MYSQL_NAME=target-mysql
TARGET_MYSQL_HOST=
TARGET_MYSQL_PORT=3306
TARGET_MYSQL_USERNAME=
TARGET_MYSQL_PASSWORD=
TARGET_MYSQL_DATABASE=
TARGET_KAFKA_NAME=target-kafka
TARGET_KAFKA_BOOTSTRAP_SERVERS=
TARGET_KAFKA_SECURITY_PROTOCOL=PLAINTEXT
TARGET_KAFKA_SASL_USERNAME=
TARGET_KAFKA_SASL_PASSWORD=
TARGET_KAFKA_SASL_MECHANISM=
TARGET_AWS_NAME=default
AWS_REGION=us-east-1
AWS_DEFAULT_REGION=us-east-1
AWS_ACCESS_KEY_ID=
AWS_SECRET_ACCESS_KEY=
AWS_SESSION_TOKEN=
AWS_PROFILE=
TARGET_AWS_ROLE_ARN=
EOL
    echo -e "${GREEN}✓ Created .env.distributable${NC}"
else
    echo -e "${GREEN}✓ Using existing .env.distributable${NC}"
fi

set -a
source .env.distributable
set +a

cat > config.distributable.yml << EOL
database:
  postgres:
    - name: app
      host: app-db
      port: 5432
      username: ${APP_DB_USER}
      password: ${APP_DB_PASSWORD}
      database: ${APP_DB_NAME}
      ssl_mode: disable
EOL

if [ -n "${TARGET_POSTGRES_HOST}" ]; then
cat >> config.distributable.yml << EOL
    - name: ${TARGET_POSTGRES_NAME}
      host: ${TARGET_POSTGRES_HOST}
      port: ${TARGET_POSTGRES_PORT}
      username: ${TARGET_POSTGRES_USERNAME}
      password: ${TARGET_POSTGRES_PASSWORD}
      database: ${TARGET_POSTGRES_DATABASE}
      ssl_mode: disable
EOL
fi

cat >> config.distributable.yml << EOL
  mysql:
EOL

if [ -n "${TARGET_MYSQL_HOST}" ]; then
cat >> config.distributable.yml << EOL
    - name: ${TARGET_MYSQL_NAME}
      host: ${TARGET_MYSQL_HOST}
      port: ${TARGET_MYSQL_PORT}
      username: ${TARGET_MYSQL_USERNAME}
      password: ${TARGET_MYSQL_PASSWORD}
      database: ${TARGET_MYSQL_DATABASE}
EOL
else
cat >> config.distributable.yml << EOL
    []
EOL
fi

cat >> config.distributable.yml << EOL
  redis: []
  opensearch: []

kafka:
  clusters:
EOL

if [ -n "${TARGET_KAFKA_BOOTSTRAP_SERVERS}" ]; then
cat >> config.distributable.yml << EOL
    - name: ${TARGET_KAFKA_NAME}
      bootstrap_servers:
EOL
OLD_IFS=$IFS
IFS=','
for server in ${TARGET_KAFKA_BOOTSTRAP_SERVERS}; do
    echo "        - ${server}" >> config.distributable.yml
done
IFS=$OLD_IFS
cat >> config.distributable.yml << EOL
      security_protocol: ${TARGET_KAFKA_SECURITY_PROTOCOL}
      sasl_username: ${TARGET_KAFKA_SASL_USERNAME}
      sasl_password: ${TARGET_KAFKA_SASL_PASSWORD}
      sasl_mechanism: ${TARGET_KAFKA_SASL_MECHANISM}
EOL
else
cat >> config.distributable.yml << EOL
    []
EOL
fi

cat >> config.distributable.yml << EOL

cloud:
  aws:
EOL

if [ -n "${AWS_ACCESS_KEY_ID}" ] || [ -n "${AWS_SECRET_ACCESS_KEY}" ] || [ -n "${TARGET_AWS_ROLE_ARN}" ]; then
cat >> config.distributable.yml << EOL
    - name: ${TARGET_AWS_NAME}
      access_key_id: ${AWS_ACCESS_KEY_ID}
      secret_access_key: ${AWS_SECRET_ACCESS_KEY}
      region: ${AWS_REGION}
      role_arn: ${TARGET_AWS_ROLE_ARN}
      profile: ${AWS_PROFILE}
EOL
else
cat >> config.distributable.yml << EOL
    []
EOL
fi

cat >> config.distributable.yml << EOL
  azure: []
EOL

echo -e "${YELLOW}Edit .env.distributable if you want Mayyam to connect to real Kafka/MySQL/Postgres/AWS before continuing.${NC}"

# 5. Start the application
echo -e "\n${YELLOW}Starting Mayyam services...${NC}"
$COMPOSE_CMD --env-file .env.distributable -f docker-compose.distributable.yml pull
$COMPOSE_CMD --env-file .env.distributable -f docker-compose.distributable.yml up -d

# 6. Success message
echo -e "\n${GREEN}==========================================${NC}"
echo -e "${GREEN}  Mayyam has been successfully started!   ${NC}"
echo -e "${GREEN}==========================================${NC}"
echo -e ""
echo -e "The application is starting up. It may take a minute or two for the database"
echo -e "migrations to complete and the web server to become ready."
echo -e ""
echo -e "🌐 Access the application at: ${BLUE}http://localhost${NC} (or your server's IP)"
echo -e ""
echo -e "To view logs, run:\n  ${YELLOW}cd $INSTALL_DIR && $COMPOSE_CMD --env-file .env.distributable -f docker-compose.distributable.yml logs -f mayyam${NC}"
echo -e ""
echo -e "To stop the application, run:\n  ${YELLOW}cd $INSTALL_DIR && $COMPOSE_CMD --env-file .env.distributable -f docker-compose.distributable.yml down${NC}"
