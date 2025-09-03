#!/bin/bash

# Mayyam Repository Structure Migration Script
# This script helps migrate from the current structure to the target structure

set -e

echo "🚀 Starting Mayyam Repository Structure Migration"
echo "================================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're in the right directory
if [ ! -f "README.md" ] || [ ! -d "backend" ] || [ ! -d "frontend" ]; then
    print_error "Please run this script from the root of the Mayyam repository"
    exit 1
fi

print_status "Current directory: $(pwd)"

# Create new directory structure
print_status "Creating new directory structure..."

# Create main directories
mkdir -p scripts/{development,deployment,utilities}
mkdir -p docs/{api,development,user-guides,architecture}
mkdir -p tools/{test-data,generators,utilities}
mkdir -p docker/{backend,frontend,nginx}
mkdir -p configs/{backend,frontend,shared}

print_success "Directory structure created"

# Move scattered files to appropriate locations
print_status "Moving scattered files..."

# Move test data files
if [ -f "test_data_file1.csv" ]; then
    mv test_data_file*.csv tools/test-data/ 2>/dev/null || true
    print_success "Test data files moved to tools/test-data/"
fi

# Move generator scripts
if [ -f "generate_csv_test_data.py" ]; then
    mv generate_*.py tools/generators/ 2>/dev/null || true
    mv generate_*.js tools/generators/ 2>/dev/null || true
    print_success "Generator scripts moved to tools/generators/"
fi

# Move docker-compose.yml
if [ -f "docker-compose.yml" ]; then
    mv docker-compose.yml docker/
    print_success "Docker compose file moved to docker/"
fi

# Move JWT and UUID generators
if [ -f "generate_jwt.js" ] || [ -f "test_uuid.js" ]; then
    mv generate_jwt.js test_uuid.js tools/generators/ 2>/dev/null || true
    print_success "JWT/UUID generators moved to tools/generators/"
fi

# Move token file to configs
if [ -f "token.txt" ]; then
    mv token.txt configs/shared/
    print_success "Token file moved to configs/shared/"
fi

# Move _work content to docs
if [ -d "_work" ]; then
    print_status "Moving _work content to docs..."

    # Move milestone content
    if [ -d "_work/milestone1" ]; then
        mkdir -p docs/development/milestones
        mv _work/milestone1/* docs/development/milestones/ 2>/dev/null || true
        print_success "Milestone documentation moved to docs/development/milestones/"
    fi

    # Move prompts to docs
    if [ -d "_work" ]; then
        mv _work/prompts docs/ 2>/dev/null || true
        print_success "Prompts moved to docs/"
    fi

    # Remove empty _work directory
    rmdir _work 2>/dev/null || true
fi

# Create basic script templates
print_status "Creating basic script templates..."

# Create setup script
cat > scripts/setup.sh << 'EOF'
#!/bin/bash
echo "Setting up Mayyam development environment..."

# Backend setup
echo "Setting up backend..."
cd backend
cargo build
cd ..

# Frontend setup
echo "Setting up frontend..."
cd frontend
npm install
cd ..

echo "Setup complete! Run ./scripts/development/start-dev.sh to start development servers."
EOF

chmod +x scripts/setup.sh

# Create development start script
cat > scripts/development/start-dev.sh << 'EOF'
#!/bin/bash
echo "Starting Mayyam development environment..."

# Start backend
echo "Starting backend server..."
cd backend
cargo run &
BACKEND_PID=$!

# Start frontend
echo "Starting frontend server..."
cd ../frontend
npm start &
FRONTEND_PID=$!

echo "Development servers started!"
echo "Backend: http://localhost:8080"
echo "Frontend: http://localhost:3000"
echo "Press Ctrl+C to stop all servers"

# Wait for Ctrl+C
trap "echo 'Stopping servers...'; kill $BACKEND_PID $FRONTEND_PID 2>/dev/null; exit" INT
wait
EOF

chmod +x scripts/development/start-dev.sh

# Create test script
cat > scripts/test.sh << 'EOF'
#!/bin/bash
echo "Running Mayyam test suite..."

# Backend tests
echo "Running backend tests..."
cd backend
cargo test
cd ..

# Frontend tests
echo "Running frontend tests..."
cd frontend
npm test -- --watchAll=false
cd ..

echo "All tests completed!"
EOF

chmod +x scripts/test.sh

print_success "Basic scripts created"

# Create .env.example files
print_status "Creating environment configuration templates..."

# Backend .env.example
cat > backend/.env.example << 'EOF'
# Database Configuration
DATABASE_URL=sqlite://mayyam.db

# Server Configuration
SERVER_HOST=127.0.0.1
SERVER_PORT=8080

# JWT Configuration
JWT_SECRET=your-super-secret-jwt-key-here
JWT_EXPIRATION=3600

# AWS Configuration (for production)
AWS_ACCESS_KEY_ID=your-access-key
AWS_SECRET_ACCESS_KEY=your-secret-key
AWS_REGION=us-east-1

# Logging
LOG_LEVEL=info
EOF

# Frontend .env.example
cat > frontend/.env.example << 'EOF'
# API Configuration
REACT_APP_API_URL=http://localhost:8080/api

# Environment
REACT_APP_ENV=development

# Analytics (optional)
REACT_APP_ANALYTICS_ID=your-analytics-id
EOF

print_success "Environment templates created"

# Create basic documentation templates
print_status "Creating documentation templates..."

# API documentation
cat > docs/api/endpoints.md << 'EOF'
# Mayyam API Endpoints

## Authentication
- `POST /api/auth/login` - User login
- `POST /api/auth/logout` - User logout
- `POST /api/auth/refresh` - Refresh token

## Users
- `GET /api/users` - List users
- `POST /api/users` - Create user
- `GET /api/users/{id}` - Get user by ID
- `PUT /api/users/{id}` - Update user
- `DELETE /api/users/{id}` - Delete user

## AWS Resources
- `GET /api/aws/accounts` - List AWS accounts
- `POST /api/aws/accounts` - Create AWS account
- `GET /api/aws/resources` - List AWS resources
EOF

# Development setup guide
cat > docs/development/setup.md << 'EOF'
# Development Setup Guide

## Prerequisites
- Rust 1.70+
- Node.js 18+
- Docker & Docker Compose
- Git

## Quick Setup
1. Clone the repository
2. Run `./scripts/setup.sh`
3. Start development with `./scripts/development/start-dev.sh`

## Manual Setup

### Backend Setup
```bash
cd backend
cargo build
cp .env.example .env
# Edit .env with your configuration
```

### Frontend Setup
```bash
cd frontend
npm install
cp .env.example .env
# Edit .env with your configuration
```

### Database Setup
```bash
cd backend
cargo run --bin migration
```

## Development Workflow
1. Create a feature branch
2. Make your changes
3. Run tests with `./scripts/test.sh`
4. Create a pull request
EOF

print_success "Documentation templates created"

# Update .gitignore if needed
print_status "Checking .gitignore..."

if [ ! -f ".gitignore" ]; then
    cat > .gitignore << 'EOF'
# Rust
target/
Cargo.lock

# Node.js
node_modules/
npm-debug.log*
yarn-debug.log*
yarn-error.log*

# Environment files
.env
.env.local
.env.development.local
.env.test.local
.env.production.local

# Logs
logs/
*.log
npm-debug.log*
yarn-debug.log*
yarn-error.log*
lerna-debug.log*

# Runtime data
pids/
*.pid
*.seed
*.pid.lock

# Coverage directory used by tools like istanbul
coverage/
*.lcov

# nyc test coverage
.nyc_output

# Dependency directories
jspm_packages/

# Optional npm cache directory
.npm

# Optional eslint cache
.eslintcache

# Microbundle cache
.rpt2_cache/
.rts2_cache_cjs/
.rts2_cache_es/
.rts2_cache_umd/

# Optional REPL history
.node_repl_history

# Output of 'npm pack'
*.tgz

# Yarn Integrity file
.yarn-integrity

# dotenv environment variables file
.env.test

# parcel-bundler cache (https://parceljs.org/)
.cache
.parcel-cache

# Next.js build output
.next

# Nuxt.js build / generate output
.nuxt
dist

# Gatsby files
.cache/
public

# Storybook build outputs
.out
.storybook-out

# Temporary folders
tmp/
temp/

# Editor directories and files
.vscode/*
!.vscode/extensions.json
.idea
.DS_Store
*.suo
*.ntvs*
*.njsproj
*.sln
*.sw?

# OS generated files
Thumbs.db
ehthumbs.db
Desktop.ini

# Database files
*.db
*.sqlite
*.sqlite3

# Application logs
logs/
*.log

# Build artifacts
build/
dist/
target/

# Test coverage
coverage/

# Docker volumes
docker/volumes/
EOF
    print_success ".gitignore created"
else
    print_status ".gitignore already exists"
fi

print_success "Migration completed!"
echo ""
echo "📋 Next Steps:"
echo "1. Review the moved files and update any broken imports"
echo "2. Update your CI/CD pipelines to use the new structure"
echo "3. Review and update documentation in docs/"
echo "4. Test the new scripts in scripts/"
echo "5. Update any hardcoded paths in your code"
echo ""
echo "📚 Documentation:"
echo "- Main structure guide: docs/REPOSITORY_STRUCTURE.md"
echo "- Quick reference: docs/REPOSITORY_QUICK_REFERENCE.md"
echo ""
echo "🎯 Key Changes Made:"
echo "- Created organized directory structure"
echo "- Moved scattered files to appropriate locations"
echo "- Created basic automation scripts"
echo "- Set up environment configuration templates"
echo "- Created documentation templates"

print_warning "Remember to review all import statements and update them to reflect the new file locations!"</content>
<parameter name="filePath">/Users/rajanpanneerselvam/work/mayyam-beta/scripts/migrate-structure.sh
