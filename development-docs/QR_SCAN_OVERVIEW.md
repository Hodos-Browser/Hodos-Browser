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

| Test | Method |
|------|--------|
| DOM scan — single QR on page | Navigate to whatsonchain.com address page (has QR), press Scan QR |
| DOM scan — multiple QR codes | Create test page with 2 QR codes |
| DOM scan — non-BSV QR | Page with URL QR code — should be ignored |
| DOM scan — CORS blocked image | Cross-origin QR image — should fall through gracefully |
| DOM scan — video frame QR | YouTube video paused on QR frame |
| Screen capture — basic | QR code in a PDF or other app window |
| BIP21 parsing | `bitcoin:1ABC...?amount=0.001&label=test` |
| Pre-fill form | Verify recipient + amount populate correctly |
