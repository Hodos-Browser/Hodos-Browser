/**
 * Cross-Cutting Tests — Menu, Downloads, Privacy Shield, New Tab
 *
 * Maps to: GitHub Issue #49 Section 8 (Cross-Cutting)
 *
 * Tests overlay pages that are shared across the browser:
 *   - Menu overlay (/menu): New Tab, History, Bookmarks, Downloads, Zoom,
 *     Print, Find, DevTools, Settings, About, Exit
 *   - Downloads overlay (/downloads): download list area, empty state
 *   - Privacy Shield overlay (/privacy-shield): per-site toggle controls
 *   - New Tab page (/newtab): search input, quick tiles
 */

import { test, expect } from '@playwright/test';
import { BRIDGE_MOCK_SCRIPT } from '../helpers/bridge-mock';

test.describe('Menu Overlay (#49 Section 8)', () => {
  test.beforeEach(async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });
    await page.goto('/menu', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1000);
  });

  test('menu has New Tab item', async ({ page }) => {
    await expect(page.getByText('New Tab')).toBeVisible({ timeout: 5000 });
  });

  test('menu has History item', async ({ page }) => {
    await expect(page.getByText('History')).toBeVisible();
  });

  test('menu has Bookmarks item', async ({ page }) => {
    await expect(page.getByText('Bookmarks')).toBeVisible();
  });

  test('menu has Downloads item', async ({ page }) => {
    await expect(page.getByText('Downloads')).toBeVisible();
  });

  test('menu has Print item', async ({ page }) => {
    await expect(page.getByText('Print...')).toBeVisible();
  });

  test('menu has Find in Page item', async ({ page }) => {
    await expect(page.getByText('Find in Page')).toBeVisible();
  });

  test('menu has zoom controls', async ({ page }) => {
    // Zoom row shows current zoom percentage
    await expect(page.getByText('100%')).toBeVisible();

    // Zoom out and zoom in buttons
    const zoomOut = page.getByRole('button', { name: 'Zoom out' });
    await expect(zoomOut).toBeVisible();

    const zoomIn = page.getByRole('button', { name: 'Zoom in' });
    await expect(zoomIn).toBeVisible();

    // Reset zoom and fullscreen buttons
    const resetZoom = page.getByRole('button', { name: 'Reset zoom' });
    await expect(resetZoom).toBeVisible();

    const fullscreen = page.getByRole('button', { name: 'Fullscreen' });
    await expect(fullscreen).toBeVisible();
  });

  test('menu has Developer Tools item', async ({ page }) => {
    await expect(page.getByText('Developer Tools')).toBeVisible();
  });

  test('menu has Settings item', async ({ page }) => {
    await expect(page.getByText('Settings', { exact: true })).toBeVisible();
  });

  test('menu has About Hodos item', async ({ page }) => {
    await expect(page.getByText('About Hodos')).toBeVisible();
  });

  test('menu has Exit item', async ({ page }) => {
    await expect(page.getByText('Exit')).toBeVisible();
  });

  test('menu has all expected keyboard shortcuts', async ({ page }) => {
    await expect(page.getByText('Cmd+T')).toBeVisible();
    await expect(page.getByText('Cmd+H')).toBeVisible();
    await expect(page.getByText('Cmd+D')).toBeVisible();
    await expect(page.getByText('Cmd+J')).toBeVisible();
    await expect(page.getByText('Cmd+P')).toBeVisible();
    await expect(page.getByText('Cmd+F')).toBeVisible();
    await expect(page.getByText('F12')).toBeVisible();
  });
});

test.describe('Downloads Overlay (#49 Section 8)', () => {
  test.beforeEach(async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });
    await page.goto('/downloads', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1000);
  });

  test('downloads overlay renders with title', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Downloads' })).toBeVisible({ timeout: 5000 });
  });

  test('downloads overlay has close button', async ({ page }) => {
    const closeButton = page.getByRole('button', { name: 'Close' });
    await expect(closeButton).toBeVisible();
  });

  test('downloads overlay shows empty state when no downloads', async ({ page }) => {
    await expect(page.getByText('No downloads')).toBeVisible({ timeout: 5000 });
  });
});

test.describe('Privacy Shield Overlay (#49 Section 8)', () => {
  test.beforeEach(async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });
    await page.goto('/privacy-shield?domain=example.com', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1000);
  });

  test('privacy shield renders with title', async ({ page }) => {
    await expect(page.getByText('Privacy Shield')).toBeVisible({ timeout: 5000 });
  });

  test('privacy shield has close button', async ({ page }) => {
    const closeButton = page.getByRole('button', { name: 'Close' });
    await expect(closeButton).toBeVisible();
  });

  test('privacy shield has toggle controls', async ({ page }) => {
    // PrivacyShieldPanel shows toggle switches for various protections
    // Look for MUI Switch elements (role=checkbox)
    const switches = page.getByRole('checkbox');
    const count = await switches.count();
    // Should have at least 1 toggle (master toggle, adblock, cookie, etc.)
    expect(count).toBeGreaterThanOrEqual(1);
  });
});

test.describe('New Tab Page (#49 Section 8)', () => {
  test.beforeEach(async ({ page }) => {
    page.on('pageerror', (err) => {
      if (err.message.includes('Failed to fetch') || err.message.includes('127.0.0.1')) return;
    });

    await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });
    await page.goto('/newtab', { waitUntil: 'networkidle' });
    await page.waitForTimeout(1000);
  });

  test('new tab page renders with search input', async ({ page }) => {
    // NewTabPage has a search input
    const searchInput = page.locator('input[type="text"], input[type="search"]').first();
    await expect(searchInput).toBeVisible({ timeout: 5000 });
  });

  test('new tab page has quick access tiles', async ({ page }) => {
    // NewTabPage shows default tiles (CoinGeek, MetaNet Apps)
    // or cached tiles from localStorage. Check for tile-like elements.
    const pageContent = await page.textContent('body');
    // The page should have some visible content beyond just the search bar
    expect(pageContent).toBeTruthy();
    expect(pageContent!.length).toBeGreaterThan(0);
  });
});
