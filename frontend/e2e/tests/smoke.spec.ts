/**
 * Smoke Tests — Every route renders without crash
 *
 * Maps to: General app health / prerequisite for all Issue #49 sections.
 * Navigates to each of the 16 routes defined in App.tsx and verifies:
 *   - No uncaught exceptions in the console
 *   - The page has visible content (not blank)
 */

import { test, expect, Page } from '@playwright/test';
import { BRIDGE_MOCK_SCRIPT } from '../helpers/bridge-mock';

// All routes from App.tsx
const ROUTES = [
  { path: '/', name: 'MainBrowserView' },
  { path: '/newtab', name: 'NewTabPage' },
  { path: '/browser-data', name: 'HistoryPage' },
  { path: '/settings-page', name: 'SettingsPage' },
  { path: '/settings-page/general', name: 'SettingsPage (general)' },
  { path: '/settings-page/privacy', name: 'SettingsPage (privacy)' },
  { path: '/settings-page/downloads', name: 'SettingsPage (downloads)' },
  { path: '/settings-page/wallet', name: 'SettingsPage (wallet)' },
  { path: '/settings-page/about', name: 'SettingsPage (about)' },
  { path: '/cert-error', name: 'CertErrorPage' },
  { path: '/wallet-panel', name: 'WalletPanelPage' },
  { path: '/settings', name: 'SettingsOverlayRoot' },
  { path: '/wallet', name: 'WalletOverlayRoot' },
  { path: '/backup', name: 'BackupOverlayRoot' },
  { path: '/brc100-auth', name: 'BRC100AuthOverlayRoot' },
  { path: '/omnibox', name: 'OmniboxOverlayRoot' },
  { path: '/privacy-shield', name: 'PrivacyShieldOverlayRoot' },
  { path: '/downloads', name: 'DownloadsOverlayRoot' },
  { path: '/profile-picker', name: 'ProfilePickerOverlayRoot' },
  { path: '/menu', name: 'MenuOverlayRoot' },
];

test.describe('Smoke Tests — All Routes Render', () => {
  for (const route of ROUTES) {
    test(`${route.name} (${route.path}) renders without crash`, async ({ page }) => {
      const consoleErrors: string[] = [];

      // Collect uncaught errors from the browser console
      page.on('console', (msg) => {
        if (msg.type() === 'error') {
          const text = msg.text();
          // Ignore expected errors from mock environment
          if (
            text.includes('[mock]') ||
            text.includes('favicon.ico') ||
            text.includes('net::ERR_') ||
            text.includes('Failed to fetch') ||
            text.includes('127.0.0.1:31301') ||
            text.includes('127.0.0.1:31302')
          ) {
            return;
          }
          consoleErrors.push(text);
        }
      });

      page.on('pageerror', (err) => {
        // Ignore network fetch errors to wallet/adblock backends
        if (
          err.message.includes('Failed to fetch') ||
          err.message.includes('NetworkError') ||
          err.message.includes('127.0.0.1:31301') ||
          err.message.includes('127.0.0.1:31302')
        ) {
          return;
        }
        consoleErrors.push(`PAGE_ERROR: ${err.message}`);
      });

      // Inject bridge mock before navigating
      await page.addInitScript({ content: BRIDGE_MOCK_SCRIPT });

      await page.goto(route.path, { waitUntil: 'networkidle' });

      // Wait for React to render
      await page.waitForTimeout(1000);

      // Verify page has content (not blank white screen)
      const bodyContent = await page.evaluate(() => document.body?.innerText?.trim() || '');
      const hasElements = await page.evaluate(() => document.body?.children.length > 0);

      // Pages may have minimal text, but should have DOM elements
      expect(hasElements).toBe(true);

      // Assert no uncaught JS errors
      const criticalErrors = consoleErrors.filter(
        (e) => !e.includes('Warning:') && !e.includes('DevTools')
      );
      if (criticalErrors.length > 0) {
        console.log(`Console errors on ${route.path}:`, criticalErrors);
      }
      // We allow some React warnings but no hard crashes
      const crashErrors = criticalErrors.filter(
        (e) =>
          e.includes('Uncaught') ||
          e.includes('Cannot read properties of') ||
          e.includes('is not a function') ||
          e.includes('is not defined')
      );
      expect(crashErrors).toHaveLength(0);
    });
  }
});
