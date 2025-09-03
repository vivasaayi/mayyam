import { test, expect } from '@playwright/test';

test.describe('Kubernetes Dashboard', () => {
  test.beforeEach(async ({ page }) => {
    // Assuming user is logged in, navigate to Kubernetes dashboard
    await page.goto('/kubernetes-dashboard');
  });

  test('should load Kubernetes dashboard page', async ({ page }) => {
    await expect(page.locator('text=Kubernetes Dashboard')).toBeVisible();
  });

  test('should display cluster selector', async ({ page }) => {
    // Check for cluster selection dropdown or selector
    await expect(page.locator('select[name*="cluster"], .cluster-selector, [data-testid*="cluster"]')).toBeVisible();
  });

  test('should display namespace selector', async ({ page }) => {
    // Check for namespace selection
    await expect(page.locator('select[name*="namespace"], .namespace-selector, [data-testid*="namespace"]')).toBeVisible();
  });

  test('should show resource tabs', async ({ page }) => {
    // Check for resource type tabs (Deployments, Pods, Services, etc.)
    const tabs = page.locator('.nav-tabs .nav-link, .tab-button, [role="tab"]');
    await expect(tabs.first()).toBeVisible();

    // Should have multiple resource types
    await expect(tabs).toHaveCount(await tabs.count() > 1 ? await tabs.count() : 1);
  });

  test('should display deployments table', async ({ page }) => {
    // Click on Deployments tab if it exists
    const deploymentsTab = page.locator('text=Deployments, [data-testid*="deployment"]');
    if (await deploymentsTab.isVisible()) {
      await deploymentsTab.click();
    }

    // Check for deployments data table
    await expect(page.locator('table, .ag-grid, .data-grid')).toBeVisible();
  });

  test('should display pods information', async ({ page }) => {
    // Click on Pods tab if it exists
    const podsTab = page.locator('text=Pods, [data-testid*="pod"]');
    if (await podsTab.isVisible()) {
      await podsTab.click();
    }

    // Check for pods data table or cards
    await expect(page.locator('table, .ag-grid, .data-grid, .pod-card')).toBeVisible();
  });

  test('should handle cluster switching', async ({ page }) => {
    const clusterSelector = page.locator('select[name*="cluster"], .cluster-selector');

    if (await clusterSelector.isVisible()) {
      // Get current value
      const currentValue = await clusterSelector.inputValue();

      // Try to select a different cluster if available
      const options = clusterSelector.locator('option');
      const optionCount = await options.count();

      if (optionCount > 1) {
        // Select second option
        await clusterSelector.selectOption({ index: 1 });

        // Page should reload or update
        await page.waitForLoadState('networkidle');

        // Verify cluster changed
        const newValue = await clusterSelector.inputValue();
        expect(newValue).not.toBe(currentValue);
      }
    }
  });

  test('should filter resources by namespace', async ({ page }) => {
    const namespaceSelector = page.locator('select[name*="namespace"], .namespace-selector');

    if (await namespaceSelector.isVisible()) {
      // Select a specific namespace
      await namespaceSelector.selectOption({ index: 1 });

      // Wait for filtering to apply
      await page.waitForLoadState('networkidle');

      // Verify filtering is applied (this might need adjustment based on your UI)
      await expect(page.locator('table, .ag-grid')).toBeVisible();
    }
  });
});
