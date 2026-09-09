# Mac relay — beta.3 Phase 5 (loopback routing & trust boundary)

**Relayed 2026-09-02 from the Windows box.** ⛔ Nothing here is claimed as verified on macOS. Every
row is *"Windows measured this; Mac must measure it too."*

**Commits:** `c4603e1` (contract + pre-fix measurement) · `1743b20` (the fix) · `36f7a66` (gate `G12`)

---

## M1 — The code is cross-platform; the *risk* is not evenly distributed

The whole change lives in files both platforms build:

| File | Change |
|---|---|
| `cef-native/include/core/PortConfig.h` | New predicates: `AuthoritySpan`, `SplitAuthority`, `IsLoopbackHost`, `IsLoopbackAuthority`, `IsCompatBridgePort`, `IsWalletOrigin`, `IsOurWalletOrigin`, `IsMessageboxOrigin`, `IsWellKnownAuthRequest`, `RepointLoopbackToWallet`, `LegacyWalletGateMatch`. `OriginFromUrl` refactored onto `AuthoritySpan` (behaviour-preserving — P0.5's 59 assertions still pass) |
| `cef-native/src/handlers/simple_handler.cpp` | The resource-dispatch gate + the trusted-overlay bypass |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | `redirectPort` → `RepointLoopbackToWallet`; scheme downgrade, `/.well-known/auth` loopback test, messagebox arm, `isSocketIOConnection`, and the `/health` arm all anchored |
| `cef-native/tests/` | `wallet_origin_test.cpp` (+ `CMakeLists.txt`) |

No `#ifdef` anywhere in the change — it is string parsing. ⚠️ **`hodos_tests` links no libcef and
already builds on macOS**, so `wallet_origin_test.cpp` should compile and pass unmodified. If it does
not, that is the first thing to report.

## M2 — 🎯 The two genuinely Mac-shaped unknowns

These are the rows `SPRINT_PLAN.md` §5 flagged as needing Mac coverage, and Windows answers do **not**
transfer.

| # | Question | Windows answer | Why Mac may differ |
|---|---|---|---|
| **R1** | Is there a **cross-wallet routing hole** on the Mac — i.e. does MetaNet Client (or any wallet) listen on `127.0.0.1:3321` / `:2121` there? | 📏 **Yes it listens** (PID 37360, both ports), but we intercept both, so the hole is **closed**. Verified at the destination: our Rust log recorded all 8 probe requests carrying `requesting_domain=example.com` | Depends entirely on what is installed on that machine. `lsof -nP -iTCP -sTCP:LISTEN \| grep -E '3321\|2121'` |
| **R2** | Does a `CefResourceHandler` take over **`https://` loopback pre-TLS**? | 📏 **Yes.** `https://127.0.0.1:2121/getVersion` and `/health` were served with no certificate interstitial and no TLS error, confirmed at the destination. Ticket §11 Q1 — open since 2026-08-18 — is settled **on Windows** | Different network stack under CEF on macOS. If TLS fires first there, the fallback is ticket §8.1: **stop matching 2121** so the probe fails fast into its `http://…:3321` retry |

## M3 — How to reproduce the measurement (it needs no special rig)

Bring up dev (`./dev-wallet.sh`, `npm run dev`, `./mac_build_run.sh`), open a real external https
page, and from its console:

```js
await (await fetch('http://127.0.0.1:3321/getVersion')).text()   // R1 + the compat port
await (await fetch('https://127.0.0.1:2121/getVersion')).text()  // R2 — the TLS question
```

🎯 **SUBJECT: read OUR Rust log, not the page and not the C++ log.** The line to look for is

```
R-INTEXT trust: path=/getVersion requesting_domain=<the page's host>
```

⛔ The C++ `Scheme downgrade` line proves nothing — it prints whether or not the rewrite took effect
(`ADVERSARIAL_PANEL_2` #29). The page cannot tell which wallet answered.

⭐ **Free negative control, and it beats stopping the other wallet:** probe
`http://127.0.0.1:3322/getNetwork` — one digit off 3321, deliberately not in our gate. It must produce
**no** `Intercepting wallet request` line, **zero** Rust lines, and `TypeError: Failed to fetch`. That
proves the gate is what causes interception, and it touches nobody's installed software.

## M4 — 🚨 The defect worth re-checking on Mac, with its exact URL

Windows reproduced this live before the fix and confirmed it gone after:

```js
await (await fetch('https://example.com/getNetwork?x=127.0.0.1:3321')).text()
```

- **Before:** `status=200`, body `{"error":"Wallet request timeout"}` — our wallet answered a request
  addressed to example.com, after silently downgrading it https→http.
- **After:** `status=404` and example.com's own HTML.

## M5 — ⚠️ What a Mac reviewer should be most suspicious of

⭐ **Read `hodos::IsLoopbackHost`'s comment before touching anything here.** The predicate is
deliberately **broader** than a strict host-equality test — it accepts `*.localhost` and all of
`127.0.0.0/8`. That looks like sloppiness and is the opposite: C++ is what stamps
`X-Requesting-Domain`, and Rust reads a *missing* header as internal and fully trusted, so a matcher
that fails to match leaves traffic **trusted**. Narrowing it is a privilege escalation.

⚠️ `IsLoopbackHost` relies on the host already being URL-canonical (GURL lowercases the host and
normalises decimal/octal/hex IPv4 to dotted-quad). That is a Chromium guarantee, so it should hold
identically on macOS — but it is an assumption, and it is the one that would break the digits-and-dots
test if it were ever false.

## M6 — Carried, still Mac-owed from earlier rounds

- `MAC_RELAY_P35_P4_ROUND.md` M3 — the tab context menu (`CreateTabContextMenuOverlay`) is Windows-only;
  macOS is at **14** overlays to Windows' **15**.
- Ticket §8.6 — W7's overlay coverage (wallet / wallet_panel / settings / backup must still reach the
  Rust wallet before and after the predicate swap). ⚠️ Phase 5 already changed the **trusted-overlay
  bypass** to `IsOurWalletOrigin`, so this is worth a quick look now rather than waiting for beta.4:
  open each of the four overlays on Mac and confirm the wallet still answers.

---

## ✅ ANSWERED BY MAC 2026-09-08 — `R1` and `R2` both resolved

Evidence: `MAC_RELAY_BETA3.md` (round 2026-09-08 Mac, §D) and
`phase-5-loopback-routing/PHASE_CONTRACT.md` §4b. Probe: `p5probe_mac.py`.

- **`R1`** — `lsof … | grep -E '3321|2121'` returns **nothing**: no other wallet listens on this
  Mac. ⚠️ A fact about this machine, not a platform guarantee.
- **`R2`** — 📏 **YES.** `https://127.0.0.1:2121/getVersion` reached our Rust wallet with
  `requesting_domain=example.com`, no cert interstitial, no TLS error. ⇒ **ticket §8.1's fallback
  (stop matching 2121) is NOT needed on macOS**, and §11 Q1 is now settled on both platforms.
- **M4** — `https://example.com/getNetwork?x=127.0.0.1:3321` → **404 + example.com's own HTML**,
  zero Rust lines. Defect absent.
- **Negative control** — `127.0.0.1:3322` → `TypeError: Failed to fetch`, zero Rust lines. ⭐ Your
  free control worked exactly as advertised and touched nobody's installed software.

⭐ **Why the page aborted while the wallet answered:** `OnShowPermissionPrompt … mapped=[loopback]`
— Chromium's LNA gate held the response after our interceptor had already reached the wallet. This
also re-confirms that **macOS does raise the loopback permission**; the 2026-08-26 "it never fires"
was an artifact of `--disable-web-security`.

⬜ **M6 / §8.6 `W7` still owed** — the four overlays open from native toolbar clicks, not an IPC I
can drive from this session.
