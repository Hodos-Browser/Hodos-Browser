# Tedious BSV Testing Log (#49)

**Tester:** John
**Date:** 2026-03-27
**Build:** feature/83-payment-indicator-permissions (includes #85 + #83)
**Platform:** macOS
**Automated:** 73/73 Playwright tests passing (UI structure verified)

---

## Pre-Test Setup

- [x] Wallet backend running (port 31301)
- [x] Adblock engine running (port 31302)
- [x] Frontend dev server running (port 5137)
- [x] Wallet exists and unlocked
- [ ] BSV balance confirmed: ______ satoshis ($______)
- [x] BSV price feed working: 1 BSV = $13.39

---

## Wave 1: UI Smoke Test (~15 min)

> Automated tests already verified: settings sections render, menu has 11 items + zoom + shortcuts, downloads panel renders, privacy shield renders, new tab renders. Below is **CEF-only manual testing**.

### Settings Page
- [x] Settings opens from menu (three-dot → Settings)
- [x] Sidebar navigation switches between sections
- [x] Change a setting → close → reopen → persisted

**Issues found:**
```
None
```

### Menu (Three-Dot)
- [x] Menu opens on click
- [x] New Tab works
- [x] History opens browser-data page
- [x] Downloads panel opens (also Cmd+J overlay works)
- [ ] ❌ Zoom in/out/reset — menu closes on click (should stay open), percentage never updates
- [x] Find opens find bar
- [x] DevTools opens (F12)
- [x] Exit closes browser cleanly (no lingering audio) ← verifies #85

**Issues found:**
```
BUG: Zoom +/- closes menu instead of staying open. Zoom % never updates in menu display. Unclear if zoom actually applies to page.
```

### Find Bar
- [x] Cmd+F opens find bar
- [x] Typing highlights matches on page
- [x] "X of Y" count displays
- [x] Enter = next, Shift+Enter = prev, Escape = close
- [x] Red background on 0 matches

**Issues found:**
```
None
```

### Tab Management
- [x] Cmd+T creates new tab
- [x] Cmd+W closes active tab
- [x] Click to switch tabs
- [x] Drag to reorder tabs
- [x] Loading spinner on tab while page loads
- [x] Right-click → "Manage Site Permissions" appears ← NEW (#83)

**Issues found:**
```
None
```

---

## Wave 2: Wallet Read-Only (~15 min)

> Automated tests verified: balance area renders with USD/BSV, refresh button exists, identity key copy/show buttons exist, send/receive buttons exist, advanced wallet link exists, dashboard 4-quadrant layout, sidebar 5 tabs, tab switching.

### Wallet Panel (CEF-only)
- [ ] Wallet icon in toolbar opens panel
- [ ] Click outside closes panel
- [ ] USD + BSV balance display with real data
- [ ] Refresh button updates balance
- [ ] Receive → generates address + QR code
- [ ] Address copies to clipboard
- [ ] "Copy ID Key" copies to clipboard
- [ ] "Show ID Key" reveals key + QR

**Issues found:**
```

```

### Advanced Wallet (CEF-only)
- [ ] "Advanced" link opens dashboard in new tab
- [ ] Real balance data displays
- [ ] Receive QR codes are scannable
- [ ] "New Address" generates fresh address

**Issues found:**
```

```

---

## Wave 3: Wallet Transactions (~40 min)

> Requires BSV in wallet. Send some to the receive address first.

### Send — BSV Address (P2PKH)
- [ ] Enter valid address (starts with 1) — accepted
- [ ] Enter invalid address — error shown
- [ ] USD ↔ BSV auto-calculation works
- [ ] Max button fills balance minus fee
- [ ] Send succeeds — TxID displayed
- [ ] WhatsOnChain link opens in new tab
- [ ] Balance updates after send

Tx: `txid: __________________ amount: ______ to: __________________`

**Issues found:**
```

```

### Send — PeerPay (Identity Key)
- [ ] Enter identity key (02/03 + 64 hex) — recognized as PeerPay
- [ ] Send succeeds via BRC-29 MessageBox

Tx: `txid: __________________ amount: ______ to: __________________`

**Issues found:**
```

```

### Send — Paymail
- [ ] Enter paymail (user@domain or $handle) — resolves with avatar
- [ ] Send succeeds
- [ ] Invalid paymail shows error

Tx: `txid: __________________ amount: ______ to: __________________`

**Issues found:**
```

```

### Activity Tab
- [ ] Transaction list shows recent transactions
- [ ] Filter buttons work (All, Sent, Received)
- [ ] Copy txid works
- [ ] WhatsOnChain link opens correctly

**Issues found:**
```

```

### PeerPay Notifications
- [ ] Green dot on wallet icon when unread payments exist
- [ ] Notification banner shows count + amount
- [ ] "Dismiss" clears notification + green dot

**Issues found:**
```

```

---

## Wave 4: BRC-100 + Sites (~30 min)

### BRC-100 Auth Flow
- [ ] Navigate to a BRC-100 app
- [ ] Domain approval modal appears
- [ ] "Allow" grants access
- [ ] "Advanced settings" expands spending limits
- [ ] Allowed domain appears in Approved Sites tab

**Issues found:**
```

```

### Auto-Approve + Payment Badge
- [ ] Small payments auto-approve (no modal)
- [ ] Payment badge appears on tab ← NEW (#83)
- [ ] Over-limit payment shows confirmation modal

**Issues found:**
```

```

### Domain Permissions (Right-Click) ← NEW (#83)
- [ ] Right-click → "Manage Site Permissions" opens overlay
- [ ] Current settings pre-filled
- [ ] Edit + Save → persists on reopen
- [ ] "Revoke All Permissions" works
- [ ] Browser works after overlay closes

**Issues found:**
```

```

### Site Compatibility
- [ ] youtube.com — video plays, no ad leakage
- [ ] x.com — timeline loads
- [ ] github.com — repos load
- [ ] google.com — search works

**Issues found:**
```

```

---

## Summary

| Wave | Pass/Fail | Issues Found |
|------|-----------|-------------|
| 1: UI Smoke | | |
| 2: Wallet Read-Only | | |
| 3: Transactions | | |
| 4: BRC-100 + Sites | | |

**Blockers:**
```

```

**Non-blocking bugs:**
```

```
