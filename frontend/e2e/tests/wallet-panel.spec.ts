/**
 * Wallet Panel Tests — Light Wallet Panel (/wallet-panel)
 *
 * Maps to: GitHub Issue #49 Section 1 (Light Wallet Panel)
 *
 * WalletPanelPage renders different UIs based on wallet status:
 *   - 'loading': spinner
 *   - 'no-wallet': create/recover/import UI
 *   - 'locked': PIN unlock UI
 *   - 'exists': live WalletPanel component with balance, send, receive, identity
 *
 * The bridge mock sets localStorage('hodos_wallet_exists', 'true') so the page
 * renders in the 'exists' state, showing the full WalletPanel.
 * We also test the 'no-wallet' state by clearing localStorage.
 */

import { test, expect } from '@playwright/test';
import { BRIDGE_MOCK_SCRIPT } from '../helpers/bridge-mock';

test.describe('Wallet Panel — Live Wallet State (#49 Section 1)', () => {
  test.beforeEach(async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });
    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });
    await page.goto('/wallet-panel', { waitUntil: 'networkidle' });
    // Wait for WalletPanel component to mount and render
    await page.waitForTimeout(1500);
  });

  test('balance display area shows USD and BSV amounts', async ({ page }) => {
    // The WalletPanel shows balance in .balance-display-light
    const balanceDisplay = page.locator('.balance-display-light');
    await expect(balanceDisplay).toBeVisible({ timeout: 5000 });

    // Check for USD amount display
    const usdText = page.locator('.balance-amount-light');
    await expect(usdText).toBeVisible();

    // Check for BSV amount display
    const bsvText = page.locator('.balance-secondary-light');
    await expect(bsvText).toBeVisible();
    await expect(bsvText).toContainText('BSV');
  });

  test('balance display has USD currency label', async ({ page }) => {
    const currencyLabel = page.locator('.balance-currency-light');
    await expect(currencyLabel).toBeVisible();
    await expect(currencyLabel).toContainText('USD');
  });

  test('refresh balance button exists', async ({ page }) => {
    const refreshButton = page.locator('.refresh-button-light');
    await expect(refreshButton).toBeVisible();
    await expect(refreshButton).toContainText('Refresh');
  });

  test('identity key section with copy and show buttons', async ({ page }) => {
    // Identity key section should be visible (localStorage has identity key)
    const identitySection = page.locator('.identity-key-section-light');
    await expect(identitySection).toBeVisible({ timeout: 5000 });

    // Copy Identity Key button
    const copyButton = page.locator('.identity-key-copy-button-light');
    await expect(copyButton).toBeVisible();
    await expect(copyButton).toContainText('Copy Identity Key');

    // Show/Hide button
    const showButton = page.locator('.identity-key-show-button-light');
    await expect(showButton).toBeVisible();
    await expect(showButton).toContainText('Show');
  });

  test('action buttons: Receive and Send', async ({ page }) => {
    const actionsArea = page.locator('.wallet-actions-light');
    await expect(actionsArea).toBeVisible();

    const receiveButton = page.locator('.receive-button-light');
    await expect(receiveButton).toBeVisible();
    await expect(receiveButton).toContainText('Receive');

    const sendButton = page.locator('.send-button-light');
    await expect(sendButton).toBeVisible();
    await expect(sendButton).toContainText('Send');
  });

  test('clicking Send shows TransactionForm', async ({ page }) => {
    const sendButton = page.locator('.send-button-light');
    await sendButton.click();

    // TransactionForm should appear in the dynamic content area
    const dynamicArea = page.locator('.dynamic-content-area-light');
    await expect(dynamicArea).toBeVisible();

    // The TransactionForm uses .transaction-form class
    const txForm = page.locator('.transaction-form');
    await expect(txForm).toBeVisible({ timeout: 5000 });
  });

  test('Advanced Wallet link exists', async ({ page }) => {
    // The "Advanced" button opens the full wallet dashboard
    const advancedButton = page.locator('text=Advanced');
    await expect(advancedButton).toBeVisible({ timeout: 5000 });
  });
});

test.describe('Wallet Panel — No Wallet State (#49 Section 1)', () => {
  test('shows create/recover UI when no wallet exists', async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    // Inject mock but clear the wallet exists flag
    await page.addInitScript({
      content: BRIDGE_MOCK_SCRIPT + `
        localStorage.removeItem('hodos_wallet_exists');
        localStorage.removeItem('hodos_identity_key');
      `,
    });

    // Also intercept the wallet status fetch to return no-wallet
    await page.route('**/wallet/status', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ exists: false, locked: false }),
      });
    });

    await page.goto('/wallet-panel', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1500);

    // Should show "Create Wallet" button
    const createButton = page.getByText('Create New Wallet');
    await expect(createButton).toBeVisible({ timeout: 5000 });

    // Should show "Recover Existing Wallet" option
    const recoverButton = page.getByText('Recover Existing Wallet');
    await expect(recoverButton).toBeVisible({ timeout: 5000 });
  });
});
