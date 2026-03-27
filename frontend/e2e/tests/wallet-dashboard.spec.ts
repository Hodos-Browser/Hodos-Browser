/**
 * Wallet Dashboard Tests — Advanced Wallet Dashboard (/wallet)
 *
 * Maps to: GitHub Issue #49 Section 2 (Advanced Wallet Dashboard)
 *
 * WalletOverlayRoot renders a sidebar + tab content layout:
 *   - WalletSidebar with 5 tabs: Dashboard, Activity, Certificates, Approved Sites, Settings
 *   - Default tab (0) = DashboardTab: balance, receive, send, recent activity
 *
 * DashboardTab fetches directly from localhost:31301 (Rust wallet backend).
 * We intercept those fetch calls to provide test data.
 */

import { test, expect } from '@playwright/test';
import { BRIDGE_MOCK_SCRIPT } from '../helpers/bridge-mock';

test.describe('Wallet Dashboard — Layout & Navigation (#49 Section 2)', () => {
  test.beforeEach(async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });

    // Mock the REST endpoints that DashboardTab calls directly
    await page.route('**/wallet/balance', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ balance: 5432100, bsvPrice: 52.47 }),
      });
    });

    await page.route('**/wallet/address/current', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          address: '1MockAddressForTesting123456789',
          publicKey: '02c5b4b8mock',
        }),
      });
    });

    await page.route('**/wallet/activity*', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          items: [],
          total: 0,
          page: 1,
          page_size: 5,
          current_price_usd_cents: 5247,
        }),
      });
    });

    await page.route('**/wallet/peerpay/status', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ count: 0, amount: 0 }),
      });
    });

    await page.route('**/wallet/sync-status', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          active: false,
          phase: 'idle',
          addresses_scanned: 0,
          utxos_found: 0,
          total_satoshis: 0,
          result_seen: true,
          error: null,
        }),
      });
    });

    await page.goto('/wallet', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1500);
  });

  test('wallet dashboard layout renders with sidebar', async ({ page }) => {
    // The main container with sidebar layout
    const dashboard = page.locator('.wallet-dashboard');
    await expect(dashboard).toBeVisible({ timeout: 5000 });
  });

  test('sidebar has all 5 navigation tabs', async ({ page }) => {
    const sidebar = page.locator('.wd-sidebar');
    await expect(sidebar).toBeVisible();

    // Check all tab labels
    await expect(page.locator('.wd-nav-item', { hasText: 'Dashboard' })).toBeVisible();
    await expect(page.locator('.wd-nav-item', { hasText: 'Activity' })).toBeVisible();
    await expect(page.locator('.wd-nav-item', { hasText: 'Certificates' })).toBeVisible();
    await expect(page.locator('.wd-nav-item', { hasText: 'Approved Sites' })).toBeVisible();
    await expect(page.locator('.wd-nav-item', { hasText: 'Settings' })).toBeVisible();
  });

  test('Dashboard tab is active by default', async ({ page }) => {
    const dashboardTab = page.locator('.wd-nav-item.active');
    await expect(dashboardTab).toContainText('Dashboard');
  });

  test('content header shows current tab title', async ({ page }) => {
    const header = page.locator('.wd-content-title');
    await expect(header).toBeVisible();
    await expect(header).toContainText('Dashboard');
  });

  test('refresh button exists in content header', async ({ page }) => {
    const refreshButton = page.locator('.wd-icon-button[title="Refresh"]');
    await expect(refreshButton).toBeVisible();
  });

  test('DashboardTab renders balance section', async ({ page }) => {
    // Balance section shows USD and BSV amounts
    // The DashboardTab uses .wd-* CSS classes
    const content = page.locator('.wd-content');
    await expect(content).toBeVisible();

    // Look for balance-related text (USD amount from mock: balance=5432100 sats, price=52.47)
    // 5432100 / 100000000 * 52.47 = ~2.85 USD
    // Use specific class to avoid strict mode violations (multiple BSV text occurrences)
    await expect(page.locator('.wd-balance-bsv')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('.wd-balance-bsv')).toContainText('BSV');
  });

  test('switching to Activity tab loads Activity content', async ({ page }) => {
    // Click on Activity tab
    const activityNav = page.locator('.wd-nav-item', { hasText: 'Activity' });
    await activityNav.click();

    // Header should change
    const header = page.locator('.wd-content-title');
    await expect(header).toContainText('Activity');
  });

  test('switching to Certificates tab loads content', async ({ page }) => {
    await page.route('**/listCertificates', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ certificates: [] }),
      });
    });

    const certsNav = page.locator('.wd-nav-item', { hasText: 'Certificates' });
    await certsNav.click();

    const header = page.locator('.wd-content-title');
    await expect(header).toContainText('Certificates');
  });

  test('switching to Approved Sites tab loads content', async ({ page }) => {
    await page.route('**/wallet/settings', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          default_per_tx_limit_cents: 10,
          default_per_session_limit_cents: 300,
          default_rate_limit_per_min: 10,
        }),
      });
    });

    await page.route('**/domain/permissions/all', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([]),
      });
    });

    const sitesNav = page.locator('.wd-nav-item', { hasText: 'Approved Sites' });
    await sitesNav.click();

    const header = page.locator('.wd-content-title');
    await expect(header).toContainText('Approved Sites');
  });

  test('switching to Settings tab loads content', async ({ page }) => {
    await page.route('**/wallet/settings', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ sender_display_name: 'Test User' }),
      });
    });

    await page.route('**/getPublicKey', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ publicKey: '02c5b4b8e7a9mock' }),
      });
    });

    const settingsNav = page.locator('.wd-nav-item', { hasText: 'Settings' });
    await settingsNav.click();

    const header = page.locator('.wd-content-title');
    await expect(header).toContainText('Settings');
  });
});
