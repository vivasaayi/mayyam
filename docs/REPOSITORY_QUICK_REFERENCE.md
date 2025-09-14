# Repository Structure Quick Reference

## 📁 Directory Structure at a Glance

```
mayyam/
├── 📁 .github/           # GitHub workflows & templates
├── 📁 backend/           # Rust backend (Cargo.toml, src/, tests/)
├── 📁 frontend/          # React frontend (package.json, src/, public/)
├── 📁 scripts/           # Build & deployment scripts
├── 📁 docs/              # Documentation & guides
├── 📁 tools/             # Development tools & test data
├── 📁 docker/            # Docker configurations
├── 📁 configs/           # Environment configurations
└── 📄 *.md               # Root documentation files
```

## 🚀 Quick Start Commands

### Development Setup
```bash
# Clone and setup
git clone <repo-url>
cd mayyam

# Setup development environment
./scripts/setup.sh

# Start development servers
./scripts/development/start-dev.sh
```

### Common Tasks
```bash
# Run all tests
./scripts/test.sh

# Build all services
./scripts/build.sh

# Deploy to staging
./scripts/deployment/deploy-staging.sh

# Format code
./scripts/format.sh

# Run linting
./scripts/lint.sh
```

## 📂 File Placement Guide

### Where to put new files:

| File Type | Location | Example |
|-----------|----------|---------|
| Rust module | `backend/src/` | `backend/src/new_feature.rs` |
| React component | `frontend/src/components/` | `frontend/src/components/MyComponent.js` |
| API route | `backend/src/api/routes/` | `backend/src/api/routes/users.rs` |
| Database migration | `backend/migrations/` | `backend/migrations/001_add_users.sql` |
| Test file | `backend/tests/` or `frontend/tests/` | `backend/tests/users_test.rs` |
| Configuration | `configs/backend/` or `configs/frontend/` | `configs/backend/database.yml` |
| Documentation | `docs/` | `docs/api/users.md` |
| Build script | `scripts/` | `scripts/deploy.sh` |
| Test data | `tools/test-data/` | `tools/test-data/sample_users.csv` |

## 🔧 Development Workflow

### Adding a New Feature

1. **Backend Feature:**
   ```bash
   # Create feature module
   touch backend/src/features/my_feature.rs

   # Add to main.rs
   # mod features/my_feature;

   # Create tests
   touch backend/tests/features/my_feature_test.rs
   ```

2. **Frontend Feature:**
   ```bash
   # Create component
   mkdir frontend/src/components/MyFeature/
   touch frontend/src/components/MyFeature/index.js

   # Create tests
   touch frontend/tests/components/MyFeature.test.js
   ```

### Database Changes

1. **Create Migration:**
   ```bash
   touch backend/migrations/$(date +%Y%m%d_%H%M%S)_my_change.sql
   ```

2. **Update Models:**
   ```bash
   # Edit existing model or create new one
   # backend/src/domain/models/my_model.rs
   ```

### Configuration Changes

1. **Add Config:**
   ```bash
   # Add to configs/backend/default.yml
   # Override in environment-specific files
   ```

## 📋 Checklist for New Features

### Backend
- [ ] Module created in appropriate directory
- [ ] Unit tests written
- [ ] Integration tests added
- [ ] Documentation updated
- [ ] Migration scripts created (if needed)
- [ ] Configuration added
- [ ] Error handling implemented

### Frontend
- [ ] Component created in components/
- [ ] Styles added (if needed)
- [ ] Tests written
- [ ] Storybook stories added (if applicable)
- [ ] Accessibility considerations
- [ ] Responsive design verified

### General
- [ ] Code formatted
- [ ] Linting passed
- [ ] Documentation updated
- [ ] CI/CD pipeline updated (if needed)
- [ ] Security review completed

## 🚨 Common Mistakes to Avoid

### ❌ Don't:
- Put business logic in controllers
- Mix frontend and backend code
- Commit sensitive data
- Use generic names like `utils.js`
- Create deep directory structures
- Forget to update documentation

### ✅ Do:
- Follow naming conventions
- Keep files small and focused
- Write tests for new code
- Update documentation
- Use environment variables for config
- Follow the established patterns

## 📞 Need Help?

- **Structure Questions:** Check `docs/REPOSITORY_STRUCTURE.md`
- **API Documentation:** See `docs/api/`
- **Development Setup:** Read `docs/development/setup.md`
- **Deployment Guide:** See `docs/development/deployment.md`

## 🔄 Maintenance

### Weekly Tasks
- [ ] Review and clean up log files
- [ ] Check for outdated dependencies
- [ ] Verify test coverage

### Monthly Tasks
- [ ] Update `.gitignore` if needed
- [ ] Review documentation accuracy
- [ ] Check repository structure compliance

### Quarterly Tasks
- [ ] Major dependency updates
- [ ] Security audit
- [ ] Performance review

---

**Remember:** A well-organized repository is easier to maintain, understand, and scale. When in doubt, refer to this guide or ask the development team!</content>
<parameter name="filePath">/Users/rajanpanneerselvam/work/mayyam-beta/docs/REPOSITORY_QUICK_REFERENCE.md
