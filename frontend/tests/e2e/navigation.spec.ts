import { test, expect } from '@playwright/test';

test.describe('Navigation', () => {
  test('should load main dashboard', async ({ page }) => {
    await page.goto('/');
    await expect(page).toHaveTitle(/Mayyam/);
  });

  test('should navigate between main sections', async ({ page }) => {
    await page.goto('/');

    // Test navigation to different sections
    const navItems = [
      { text: 'Dashboard', url: '/dashboard' },
      { text: 'Kubernetes', url: '/kubernetes' },
      { text: 'Cloud', url: '/cloud' },
      { text: 'Databases', url: '/databases' },
      { text: 'Kafka', url: '/kafka' },
      { text: 'Chaos', url: '/chaos' }
    ];

    for (const item of navItems) {
      const navLink = page.locator(`a:has-text("${item.text}"), button:has-text("${item.text}")`);

      if (await navLink.isVisible()) {
        await navLink.click();

        // Wait for navigation
        await page.waitForLoadState('networkidle');

        // Verify URL contains expected path
        await expect(page).toHaveURL(new RegExp(item.url.replace('/', '\\/')));
      }
    }
  });

  test('should handle 404 pages', async ({ page }) => {
    await page.goto('/non-existent-page');
    await expect(page.locator('text=404, text=Not Found, text=Page not found')).toBeVisible();
  });

  test('should navigate to Kubernetes dashboard', async ({ page }) => {
    await page.goto('/');

    const k8sLink = page.locator('a:has-text("Kubernetes"), button:has-text("Kubernetes")');
    if (await k8sLink.isVisible()) {
      await k8sLink.click();
      await expect(page).toHaveURL(/kubernetes/);
      await expect(page.locator('text=Kubernetes')).toBeVisible();
    }
  });

  test('should navigate to cloud resources', async ({ page }) => {
    await page.goto('/');

    const cloudLink = page.locator('a:has-text("Cloud"), button:has-text("Cloud")');
    if (await cloudLink.isVisible()) {
      await cloudLink.click();
      await expect(page).toHaveURL(/cloud/);
      await expect(page.locator('text=Cloud, text=AWS, text=Resources')).toBeVisible();
    }
  });

  test('should navigate to database management', async ({ page }) => {
    await page.goto('/');

    const dbLink = page.locator('a:has-text("Databases"), button:has-text("Databases")');
    if (await dbLink.isVisible()) {
      await dbLink.click();
      await expect(page).toHaveURL(/database/);
      await expect(page.locator('text=Database, text=Connection')).toBeVisible();
    }
  });
});
