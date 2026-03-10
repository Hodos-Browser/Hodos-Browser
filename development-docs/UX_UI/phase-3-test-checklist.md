# Phase 3 — Test Checklist

**Phase:** Light Wallet Polish, QR Code, PeerPay Integration
**Date:** 2026-03-04
**Prerequisites:** All three servers running (Rust wallet :3301, Frontend :5137, CEF shell)

---

## Pre-Test Setup

- [ ] `cargo build --release` in `rust-wallet/` — no errors
- [ ] `npm run build` in `frontend/` — no errors
- [ ] `cmake --build build --config Release` in `cef-native/` — no errors
- [ ] Launch all three layers in order: Rust wallet → Frontend dev → CEF browser
- [ ] Open DevTools (Ctrl+Shift+I) for console log monitoring

---

## 1. Button Polish (Sprint 3.1a)

### 1.1 Advanced Button
- [ ] Open wallet panel → scroll to Advanced section
- [ ] Button renders as native `<button>` (not MUI)
- [ ] **Hover**: border/text color change visible
- [ ] **Active (click)**: scale down feedback (0.95)
- [ ] **Disabled state**: reduced opacity when applicable

### 1.2 "Manage Approved Sites" Link
- [ ] Visible below domain permissions section
- [ ] **Hover**: text gets underline, color darkens
- [ ] **Active (click)**: opacity reduces briefly
- [ ] Clicking navigates to correct settings page

### 1.3 Other Buttons
- [ ] **Send button**: hover lifts (translateY -1px), active state when form open
- [ ] **Receive button**: hover/active feedback visible
- [ ] **Refresh button**: active scales to 0.95
- [ ] **Close button**: active scales to 0.95

---

## 2. QR Code for Receive (Sprint 3.1b)

### 2.1 QR Code Display
- [ ] Click **Receive** in wallet panel
- [ ] Address generates and copies to clipboard
- [ ] QR code appears below the address text
- [ ] QR code inside white box with light gray border (8px radius)
- [ ] QR code is approximately 160x160px

### 2.2 QR Code Content
- [ ] QR encodes `bitcoin:<address>` (BIP21 format)
- [ ] Scan QR with mobile wallet (HandCash, RockWallet, ElectrumSV, Centbee) → confirms same address
- [ ] Alternatively, scan with any QR reader app → shows `bitcoin:1...` URI

### 2.3 QR Code Lifecycle
- [ ] QR only appears when `currentAddress` is set (after clicking Receive)
- [ ] Clicking Receive again with new address → QR updates
- [ ] Closing receive section → QR disappears

---

## 3. Notification Badge (Sprint 3.2)

### 3.1 Green Dot on Wallet Icon
- [ ] Wallet toolbar button shows `AccountBalanceWalletIcon`
- [ ] **No payments**: badge invisible (no green dot)
- [ ] **Unread payments exist**: green dot (8px, #2e7d32) at top-right of icon
- [ ] Dot appears without needing to click wallet

### 3.2 Polling Behavior
- [ ] Badge polls `/wallet/peerpay/status` every 60 seconds
- [ ] First poll fires 5 seconds after page load
- [ ] Console: no errors if wallet server is temporarily unavailable
- [ ] Badge updates on next successful poll after payments arrive

### 3.3 Dismiss Integration
- [ ] Open wallet panel → dismiss notification banner → green dot clears
- [ ] Badge correctly reflects dismissed state on next poll cycle

---

## 4. PeerPay Auto-Accept Setting (Sprint 3.2)

### 4.1 Settings UI
- [ ] Navigate to Settings → Wallet section
- [ ] "PeerPay" card visible with toggle switch
- [ ] Label: "Auto-accept PeerPay payments"
- [ ] Description text explains auto vs manual accept
- [ ] Default: toggle is ON

### 4.2 Toggle Persistence
- [ ] Toggle OFF → close settings → reopen settings → toggle still OFF
- [ ] Toggle ON → close browser → relaunch → toggle still ON
- [ ] Check `settings.json` in profile folder contains `peerpayAutoAccept` field

---

## 5. PeerPay Notification Banner (Sprint 3.2)

### 5.1 Banner Appearance (Auto-Accept ON)
- [ ] When unread payments exist, banner shows at top of wallet panel
- [ ] Green left border (3px, #2e7d32), light green background (#f1f8e9)
- [ ] Text: "Received N payment(s) (X.XXXXXXXX BSV)"
- [ ] "Dismiss" button visible (green background, white text)
- [ ] Banner fades in smoothly (0.3s animation)

### 5.2 Banner Appearance (Auto-Accept OFF)
- [ ] Toggle auto-accept OFF in settings
- [ ] Banner text: "You have N pending PeerPay payment(s)"
- [ ] "View on PeerPay" link visible (green, underlined)
- [ ] Clicking link opens `https://peerpay.babbage.systems` in new tab

### 5.3 Dismiss Flow
- [ ] Click "Dismiss" button
- [ ] Banner disappears immediately
- [ ] `POST /wallet/peerpay/dismiss` fires (check Rust console logs)
- [ ] IPC `wallet_payment_dismissed` sent (green dot on toolbar clears)
- [ ] Re-opening wallet panel: banner no longer shows

### 5.4 No Payments State
- [ ] When no unread payments exist, no banner shows
- [ ] Wallet panel loads normally without delay

---

## 6. PeerPay Send (Sprint 3.3)

### 6.1 Identity Key Detection
- [ ] Type a valid identity key (66-char hex starting with `02` or `03`)
- [ ] **Field hint changes**: "Sending via PeerPay (identity key detected)"
- [ ] **Button text changes**: "Send via PeerPay"
- [ ] Erase and type a BSV address → hint reverts to "Enter BSV address or identity key"
- [ ] Button text reverts to "Send Transaction"

### 6.2 Validation
- [ ] Empty recipient → error "Recipient is required"
- [ ] Invalid format (not address or identity key) → error "Enter a BSV address or identity key (66-char hex)"
- [ ] 65-char hex → rejected
- [ ] 67-char hex → rejected
- [ ] Key starting with `04` (uncompressed) → rejected
- [ ] Valid identity key + zero amount → error "Amount must be a positive number"
- [ ] Valid identity key + amount > balance → error "Insufficient balance"

### 6.3 Send Flow (with sufficient balance)
- [ ] Enter valid identity key + valid amount
- [ ] Click "Send via PeerPay"
- [ ] Button shows "Creating Transaction..." (disabled)
- [ ] Rust console shows: `PeerPay: X sats to XXXX... (derived key)`
- [ ] Success response: "Transaction Sent!" message with txid
- [ ] "Sent via PeerPay" status label in result
- [ ] WhatsOnChain link present and clickable
- [ ] Form resets after success
- [ ] Balance updates (decremented by sent amount + fee)

### 6.4 Send Error Handling
- [ ] Send with wallet server stopped → error shown (not crash)
- [ ] Send to self (own identity key) → should still succeed (self-payment)
- [ ] Network error during broadcast → error message shown, can retry

### 6.5 Standard Send (BSV Address) Still Works
- [ ] Enter valid BSV address (starts with `1` or `3`)
- [ ] Hint shows "Enter BSV address or identity key"
- [ ] Button shows "Send Transaction" (not PeerPay)
- [ ] Send succeeds via standard flow (no PeerPay endpoint used)

---

## 7. PeerPay Receive / Background Poller (Sprint 3.4)

### 7.1 Monitor Task Registration
- [ ] Rust console on startup shows: `TaskCheckPeerPay: every 60s`
- [ ] Monitor reports 8 tasks (was 7 before Phase 3)

### 7.2 Remote API Polling
- [ ] With internet: task runs silently every 60s
- [ ] Without internet: task logs debug message, no crash, retries next tick
- [ ] With no wallet created: task logs "no wallet yet, skipping"

### 7.3 Message Processing
- [ ] When remote MessageBox has BRC-29 payment (protocol `3241645161d8`):
  - [ ] Message stored in local MessageStore
  - [ ] Rust console: "TaskCheckPeerPay: stored N incoming payment(s)"
- [ ] Non-BRC-29 messages (wrong protocol) → silently ignored
- [ ] Malformed JSON body → silently skipped

### 7.4 End-to-End Receive Flow
- [ ] Payment sent to our identity key from another wallet
- [ ] Wait up to 60s for monitor task to poll
- [ ] `/wallet/peerpay/status` returns `unread_count > 0`
- [ ] Green dot appears on wallet toolbar icon (within 60s poll)
- [ ] Open wallet → notification banner visible
- [ ] Dismiss → banner + badge cleared

---

## 8. API Endpoint Verification

### 8.1 GET /wallet/peerpay/status
```bash
curl http://127.0.0.1:3301/wallet/peerpay/status
```
- [ ] Returns `{ "unread_count": N, "unread_amount": 0, "auto_accept": true }`
- [ ] `unread_count` matches actual unacknowledged messages

### 8.2 POST /wallet/peerpay/send
```bash
curl -X POST http://127.0.0.1:3301/wallet/peerpay/send \
  -H "Content-Type: application/json" \
  -d '{"recipient_identity_key":"02...","amount_satoshis":1000}'
```
- [ ] Valid request → `{ "success": true, "txid": "...", "message": "Sent via PeerPay", ... }`
- [ ] Invalid key → `{ "success": false, "error": "Invalid identity key..." }`
- [ ] Zero amount → `{ "success": false, "error": "Amount must be greater than 0" }`

### 8.3 POST /wallet/peerpay/check
```bash
curl -X POST http://127.0.0.1:3301/wallet/peerpay/check
```
- [ ] Returns `{ "success": true, "payments": [...], "count": N }`

### 8.4 POST /wallet/peerpay/dismiss
```bash
curl -X POST http://127.0.0.1:3301/wallet/peerpay/dismiss
```
- [ ] Returns `{ "success": true }`
- [ ] Subsequent status call shows `unread_count: 0`

---

## 9. Edge Cases & Error Resilience

### 9.1 Wallet Server Down
- [ ] Stop Rust wallet → frontend polling silently fails (no error popups)
- [ ] Badge stays in last known state (no flicker)
- [ ] Restart wallet → polling resumes, badge updates

### 9.2 No Wallet Created Yet
- [ ] Fresh install → peerpay endpoints return gracefully (empty/zero)
- [ ] Monitor task skips without errors
- [ ] No badge, no banner shown

### 9.3 Rapid Actions
- [ ] Dismiss banner twice quickly → no errors, no duplicate requests
- [ ] Open/close wallet panel rapidly → no leaked fetch requests or stale state
- [ ] Toggle auto-accept rapidly → last state wins, settings file correct

### 9.4 Amount Edge Cases
- [ ] Dust amount (< 546 sats) via PeerPay → validation catches it
- [ ] Very large amount (> balance) → validation catches it
- [ ] Decimal precision in USD mode → correct satoshi conversion

---

## 10. Visual & CSS Verification

- [ ] QR code container: white background, 1px #e0e0e0 border, 8px radius
- [ ] PeerPay banner: 3px green left border, #f1f8e9 background
- [ ] Dismiss button: #2e7d32 green background, white text, rounded
- [ ] PeerPay link: green text with underline
- [ ] Field hint under recipient: gray (#6b7280), 11px, italic
- [ ] Notification badge dot: green (#2e7d32), 8px circle, top-right
- [ ] All button hover states: visible color/position change
- [ ] All button active states: visible press feedback (scale/opacity)

---

## 11. Cross-Feature Integration

### 11.1 Full Send → Receive Loop
1. [ ] Note wallet identity key (from `/getPublicKey` or wallet status)
2. [ ] From another wallet/session: send PeerPay to that identity key
3. [ ] Wait for monitor task poll (up to 60s)
4. [ ] Green dot appears on wallet icon
5. [ ] Open wallet → banner shows received payment
6. [ ] Dismiss → badge clears, banner gone
7. [ ] Balance reflects received payment (after sync)

### 11.2 Settings ↔ Banner Interaction
1. [ ] Settings → Wallet → toggle auto-accept OFF
2. [ ] Receive payment → banner shows "pending" text with PeerPay link
3. [ ] Settings → Wallet → toggle auto-accept ON
4. [ ] Re-open wallet → banner shows "received" text with Dismiss button

### 11.3 Standard Site Testing (Minimal)
- [ ] youtube.com loads and plays video normally
- [ ] x.com loads and scrolls normally
- [ ] github.com loads and navigation works
- [ ] None of the Phase 3 changes break normal browsing

---

---

## Phase 3b: Paymail + Identity Resolution

### 12. Paymail Send (Sprint 3b.1-2)

#### 12.1 Paymail Detection
- [ ] Type `alice@handcash.io` → recipient dropdown appears with spinner
- [ ] Type `$alice` → converts to `alice@handcash.io`, dropdown appears
- [ ] Resolution completes: name + avatar + P2P badge shown
- [ ] Invalid paymail (e.g., `foo@nonexistent.xyz`) → dropdown shows error
- [ ] Clear field → dropdown disappears

#### 12.2 Paymail Send Flow
- [ ] Enter valid paymail + amount → click Send
- [ ] P2P path: transaction submitted back to receiver's server
- [ ] Basic fallback: works for paymails without P2P support
- [ ] Success: txid shown, form resets, balance updates
- [ ] Error: friendly message shown (network, invalid paymail, insufficient funds)

#### 12.3 HandCash Handle
- [ ] `$handle` format detected and converted to `handle@handcash.io`
- [ ] Resolution works (shows HandCash profile if available)
- [ ] Send works via P2P path

### 13. Identity Name Resolution (Sprint 3b.3-4)

#### 13.1 Identity Key Resolution
- [ ] Paste 66-char identity key (02.../03...) → dropdown appears with spinner
- [ ] If identity found: avatar + name + source (e.g., "X/Twitter via SocialCert") + checkmark
- [ ] If not found: "Identity key detected" (still sends via PeerPay)
- [ ] Debounce: no flicker during typing, resolves after 300ms pause

#### 13.2 Unified Detection
- [ ] BSV address (1... or 3...): no dropdown, standard send
- [ ] Identity key: PeerPay routing + identity resolution
- [ ] Paymail: paymail routing + name resolution
- [ ] $handle: paymail routing via handcash.io
- [ ] Invalid input: no resolution, validation error on submit

#### 13.3 Caching
- [ ] Same identity key resolves instantly on second attempt (frontend cache)
- [ ] Backend caches overlay results for 10 minutes

---

## Phase 4: Advanced Wallet Dashboard

### 14. Dashboard Layout (Sprint 4.1)

#### 14.1 Sidebar Navigation
- [ ] Wallet overlay opens with sidebar visible (5 tabs)
- [ ] Clicking each tab switches content area: Dashboard, Activity, Certificates, Approved Sites, Settings
- [ ] Active tab highlighted with gold accent
- [ ] Sidebar width consistent, no layout shift

#### 14.2 Dashboard Tab
- [ ] Balance displays correctly (BSV + USD)
- [ ] QR code generates for receive address (BIP21 format)
- [ ] Send form works (TransactionForm embedded)
- [ ] Recent activity shows 5 most recent transactions (sent + received)
- [ ] Recent activity shows USD values per row

### 15. Activity Tab (Sprint 4.2 + 4.6)

#### 15.1 Transaction List
- [ ] Shows BOTH sent AND received transactions
- [ ] Sorted newest-first
- [ ] Direction arrow: up for sent, down for received
- [ ] Description, BSV amount, human-readable date shown per row

#### 15.2 USD Display
- [ ] USD at transaction time shown (primary, right side)
- [ ] Current USD shown as secondary if different: `(now: $X.XX)`
- [ ] Old transactions (no historical price): show current price or `--`

#### 15.3 Pagination
- [ ] 10 items per page
- [ ] `[<] Page X of Y [>]` controls at bottom
- [ ] Prev button disabled on page 1
- [ ] Next button disabled on last page
- [ ] "Showing X-Y of Z" counter visible

#### 15.4 Filters
- [ ] All / Sent / Received filter buttons
- [ ] Changing filter resets to page 1
- [ ] Active filter highlighted
- [ ] Counts update correctly per filter

#### 15.5 TxID & External Links
- [ ] Copy icon: click copies full txid to clipboard
- [ ] Copy icon: tooltip shows truncated txid on hover
- [ ] Copy icon: checkmark shows for 2s after copy
- [ ] WhatsOnChain icon: opens `https://whatsonchain.com/tx/{txid}` in new browser tab

### 16. Certificates Tab (Sprint 4.3)
- [ ] BRC-52 certificates listed (if any exist)
- [ ] Certificate details expandable
- [ ] Empty state shown when no certificates

### 17. Approved Sites Tab (Sprint 4.3)
- [ ] Domain permissions list displayed
- [ ] Default limit controls visible (per-tx, per-session, rate limit)
- [ ] Changing defaults saves to backend
- [ ] "Reset All" button clears all permissions
- [ ] Embedded MUI DomainPermissionsTab renders correctly in dark theme

### 18. Settings Tab (Sprint 4.4)

#### 18.1 Display Name
- [ ] Shows current display name (default: "Anonymous")
- [ ] Editable text field, saves on blur or Enter
- [ ] Persists after closing/reopening wallet

#### 18.2 Mnemonic Reveal
- [ ] "Reveal Recovery Phrase" button visible
- [ ] Requires PIN entry before showing mnemonic
- [ ] Correct PIN: mnemonic displayed
- [ ] Wrong PIN: error message shown
- [ ] Mnemonic hidden again after closing section

#### 18.3 Export Backup
- [ ] "Export Backup" button visible
- [ ] Clicking downloads `.hodos` encrypted backup file

#### 18.4 Delete Wallet
- [ ] "Delete Wallet" button visible (red/destructive styling)
- [ ] 2-step confirmation: first click shows warning, second click confirms
- [ ] Refuses to delete if spendable balance > 0 satoshis
- [ ] Successful deletion: wallet cleared, returns to setup flow

### 19. Dark Theme & Layout
- [ ] All tabs render with dark background, light text
- [ ] No visual artifacts or unreadable text
- [ ] Embedded MUI components styled correctly (tables, inputs, switches)
- [ ] Layout responsive — no horizontal scroll, content fills available space

---

## Sign-Off

| Area | Tester | Pass/Fail | Notes |
|------|--------|-----------|-------|
| Button Polish | | | |
| QR Code | | | |
| Notification Badge | | | |
| Auto-Accept Setting | | | |
| Notification Banner | | | |
| PeerPay Send | | | |
| PeerPay Receive | | | |
| API Endpoints | | | |
| Edge Cases | | | |
| Visual/CSS | | | |
| Integration | | | |
| Standard Sites | | | |
| Paymail Send | | | |
| Identity Resolution | | | |
| Dashboard Layout | | | |
| Activity Tab | | | |
| Certificates Tab | | | |
| Approved Sites Tab | | | |
| Settings Tab | | | |
| Dark Theme | | | |
