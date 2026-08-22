# Phase 0.6 — QR scanner accepts `bsv:` payment URIs · PHASE CONTRACT

**Workstream:** WS6 · **Ticket:** `../TICKET_qr_bsv_uri_scheme_rejected.md` · **Status:** ⬜ NOT STARTED
**Opened:** 2026-08-18 · **Platforms:** both (4 sites: 2 C++, 1 injected JS, 1 React util)
**Standard:** `../HARNESS.md`.

---

## 1. Goal

A BSV payment QR that uses the `bsv:` URI scheme scans successfully by both routes — DOM and
drag-capture — on both platforms, and pre-fills the send form with the correct address and amount.

## 2. Done means

- [x] `bsv:` accepted at **all four** sites, with the scheme stripped by the first `:` and never by a
      fixed offset — #1/#2 C++, #3 regenerated header, #4 `bip21.ts`; verified by `T1e` (real #3/#4) +
      `T1c` (real #1) + real-binary DOM scan
- [x] The logged PaiyBit payload scans **end-to-end into a correct send form** (DOM path, real binary)
      — capture path's classifier proven by `T1c`; physical capture-drag not drivable here (see A1)
- [x] `bitcoin:` still works — measured, not assumed (A3, same run as `bsv:`)
- [x] A QR that decodes but classifies to nothing **says so** instead of reporting "no QR found" (A4)
- [x] `Q4` (amount units) confirmed — whole-coin BSV, verified in the send field + code path (A5)
- [x] `Q1` — **SETTLED (case a), empirically.** The owner re-tested the original failing PaiyBit page
      after the fix and the **DOM path worked** — which means the scanner *had* found and decoded the QR
      element all along and was only rejecting the `bsv:` scheme (case a), not failing to find it (case
      b). No instrumentation needed; the real-page pass is the evidence. `Q3` (shadow-DOM / iframe /
      CSS-bg coverage for the DOM path) is therefore **not** triggered by PaiyBit and stays out of scope.
- [x] `QR_SCAN_OVERVIEW.md`'s pattern table updated — now lists `bitcoin:` **and** `bsv:`

## 3. The four sites — all must change together

| # | Site | Scheme test | Strips by | Risk |
|---|---|---|---|---|
| 1 | `cef-native/src/core/QRScreenCapture.cpp:51` | `^bitcoin:` | `text.find(':')` | ✅ safe |
| 2 | `cef-native/cef_browser_shell_mac.mm:3028` | `^bitcoin:` | `text.find(':')` | ✅ safe |
| 3 | `cef-native/build_tools/qr-scanner-logic.js:22` | `^bitcoin:` | ⛔ `uri.slice(8)` | 🚨 truncates |
| 4 | `frontend/src/utils/bip21.ts:9` | `startsWith('bitcoin:')` | ⛔ `uri.slice(8)` | 🚨 truncates |

⛔ **The trap.** `8` is the byte length of `"bitcoin:"`. Widen the regex in #3/#4 without fixing the
strip and `bsv:16cezrim…` becomes `"zrim1PR2…"` — which fails the address regex and **fails closed**.
The user sees the identical symptom to today while the diff looks like a completed fix.

⚠️ #3 is **generated** — edit `qr-scanner-logic.js` and regenerate `QRScannerScript.h` via
`build_tools/generate-qr-header.js`. Editing the header directly is silently reverted by the next
regeneration.

⭐ **Reuse note, not a mandate:** these four are one rule with four spellings. `qr-scanner-logic.js`
genuinely cannot import from React, but #1/#2 are a copy-paste pair and #3/#4 duplicate each other.
Deduplicating is out of scope here — but do not add a **fifth** spelling.

## 4. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-INTEXT` | Internal never prompts, external always gates | Untouched — the scan is initiated by the wallet UI, not a page. Recorded so the boundary run is not skipped |
| `R-GOLD` | Gold pill fires | A scanned send is still a send; the indicator must fire on it |
| — | **Scheme allowlist stays an allowlist** | ⛔ Do **not** widen to "any scheme". The address regex is the real guard, but the scheme signals intent-to-pay, and this is the money path |

## 5. Evidence table

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P0.6-A1` | Capture path: the logged payload `bsv:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?amount=0.11828417&label=PaiyBit%20media` scans and populates the send form | **Already evidenced RED in production** — `ClassifyAndBuildJson` returns `""` today. Re-observe on the pre-fix binary before changing it | The **address that reaches the send form**, char-for-char. "The scanner returned something" would pass with a truncated address | T2 | 🟡 **Capture CLASSIFIER GREEN** via `T1c` unit test `QrClassify.BsvSchemeAcceptedFullAddressAndAmount` (real compiled `ClassifyQRPayload`, full address, NC seen). The physical drag-capture chain could **not** be driven end-to-end in this environment — injected mouse-button events are filtered before reaching the overlay WndProc (5 `SendInput` events, overlay unmoved; MOVE works, DOWN/UP dropped). The classifier→delivery→form chain is code-read + unit-tested; the end-to-end *into the send form* is GREEN via the DOM path (A2). **Physical capture-drag verification owed on a machine where input injection works (or by the owner).** |
| `P0.6-A2` | DOM path: same payload scans from the live page | Pre-fix must return `[]` | Rust/browser log shows the payload was **decoded** — distinguishes reject-scheme from never-found-element | T2 | 🟢 **GREEN** (real binary). `bsv:` QR in the page → send form recipient `16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ`, amount `0.11828417` BSV. Pre-fix RED captured on the real source via `T1e` (`bsv` → null). Log shows the payload decoded (`QR scan results received: [{"type":"bip21"…`). |
| `P0.6-A3` | `bitcoin:` payload still scans | Revert the widening → the `bsv:` case fails **and** `bitcoin:` still passes, proving the rule was widened, not swapped | Both schemes in one run | T2 | 🟢 **GREEN** (real binary). Same run: `bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=0.001` → recipient + `0.001` BSV. Widened, not swapped. |
| `P0.6-A4` | `https://example.com` QR is **rejected**, with a message naming the scheme | Remove the message → it silently reports "no QR found" again | The user-visible string, not a log line | T2 | 🟢 **GREEN**. React half (real overlay): renders `Found a QR code, but it isn't a BSV payment (starts with "https:")`; NC — a `not_found` result shows the old "No QR found" and **not** the new message. C++ half: `T1c` `QrClassify.ForeignSchemeRejectedButNamed` (`SchemeForMessage("https://…")=="https"`, capped ≤12). |
| `P0.6-A5` | 🚨 Amount units correct: `amount=0.11828417` reaches the form as **0.11828417 BSV**, not 0.11828417 satoshis | Feed the same payload with the units mapping inverted → the form shows a value 10⁸ out | The number in the send field, checked against the invoice | T2 | 🟢 **GREEN** (real binary). Send form shows `0.11828417` in the BSV field and `$2.18`/`$2.19` in USD (= 0.11828417 × ~$18.4) — proving whole-coin BSV, not satoshis. Confirmed by code path: `parsed.amount.toFixed(8)` → `initialAmount` → `formData.amount` × 1e8 only at submit. |
| `P0.6-A6` | The `slice(8)` trap is closed: a 4-char scheme yields the full address | Set the strip back to `slice(8)` → address truncates to `zrim1PR2…` and the scan fails **closed** | Assert the extracted address string, not scan success/failure | T1 | 🟢 **GREEN** via `T1e` (real `bip21.ts` + `qr-scanner-logic.js`: `bsv` → full `16cezrim…`) and `T1c` `QrClassify` (full address field). NC seen both ways: `#4` truncates to `zrim1PR2DGuFivZr8kWUSan1LD6XFZ`, `#3` fails **closed** to null. |
| `P0.6-A7` | macOS: same payload scans via CIDetector | Pre-fix must fail on macOS too | ⛔ macOS decodes with **CIDetector**, not quirc — **a Windows green does not imply a macOS green** | T2 | 🔁 **RELAYED to Mac Claude.** Source #2 (`cef_browser_shell_mac.mm`) fixed identically (scheme widen + amount validation). The macOS SCAN verification (CIDetector) is a Mac's to run. |

**Plus — the amount-injection finding (in scope per owner, option (a)):** measured RED on the real wallet-overlay V8 (`MEASUREMENT_amount_injection.md`): `amount=(window.__hodos_qr_probe=typeof fetch,0)` executed as JS in the privileged overlay. Fixed by validating `amount` is a plain decimal before emitting it (both C++ classifiers). GREEN + NC via `T1c` `QrClassify.InjectionAmountIsDroppedNotEmitted` / `NonNumericAmountDropped` / `AmountValidationBoundaries` (guard removed → all 3 fail; restored → pass). Also fixes the silent-failure on any non-numeric amount.

**Pairing:** `A1`/`A3` are two-sided — `bsv:` must start working **and** `bitcoin:` must keep working.
`A6` exists because `A1` alone can be satisfied by a truncated address in some orderings.

## 6. Blast radius

- Four scheme sites above, plus `QRScannerScript.h` regeneration.
- `TransactionForm.tsx` — `initialRecipient` / `initialAmount` are the landing point for `A5`.
- `frontend/src/utils/bip21.ts` is used by more than the scanner — check its other callers before
  changing its contract.
- ⚠️ Nothing here touches the permission engine or the wallet backend. If a diff in this phase
  reaches either, the scope is wrong.

## 7. Out of scope

- `paymail:` / `handcash:` schemes — `Q2`, a product call.
- Deduplicating the four spellings into one source — worth doing, not here.
- Shadow-DOM / cross-origin-iframe / CSS-background coverage in the DOM scanner — falls out of `Q1`;
  file separately if `Q1` shows the element was never found.
- BIP21 **generation** for our own receive QRs — deliberately deferred since Phase 3 of the QR sprint;
  this phase only changes what we *accept*.

## 8. Rollback

Revert the four one-line scheme changes plus the regenerated header. No schema, no persisted state,
no protocol change — the phase only widens an input filter.

---

## Sign-off

- [~] Every evidence row GREEN **and** its RED observed — A2/A3/A4/A5/A6 GREEN+RED; A1 capture-*classifier*
      GREEN+NC (physical drag not drivable here); A7 relayed to Mac
- [x] `A5` (amount units) confirmed in the send field (0.11828417 BSV / ~$2.18) + code path — money path
- [ ] `A7` verified **on a Mac** by someone with a Mac — **relayed to Mac Claude**
- [x] `scripts/preflight.ps1` + `-NegativeControl` recorded (T1e wired into both; both PASS)
- [ ] `../REGRESSION_SET.md` run at the 0.6 → 1 boundary — owed at the boundary
- [ ] Adversarial review — four questions answered in writing — owed
- [x] `QR_SCAN_OVERVIEW.md` pattern table updated — now `bitcoin:` + `bsv:`
- [ ] Commits cite row IDs — at commit time

| Item | Result | Date | By |
|---|---|---|---|
| preflight (`-Full`) | ✅ PASS (G1–G5, T1a–T1e) | 2026-08-21 | Windows |
| preflight -NegativeControl | ✅ PASS (5 gates + T1e trap seen to fail) | 2026-08-21 | Windows |
| regression set (0.6 → 1) | ⬜ owed at boundary | | |
| adversarial review | ⬜ owed | | |
| macOS verification (`A7`) | 🔁 relayed to Mac Claude | 2026-08-21 | Windows→Mac |
