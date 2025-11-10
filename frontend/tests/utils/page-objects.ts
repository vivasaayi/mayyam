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


import { Page, expect } from '@playwright/test';

export class AppPage {
  constructor(private page: Page) {}

  async goto(path: string = '/') {
    await this.page.goto(path);
    await this.page.waitForLoadState('networkidle');
  }

  async login(email: string, password: string) {
    await this.page.fill('input[type="email"], input[type="text"]', email);
    await this.page.fill('input[type="password"]', password);
    await this.page.click('button:has-text("Login"), button:has-text("Sign In")');
    await this.page.waitForLoadState('networkidle');
  }

  async waitForLoading() {
    await this.page.waitForLoadState('networkidle');
  }

  async isLoggedIn(): Promise<boolean> {
    // Check for logout button or user profile indicator
    const logoutBtn = this.page.locator('button:has-text("Logout"), a:has-text("Logout")');
    return await logoutBtn.isVisible();
  }
}

export class KubernetesDashboardPage {
  constructor(private page: Page) {}

  async selectCluster(clusterName: string) {
    const selector = this.page.locator('select[name*="cluster"], .cluster-selector');
    await selector.selectOption({ label: clusterName });
    await this.page.waitForLoadState('networkidle');
  }

  async selectNamespace(namespace: string) {
    const selector = this.page.locator('select[name*="namespace"], .namespace-selector');
    await selector.selectOption({ label: namespace });
    await this.page.waitForLoadState('networkidle');
  }

  async switchToTab(tabName: string) {
    const tab = this.page.locator(`text=${tabName}, [data-testid*="${tabName.toLowerCase()}"]`);
    await tab.click();
    await this.page.waitForLoadState('networkidle');
  }

  async getResourceCount(resourceType: string): Promise<number> {
    const table = this.page.locator('table, .ag-grid, .data-grid');
    const rows = table.locator('tbody tr, .ag-row, .grid-row');
    return await rows.count();
  }

  async searchResources(searchTerm: string) {
    const searchInput = this.page.locator('input[type="search"], input[placeholder*="search"], input[placeholder*="filter"]');
    if (await searchInput.isVisible()) {
      await searchInput.fill(searchTerm);
      await this.page.waitForLoadState('networkidle');
    }
  }
}

export class CloudResourcesPage {
  constructor(private page: Page) {}

  async selectService(serviceType: string) {
    const serviceTab = this.page.locator(`text=${serviceType}, [data-testid*="${serviceType.toLowerCase()}"]`);
    await serviceTab.click();
    await this.page.waitForLoadState('networkidle');
  }

  async refreshResources() {
    const refreshBtn = this.page.locator('button:has-text("Refresh"), button:has-text("Sync")');
    if (await refreshBtn.isVisible()) {
      await refreshBtn.click();
      await this.page.waitForLoadState('networkidle');
    }
  }

  async filterByRegion(region: string) {
    const regionSelector = this.page.locator('select[name*="region"], .region-selector');
    if (await regionSelector.isVisible()) {
      await regionSelector.selectOption({ label: region });
      await this.page.waitForLoadState('networkidle');
    }
  }
}

// Test utilities
export async function waitForTableLoad(page: Page, timeout = 10000) {
  await page.waitForSelector('table, .ag-grid, .data-grid', { timeout });
}

export async function expectTableHasData(page: Page, minRows = 1) {
  const table = page.locator('table, .ag-grid, .data-grid');
  await expect(table).toBeVisible();

  const rows = table.locator('tbody tr, .ag-row, .grid-row');
  await expect(rows).toHaveCount(minRows);
}

export async function mockApiResponse(page: Page, url: string, response: any) {
  await page.route(url, route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify(response)
  }));
}

export async function takeScreenshotOnFailure(page: Page, testName: string) {
  await page.screenshot({
    path: `test-results/${testName}-failure.png`,
    fullPage: true
  });
}
