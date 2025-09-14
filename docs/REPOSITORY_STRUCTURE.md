# Mayyam Repository Structure Guide

## Overview

This document outlines the target repository structure for the Mayyam project, a comprehensive SRE toolbox with Rust backend and React frontend. This structure provides better organization, maintainability, and scalability for the project.

## Repository Structure

```
mayyam/
├── 📁 .github/                    # GitHub configuration and workflows
├── 📁 backend/                    # Rust backend application
├── 📁 frontend/                   # React frontend application
├── 📁 scripts/                    # Build and deployment scripts
├── 📁 docs/                       # Documentation
├── 📁 tools/                      # Development tools and utilities
├── 📁 docker/                     # Docker configurations
├── 📁 configs/                    # Configuration files
├── 📁 .gitignore                  # Git ignore rules
├── 📁 LICENSE                     # Project license
└── 📁 README.md                   # Project documentation
```

## Detailed Directory Structure

### Root Level Files

- **`.gitignore`** - Git ignore rules for all environments
- **`LICENSE`** - Project license file
- **`README.md`** - Main project documentation
- **`package.json`** - Root package.json for monorepo management (optional)

### 1. .github/ Directory

```
.github/
├── 📁 workflows/                  # GitHub Actions workflows
│   ├── 📄 ci.yml                  # Continuous integration
│   ├── 📄 cd.yml                  # Continuous deployment
│   └── 📄 security.yml            # Security scanning
├── 📁 ISSUE_TEMPLATE/             # Issue templates
├── 📁 PULL_REQUEST_TEMPLATE/      # PR templates
└── 📁 dependabot.yml              # Dependency updates
```

### 2. backend/ Directory (Rust)

```
backend/
├── 📁 src/
│   ├── 📁 api/                    # API routes and handlers
│   │   ├── 📁 routes/             # Route definitions
│   │   └── 📁 handlers/           # Request handlers
│   ├── 📁 bin/                    # Binary executables
│   ├── 📁 cli/                    # CLI commands
│   ├── 📁 config/                 # Configuration management
│   ├── 📁 controllers/            # HTTP request controllers
│   ├── 📁 core/                   # Core business logic
│   ├── 📁 domain/                 # Domain models and entities
│   │   ├── 📁 models/             # Data models
│   │   ├── 📁 entities/           # Database entities
│   │   └── 📁 value_objects/      # Value objects
│   ├── 📁 infrastructure/         # External integrations
│   │   ├── 📁 database/           # Database connections
│   │   ├── 📁 messaging/          # Message queues
│   │   ├── 📁 external/           # External APIs
│   │   └── 📁 cache/              # Caching layer
│   ├── 📁 middleware/             # HTTP middleware
│   ├── 📁 services/               # Business logic services
│   ├── 📁 utils/                  # Utility functions
│   ├── 📁 tests/                  # Unit tests (src/tests/)
│   └── 📄 main.rs                 # Application entry point
├── 📁 tests/                      # Integration tests
│   ├── 📁 fixtures/               # Test data fixtures
│   ├── 📁 helpers/                # Test helper functions
│   └── 📁 integration/            # Integration test files
├── 📁 migrations/                 # Database migrations
├── 📁 config/                     # Configuration files
│   ├── 📄 default.yml             # Default configuration
│   ├── 📄 development.yml         # Development config
│   ├── 📄 staging.yml             # Staging config
│   └── 📄 production.yml          # Production config
├── 📁 logs/                       # Application logs (gitignored)
├── 📄 Cargo.toml                  # Rust dependencies
├── 📄 Dockerfile                  # Backend Docker image
└── 📄 .env.example                # Environment variables template
```

### 3. frontend/ Directory (React)

```
frontend/
├── 📁 public/                     # Static assets
│   ├── 📄 index.html              # Main HTML file
│   ├── 📄 favicon.ico             # Favicon
│   ├── 📄 manifest.json           # PWA manifest
│   └── 📁 assets/                 # Static assets
├── 📁 src/
│   ├── 📁 assets/                 # Images, icons, fonts
│   │   ├── 📁 images/             # Image files
│   │   ├── 📁 icons/              # Icon files
│   │   └── 📁 fonts/              # Font files
│   ├── 📁 components/             # Reusable UI components
│   │   ├── 📁 common/             # Generic components
│   │   ├── 📁 forms/              # Form components
│   │   ├── 📁 layout/             # Layout components
│   │   └── 📁 ui/                 # UI library components
│   ├── 📁 pages/                  # Page components
│   │   ├── 📁 auth/               # Authentication pages
│   │   ├── 📁 dashboard/          # Dashboard pages
│   │   └── 📁 settings/           # Settings pages
│   ├── 📁 hooks/                  # Custom React hooks
│   │   ├── 📄 useAuth.js          # Authentication hook
│   │   ├── 📄 useApi.js           # API hook
│   │   └── 📄 useLocalStorage.js  # Local storage hook
│   ├── 📁 context/                # React context providers
│   │   ├── 📄 AuthContext.js      # Authentication context
│   │   └── 📄 AppContext.js       # Application context
│   ├── 📁 services/               # API services
│   │   ├── 📄 api.js              # Main API service
│   │   ├── 📄 authService.js      # Authentication service
│   │   └── 📄 userService.js      # User service
│   ├── 📁 utils/                  # Utility functions
│   │   ├── 📄 helpers.js          # Helper functions
│   │   ├── 📄 constants.js        # App constants
│   │   └── 📄 validation.js       # Validation functions
│   ├── 📁 types/                  # TypeScript definitions
│   │   ├── 📄 index.ts            # Type exports
│   │   └── 📄 api.types.ts        # API types
│   ├── 📁 styles/                 # Global styles and themes
│   │   ├── 📄 global.css          # Global styles
│   │   ├── 📄 theme.js            # Theme configuration
│   │   └── 📄 variables.css       # CSS variables
│   ├── 📁 constants/              # App constants
│   ├── 📄 App.js                  # Main App component
│   ├── 📄 index.js                # Application entry point
│   └── 📄 setupTests.js           # Test setup
├── 📁 config/                     # Build configurations
│   ├── 📄 webpack.config.js       # Webpack config
│   ├── 📄 jest.config.js          # Jest config
│   └── 📄 .eslintrc.js            # ESLint config
├── 📁 tests/                      # Test files
│   ├── 📁 __mocks__/              # Mock files
│   ├── 📁 fixtures/               # Test fixtures
│   ├── 📁 utils/                  # Test utilities
│   └── 📁 e2e/                    # End-to-end tests
├── 📄 package.json                # Dependencies and scripts
├── 📄 Dockerfile                  # Frontend Docker image
├── 📄 nginx.conf                  # Nginx configuration
└── 📄 .env.example                # Environment variables template
```

### 4. scripts/ Directory

```
scripts/
├── 📄 build.sh                    # Build all services
├── 📄 deploy.sh                   # Deploy to environment
├── 📄 setup.sh                    # Development environment setup
├── 📄 test.sh                     # Run all tests
├── 📄 lint.sh                     # Run linting
├── 📄 format.sh                   # Format code
├── 📁 development/                # Development scripts
│   ├── 📄 start-dev.sh            # Start development environment
│   ├── 📄 reset-db.sh             # Reset database
│   ├── 📄 seed-db.sh              # Seed database with test data
│   └── 📄 generate-test-data.sh   # Generate test data
├── 📁 deployment/                 # Deployment scripts
│   ├── 📄 deploy-staging.sh       # Deploy to staging
│   ├── 📄 deploy-production.sh    # Deploy to production
│   └── 📄 rollback.sh             # Rollback deployment
└── 📁 utilities/                  # Utility scripts
    ├── 📄 backup-db.sh            # Database backup
    ├── 📄 restore-db.sh           # Database restore
    └── 📄 health-check.sh         # Health check
```

### 5. docs/ Directory

```
docs/
├── 📄 README.md                   # Main documentation
├── 📄 CONTRIBUTING.md            # Contribution guidelines
├── 📄 ARCHITECTURE.md            # System architecture
├── 📄 CHANGELOG.md               # Change log
├── 📁 api/                       # API documentation
│   ├── 📄 endpoints.md           # API endpoints
│   ├── 📄 schemas.md             # Data schemas
│   └── 📄 authentication.md      # Authentication guide
├── 📁 development/               # Development guides
│   ├── 📄 setup.md               # Development setup
│   ├── 📄 testing.md             # Testing guide
│   ├── 📄 debugging.md           # Debugging guide
│   └── 📄 deployment.md          # Deployment guide
├── 📁 user-guides/               # User documentation
│   ├── 📄 getting-started.md     # Getting started
│   ├── 📄 features.md            # Feature documentation
│   └── 📄 troubleshooting.md     # Troubleshooting guide
└── 📁 architecture/              # Architecture documentation
    ├── 📄 database.md            # Database design
    ├── 📄 api-design.md          # API design principles
    ├── 📄 security.md            # Security considerations
    └── 📄 performance.md         # Performance guidelines
```

### 6. tools/ Directory

```
tools/
├── 📁 test-data/                 # Test data files
│   ├── 📄 users.csv              # User test data
│   ├── 📄 resources.csv          # Resource test data
│   └── 📄 generate-data.py       # Data generation script
├── 📁 generators/                # Code generators
│   ├── 📄 model-generator.js     # Model generator
│   ├── 📄 api-generator.js       # API generator
│   └── 📄 component-generator.js # Component generator
└── 📁 utilities/                 # Development utilities
    ├── 📄 db-migrator.js         # Database migration tool
    ├── 📄 log-analyzer.js        # Log analysis tool
    └── 📄 performance-monitor.js # Performance monitoring
```

### 7. docker/ Directory

```
docker/
├── 📄 docker-compose.yml         # Main docker-compose file
├── 📄 docker-compose.dev.yml     # Development environment
├── 📄 docker-compose.test.yml    # Testing environment
├── 📄 docker-compose.staging.yml # Staging environment
├── 📄 docker-compose.prod.yml    # Production environment
├── 📁 backend/                   # Backend Docker files
│   ├── 📄 Dockerfile             # Backend Dockerfile
│   └── 📄 .dockerignore          # Docker ignore rules
├── 📁 frontend/                  # Frontend Docker files
│   ├── 📄 Dockerfile             # Frontend Dockerfile
│   └── 📄 .dockerignore          # Docker ignore rules
└── 📁 nginx/                     # Nginx configuration
    ├── 📄 nginx.conf             # Nginx config
    └── 📄 default.conf           # Default site config
```

### 8. configs/ Directory

```
configs/
├── 📁 backend/                   # Backend configurations
│   ├── 📄 default.yml            # Default configuration
│   ├── 📄 development.yml        # Development overrides
│   ├── 📄 staging.yml            # Staging overrides
│   └── 📄 production.yml         # Production overrides
├── 📁 frontend/                  # Frontend configurations
│   ├── 📄 .env.development        # Development env vars
│   ├── 📄 .env.staging           # Staging env vars
│   └── 📄 .env.production        # Production env vars
└── 📁 shared/                    # Shared configurations
    ├── 📄 logging.yml            # Logging configuration
    └── 📄 monitoring.yml         # Monitoring configuration
```

## File Naming Conventions

### General Rules
- Use kebab-case for file names: `user-service.js`
- Use PascalCase for component files: `UserProfile.js`
- Use camelCase for utility files: `apiHelper.js`
- Use lowercase with underscores for config files: `database_config.yml`

### Specific Conventions

#### Backend (Rust)
- Modules: `snake_case` (e.g., `user_service.rs`)
- Structs: `PascalCase` (e.g., `UserService`)
- Functions: `snake_case` (e.g., `get_user_by_id`)
- Constants: `SCREAMING_SNAKE_CASE`

#### Frontend (JavaScript/React)
- Components: `PascalCase` (e.g., `UserProfile.js`)
- Utilities: `camelCase` (e.g., `formatDate.js`)
- Hooks: `camelCase` with `use` prefix (e.g., `useAuth.js`)
- Services: `camelCase` (e.g., `userService.js`)

#### Configuration Files
- YAML: `snake_case` (e.g., `database_config.yml`)
- JSON: `camelCase` (e.g., `package.json`)
- Environment: `SCREAMING_SNAKE_CASE` (e.g., `DATABASE_URL`)

## Directory Organization Guidelines

### 1. Separation of Concerns
- Keep business logic separate from infrastructure
- Isolate external dependencies
- Maintain clear boundaries between layers

### 2. Scalability Considerations
- Group related functionality together
- Use feature-based organization for large applications
- Keep directory depth reasonable (max 3-4 levels)

### 3. Test Organization
- Mirror source structure in test directories
- Keep test files close to source files
- Separate unit, integration, and e2e tests

### 4. Configuration Management
- Never commit sensitive data
- Use environment-specific configurations
- Provide clear examples and documentation

## Migration from Current Structure

### Phase 1: Planning and Setup
1. Create new directory structure
2. Set up `.gitignore` rules
3. Create documentation templates
4. Plan file migrations

### Phase 2: Core Structure Migration
1. Move backend source files
2. Move frontend source files
3. Reorganize configuration files
4. Update import paths

### Phase 3: Tooling and Scripts
1. Create build and deployment scripts
2. Set up development environment
3. Configure CI/CD pipelines
4. Update documentation

### Phase 4: Testing and Validation
1. Update all import statements
2. Run tests to ensure functionality
3. Update CI/CD configurations
4. Validate deployment process

## Maintenance Guidelines

### Regular Tasks
- Review and update `.gitignore` monthly
- Clean up old log files weekly
- Update dependencies quarterly
- Review documentation annually

### Code Organization
- Keep files under 500 lines
- Use consistent naming conventions
- Document complex business logic
- Maintain clear separation of concerns

### Documentation Updates
- Update README for new features
- Document API changes immediately
- Keep architecture docs current
- Review and update guides regularly

## Tools and Automation

### Recommended Tools
- **Pre-commit hooks**: Code quality checks
- **Makefile**: Common development tasks
- **Docker**: Containerization
- **GitHub Actions**: CI/CD automation

### Automation Scripts
- Build automation
- Testing automation
- Deployment automation
- Environment setup automation

## Security Considerations

### File Permissions
- Configuration files: Read-only for application
- Log files: Write-only for application
- Sensitive data: Encrypted at rest

### Access Control
- Separate development and production configs
- Use environment variables for secrets
- Implement least privilege principle

## Performance Guidelines

### Build Optimization
- Use appropriate Docker layers
- Minimize image sizes
- Optimize bundle sizes
- Cache dependencies effectively

### Runtime Optimization
- Implement proper logging levels
- Use connection pooling
- Cache frequently accessed data
- Monitor resource usage

## Conclusion

This repository structure provides a solid foundation for the Mayyam project, ensuring maintainability, scalability, and developer productivity. Regular reviews and updates to this structure will help keep the project organized as it grows.

For questions or clarifications about this structure, please refer to the development team or create an issue in the repository.</content>
<parameter name="filePath">/Users/rajanpanneerselvam/work/mayyam-beta/docs/REPOSITORY_STRUCTURE.md
