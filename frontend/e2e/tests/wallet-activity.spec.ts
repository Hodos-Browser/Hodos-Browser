/**
 * Wallet Activity Tests — Activity Tab (/wallet?tab=1)
 *
 * Maps to: GitHub Issue #49 Section 3 (Activity Tab)
 *
 * The ActivityTab component:
 *   - Fetches from /wallet/activity?page=X&limit=10&filter=Y
 *   - Has filter buttons: All, Sent, Received
 *   - Shows paginated transaction list or empty state
 *   - Each item has txid copy button and WhatsOnChain link
 */

import { test, expect } from '@playwright/test';
import { BRIDGE_MOCK_SCRIPT } from '../helpers/bridge-mock';

test.describe('Wallet Activity Tab — Empty State (#49 Section 3)', () => {
  test.beforeEach(async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });

    // Mock balance for dashboard (needed during initial load)
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
        body: JSON.stringify({ address: '1MockAddr', publicKey: '02mock' }),
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
        body: JSON.stringify({ active: false, phase: 'idle', addresses_scanned: 0, utxos_found: 0, total_satoshis: 0, result_seen: true, error: null }),
      });
    });

    // Empty activity
    await page.route('**/wallet/activity*', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          items: [],
          total: 0,
          page: 1,
          page_size: 10,
          current_price_usd_cents: 5247,
        }),
      });
    });

    // Navigate to wallet with Activity tab selected
    await page.goto('/wallet?tab=1', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1500);
  });

  test('Activity tab is active when tab=1 URL param', async ({ page }) => {
    const header = page.locator('.wd-content-title');
    await expect(header).toContainText('Activity');
  });

  test('filter buttons exist: All, Sent, Received', async ({ page }) => {
    // ActivityTab renders filter buttons
    await expect(page.getByRole('button', { name: 'All' })).toBeVisible({ timeout: 5000 });
    await expect(page.getByRole('button', { name: 'Sent' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Received' })).toBeVisible();
  });

  test('empty state message shows when no transactions', async ({ page }) => {
    // ActivityTab shows "No transactions yet" when items=[]
    const emptyState = page.getByText('No transactions yet');
    await expect(emptyState).toBeVisible({ timeout: 5000 });
  });
});

test.describe('Wallet Activity Tab — With Transactions (#49 Section 3)', () => {
  test('transaction list renders with mock data', async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });

    // Mock all required endpoints
    await page.route('**/wallet/balance', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ balance: 5432100, bsvPrice: 52.47 }) });
    });
    await page.route('**/wallet/address/current', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ address: '1MockAddr', publicKey: '02mock' }) });
    });
    await page.route('**/wallet/peerpay/status', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ count: 0, amount: 0 }) });
    });
    await page.route('**/wallet/sync-status', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ active: false, phase: 'idle', addresses_scanned: 0, utxos_found: 0, total_satoshis: 0, result_seen: true, error: null }) });
    });

    // Activity with transactions
    await page.route('**/wallet/activity*', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          items: [
            {
              txid: 'abc123def456789',
              direction: 'received',
              satoshis: 100000,
              status: 'completed',
              timestamp: '2025-01-15T10:30:00Z',
              description: 'Payment received',
              labels: ['incoming'],
              price_usd_cents: 5200,
              source: 'on-chain',
            },
            {
              txid: 'def789ghi012345',
              direction: 'sent',
              satoshis: 50000,
              status: 'completed',
              timestamp: '2025-01-14T08:15:00Z',
              description: 'Payment sent',
              labels: ['outgoing'],
              price_usd_cents: 5100,
              source: 'on-chain',
            },
          ],
          total: 2,
          page: 1,
          page_size: 10,
          current_price_usd_cents: 5247,
        }),
      });
    });

    await page.goto('/wallet?tab=1', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1500);

    // Should see transaction items with their descriptions
    await expect(page.getByText('Payment received')).toBeVisible({ timeout: 5000 });
    await expect(page.getByText('Payment sent')).toBeVisible({ timeout: 5000 });

    // Should show "Showing 1-2 of 2" count
    await expect(page.getByText('Showing 1-2 of 2')).toBeVisible();

    // TXID buttons should exist (DOM text is lowercase "txid", displayed uppercase via CSS)
    const txidButtons = page.locator('.wd-txid-pill');
    expect(await txidButtons.count()).toBe(2);
  });
});
