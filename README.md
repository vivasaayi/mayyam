# Mayyam - DevOps/SRE Toolbox

Mayyam is a comprehensive toolbox for SRE/DevOps/Performance Engineers/DBAs, designed to integrate various cloud pillars like operational excellence, security, reliability, performance efficiency, cost optimization, and sustainability into a single tool.

## Features

- **AI Integration**: First-class integration with AI capabilities
- **Database Management**: Connect to PostgreSQL, MySQL, Redis, OpenSearch, and more
- **Kafka Management**: Manage multiple Kafka clusters, topics, and messages
- **Cloud Management**: Interact with AWS and Azure resources
- **Kubernetes Management**: Work with multiple Kubernetes clusters
- **Chaos Engineering**: Design and execute chaos experiments

## Architecture

Mayyam is built with:

- **Backend**: Rust with Actix web framework, with both server and CLI functionality
- **Frontend**: React with CoreUI and AG Grid

## Getting Started

### Prerequisites

- Docker and Docker Compose
- Rust (for local development)
- Node.js (for local development)

### Running with Docker Compose

The easiest way to get started is using the two supported modes below.

#### Local Mode

Use this when you want everything local for development, manual testing, and full integration testing.

```bash
bash scripts/bootstrap.sh local up
bash scripts/bootstrap.sh local test
bash scripts/bootstrap.sh local down
```

This mode runs:
- PostgreSQL
- MySQL
- Kafka
- Zookeeper
- LocalStack
- Backend dev container
- Frontend dev container
- Integration test container

Access points:
- Frontend: http://localhost:3000
- Backend API: http://localhost:8085
- PostgreSQL: localhost:5432
- MySQL: localhost:3306
- Kafka: localhost:9092
- LocalStack: localhost:4566

#### Distributable Mode

Use this when you want to ship Mayyam with a simple bootstrap flow, while still allowing the user to connect Mayyam to real Kafka, MySQL, PostgreSQL, and AWS.

Important: Mayyam itself requires its own application database. In distributable mode, `docker-compose.distributable.yml` starts an internal PostgreSQL container just for Mayyam app state. That is separate from the real databases or Kafka clusters that Mayyam connects to as managed targets.

```bash
cp .env.distributable.example .env.distributable
bash scripts/bootstrap.sh distributable up
```

Set these values in `.env.distributable` as needed:
- `TARGET_POSTGRES_*`
- `TARGET_MYSQL_*`
- `TARGET_KAFKA_*`
- `AWS_*`
- `TARGET_AWS_*`

The bootstrap script generates `config.distributable.yml` and mounts it into the packaged container.

#### Legacy Compose Files

The older profile-based compose files are still present, but the recommended entry points are now:

```bash
bash scripts/bootstrap.sh local up
bash scripts/bootstrap.sh distributable up
```

For more detail, see `docs/DISTRIBUTION.md`.

#### 🚀 Production Mode (Single Container)
Perfect for end users who just want to run the application:

#### 🚀 Production Mode (Single Container)
Perfect for end users who just want to run the application:

```bash
# Use the production compose file
docker-compose -f docker-compose.prod.yml up --build

# Or use the main compose with prod profile
docker-compose --profile prod up --build
```

**Access:**
- Application: http://localhost
- Health Check: Built-in container health monitoring

#### 🛠️ Development Mode
For developers working on the codebase with hot reloading:

```bash
# Start development environment
docker-compose --profile dev up --build

# Run integration tests
docker-compose --profile dev up integration-tests
```

**Features:**
- Hot reloading for both frontend and backend
- Local code mounting for instant changes
- Debug logging enabled
- All services (DB, Kafka, etc.) available

**Access:**
- Frontend: http://localhost:3000 (with hot reloading)
- Backend API: http://localhost:8080
- PHPMyAdmin: http://localhost:8081
- Kafka: localhost:9092

#### 🧪 UAT Mode
Production-like environment for testing before release:

```bash
# Start UAT environment
docker-compose --profile uat up --build

# Run integration tests in UAT
docker-compose --profile uat up integration-tests
```

**Features:**
- Production-like configuration
- All services running (same as dev)
- Optimized builds (no dev dependencies)
- Integration testing capability

#### 📊 Available Services

All modes include:

| Service | Dev/UAT Port | Description |
|---------|-------------|-------------|
| Frontend | 3000 | React application |
| Backend API | 8080 | Rust REST API |
| PostgreSQL | 5432 | Primary database |
| MySQL | 3306 | Secondary database |
| Kafka | 9092 | Message broker |
| Zookeeper | 2181 | Kafka coordination |
| PHPMyAdmin | 8081 | Database management |

#### 🔧 Environment Variables

Configure the application using environment variables:

```bash
# Database connections
DATABASE_URL=postgres://user:pass@postgres:5432/mayyam
MYSQL_URL=mysql://user:pass@mysql:3306/mayyam_db

# Kafka configuration
KAFKA_BROKERS=kafka:29092

# Logging
RUST_LOG=info  # dev mode
RUST_LOG=debug # development
```

Tip: A template `.env.example` is included — copy it to `.env` and fill your credentials. Never check `.env` or secrets into version control. If you generated tokens locally (by running `node generate_jwt.js`) make sure to remove them or add to `.gitignore`.

### Quick Commands

```bash
# Using the convenience script (recommended)
./run.sh dev up        # Start development environment
./run.sh uat test      # Run tests in UAT mode
./run.sh prod up       # Start production environment
./run.sh dev logs      # Show development logs
./run.sh down          # Stop all services

# Or using docker-compose directly
docker-compose --profile dev up --build
docker-compose --profile uat up --build
docker-compose -f docker-compose.prod.yml up -d
```

### Local test runner

To run both backend and frontend unit tests locally use:

```bash
./run-local-tests.sh
Note: Make the script executable once using:

```bash
chmod +x ./run-local-tests.sh
```
```

You can opt to skip either set of tests:

```bash
./run-local-tests.sh --skip-backend
./run-local-tests.sh --skip-frontend
You can also run tests using `make test` from the repo root which will call the same test steps.

For contribution guidelines and PR checklist refer to `CONTRIBUTING.md`.
```

### Integration tests (gated)

Some integration tests depend on external services. They are gated with environment variables so your CI can opt in selectively:

- ENABLE_AWS_TESTS=1 to run AWS tests
- ENABLE_KAFKA_TESTS=1 to run Kafka tests
- ENABLE_K8S_TESTS=1 to run extended Kubernetes tests

The test harness auto-starts the backend on an ephemeral port and waits for `/health`. If the backend can't become healthy (for example, no local Postgres is running for `config.test.yml`), Kubernetes smoke tests will skip gracefully with a message. To get full coverage locally, bring up the dev compose stack first so DB/Kafka are available, then run `cargo test` in `backend/`.

Note: The `/health` endpoint performs DB-level readiness checks (Postgres and optionally MySQL if configured) — it may return 503 until the database(s) are ready. Use `./scripts/wait-for-db.sh` to test database connectivity from CI/local machines before calling `/health`.

Local integration tests (Docker Compose)
--------------------------------------------------
We provide a `test` profile for Docker Compose that starts Postgres, MySQL, Kafka, and Localstack for AWS mocking. To run local integration tests:

```bash
# Start services and run integration tests in a containerized environment
docker compose --profile dev --profile test up --build integration-tests

# Or use the helper script (recommended)
./scripts/run-integration-tests.sh
```

Notes:
- The `AWS_ENDPOINT` environment variable is respected by the backend and points to Localstack in the test profile.
- Integration tests use `API_URL` to call the actual backend process (http://backend-uat:8080) so you get real API behavior.

DB readiness and seeding
--------------------------------
To ensure DBs are truly ready for queries, we added `scripts/wait-for-db.sh`.

To seed test data, add a SQL file to `backend/test_data/seed.sql` then run:

```bash
./scripts/seed-db.sh backend/test_data/seed.sql
```

To wipe volumes and re-seed from scratch:

```bash
make cleanup
docker compose --profile dev --profile test up -d
./scripts/seed-db.sh backend/test_data/seed.sql
```

Gating: Integration and e2e tests are gated in CI; they run automatically for the `main` branch or manually via `workflow_dispatch`. To execute integration jobs for feature branches, run the workflow manually from GitHub Actions and enable the tests.

### Development Setup

#### Backend

```bash
cd backend

# Install dependencies and run in development mode
cargo run -- server --host 127.0.0.1 --port 8080
```

#### Frontend

```bash
cd frontend

# Install dependencies
npm install

# Start development server
npm start
```

## Configuration

Configuration is handled through:

1. Environment variables
2. Configuration files:
   - `config.yml` - Main configuration
   - `config.default.yml` - Default configuration

## Authentication

Mayyam supports multiple authentication methods:

- Username/password
- Token-based authentication
- SAML integration (for enterprise deployments)

## License

MIT License
