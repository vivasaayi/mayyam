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

The easiest way to get started is using Docker Compose:

```bash
# Clone the repository
git clone https://github.com/yourusername/mayyam.git
cd mayyam

# Start the application stack
docker-compose up -d
```

The application will be available at:
- Frontend: http://localhost:3000
- Backend API: http://localhost:8080

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
