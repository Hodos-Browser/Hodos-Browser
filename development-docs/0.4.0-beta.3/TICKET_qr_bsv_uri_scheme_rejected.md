# TICKET — the QR scanner reads `bsv:` URIs perfectly and then throws them away

**Filed:** 2026-08-18, from an owner report against a live payment page
**Severity:** user-facing — a real BSV payment QR cannot be scanned, on both scan paths
**Status:** OPEN — beta.3, **WS6 / Phase 0.6**. Cheap, evidence-complete, no unknowns.
**Verdict on the owner's question:** ⭐ **Fix ours. Do not ask PaiyBit to change anything.**

---

## 1. The report

> "I tried to scan the QR code on `https://paiybit.com/paiybit/6a8446e078769996f3bbc7b4` — it didn't
> pick it up from the DOM so I used the drag-over capture method and it did not see it either."

## 2. What actually happened — measured, not inferred

The decoder read the code **perfectly, three times**. From the owner's own production log
(`%APPDATA%\HodosBrowser\logs\debug_output.log`):

```
[2026-08-18 10:07:41.329] [BROWSER] [INFO] quirc found 1 QR code(s) in selection
[2026-08-18 10:07:41.329] [BROWSER] [INFO] QR payload: bsv:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?amount=0.11828417&label=PaiyBit%20media
[2026-08-18 10:07:51.778] [BROWSER] [INFO] quirc found 1 QR code(s) in selection
[2026-08-18 10:07:51.778] [BROWSER] [INFO] QR payload: bsv:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?amount=0.11828417&label=PaiyBit%20media
[2026-08-18 13:28:03.545] [BROWSER] [INFO] quirc found 1 QR code(s) in selection
[2026-08-18 13:28:03.545] [BROWSER] [INFO] QR payload: bsv:1MMFSGAPykQWanbqVkhkQPrAbK8kBDjz8B?amount=0.11828417&label=PaiyBit%20media
```

**The scanner is not broken.** quirc decoded a clean, well-formed, standards-shaped payment URI on the
first attempt each time. What rejected it was our own classifier, one character at a time:

```cpp
static const std::regex RE_BIP21(R"(^bitcoin:)", std::regex_constants::icase);
```

`bsv:16cez…` does not start with `bitcoin:`. `ClassifyAndBuildJson` therefore returns `""`, the caller
reads that as "no BSV QR here", and the user is told nothing was found.

⭐ Note what else the payload proves: the address `16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ` matches our
`RE_BSV_ADDRESS` exactly, and `amount` / `label` are standard BIP21 parameters. **Everything about
this URI is acceptable to us except the four characters of its scheme.**

## 3. Why the DOM path failed too — same cause, one filter

The DOM scan ran and returned an empty array:

```
[2026-08-18 10:07:46.156] [BROWSER] [INFO] 📷 Injecting QR scanner into active tab: https://paiybit.com/paiybit/…
[2026-08-18 10:07:46.252] [BROWSER] [INFO] 📷 QR scan results received: []
[2026-08-18 10:07:46.252] [BROWSER] [INFO] 📷 DOM scan empty — falling through to screen capture
```

`qr-scanner-logic.js` applies the **same** `^bitcoin:` test (`BIP21_RE`, line 22) and `classifyQR`
returns `null` for anything else. So one rule, applied in two places, produced two identical failures.

⚠️ **One honest caveat.** The DOM result is `[]`, which cannot distinguish "found the element,
decoded it, rejected the scheme" from "never found the element at all". PaiyBit is a client-rendered
SPA, so the QR may also be in a shadow root, a cross-origin `<iframe>`, or a CSS `background-image` —
none of which the DOM scanner walks. The capture-path failure is **fully explained**; the DOM-path
failure is **explained-consistent but not proven**. §7 `Q1` settles it.

## 4. Is `bsv:` legitimate? Yes — and the owner's framing is right

The owner's note that *"BSV doesn't really have a standard format for QR codes"* is accurate, and it
cuts in favour of accepting more, not demanding conformity:

- There is no BRC that mandates `bitcoin:` for BSV payment URIs.
- `bsv:` is arguably the **better** scheme for this chain — `bitcoin:` is claimed by BTC wallets, so a
  `bitcoin:` URI carrying a BSV address is the ambiguous one.
- We already made the same call in the other direction: `QR_SCAN_OVERVIEW.md` §Phase 3 records that we
  **reverted** switching our own receive QRs to `bitcoin:` because it risked breaking wallets.

⇒ Asking PaiyBit to change would be asking a working, defensible implementation to conform to an
unwritten preference of ours, on a chain with no such standard. **The interoperable move is ours.**

## 5. The fix

Accept `bsv:` alongside `bitcoin:` everywhere the rule is spelled. It is spelled **four times**:

| # | Site | Scheme test | Strips the scheme by |
|---|---|---|---|
| 1 | `cef-native/src/core/QRScreenCapture.cpp:51` | `^bitcoin:` | `text.find(':')` ✅ scheme-agnostic |
| 2 | `cef-native/cef_browser_shell_mac.mm:3028` | `^bitcoin:` | `text.find(':')` ✅ scheme-agnostic |
| 3 | `cef-native/build_tools/qr-scanner-logic.js:22` | `^bitcoin:` | ⛔ `uri.slice(8)` — **hardcoded length of "bitcoin:"** |
| 4 | `frontend/src/utils/bip21.ts:9` | `startsWith('bitcoin:')` | ⛔ `uri.slice(8)` — same |

⛔ **The trap, stated plainly.** Widening only the regex in #3 and #4 makes `bsv:16cezrim…` become
`slice(8)` ⇒ `"zrim1PR2…"` — a truncated address. It fails the address regex, so it fails **closed**
and looks exactly like today's symptom: still "no QR found", but now for a different reason, and the
regex change looks done. **Split on the first `:` in all four sites**, matching what the two C++ sites
already do correctly.

⭐ **Better than four fixes: stop having four copies.** These four are one rule. `qr-scanner-logic.js`
genuinely cannot import from React (it runs in page context), but #1/#2 are a copy-paste pair, and
`bip21.ts` and the scanner script duplicate each other. At minimum, generate #3 from the same source
as #4, and note in each C++ copy that the other exists.

### Also fix the diagnosability, which is the deeper defect

"Decoded, but not a shape we accept" and "could not read a QR at all" are **completely different
problems** and today they produce an identical, silent, empty result. That is why this cost the owner
three attempts and a support round-trip instead of showing a message.

Minimum bar: when a QR decodes but classifies to nothing, tell the user *"Found a QR code, but it
isn't a BSV payment (starts with `xyz:`)"* rather than *"no QR found"*. The payload is already in
hand at that point — `QRScreenCapture.cpp:225` logs it.

## 6. Scope check — do not over-widen

Accept `bsv:` and `bitcoin:`. **Do not** turn this into "accept any scheme":

- ⛔ Never auto-populate a send form from a scheme we do not recognise. The address regex is the real
  guard, but the scheme is a meaningful signal of *intent to pay*, and this is the money path.
- `paymail:` and `handcash:` are worth a decision — filed as `Q2`, not assumed.
- ⚠️ **Unit convention needs confirming, separately from the scheme.** BIP21 `amount` is denominated
  in whole coins; the observed `amount=0.11828417` is BSV, not satoshis. Verify what
  `initialAmount` → `TransactionForm` expects before declaring this fixed, or the first successful
  scan pre-fills a send **eight decimal places wrong**. That is a money-path error and it is a
  *separate* bug from the scheme.

## 7. Open questions

| ID | Question | How to settle |
|---|---|---|
| `Q1` | Did the DOM scan reject the scheme, or never find the element? | Instrument `qr-scanner-logic.js` to report decoded-but-unclassified payloads and re-run against the same page. Settles §3's caveat |
| `Q2` | Accept `paymail:` / `handcash:` too? | Product call. Survey what BSV wallets actually emit |
| `Q3` | Does the DOM scanner need shadow-DOM / same-origin-iframe / CSS-background coverage? | Falls out of `Q1` |
| `Q4` | Is `initialAmount` satoshis or BSV? | Read `TransactionForm`; confirm with one end-to-end scan |

## ⛔ Negative control

- **Before the fix**, the exact payload
  `bsv:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?amount=0.11828417&label=PaiyBit%20media` must be **rejected**
  by all four sites. That is the RED half, and it is already evidenced in production for #1.
- **After**, the same payload populates the send form with address `16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ`
  and the correct amount — and a `bitcoin:` payload still works, proving the change **widened** the
  rule rather than swapping it.
- A payload with a genuinely foreign scheme (`https://example.com`) must **still** be rejected, and
  must now say so rather than reporting "no QR found".
- ⚠️ Subject: assert on the **address that reaches the send form**, not on "the scanner returned
  something". A truncated address from the `slice(8)` trap would satisfy a weaker assertion.

## 8. Acceptance

- [ ] `bsv:` accepted at all four sites; scheme stripped by first `:`, never a fixed offset
- [ ] The three logged production payloads scan end-to-end into a correct send form
- [ ] `bitcoin:` unchanged — measured, not assumed
- [ ] Decoded-but-unclassified reports what it found instead of "no QR found"
- [ ] `Q4` (amount units) confirmed before the phase closes — money path
- [ ] macOS twin (`cef_browser_shell_mac.mm:3028`) fixed and verified on a Mac
- [ ] `Q1` settled, so the DOM-path story is proven rather than assumed
- [ ] `QR_SCAN_OVERVIEW.md`'s pattern table updated — it lists only `bitcoin:`

## Provenance

Owner report 2026-08-18. Root cause found by reading the owner's own production log, which already
contained the decoded payload — no reproduction attempt was needed to identify it, and none was made.
The one thing this ticket does **not** claim is the DOM-path mechanism; see §3's caveat and `Q1`.
