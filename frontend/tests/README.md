# Playwright UI Testing Setup

This directory contains end-to-end (E2E) and component integration tests using Playwright for the Mayyam SRE Toolbox frontend.

## 🚀 Quick Start

### Prerequisites
- Node.js 16+
- npm or yarn
- Backend server running on `http://localhost:8080` (configured in playwright.config.ts)

### Installation
```bash
# Install dependencies (already done)
npm install

# Install Playwright browsers
npx playwright install
```

### Running Tests

```bash
# Run all tests
npm run test:e2e

# Run tests with UI mode (interactive)
npm run test:e2e:ui

# Run tests in debug mode
npm run test:e2e:debug

# Run tests in headed mode (see browser)
npm run test:e2e:headed

# Generate test code (record interactions)
npm run test:e2e:codegen

# View test reports
npm run test:e2e:report
```

## 📁 Test Structure

```
tests/
├── e2e/                    # End-to-end tests
│   ├── auth.spec.ts       # Authentication tests
│   ├── kubernetes-dashboard.spec.ts  # K8s dashboard tests
│   └── navigation.spec.ts # Navigation tests
├── components/            # Component integration tests
│   └── component-integration.spec.ts
└── utils/                 # Test utilities and page objects
    └── page-objects.ts
```

## 🧪 Test Categories

### End-to-End Tests (E2E)
- **Authentication**: Login/logout flows
- **Kubernetes Dashboard**: Cluster management, resource viewing
- **Navigation**: Page routing and menu navigation

### Component Integration Tests
- **AG Grid**: Data table functionality
- **CoreUI Components**: UI library integration
- **Charts**: Data visualization components
- **Forms**: Validation and submission
- **Responsive Design**: Mobile compatibility

## 🛠️ Configuration

### Playwright Config (`playwright.config.ts`)
- **Base URL**: `http://localhost:3000`
- **Browsers**: Chromium, Firefox, WebKit
- **Mobile**: Pixel 5, iPhone 12
- **Parallel Execution**: Enabled for speed
- **Auto-server**: Starts dev server automatically

### Test Configuration
- **Retries**: 2 on CI, 0 locally
- **Traces**: Collected on first retry
- **Screenshots**: Taken on failures
- **Videos**: Recorded for debugging

## 📝 Writing Tests

### Basic Test Structure
```typescript
import { test, expect } from '@playwright/test';

test('should load dashboard', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('text=Dashboard')).toBeVisible();
});
```

### Using Page Objects
```typescript
import { test } from '@playwright/test';
import { KubernetesDashboardPage } from '../utils/page-objects';

test('should manage clusters', async ({ page }) => {
  const k8sPage = new KubernetesDashboardPage(page);
  await k8sPage.selectCluster('production');
  await k8sPage.selectNamespace('default');
});
```

### API Mocking
```typescript
import { test } from '@playwright/test';
import { mockApiResponse } from '../utils/page-objects';

test('should handle API errors', async ({ page }) => {
  await mockApiResponse(page, '/api/clusters', { error: 'Server error' });
  await page.goto('/kubernetes-dashboard');
  await expect(page.locator('text=Error loading clusters')).toBeVisible();
});
```

## 🎯 Best Practices

### Test Organization
- Group related tests in `describe` blocks
- Use descriptive test names
- Keep tests focused on single functionality

### Selectors
- Prefer semantic selectors over CSS/XPath
- Use data-testid attributes for reliable targeting
- Avoid flaky selectors (position-based, text-based)

### Assertions
- Use `expect` for clear, readable assertions
- Wait for elements before interacting
- Use appropriate wait strategies (`waitForLoadState`, `waitForSelector`)

### Performance
- Use parallel execution when possible
- Mock external dependencies
- Keep tests lightweight and fast

## 🔧 Debugging

### Visual Debugging
```bash
# Run with browser visible
npm run test:e2e:headed

# Interactive UI mode
npm run test:e2e:ui

# Debug mode with breakpoints
npm run test:e2e:debug
```

### Code Generation
```bash
# Record interactions to generate test code
npm run test:e2e:codegen
```

### Trace Analysis
- Traces are automatically saved on failures
- View traces in Playwright UI mode
- Analyze network requests and DOM changes

## 📊 CI/CD Integration

### GitHub Actions Example
```yaml
- name: Run E2E Tests
  run: |
    npm run test:e2e
  env:
    CI: true
```

### Parallel Execution
```yaml
- name: Run E2E Tests (Sharded)
  run: |
    npx playwright test --shard=${{ matrix.shard }}/${{ matrix.total }}
```

## 🎨 Customizing Tests

### Adding New Page Objects
1. Create class in `tests/utils/page-objects.ts`
2. Implement methods for page interactions
3. Use in tests for better maintainability

### Environment-Specific Configs
```typescript
// playwright.config.ts
export default defineConfig({
  use: {
    baseURL: process.env.BASE_URL || 'http://localhost:3000',
  },
});
```

### Custom Fixtures
```typescript
// Add to playwright.config.ts
export const test = base.extend({
  authenticatedPage: async ({ page }, use) => {
    await page.goto('/login');
    await page.fill('input[type="email"]', 'user@example.com');
    await page.fill('input[type="password"]', 'password');
    await page.click('button[type="submit"]');
    await page.waitForURL('/dashboard');
    await use(page);
  },
});
```

## 🚨 Common Issues & Solutions

### Tests Flaking
- Use `waitForLoadState('networkidle')` for dynamic content
- Add retry logic for unstable elements
- Use more specific selectors

### Slow Tests
- Mock API calls when possible
- Use `page.route()` to intercept requests
- Run tests in parallel

### Element Not Found
- Check if elements load asynchronously
- Use `waitForSelector()` with appropriate timeouts
- Verify selectors in browser dev tools

## 📈 Test Coverage

### Current Coverage
- ✅ Authentication flows
- ✅ Kubernetes dashboard functionality
- ✅ Navigation and routing
- ✅ Component integration
- ✅ Responsive design

### Future Enhancements
- API error handling tests
- Performance testing
- Accessibility testing
- Visual regression testing

## 🤝 Contributing

1. Follow the existing test structure
2. Add descriptive test names and comments
3. Include both positive and negative test cases
4. Update this README for new test categories

## 📚 Resources

- [Playwright Documentation](https://playwright.dev/docs/intro)
- [Playwright API Reference](https://playwright.dev/docs/api/class-playwright)
- [Testing Best Practices](https://playwright.dev/docs/best-practices)
- [Debugging Tests](https://playwright.dev/docs/debug)
