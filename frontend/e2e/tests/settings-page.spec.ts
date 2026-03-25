/**
 * Settings Page Tests — Full-Page Settings (/settings-page)
 *
 * Maps to: GitHub Issue #49 Section 6 (Wallet Settings) + browser settings
 *
 * SettingsPage has a sidebar with 5 sections:
 *   - General: startup, search engine
 *   - Privacy & Security: adblock, cookies, fingerprinting, DNT
 *   - Downloads: download folder, save prompt
 *   - Wallet: opens wallet overlay via IPC (externalAction)
 *   - About Hodos: static version info
 */

import { test, expect } from '@playwright/test';
import { BRIDGE_MOCK_SCRIPT } from '../helpers/bridge-mock';

test.describe('Settings Page — Layout & Navigation (#49 Section 6)', () => {
  test.beforeEach(async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });
    await page.goto('/settings-page', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1000);
  });

  test('settings page renders with sidebar', async ({ page }) => {
    // SettingsPage uses a flex layout with sidebar
    await expect(page.getByText('Settings').first()).toBeVisible({ timeout: 5000 });
  });

  test('sidebar has all 5 navigation sections', async ({ page }) => {
    // Sidebar items are inside a 220px sidebar panel
    const sidebar = page.locator('div').filter({ has: page.getByText('Settings').first() }).first();
    await expect(page.getByText('General').first()).toBeVisible();
    await expect(page.getByText('Privacy & Security')).toBeVisible();
    await expect(page.getByText('Downloads').first()).toBeVisible();
    await expect(page.getByText('Wallet').first()).toBeVisible();
    await expect(page.getByText('About Hodos')).toBeVisible();
  });

  test('General section is active by default', async ({ page }) => {
    // GeneralSettings has "General" heading
    const heading = page.locator('h5', { hasText: 'General' });
    await expect(heading).toBeVisible({ timeout: 5000 });
  });

  test('General section has startup and search engine settings', async ({ page }) => {
    await expect(page.getByText('Restore previous session')).toBeVisible();
    await expect(page.getByText('Default search engine')).toBeVisible();
  });
});

test.describe('Settings Page — Privacy Section (#49 Section 6)', () => {
  test('Privacy section renders with toggle controls', async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });
    await page.goto('/settings-page/privacy', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1000);

    // Privacy heading
    const heading = page.locator('h5', { hasText: 'Privacy' });
    await expect(heading).toBeVisible({ timeout: 5000 });

    // Shield controls (actual text from the rendered UI)
    await expect(page.getByText('Ad & tracker blocking')).toBeVisible();
    await expect(page.getByText('Third-party cookie blocking')).toBeVisible();
  });

  test('Privacy section has fingerprint protection toggle', async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });
    await page.goto('/settings-page/privacy', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1000);

    await expect(page.getByText('Fingerprint protection')).toBeVisible({ timeout: 5000 });
  });
});

test.describe('Settings Page — Downloads Section (#49 Section 6)', () => {
  test('Downloads section renders', async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });
    await page.goto('/settings-page/downloads', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1000);

    // Downloads heading
    const heading = page.locator('h5', { hasText: 'Downloads' });
    await expect(heading).toBeVisible({ timeout: 5000 });

    // Download location setting
    await expect(page.getByText('Download Location')).toBeVisible();
  });
});

test.describe('Settings Page — About Section (#49 Section 6)', () => {
  test('About section renders with version info', async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });
    await page.goto('/settings-page/about', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1000);

    // About heading
    const heading = page.locator('h5', { hasText: 'About Hodos Browser' });
    await expect(heading).toBeVisible({ timeout: 5000 });

    // Version info
    await expect(page.getByText('Hodos Browser 1.0.0')).toBeVisible();
    await expect(page.getByText('Chromium (CEF 136)')).toBeVisible();
    await expect(page.getByText('Rust + SQLite')).toBeVisible();
    await expect(page.getByText('BRC-100 (BSV)')).toBeVisible();
  });

  test('About section has project description', async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });
    await page.goto('/settings-page/about', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1000);

    await expect(page.getByText('Web3 browser')).toBeVisible({ timeout: 5000 });
    await expect(page.getByText('hodosbrowser.com')).toBeVisible();
  });
});

test.describe('Settings Page — Section Navigation (#49 Section 6)', () => {
  test('clicking Privacy sidebar item switches content', async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });
    await page.goto('/settings-page', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1000);

    // Click Privacy & Security in sidebar
    await page.getByText('Privacy & Security').click();
    await page.waitForTimeout(500);

    // Should show privacy content (actual text: "Ad & tracker blocking")
    await expect(page.getByText('Ad & tracker blocking')).toBeVisible({ timeout: 5000 });
  });

  test('clicking About sidebar item switches content', async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });
    await page.goto('/settings-page', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1000);

    // Click About Hodos in sidebar
    await page.getByText('About Hodos').click();
    await page.waitForTimeout(500);

    // Should show about content
    await expect(page.getByText('Hodos Browser 1.0.0')).toBeVisible({ timeout: 5000 });
  });
});
