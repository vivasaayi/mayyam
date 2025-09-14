import { test, expect } from '@playwright/test';

test.describe('Authentication', () => {
  test('should load login page', async ({ page }) => {
    await page.goto('/login');
    await expect(page).toHaveTitle(/Mayyam/);
    await expect(page.locator('text=Login')).toBeVisible();
  });

  test('should show login form elements', async ({ page }) => {
    await page.goto('/login');

    // Check for login form elements
    await expect(page.locator('input[type="text"], input[type="email"]')).toBeVisible();
    await expect(page.locator('input[type="password"]')).toBeVisible();
    await expect(page.locator('button:has-text("Login"), button:has-text("Sign In")')).toBeVisible();
  });

  test('should navigate to dashboard after login', async ({ page }) => {
    await page.goto('/login');

    // Mock successful login - you'll need to adjust based on your actual login implementation
    await page.fill('input[type="text"], input[type="email"]', 'test@example.com');
    await page.fill('input[type="password"]', 'password123');

    // Click login button
    await page.click('button:has-text("Login"), button:has-text("Sign In")');

    // Should redirect to dashboard or show success
    await expect(page).toHaveURL(/\/dashboard|\/$/);
  });
});
