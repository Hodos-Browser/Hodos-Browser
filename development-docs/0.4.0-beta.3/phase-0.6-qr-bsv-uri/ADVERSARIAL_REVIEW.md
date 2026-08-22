# Phase 0.6 — adversarial review (HARNESS.md §6, four questions)

A pass whose job is to refute the Phase 0.6 evidence. Written 2026-08-21, Windows.

## 1. Can I make each test pass with the feature removed?

- **T1e (scheme, sites #3/#4):** No. Run against the pre-fix sources it went **RED** (`bsv` → null,
  captured before any edit). `--negative-control` reverts the strip to `slice(8)` and the GREEN
  assertion goes RED (`#4` truncates to `zrim1PR2…`, `#3` fails closed to null). Wired into
  `preflight -NegativeControl`, which PASSES only because T1e is *seen* to fail there.
- **T1c `QrClassify` (site #1 + injection):** No. Removing the amount-validation guard from the header
  and rebuilding made `InjectionAmountIsDroppedNotEmitted`, `NonNumericAmountDropped` and
  `AmountValidationBoundaries` all **FAIL** (measured — guard out → 3 fail, guard in → pass). The
  scheme rows go RED if `RE_BIP21` is narrowed back.
- **A2/A3/A5 (DOM path):** The RED half is the real pre-fix source rejecting `bsv:` (`T1e` on-disk
  before edit). A test that only asserted "scanner returned something" would be void — so the
  assertions are on the **address string** (`16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ`, char-for-char) and
  the **amount string** in the send field, not on scan success.
- **A4:** Removing the message arm makes the overlay show "No QR found" again — the NC is exactly that
  (a `not_found` result shows the old string and **not** the new one, measured).

## 2. What is the subject? (proven, not asserted)

- **DOM path (A2/A3/A5):** the real wallet-panel overlay V8, `http://127.0.0.1:5137/wallet-panel?iro=50`
  (proved by `location.href` in every probe, via `cdp.py` which refuses on ambiguous URL match), driven
  by the real `HodosBrowser.exe` under `build/bin/Release` (dev, CDP 9322 — **not** production's 9222).
  The scanned bytes are the real compiled `QRScannerScript.h` (log: `QR scan results received:
  [{"type":"bip21"…`).
- **Injection RED (MEASUREMENT):** the same real overlay V8 executed an attacker `amount`
  (`window.__hodos_qr_probe` set to `typeof fetch` == `"function"`). Not a model — the actual V8.
- **T1c:** the real compiled `ClassifyQRPayload` from `QRPayloadClassify.h`, the exact code
  `QRScreenCapture.cpp` calls after quirc.
- ⚠️ **Subject gap, stated:** the capture path's *physical* decode chain (real QR on screen → drag →
  quirc → classifier → renderer) was **not** driven — injected mouse-button events are filtered before
  reaching the overlay WndProc in this environment (MOVE injects and moves the cursor; DOWN/UP are
  dropped; 5 `SendInput` events left the overlay unmoved). The capture *classifier* is the real
  compiled function under `T1c`; the *transport around it* is code-read.

## 3. What would I expect to see if this were broken — and did anyone look?

- **If the scheme fix truncated (the `slice(8)` trap):** the send-form recipient would read
  `zrim1PR2…` and fail the address regex. Looked for: A6 asserts the **full** address and the NC shows
  the truncation explicitly.
- **If units were inverted (satoshis vs BSV):** the USD field would be 10⁸ out. Looked for: A5 checks
  `0.11828417` BSV against `$2.18` (= ×~$18.4), which is only right if it's whole-coin BSV.
- **If the injection fix were cosmetic (e.g. escaped but still emitted):** the `amount` field would
  still be present. Looked for: `T1c` asserts the `"amount":` field is **absent** for non-numeric
  input, and that the raw expression survives only inside the quoted/escaped `value` echo (inert).
- **If A4 conflated "decoded-unknown" with "nothing found":** both would say "No QR found". Looked
  for: A4 asserts the distinct strings for each and the NC proves `not_found` ≠ `unrecognized`.

## 4. Measurement or code reading? (labelled)

| Claim | Kind |
|---|---|
| DOM path `bsv:`/`bitcoin:` → send form (A2/A3/A5/A6) | **Measurement** (real overlay V8) |
| Injection executes pre-fix (RED) | **Measurement** (real overlay V8) |
| A4 message renders + NC | **Measurement** (real overlay V8) |
| Capture classifier drops injection / widens scheme (A1 classifier, injection GREEN) | **Measurement** (real compiled `ClassifyQRPayload` via `T1c`) |
| Scheme fix on real #3/#4 (T1e) | **Measurement** (real sources) |
| Capture path's quirc→classifier→renderer *transport* end-to-end | **Code reading** (physical drag not drivable here; owed at T2 / owner / a Mac for A7) |
| Amount unit chain `parsed.amount → formData.amount → ×1e8 at submit` | **Code reading** (corroborated by the A5 measurement) |

## Residuals (honest)

- **A1 physical capture drag** and **A7 macOS CIDetector scan** are the two things not measured on a
  real end-to-end path here. A1's classifier is unit-tested; A7's source is fixed identically and
  relayed to Mac Claude.
- **Q1** (did PaiyBit's DOM scan reject the scheme or never find the element?) is now **SETTLED —
  case (a)**, by the owner re-testing the real page: the DOM path scans it successfully after the fix,
  so the element was always found and only the scheme was rejected. `Q3` (shadow-DOM/iframe/CSS-bg
  coverage) is not triggered by PaiyBit. (Had it been case (b), the screen-capture fallback would still
  cover it — the payment path was never the exposure.)
