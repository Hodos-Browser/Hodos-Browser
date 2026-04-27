# QR Code Scanning — Feature Overview

> Parent document for the QR scan sprint. Platform-specific implementation details in [QR_SCAN_WINDOWS.md](./QR_SCAN_WINDOWS.md) and [QR_SCAN_MACOS.md](./QR_SCAN_MACOS.md).

## Status

| Phase | Status | Notes |
|-------|--------|-------|
| **Phase 1: DOM scan** | **COMPLETE** | `<img>`, `<canvas>`, `<svg>` (async), `<video>` (CORS-limited) |
| **Phase 2 Windows: Screen capture** | **COMPLETE** | BitBlt + quirc. Auto-triggers when DOM scan returns 0 results |
| **Phase 2 macOS: Screen capture** | **COMPLETE** | CGWindowListCreateImage + CIDetector. See [QR_SCAN_MACOS.md](./QR_SCAN_MACOS.md) |
| **Phase 3: BIP21 generation** | Deferred | Our plain-address QR already works with HandCash/RockWallet |

## Goal

"Scan QR" button on the light wallet home screen. Scans the active browser tab for QR codes containing BSV payment data and populates the send form.

## User Flow (Phase 1 — Implemented)

```
[Scan QR] button on wallet home (2x2 grid: Receive Legacy | Receive BRC-100 | Send | Scan QR)
    |
    +-- DOM scan (async, typically < 500ms)
    |   +-- 1 BSV result  --> opens send form, auto-populates recipient (+ amount for BIP21)
    |   +-- N BSV results --> picker overlay with type/address/amount previews
    |   +-- 0 results     --> auto-triggers Phase 2 screen capture
    |
    +-- Phase 2: screen capture (auto-triggered when DOM scan finds 0 results)
        +-- C++ hides wallet overlay, shows full-screen selection overlay
        +-- User drags rectangle over QR code on screen
        +-- BitBlt (Win) / CGWindowListCreateImage (Mac) captures pixels
        +-- quirc (Win) / CIDetector (Mac) decodes QR
        +-- BSV pattern filter applied, wallet reopened with result
```

## Phase 1: DOM Scanning (Cross-Platform) — COMPLETE

JavaScript + C++ IPC. Identical on Windows and macOS.

### Architecture (Implemented)

```
Page Context (renderer process)         Wallet Overlay (WalletPanel.tsx)
+----------------------------------+    +-----------------------------+
| Injected QR scanner script       |    | Scan QR button              |
| - jsQR library (130KB minified) |    | - QR result picker          |
| - Scans <img>, <canvas>, <svg>  |    | - initialRecipient prop     |
| - Captures <video> current frame|    | - initialAmount prop        |
| - Filters by BSV patterns       |    | → opens TransactionForm     |
+----------------------------------+    +-----------------------------+
         |                                        ^
         | cefMessage.send('qr_found', [json])    |
         v                                        |
+----------------------------------+              |
| SimpleHandler (browser process)  |--------------+
| g_qr_scan_requester tracking     |  qr_scan_result via
| qr_scan_request → inject script  |  SendProcessMessage → postMessage
| qr_found → forward to requester  |
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

### Key Files (Phase 1 — Implemented)

| File | Purpose |
|------|---------|
| `cef-native/include/core/QRScannerScript.h` | Auto-generated C++ header: jsQR (minified) + DOM scanner + BSV filter. 10 string chunks for MSVC compatibility |
| `cef-native/build_tools/qr-scanner-logic.js` | Scanner IIFE source: async scan of img/canvas/svg/video, BSV pattern filter, BIP21 parser, IPC dispatch |
| `cef-native/build_tools/generate-qr-header.js` | Node.js script: bundles jsqr.min.js + scanner logic → QRScannerScript.h |
| `cef-native/build_tools/jsqr.min.js` | jsQR library minified via esbuild (130KB) |
| `cef-native/src/handlers/simple_handler.cpp` | `qr_scan_request` (inject script into active tab) + `qr_found` (forward results to requester) |
| `cef-native/src/handlers/simple_render_process_handler.cpp` | `qr_scan_result` → `window.dispatchEvent(MessageEvent)` to React |
| `frontend/src/components/WalletPanel.tsx` | Scan QR button, result listener, picker UI, passes `initialRecipient`/`initialAmount` to TransactionForm |
| `frontend/src/components/TransactionForm.tsx` | `initialRecipient`/`initialAmount` props for QR pre-fill. Exported regex constants |
| `frontend/src/utils/bip21.ts` | BIP21 `bitcoin:` URI parser |
| `frontend/src/components/TransactionComponents.css` | `.qr-scan-btn`, `.qr-picker`, `.qr-scan-message` styles |
| `frontend/public/qr-test.html` | Test page with 9 QR codes covering all BSV + non-BSV patterns |

### BIP21 URI Parsing — Implemented

Parser at `frontend/src/utils/bip21.ts`. Also duplicated inline in the injected scanner script (runs in page context, can't import from React).

**Note on BIP21 generation (Phase 3):** Our receive QR codes currently use plain addresses (`value={currentAddress}`). This already works with HandCash and RockWallet. Changing to `bitcoin:` prefix was tested and reverted — it may break compatibility with some wallets. Phase 3 deferred until we have confirmation that BIP21 format is universally supported by BSV wallets we care about.

### Dependencies

- `jsqr` npm package (130KB minified via esbuild, pure JS, no WASM) — bundled into C++ header as inline string
- `qrcode` npm devDependency — used to generate test QR images
- No native dependencies for Phase 1

### Known Limitations (Phase 1)

- **CORS-blocked images**: Cross-origin `<img>` elements throw `SecurityError` on `getImageData()`. Silently skipped. Phase 2 screen capture handles these.
- **Video frames (YouTube etc.)**: CORS prevents reading pixel data from cross-origin `<video>`. Silently skipped. Phase 2 handles this.
- **MSVC string literal limit**: jsQR + scanner (130KB) exceeds MSVC's 16380-char limit. Solved by splitting into 10 concatenated string chunks in QRScannerScript.h.
- **Fingerprint farbling**: Canvas `getImageData()` is slightly perturbed by our fingerprint protection script. jsQR's error correction handles this — QR codes are high-contrast black/white so LSB perturbation doesn't affect decoding.
- **50-element cap**: Scanner stops after 50 DOM elements to prevent hangs on image-heavy pages.
- **SVG 2s timeout**: Each SVG scan has a 2-second timeout for image load. Pages with many SVGs may not scan all of them.

## Phase 2: Screen Capture (Platform-Specific)

See platform docs:
- [QR_SCAN_WINDOWS.md](./QR_SCAN_WINDOWS.md) — **COMPLETE** — BitBlt + quirc
- [QR_SCAN_MACOS.md](./QR_SCAN_MACOS.md) — **NOT STARTED** — Core Graphics + CIDetector

### Shared Architecture (Phase 2 — Implemented on Windows)

```
Wallet Overlay                    C++ Browser Shell              QR Decoder
+------------------+             +----------------------+       +----------+
| "Scan QR" button |             | qr_found handler     |       |          |
| (DOM scan runs)  |             | detects empty "[]"   |       |          |
|                  |             | Auto-triggers:       |       |          |
|                  |             |  1. Hide wallet      |       |          |
|                  |             |  2. Show selection UI |       |          |
|                  |             |  3. User drags rect  |       | quirc    |
|                  |             |  4. BitBlt capture   |------>| (Win)    |
|                  |<-----------+  5. Show wallet      |       | CIDetect |
|  Form populated  |    IPC     |  6. Deliver result   |       | (Mac)    |
+------------------+            +----------------------+       +----------+
```

### Shared Flow (Implemented)

1. User clicks "Scan QR" → DOM scan runs first (~500ms)
2. If DOM scan returns results → Phase 1 handles it (auto-populate or picker)
3. If DOM scan returns `"[]"` → `qr_found` handler auto-triggers screen capture:
   a. Sends `qr_screen_capture_starting` IPC to wallet (React shows status briefly)
   b. Calls `HideWalletOverlay()` (keeps CEF alive via keep-alive pattern)
   c. Creates full-screen selection overlay (crosshair cursor, dark tint, gold border)
   d. User drags rectangle over QR code
   e. Captures pixels from screen (BitBlt on Windows, CGWindowListCreateImage on macOS)
   f. Decodes QR (quirc on Windows, CIDetector on macOS)
   g. Applies BSV pattern filter (same 4 regexes)
   h. Calls `ShowWalletOverlay()`
   i. Delivers result via `qr_screen_capture_result` IPC → React applies to form

### Key Files (Phase 2 — Windows, Implemented)

| File | Purpose |
|------|---------|
| `cef-native/include/core/QRScreenCapture.h` | Header: `StartQRScreenCapture()`, `FinishQRScreenCapture()` |
| `cef-native/src/core/QRScreenCapture.cpp` | Core: selection overlay, BitBlt, quirc decode, BSV filter, result delivery |
| `cef-native/third_party/quirc/` | Vendored quirc QR decoder (ISC license, 6 C files) |
| `cef-native/src/handlers/simple_handler.cpp` | `qr_found` handler modified to auto-trigger screen capture on empty results |
| `cef-native/src/handlers/simple_render_process_handler.cpp` | `qr_screen_capture_starting` + `qr_screen_capture_result` IPC forwarders |
| `frontend/src/components/WalletPanel.tsx` | Message listeners for screen capture starting/result events |

### BIP21 Extension (Implemented)

BIP21 `bitcoin:` URIs now accept any BSV recipient pattern in the address position:
- BSV address: `bitcoin:1A1z...?amount=0.001` (standard)
- Paymail: `bitcoin:user@domain?amount=0.005` (extended)
- Identity key: `bitcoin:02abc...?amount=0.01` (extended)
- $handle: `bitcoin:$testhandle?amount=0.002` (extended)

This allows paymails and identity keys to carry an amount via BIP21. `TransactionForm` detects the recipient type by regex regardless of how it arrived. No BRC standard exists for this yet — may propose one to the BSV community.

Test page: `http://127.0.0.1:5137/qr-test-bip21.html`

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

Test page: `http://127.0.0.1:5137/qr-test.html` — generated QR PNGs in `frontend/public/qr-images/`.
Generator script: `frontend/scripts/generate-qr-test-images.cjs` (uses `qrcode` npm package).

### Test Results (Phase 1)

| Test | Status | Notes |
|------|--------|-------|
| DOM scan — plain address | **PASS** | Auto-populates recipient |
| DOM scan — BIP21 with amount | **PASS** | Populates recipient + amount |
| DOM scan — BIP21 no amount | **PASS** | Populates recipient, amount blank |
| DOM scan — identity key (BRC-100) | **PASS** | Populates recipient, detected as BRC-100 |
| DOM scan — paymail | **PASS** | Populates recipient |
| DOM scan — HandCash handle | **PASS** | Populates recipient |
| DOM scan — non-BSV filtered | **PASS** | Website URL, SegWit, random text all ignored |
| DOM scan — multiple BSV | **PASS** | Picker shows 6 BSV results, each populates correctly on click |
| DOM scan — SVG QR (our wallet) | **PASS** | Async SVG load fix works — detects our identity key + receive address QR |
| DOM scan — whatsonchain.com | **PASS** | Auto-populates correct address from WoC address page QR |
| DOM scan — video frame QR | **KNOWN LIMITATION** | YouTube CORS blocks `getImageData()`. Phase 2 screen capture handles this |
| DOM scan — BIP21 with paymail | **PASS** | `bitcoin:user@handcash.io?amount=0.005` populates recipient + amount |
| DOM scan — BIP21 with identity key | **PASS** | `bitcoin:02abc...?amount=0.01` populates recipient + amount |
| DOM scan — BIP21 with $handle | **PASS** | `bitcoin:$testhandle?amount=0.002` populates recipient + amount |
| Pre-fill form opens from button | **PASS** | Scan QR button on wallet home opens send form with data |
| Screen capture — auto-trigger | **PASS** | DOM scan returns 0 → wallet hides → selection overlay appears |
| Screen capture — selection + decode | **PASS** | Drag rectangle over QR → decoded → wallet reopens with result |
| Screen capture — ESC cancel | **PASS** | Press ESC → overlay closes → wallet reopens, no result |
| Screen capture — no QR in region | **PASS** | Select area with no QR → "No QR found in selected area" message |
| Screen capture (Phase 2) | Not started | —
