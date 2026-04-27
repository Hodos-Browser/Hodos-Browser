# QR Code Scanning — Feature Overview

> Parent document for the QR scan sprint. Platform-specific implementation details in [QR_SCAN_WINDOWS.md](./QR_SCAN_WINDOWS.md) and [QR_SCAN_MACOS.md](./QR_SCAN_MACOS.md).

## Goal

Add a "Scan QR" button to the light wallet send form. One button, two modes:

1. **DOM scan (Phase 1)** — instant, invisible, cross-platform JavaScript
2. **Screen capture (Phase 2)** — manual region selection, platform-native C++

The user never chooses between modes. The system tries DOM first, falls through to screen capture if nothing is found.

## User Flow

```
[Scan QR] button in light wallet send form
    |
    +-- DOM scan (instant, < 100ms, invisible to user)
    |   +-- 1 BSV result  --> auto-populate form + toast "Found QR on page"
    |   +-- N BSV results --> picker: "Which one?" with address/amount previews
    |   +-- 0 results     --> fall through to screen capture
    |
    +-- Screen capture mode (Phase 2)
        +-- Brief instruction overlay: "Drag over a QR code"
        +-- Wallet overlay closes, crosshair/selection UI appears
        +-- User drags rectangle over QR code
        +-- Decode captured region
        +-- Wallet reopens with form populated
        +-- If decode fails --> "No QR found in selection, try again"
```

## Phase 1: DOM Scanning (Cross-Platform)

All work is JavaScript + minimal C++ IPC wiring. Identical on Windows and macOS.

### Architecture

```
Page Context (renderer process)         Wallet Overlay (separate process)
+----------------------------------+    +-----------------------------+
| Injected QR scanner script       |    | TransactionForm.tsx         |
| - jsQR library (~50KB)          |    | - initialRecipient prop     |
| - Scans <img>, <canvas>, <svg>  |    | - initialAmount prop        |
| - Captures <video> current frame|    |                             |
| - Filters by BSV patterns       |    |                             |
+----------------------------------+    +-----------------------------+
         |                                        ^
         | cefMessage.send('qr_found', data)      |
         v                                        |
+----------------------------------+              |
| SimpleHandler (browser process)  |--------------+
| OnProcessMessageReceived()       |  Forward to wallet overlay
| Route qr_found --> wallet        |  via IPC or URL params
+----------------------------------+
```

### QR Content Detection & Filtering

Decoded QR content is matched against BSV payment patterns:

| Pattern | Type | Extract |
|---------|------|---------|
| `bitcoin:{address}?amount={n}&label={s}` | BIP21 URI | address, amount, label |
| `^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$` | BSV address | address only |
| `^(02\|03)[0-9a-fA-F]{64}$` | Identity key (PeerPay) | pubkey |
| `user@domain` or `$handle` | Paymail | paymail address |
| Anything else | Non-BSV | Ignored |

These regexes already exist in `TransactionForm.tsx` (lines 7-11). Reuse them.

### DOM Elements to Scan

1. **`<img>` elements** — draw to canvas, getImageData, decode
2. **`<canvas>` elements** — getImageData directly
3. **`<svg>` elements** — serialize to blob, render to canvas, decode
4. **`<video>` elements** — drawImage of current frame to canvas, decode (catches paused video QR codes)

### CORS Handling

Cross-origin images will throw `SecurityError` on `getImageData()`. Workarounds:

- **Try-catch each image** — skip CORS-blocked ones silently
- **For blocked images**: send URL to browser process via IPC, fetch via C++ HTTP client (no CORS restrictions), return pixel data
- **Fallback**: if DOM scan finds nothing (possibly due to CORS), screen capture handles it

### Edge Case: QR in DOM + QR in Video

If a page has a QR code as an `<img>` AND one playing in a `<video>`:
- DOM scan captures both (video frame snapshot at scan time)
- If both decode to BSV patterns, show the picker with source labels: "Image on page" vs "Video frame"
- If the video QR is not on the current frame (already passed), DOM scan won't find it — user gets screen capture fallback where they can pause and select

### Key Files to Modify (Phase 1)

| File | Change |
|------|--------|
| `frontend/src/components/TransactionForm.tsx` | Add `initialRecipient`, `initialAmount` props; add "Scan QR" button |
| `frontend/src/pages/WalletPanelPage.tsx` | Pass QR data to TransactionForm; handle `qr_scan_result` IPC |
| `cef-native/src/handlers/simple_handler.cpp` | Handle `qr_scan_request` IPC → send scan command to active browser; handle `qr_found` IPC → forward to wallet overlay |
| `cef-native/src/handlers/simple_render_process_handler.cpp` | Handle `qr_scan_execute` IPC → inject/trigger scanner script in page context |
| New: `frontend/src/utils/qr-scanner.js` or embedded in C++ | jsQR library + DOM scanning logic, injected into pages |

### BIP21 URI Parsing

Need to add a parser for `bitcoin:` URIs. Simple function:

```typescript
function parseBIP21(uri: string): { address: string; amount?: number; label?: string } | null {
  if (!uri.startsWith('bitcoin:')) return null;
  const [address, queryString] = uri.slice(8).split('?');
  const params = new URLSearchParams(queryString || '');
  return {
    address,
    amount: params.has('amount') ? parseFloat(params.get('amount')!) : undefined,
    label: params.get('label') || undefined,
  };
}
```

Also update our own QR code generation (DashboardTab.tsx) to emit BIP21 URIs instead of plain addresses, so other wallets can scan ours.

### Dependencies

- `jsQR` npm package (~50KB, pure JS, no WASM) — or bundle as inline script for injection
- No native dependencies for Phase 1

## Phase 2: Screen Capture (Platform-Specific)

See platform docs:
- [QR_SCAN_WINDOWS.md](./QR_SCAN_WINDOWS.md) — WinAPI region selection + BitBlt capture
- [QR_SCAN_MACOS.md](./QR_SCAN_MACOS.md) — Core Graphics capture + NSView region selection

### Shared Architecture (Phase 2)

```
Wallet Overlay                    C++ Browser Shell              QR Decoder
+------------------+    IPC      +----------------------+       +----------+
| "Scan QR" button |----------->| Close wallet overlay |       |          |
| (DOM scan: 0     |            | Show selection UI    |       |          |
|  results)        |            | Capture region pixels|------>| jsQR or  |
|                  |<-----------| Reopen wallet with   |       | zxing    |
|  Form populated  |    IPC     | decoded QR data      |       |          |
+------------------+            +----------------------+       +----------+
```

### Shared Flow

1. Wallet sends `qr_screen_capture_start` IPC
2. C++ closes wallet overlay (with prevent-close flag to remember state)
3. C++ shows platform-specific selection UI (full-screen transparent overlay with crosshair)
4. User drags rectangle
5. C++ captures pixels from screen within rectangle (platform API)
6. C++ passes pixel buffer to QR decoder (could be JS via a hidden browser, or Rust/C++ library)
7. C++ reopens wallet overlay with decoded data as URL params
8. Wallet form populates from URL params

## Phase 3: BIP21 Generation (Our QR Codes)

Update `DashboardTab.tsx` QR generation to use BIP21 format:
- Current: `value={currentAddress}` (plain address)
- Updated: `value={\`bitcoin:${currentAddress}\`}` (BIP21 URI)
- Optional: include amount if user specifies one

This makes our QR codes scannable by other BSV wallets that support BIP21.

## Sprint Order

1. **Phase 1**: DOM scan + UI + BIP21 parsing (cross-platform, ~2-3 days)
2. **Phase 2 Windows**: Screen capture fallback (~1-2 days)
3. **Phase 2 macOS**: Screen capture fallback (~1-2 days)
4. **Phase 3**: BIP21 generation upgrade (< 1 day)

## Testing

### Localhost Test Pages

Create a `frontend/public/qr-test.html` page (served by Vite dev server at `http://127.0.0.1:5137/qr-test.html`) with QR codes for every format. Generate QR images using any online generator or the `qrcode` npm package.

**Test QR codes to generate:**

| # | QR Content | Type | Expected Result |
|---|-----------|------|-----------------|
| 1 | `1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa` | Plain BSV address | Populate recipient |
| 2 | `bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=0.001&label=Test` | BIP21 URI | Populate recipient + amount |
| 3 | `bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa` | BIP21 no amount | Populate recipient only |
| 4 | `02abc...(real 66-char pubkey)` | Identity key | Populate recipient as PeerPay |
| 5 | `user@handcash.io` | Paymail | Populate recipient, trigger paymail resolution |
| 6 | `$testhandle` | HandCash handle | Populate recipient, resolve via paymail |
| 7 | `https://www.google.com` | Website URL | Ignored (not BSV) |
| 8 | `bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4` | SegWit address | Ignored (not BSV) |
| 9 | `Hello World` | Random text | Ignored |

Use the wallet's own identity key (from `/getPublicKey`) for test #4.

**Test page structure:**
```html
<!-- frontend/public/qr-test.html -->
<h2>QR Scan Test Page</h2>

<!-- Single QR tests -->
<h3>Test 1: Plain BSV Address</h3>
<img src="qr-bsv-address.png" />

<h3>Test 2: BIP21 with Amount</h3>
<img src="qr-bip21-amount.png" />

<!-- Multiple QR test (should show picker) -->
<h3>Test: Multiple BSV QR Codes</h3>
<img src="qr-bsv-address.png" />
<img src="qr-identity-key.png" />

<!-- Non-BSV (should be ignored) -->
<h3>Non-BSV QR (should be filtered out)</h3>
<img src="qr-website-url.png" />
<img src="qr-segwit.png" />
```

Generate the QR PNG files with a script or use an online tool. Place them in `frontend/public/` so Vite serves them.

### Test Matrix

| Test | Method |
|------|--------|
| DOM scan — plain address | Test page #1, single QR, auto-populate |
| DOM scan — BIP21 with amount | Test page #2, verify both recipient and amount fill |
| DOM scan — BIP21 no amount | Test page #3, recipient fills, amount blank |
| DOM scan — identity key | Test page #4, recipient fills, detected as PeerPay |
| DOM scan — paymail | Test page #5, recipient fills, paymail resolution triggers |
| DOM scan — HandCash handle | Test page #6, `$handle` resolves via paymail client |
| DOM scan — non-BSV filtered | Test page #7-9 only on page, should get "no payment QR found" |
| DOM scan — mixed BSV + non-BSV | Page with #1 + #7, should find only #1 |
| DOM scan — multiple BSV | Page with #1 + #4, should show picker |
| DOM scan — real site | whatsonchain.com address page (has QR code) |
| DOM scan — CORS blocked image | Cross-origin QR image, should skip gracefully |
| DOM scan — video frame QR | YouTube video paused on QR frame |
| Pre-fill form | Verify recipient + amount populate and validation runs |
| Screen capture — basic (Phase 2) | QR code in a PDF or other app window |
