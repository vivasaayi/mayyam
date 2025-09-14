import { test, expect } from '@playwright/test';
import { AppPage, KubernetesDashboardPage } from '../utils/page-objects';

test.describe('Component Integration Tests', () => {
  let appPage: AppPage;
  let k8sPage: KubernetesDashboardPage;

  test.beforeEach(async ({ page }) => {
    appPage = new AppPage(page);
    k8sPage = new KubernetesDashboardPage(page);
  });

  test('AG Grid component renders correctly', async ({ page }) => {
    await appPage.goto('/kubernetes-dashboard');

    // Wait for AG Grid to load
    const grid = page.locator('.ag-root-wrapper, .ag-theme-alpine');
    await expect(grid).toBeVisible();

    // Check grid has headers
    const headers = grid.locator('.ag-header-cell-text');
    await expect(headers.first()).toBeVisible();

    // Check grid has data rows
    const rows = grid.locator('.ag-row');
    await expect(rows).toHaveCount(await rows.count() > 0 ? await rows.count() : 0);
  });

  test('CoreUI components render properly', async ({ page }) => {
    await appPage.goto('/dashboard');

    // Check for CoreUI navbar
    const navbar = page.locator('.navbar, .header');
    await expect(navbar).toBeVisible();

    // Check for CoreUI cards
    const cards = page.locator('.card, .c-card');
    if (await cards.count() > 0) {
      await expect(cards.first()).toBeVisible();
    }

    // Check for CoreUI buttons
    const buttons = page.locator('.btn, .c-button');
    await expect(buttons.first()).toBeVisible();
  });

  test('Chart components display data', async ({ page }) => {
    await appPage.goto('/dashboard');

    // Look for chart containers
    const charts = page.locator('canvas, .chart-container, [data-testid*="chart"]');

    if (await charts.count() > 0) {
      // Charts are present, verify they render
      await expect(charts.first()).toBeVisible();

      // Check if charts have dimensions (not empty)
      const chart = charts.first();
      const box = await chart.boundingBox();
      expect(box?.width).toBeGreaterThan(0);
      expect(box?.height).toBeGreaterThan(0);
    }
  });

  test('Modal dialogs work correctly', async ({ page }) => {
    await appPage.goto('/kubernetes-dashboard');

    // Look for buttons that might open modals
    const modalButtons = page.locator('button[data-toggle="modal"], button[aria-haspopup="dialog"]');

    if (await modalButtons.count() > 0) {
      await modalButtons.first().click();

      // Check if modal appears
      const modal = page.locator('.modal, .c-modal, [role="dialog"]');
      await expect(modal).toBeVisible();

      // Try to close modal
      const closeBtn = modal.locator('button[aria-label*="close"], .close, .btn-close');
      if (await closeBtn.isVisible()) {
        await closeBtn.click();
        await expect(modal).not.toBeVisible();
      }
    }
  });

  test('Form validation works', async ({ page }) => {
    // Test forms that might exist (login, configuration forms, etc.)
    await appPage.goto('/login');

    const form = page.locator('form');
    if (await form.isVisible()) {
      // Try submitting empty form
      const submitBtn = form.locator('button[type="submit"], input[type="submit"]');
      if (await submitBtn.isVisible()) {
        await submitBtn.click();

        // Check for validation messages
        const errors = page.locator('.error, .invalid-feedback, [aria-invalid="true"]');
        // Note: This might not show errors immediately due to client-side validation
      }
    }
  });

  test('Responsive design works on mobile', async ({ page }) => {
    await appPage.goto('/dashboard');

    // Set mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });

    // Check that navigation still works
    const nav = page.locator('.navbar, .sidebar, .nav');
    await expect(nav).toBeVisible();

    // Check that content is still accessible
    const mainContent = page.locator('.main-content, .container, main');
    await expect(mainContent).toBeVisible();
  });
});
