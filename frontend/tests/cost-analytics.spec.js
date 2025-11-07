import { test, expect } from '@playwright/test';

test.describe('Cost Analytics Page', () => {
  test('should load cost analytics page', async ({ page }) => {
    // Navigate to the cost analytics page
    await page.goto('http://localhost:3000/cost-analytics');

    // Check if the page title is present
    await expect(page.locator('h1').filter({ hasText: 'AWS Cost Analytics' })).toBeVisible();

    // Check if the description is present
    await expect(page.locator('text=Analyze AWS costs, detect anomalies, and get AI-powered insights')).toBeVisible();

    // Check if the form elements are present
    await expect(page.locator('label').filter({ hasText: 'AWS Account ID' })).toBeVisible();
    await expect(page.locator('label').filter({ hasText: 'Start Date' })).toBeVisible();
    await expect(page.locator('label').filter({ hasText: 'End Date' })).toBeVisible();
    await expect(page.locator('label').filter({ hasText: 'Granularity' })).toBeVisible();

    // Check if the fetch button is present
    await expect(page.locator('button').filter({ hasText: 'Fetch Cost Data' })).toBeVisible();
  });

  test('should show error when form is submitted without required fields', async ({ page }) => {
    await page.goto('http://localhost:3000/cost-analytics');

    // Click the fetch button without filling the form
    await page.locator('button').filter({ hasText: 'Fetch Cost Data' }).click();

    // Check if error message appears
    await expect(page.locator('text=Please fill in all required fields')).toBeVisible();
  });

  test('should allow filling the form', async ({ page }) => {
    await page.goto('http://localhost:3000/cost-analytics');

    // Fill the form fields
    await page.locator('input[placeholder="123456789012"]').fill('123456789012');
    await page.locator('input[type="date"]').first().fill('2024-01-01');
    await page.locator('input[type="date"]').last().fill('2024-02-28');

    // Select granularity
    await page.locator('select').selectOption('MONTHLY');

    // Verify the values are filled
    await expect(page.locator('input[placeholder="123456789012"]')).toHaveValue('123456789012');
    await expect(page.locator('input[type="date"]').first()).toHaveValue('2024-01-01');
    await expect(page.locator('input[type="date"]').last()).toHaveValue('2024-02-28');
  });

  test('should have navigation link in sidebar', async ({ page }) => {
    await page.goto('http://localhost:3000/');

    // Check if Cost Analytics link exists in navigation
    await expect(page.locator('a').filter({ hasText: 'Cost Analytics' })).toBeVisible();

    // Click the link and verify navigation
    await page.locator('a').filter({ hasText: 'Cost Analytics' }).click();

    // Should navigate to cost analytics page
    await expect(page).toHaveURL('http://localhost:3000/cost-analytics');
    await expect(page.locator('h1').filter({ hasText: 'AWS Cost Analytics' })).toBeVisible();
  });
});