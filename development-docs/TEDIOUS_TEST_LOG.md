# Tedious BSV Testing Log (#49)

**Tester:** John
**Date:** 2026-03-27
**Build:** feature/83-payment-indicator-permissions (includes #85 + #83)
**Platform:** macOS
**Automated:** 73/73 Playwright tests passing (UI structure verified)

---

## Pre-Test Setup

- [x] Fresh wallet created (or recovered from known mnemonic with known balance)
- [ ] At least 2 UTXOs available for testing sends
- [x] BSV price feed working: 1 BSV = $13.44
- [x] Rust wallet backend running (`cargo run --release`)
- [x] Adblock engine running
- [x] Frontend dev server running
- [x] BSV balance confirmed: 5000 satoshis ($0.00 — dust, correct)

---

## 1. Light Wallet Panel (Toolbar Overlay)

### Balance Display
- [x] 🤖 USD balance displays correctly ($0.00 — correct for 5000 sat)
- [x] 🤖 BSV balance displays correctly (0.00005000 BSV)
- [x] 🤖 Exchange rate shows (1 BSV = $13.44 USD)
- [x] 🤖 Refresh button exists and updates balance
- [x] Balance updates after send/receive (PeerPay receive: 5000→6000 sat)

### Send — Legacy (P2PKH Address)
- [x] Enter valid BSV address (starts with 1) — accepted
- [x] Enter invalid address — error shown (on send click)
- [x] Enter amount in USD — BSV auto-calculates
- [x] Enter amount in BSV — USD auto-calculates
- [ ] ❌ Max button fills total balance — does NOT subtract fee
- [x] Send succeeds — success message with TxID (slow/janky but works)
- [x] WhatsOnChain link works (opens in new tab)
- [x] Balance updates after send
- [x] Send form Close button works (moves to bottom when form open)

**Issues found:**
```
NOTE: Send is slow and janky
BUG: Minimum send amount $0.01 USD too high for micro-amounts
BUG: Max button doesn't subtract fee from total balance
BUG: Send button disabled for PeerPay (identity key) and Paymail — only P2PKH address works
BUG: New approved sites don't use updated default limits — still get old defaults (10 cents per-tx)
CRITICAL: All 12 recovery phrase words display as the same word — mnemonic reveal is broken
NOTE: google.com detects unusual traffic — triggers captcha before search works
```

### Send — PeerPay (Identity Key)
- [x] Enter valid identity key (02/03 + 64 hex) — accepted, routes to PeerPay
- [ ] ❌ Send button disabled — cannot send via PeerPay
- [ ] Recipient receives notification (blocked by above)

### Send — Paymail
- [x] Enter valid paymail (user@domain or $handle) — resolves with avatar
- [ ] ❌ Send button disabled — can't send to paymail
- [ ] P2P-capable paymail: sends via P2P protocol (blocked)
- [ ] Basic paymail: falls back to P2PKH (blocked)
- [x] Invalid paymail — error shown ("paymail not found")

### Receive
- [x] Receive button generates new address
- [x] Address copied to clipboard
- [x] QR code displays correctly and is scannable
- [x] "Copy Again" button works

### Identity Key
- [x] 🤖 "Copy ID Key" button works — copied 03d902f35f...
- [x] 🤖 "Show ID Key" reveals key + QR code
- [x] "Hide ID Key" collapses the section
- [x] QR code is scannable and matches displayed key

### PeerPay Notifications
- [x] Green dot appears on wallet icon when unread payments exist
- [ ] ❌ Notification banner shows count + amount — amount shows 0.00000000 BSV (~$0.00) instead of actual
- [ ] ❌ "Details" button not visible on notification banner
- [x] "Dismiss" button clears notification and green dot
- [x] Green dot clears after dismiss

**Issues found:**
```
BUG: Received payment notification shows "0.00000000 BSV (~$0.00)" instead of actual received amount (5000 sat)
```

### Panel Behavior
- [x] Panel opens on wallet button click
- [x] Click outside closes panel
- [x] Receive/Identity sections hide when send form is open
- [x] Send form Close button works — restores Receive/Identity sections
- [x] All hover effects work (gold on send/receive, silver on ID key buttons, gold on advanced)

---

## 2. Advanced Wallet Dashboard

### Balance Section (Top-Left Quadrant)
- [x] "Total Balance" header matches other section headers
- [x] 🤖 USD balance displays correctly (gold color)
- [x] BSV balance displays correctly (silver color)
- [x] Exchange rate shows
- [x] Refresh button works
- [x] Incoming payment notification banner appears when PeerPay payments received
- [x] Dismiss clears notification

### Receive Section (Top-Right Quadrant)
- [x] Identity Key: QR code displays, "Copy Key" works
- [x] Identity Key: tooltip info icon shows explanation
- [x] Subtitle shows "(Public Key - use with BRC-100 wallets)"
- [x] Receive Address: QR code displays, "Copy Address" works
- [x] "New Address" generates fresh address
- [x] Subtitle shows "(P2PKH Address - use with Handcash, RockWallet, etc.)"
- [x] Tooltip info icon shows explanation

### Send Section (Bottom-Left Quadrant)
- [x] "Send Bitcoin SV" header with divider line
- [ ] ❌ Only P2PKH works — PeerPay and Paymail send buttons disabled (same bug as wallet panel)
- [x] Transaction result banner (success/error) displays correctly
- [x] WhatsOnChain link in success banner opens new tab
- [x] Copy TxID button works
- [x] Dismiss (X) clears result banner

### Recent Activity Section (Bottom-Right Quadrant)
- [x] Shows last 5 transactions
- [x] Direction arrows correct (up for sent, down for received)
- [ ] USD amounts show historical price when available
- [x] Relative timestamps accurate (Just now, Xm ago, Xh ago, etc.)
- [x] Status badges correct (completed, unproven, failed)
- [x] "txid" pill button copies TxID to clipboard
- [x] WhatsOnChain icon opens tx in new tab
- [x] "View All" navigates to Activity tab
- [ ] Empty state message when no transactions (N/A — has transactions)

### Layout
- [x] Four quadrants display correctly
- [x] Gold borders on all four sections
- [x] Section headers consistent (16px, bold, white, divider line)
- [x] No unexpected scrollbars
- [x] Responsive: collapses to single column on narrow window

---

## 3. Activity Tab

- [x] 🤖 Transaction list renders
- [x] 🤖 Filter buttons work (All, Sent, Received)
- [x] Filter buttons work (All, Sent, Received) — manually verified
- [ ] Pagination controls work — N/A, insufficient transactions to trigger pagination
- [ ] "Go to" page jump works when >7 pages — N/A, insufficient transactions
- [x] Each transaction shows: direction, description, time, status, USD amount, BSV amount
- [x] USD amounts display on transactions
- [x] "txid" pill copies to clipboard
- [x] WhatsOnChain icon opens in new tab

---

## 4. Certificates Tab

### Acquire Certificate Flow
- [x] Navigate to a BRC-100 site that issues certificates (socialcerts.net)
- [x] Certificate acquisition flow completed on site
- [ ] ❌ Certificate does NOT appear in Certificates tab — acquireCertificate never called in wallet logs
- [ ] Field disclosure checkboxes work (not tested — cert didn't store)
- [ ] "Remember for this site" option works (not tested)

**Issues found:**
```
BUG 13: Certificate acquisition from socialcerts.net doesn't store cert — no acquireCertificate call reaches wallet backend. Site flow completes but cert is lost.
```

### Publish Certificate
- [ ] Select certificate → Publish action
- [ ] Certificate published on-chain (creates PushDrop output)
- [ ] Published status reflected in UI
- [ ] WhatsOnChain link for publish transaction works

### Unpublish Certificate
- [ ] Select published certificate → Unpublish action
- [ ] Certificate reclaimed from on-chain
- [ ] Status updates in UI
- [ ] Balance restored (PushDrop UTXO reclaimed)

### Certificate Verification
- [ ] Verify published certificate is provable via `proveCertificate`
- [ ] Keyring decryption works for selective field disclosure

> **Known issue:** proveCertificate keyring decryption (raw vs encrypted)

---

## 5. Approved Sites Tab

### Default Limits
- [x] Default per-transaction limit displays (in USD)
- [x] Default per-session limit displays (in USD)
- [x] Default rate limit displays (requests/min)
- [x] Edit defaults → saves correctly
- [x] "Reset All" with confirmation → resets all sites to defaults

### Per-Site Permissions
- [x] All approved domains listed (teragun.com visible)
- [x] Edit site → per-tx limit, per-session limit, rate limit editable
- [x] Save changes → persisted
- [x] Revoke site → domain removed, confirmation required
- [ ] Revoked site re-prompts on next visit (need to revisit site to test)

---

## 6. Wallet Settings Tab

### Display Name
- [x] Current display name shows
- [x] Edit and save → persisted

### Security & Keys
- [x] Identity key show/hide toggle works
- [x] Copy identity key works
- [x] Recovery phrase reveal: requires PIN → shows numbered word grid
- [ ] ❌ All 12 recovery phrase words display as THE SAME WORD — CRITICAL BUG
- [ ] Incorrect PIN → error message (not tested, PIN was 0000 default)

### Wallet Rescan
- [x] Rescan button triggers blockchain scan (slow but works)
- [x] Results show: addresses scanned, new UTXOs, balance

### Export Backup
- [x] Requires password (8+ chars)
- [x] Downloads `.hodos-wallet` file
- [ ] File can be imported on fresh install (not tested)

### Delete Wallet
> Moved to Wave 5 (not ready)

---

## 7. BRC-100 Site Integration

### Domain Approval Flow
- [x] Navigate to BRC-100 site (metanetapps.com)
- [x] First visit → domain approval notification appears (slow but works)
- [x] Shows domain name and requested permissions
- [x] "Advanced" expands spending limit configuration
- [x] Allow → domain whitelisted, site proceeds
- [ ] Deny → site gets error response (not tested — allowed on first try)

**Issues found:**
```
NOTE: Domain approval notification is slow to appear
NOTE: Opening a link on the site opened a stripped-down "lite" popup window instead of a new tab
BUG 12 (LOW): UTXO sync doesn't pick up unconfirmed incoming transactions — only detects after confirmation. WoC shows UTXOs at height 0 but wallet ignores them until confirmed.
```

### Authentication (BRC-103/104)
- [x] Site triggers auth → auth notification appears (teragun.com)
- [x] Approve → mutual authentication completes
- [x] Session established (subsequent requests auto-approved)
- [ ] Different tab/site → separate auth session (not tested)

### Payment Confirmation
- [x] Site requests payment within auto-approve limit → auto-approved silently (teragun.com)
- [ ] Site requests payment OVER per-tx limit → payment confirmation notification (not tested)
- [ ] Shows amount in satoshis + USD conversion (not tested)
- [ ] Approve → payment executes (not tested)
- [ ] Deny → site gets error (not tested)
- [ ] Session spending tracked → over per-session limit triggers notification (not tested)

### Rate Limiting
- [ ] Rapid requests (>10/min default) → rate limit notification (not tested)
- [ ] Shows current limits (not tested)
- [ ] "Update Limits" option (not tested)
- [ ] Deny → blocks further requests (not tested)

### Certificate Disclosure
- [ ] Site requests certificate fields → disclosure notification
- [ ] Field checkboxes for selective disclosure
- [ ] "Remember for this site" saves preferences
- [ ] Share → fields disclosed to site
- [ ] Deny → site gets error

### No Wallet State
- [ ] Visit BRC-100 site with no wallet → "Set up wallet" prompt
- [ ] "Setup Wallet" button opens wallet panel

---

## 8. Cross-Cutting Checks

### Settings Page
- [x] Settings opens from menu (three-dot → Settings)
- [x] Sidebar navigation switches between sections
- [x] Change a setting → close → reopen → persisted

### Menu (Three-Dot)
- [x] Menu opens on click
- [x] New Tab works
- [x] History opens browser-data page
- [x] Downloads panel opens (also Cmd+J overlay)
- [ ] ❌ Zoom in/out/reset — menu closes on click (should stay open), percentage never updates
- [x] Find opens find bar
- [x] DevTools opens (F12)
- [x] Exit closes browser cleanly (no lingering audio) ← verifies #85

**Issues found:**
```
BUG: Zoom +/- closes menu instead of staying open. Zoom % never updates.
```

### Find Bar
- [x] Cmd+F opens find bar
- [x] Typing highlights matches on page
- [x] "X of Y" count displays
- [x] Enter = next, Shift+Enter = prev, Escape = close
- [x] Red background on 0 matches

### New Tab Page
- [x] New tab shows search bar
- [x] Quick-access tiles display
- [x] Search bar works (navigates to search engine)
- [x] Tiles navigate to correct sites

### Privacy Shield
- [x] Shield icon in toolbar opens overlay
- [x] Shows domain name of current site
- [x] Ad blocking toggle works
- [x] Cookie blocking toggle works
- [ ] ❌ Scriptlet injection toggle doesn't work
- [ ] ❌ Blocked counts not displaying

### Downloads
- [x] Cmd+J opens downloads panel
- [x] Download a file → progress bar shows
- [x] Pause/resume works
- [x] Open file works
- [x] Show in folder works
- [x] Clear completed works

### Tab Management
- [x] Cmd+T creates new tab
- [x] Cmd+W closes active tab
- [x] Click to switch tabs
- [x] Drag to reorder tabs
- [x] Loading spinner on tab while page loads
- [x] Right-click → "Manage Site Permissions" appears ← NEW (#83)

### WhatsOnChain Links
- [x] All WoC links throughout the app open correctly in new tab
- [x] Transaction links: `https://whatsonchain.com/tx/{txid}`
- [x] Links work for both sent and received transactions

### Notification Behavior
- [x] Only one notification overlay visible at a time — verified with two BRC-100 sites
- [ ] Notification timeout works (auto-dismiss after period) — not observed, dismissed manually
- [x] No double-fire crashes observed during testing
- [ ] Notification appears for correct window in multi-window (not tested — single window)

### Error Handling
- [ ] Wallet backend down → graceful error messages (skipped for now)
- [ ] Network failure during send → clear error, inputs restored (skipped for now)
- [x] Invalid recipient → validation error before send attempt

---

## Wave 5: Not Ready (Blocked — Needs Code Changes)

- [ ] Certificate info in domain approval DB/UI/forms
- [ ] Wallet delete flow — two-step confirmation (type "DELETE" → enter PIN)
- [ ] Wallet delete — balance warning if funds remain
- [ ] Wallet delete — successful deletion clears all data
- [ ] Wallet recovery from settings
- [ ] Certificate publish error handling (auto-reclaim PushDrop)
- [ ] Unpublish refactor (needs createAction)

---

## Site Compatibility

- [x] youtube.com — video plays, no ad leakage
- [ ] x.com — skipped (requires login)
- [x] github.com — repos load, can browse code
- [x] google.com — search works (detected unusual traffic, captcha required first)
- [x] whatsonchain.com — BSV explorer works

---

## Summary

| Section | Pass | Fail | Untested | Status |
|---------|------|------|----------|--------|
| §1 Wallet Panel | 18 | 2 | 10 | 🟡 Sends left |
| §2 Dashboard | 21 | 0 | 6 | 🟡 Sends left |
| §3 Activity | 6 | 0 | 2 | ✅ Done (pagination N/A — insufficient txs) |
| §4 Certificates | 2 | 1 | 8 | 🔴 Cert acquisition broken |
| §5 Approved Sites | 9 | 0 | 1 | ✅ Done |
| §6 Wallet Settings | 7 | 1 | 1 | ✅ Done |
| §7 BRC-100 Integration | 8 | 0 | 14 | 🟡 Auth+payment work, limits/deny untested |
| §8 Cross-Cutting | 32 | 3 | 4 | 🟡 Error handling + multi-window skipped |
| Site Compatibility | 4 | 0 | 1 | ✅ Done (x.com skipped — login) |
| **Testable Total** | **121** | **10** | **31** | **85%** |
| Wave 5 (Blocked) | — | — | 7 | 🔴 Needs code changes |

**Bugs found: 13 (1 critical, 2 high, 7 medium, 3 low)**
```
CRITICAL:
11. All 12 recovery phrase words display as the same word — NOT A CODE BUG, test wallet had BIP39 zeros mnemonic

HIGH (FIXED in PR #91):
5.  Closing last tab closes entire window — FIXED
9.  Send button disabled for PeerPay/Paymail — FIXED

MEDIUM (FIXED in PR #91):
1.  Zoom +/- closes menu, % never updates — FIXED
7.  Minimum send amount $0.01 USD too high — FIXED
8.  Max button doesn't subtract fee — FIXED

MEDIUM (Archie's / unfixed):
2.  Received payment notification shows 0.00000000 BSV
4.  Scriptlet injection toggle doesn't work
10. New approved sites don't use updated default limits

LOW (FIXED in PR #91):
3.  Privacy shield blocked counts not displaying — FIXED

LOW (Archie's / unfixed):
6.  PeerPay notification banner "Details" button missing

NEW BUGS (found during Wave 2 testing):
12. (LOW) UTXO sync doesn't pick up unconfirmed incoming transactions — only after confirmation
13. (MEDIUM) Certificate acquisition from socialcerts.net doesn't store cert — acquireCertificate never reaches backend
```

**Tracking issue:** #89 — Fix bugs found during tedious testing (#49)
