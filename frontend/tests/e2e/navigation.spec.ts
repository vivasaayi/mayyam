// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


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
