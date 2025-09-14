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

The easiest way to get started is using Docker Compose with different modes:

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

### Integration tests (gated)

Some integration tests depend on external services. They are gated with environment variables so your CI can opt in selectively:

- ENABLE_AWS_TESTS=1 to run AWS tests
- ENABLE_KAFKA_TESTS=1 to run Kafka tests
- ENABLE_K8S_TESTS=1 to run extended Kubernetes tests

The test harness auto-starts the backend on an ephemeral port and waits for `/health`. If the backend can't become healthy (for example, no local Postgres is running for `config.test.yml`), Kubernetes smoke tests will skip gracefully with a message. To get full coverage locally, bring up the dev compose stack first so DB/Kafka are available, then run `cargo test` in `backend/`.

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
