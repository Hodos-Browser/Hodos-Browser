import { test, expect } from '@playwright/test';

/**
 * beta.3 `D-h4` — the "1 of N" line went missing on a CONCURRENT prompt burst.
 *
 * SUBJECT: the React overlay's own timing, driven exactly as C++ drives it —
 * `window.showNotification(queryString)` then `window.updateQueuedCount(n)`.
 * Those are plain window functions, so the race reproduces in ordinary Chromium;
 * no CEF is needed to exercise it.
 *
 * ⛔ WHAT THIS DOES NOT PROVE: that C++ pushes at the right moment, or that the
 * overlay is reused rather than recreated. macOS measured that half live
 * (`Reusing existing notification overlay` at .860, push at .862). This covers
 * only the half that was broken — that a push arriving during the async window
 * survives the `applyParams` that lands after it.
 */

// Pinned to the dev server started from THIS source tree. 5137 was occupied and a
// stale server there would serve old code — the classic false green.
const OVERLAY = 'http://127.0.0.1:5138/brc100-auth';

// `applyParams` runs from a Promise.race bounded at 1200 ms, so any assertion made
// before that has not yet been exposed to the clobber this test exists to catch.
const PAST_APPLY_PARAMS_MS = 1800;

// How long the stubbed wallet call is held open. This is the whole reason the test
// is a test.
//
// 🚨 The first version of this file had NO stub, and its negative control PASSED
// with the fix reverted. `walletFetch` rides `window.__hodos_walletCall`, which does
// not exist in plain Chromium, so it threw immediately, `Promise.race` resolved in
// ~1 ms, and `applyParams` ran BEFORE the 2 ms push — the opposite order from the
// one production hits, where that call reaches a real wallet. The test was green
// either way because the clobber never had anything to clobber.
const WALLET_CALL_DELAY_MS = 600;

/** Hold the wallet settings call open so `applyParams` lands AFTER the push, as it does live. */
async function stubSlowWalletBridge(page: import('@playwright/test').Page) {
  await page.addInitScript((delay) => {
    (window as any).__hodos_walletCall = () =>
      new Promise((resolve) => setTimeout(() => resolve({}), delay));
  }, WALLET_CALL_DELAY_MS);
}

test('a queued-count push during the async window survives applyParams', async ({ page }) => {
  await stubSlowWalletBridge(page);
  await page.goto(`${OVERLAY}?type=payment_confirmation&domain=example.com&queuedFromSite=0`);

  // Wait for C++'s injection surface to exist, as C++ does.
  await page.waitForFunction(() => typeof (window as any).showNotification === 'function');

  // The burst, in the order macOS measured it: the overlay is (re)shown for the
  // FIRST request — whose frozen param is necessarily 0, nothing else had arrived —
  // and 2 ms later the SECOND request pushes the live count.
  await page.evaluate(() => {
    (window as any).showNotification('type=payment_confirmation&domain=example.com&queuedFromSite=0');
  });
  await page.waitForTimeout(2);
  await page.evaluate(() => { (window as any).updateQueuedCount(1); });

  const line = page.getByText(/1 of 2 requests from this site/);

  // It rendered at the push — this part was never broken.
  await expect(line).toBeVisible();

  // 🚨 THE ASSERTION THAT MATTERS: still there after applyParams lands.
  // Pre-fix, applyParams reset it to the URL's snapshot (0) and the line vanished.
  await page.waitForTimeout(PAST_APPLY_PARAMS_MS);
  await expect(line).toBeVisible();
});

test('with no live push, the URL snapshot still applies', async ({ page }) => {
  // The fix must not break the ordinary case: a prompt that really is 1-of-3 at
  // creation reads its count from the params, with no push involved.
  await stubSlowWalletBridge(page);
  await page.goto(`${OVERLAY}?type=payment_confirmation&domain=example.com&queuedFromSite=0`);
  await page.waitForFunction(() => typeof (window as any).showNotification === 'function');

  await page.evaluate(() => {
    (window as any).showNotification('type=payment_confirmation&domain=example.com&queuedFromSite=2');
  });

  await expect(page.getByText(/1 of 3 requests from this site/)).toBeVisible();
  await page.waitForTimeout(PAST_APPLY_PARAMS_MS);
  await expect(page.getByText(/1 of 3 requests from this site/)).toBeVisible();
});

test('a push from a PREVIOUS prompt is not inherited by the next one', async ({ page }) => {
  // The overlay is keep-alive, so anything not reset per prompt leaks across
  // prompts — P0.8 defect 5, same overlay. The ref is cleared in showNotification.
  await stubSlowWalletBridge(page);
  await page.goto(`${OVERLAY}?type=payment_confirmation&domain=example.com&queuedFromSite=0`);
  await page.waitForFunction(() => typeof (window as any).showNotification === 'function');

  await page.evaluate(() => {
    (window as any).showNotification('type=payment_confirmation&domain=a.com&queuedFromSite=0');
  });
  await page.evaluate(() => { (window as any).updateQueuedCount(4); });
  await expect(page.getByText(/1 of 5 requests from this site/)).toBeVisible();

  // Next prompt, genuinely alone: the stale 4 must not come with it.
  await page.evaluate(() => {
    (window as any).showNotification('type=payment_confirmation&domain=b.com&queuedFromSite=0');
  });
  await page.waitForTimeout(PAST_APPLY_PARAMS_MS);
  await expect(page.getByText(/requests from this site/)).toHaveCount(0);
});
