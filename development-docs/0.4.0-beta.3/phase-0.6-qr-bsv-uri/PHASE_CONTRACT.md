# Phase 0.6 — QR scanner accepts `bsv:` payment URIs · PHASE CONTRACT

**Workstream:** WS6 · **Ticket:** `../TICKET_qr_bsv_uri_scheme_rejected.md` · **Status:** ⬜ NOT STARTED
**Opened:** 2026-08-18 · **Platforms:** both (4 sites: 2 C++, 1 injected JS, 1 React util)
**Standard:** `../HARNESS.md`.

---

## 1. Goal

A BSV payment QR that uses the `bsv:` URI scheme scans successfully by both routes — DOM and
drag-capture — on both platforms, and pre-fills the send form with the correct address and amount.

## 2. Done means

- [ ] `bsv:` accepted at **all four** sites, with the scheme stripped by the first `:` and never by a
      fixed offset
- [ ] The three payloads from the owner's production log scan **end-to-end into a correct send form**
- [ ] `bitcoin:` still works — measured, not assumed
- [ ] A QR that decodes but classifies to nothing **says so** instead of reporting "no QR found"
- [ ] `Q4` (amount units) confirmed — this is the money path
- [ ] `Q1` settled, so the DOM-path story is proven rather than assumed
- [ ] `QR_SCAN_OVERVIEW.md`'s pattern table updated — it lists only `bitcoin:`

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
| `P0.6-A1` | Capture path: the logged payload `bsv:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?amount=0.11828417&label=PaiyBit%20media` scans and populates the send form | **Already evidenced RED in production** — `ClassifyAndBuildJson` returns `""` today. Re-observe on the pre-fix binary before changing it | The **address that reaches the send form**, char-for-char. "The scanner returned something" would pass with a truncated address | T2 | ⬜ |
| `P0.6-A2` | DOM path: same payload scans from the live page | Pre-fix must return `[]` | Rust/browser log shows the payload was **decoded** — distinguishes reject-scheme from never-found-element | T2 | ⬜ |
| `P0.6-A3` | `bitcoin:` payload still scans | Revert the widening → the `bsv:` case fails **and** `bitcoin:` still passes, proving the rule was widened, not swapped | Both schemes in one run | T2 | ⬜ |
| `P0.6-A4` | `https://example.com` QR is **rejected**, with a message naming the scheme | Remove the message → it silently reports "no QR found" again | The user-visible string, not a log line | T2 | ⬜ |
| `P0.6-A5` | 🚨 Amount units correct: `amount=0.11828417` reaches the form as **0.11828417 BSV**, not 0.11828417 satoshis | Feed the same payload with the units mapping inverted → the form shows a value 10⁸ out | The number in the send field, checked against the invoice | T2 | ⬜ |
| `P0.6-A6` | The `slice(8)` trap is closed: a 4-char scheme yields the full address | Set the strip back to `slice(8)` → address truncates to `zrim1PR2…` and the scan fails **closed** | Assert the extracted address string, not scan success/failure | T1 | ⬜ |
| `P0.6-A7` | macOS: same payload scans via CIDetector | Pre-fix must fail on macOS too | ⛔ macOS decodes with **CIDetector**, not quirc — **a Windows green does not imply a macOS green** | T2 | ⬜ |

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

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `A5` (amount units) confirmed against a real invoice — money path, no assumption
- [ ] `A7` verified **on a Mac** by someone with a Mac
- [ ] `scripts/preflight.ps1` + `-NegativeControl` recorded
- [ ] `../REGRESSION_SET.md` run at the 0.6 → 1 boundary
- [ ] Adversarial review — four questions answered in writing
- [ ] `QR_SCAN_OVERVIEW.md` pattern table updated
- [ ] Commits cite row IDs

| Item | Result | Date | By |
|---|---|---|---|
| preflight | | | |
| preflight -NegativeControl | | | |
| regression set (0.6 → 1) | | | |
| adversarial review | | | |
| macOS verification (`A7`) | | | |
