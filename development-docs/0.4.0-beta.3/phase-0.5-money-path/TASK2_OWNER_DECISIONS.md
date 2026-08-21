# Task 2 — six decisions owed to the owner

**Written 2026-08-20**, after clearing panel #2's Task 0 and Task 1.
**Updated 2026-08-21** with the owner's decisions.

| Status | Items |
|---|---|
| ✅ **DECIDED + FIXED** | **2** (`reveal-mnemonic`), **3** (`wallet/settings`) — `c8558dc` |
| ✅ **ANSWERED** | **6** (macOS ships ⇒ the SSRF is a BLOCKER, relayed as E1) |
| ⬜ **STILL OWED** | **1** (sharpened below — worse than filed), **4**, **5** |

⛔ **Items 1, 4 and 5 have NOT been changed.** CLAUDE.md #13: where the evidence points at
production code, present the evidence and get approval first.

All six are **pre-existing** — none was introduced by Phase 0.5. Every file:line
below was re-grepped against the current tree this session.

---

## 1. `IsInternalOrigin("") == true` + a second, unhardened derivation

**Verified.** `HttpRequestInterceptor.cpp` — `if (origin.empty()) return true;`, plus
`matchesHostOrHostColon` accepting any port. Separately, `extractDomain` is a
**second** derivation site that never got the C1 hardening.

**Consequence:** C1's banner claim — *"ONE derivation, applied ONCE"* — is **FALSE**.
That is true whether or not the code changes, so **the contract must be corrected
either way.**

Two halves, very different cost:
- **empty-origin half** — an empty origin is treated as fully-trusted wallet-internal.
  No local server needed. This is the part the panel rates as a blocker.
- **loopback-port half** — any page on any `127.0.0.1:<port>` gets full first-party
  trust. Needs the user to be running a local server (see #4).

### ⚠️ SHARPENED 2026-08-21 — I traced the whole chain, and it is worse than filed

The panel filed this as `IsInternalOrigin("") == true`. That framing is **too narrow, and the
empty-origin half is NOT the reachable part on the IPC transport** — `ResolveIpcOrigin` already
substitutes `"opaque-origin.invalid"` for an empty origin, so C1's cascade genuinely fails closed
there. Credit where due: that half works.

**The real defect is the SECOND derivation, on the HTTP transport, and it is a userinfo bypass.**
Chain verified end to end this session:

```
extractDomain(browser, request)            HttpRequestInterceptor.cpp :: extractDomain
  -> domain                                 (hand-rolled: find "://", read to next "/")
  -> AsyncWalletResourceHandler(..., domain, ...)
  -> requestDomain_
  -> IsInternalOrigin(requestDomain_)       -> "🔒 Internal origin … — bypassing domain check"
```

`extractDomain` **does not strip userinfo**. So for a page at
`https://127.0.0.1:31301@evil.com/` — which actually loads from **evil.com** — it returns the
string `127.0.0.1:31301@evil.com`. `IsInternalOrigin`'s `matchesHostOrHostColon("127.0.0.1")` sees
`127.0.0.1` followed by `:` and returns **true**. The page is treated as **wallet-internal** and the
domain check is skipped entirely.

The empty case is the same defect's second face: a top-level document whose URL has no `://`
(e.g. `about:blank`) with no referrer yields `""`, and `IsInternalOrigin("")` returns true.

**Why this is embarrassing rather than merely bad:** the correct derivation already exists, is
already used by the IPC path, and is **already unit-tested against this exact input** —
`tests/port_config_origin_test.cpp :: UserinfoIsStripped` asserts
`OriginFromUrl("http://127.0.0.1:5137@evil.com/") == "evil.com"`. We wrote the rule down, pinned it
with a test, and then left a second parser that violates it on the money transport.

**Status: CODE_READING, not measured.** ⛔ I have not driven a browser to
`https://127.0.0.1:31301@evil.com/` and watched the bypass fire. Two things could still block it:
Chromium may strip or refuse userinfo on top-level navigation, and **Chromium 150 gates direct
fetch to `127.0.0.1` behind a Local Network Access prompt**, so the HTTP transport may be
unreachable from a public page without a grant. **Both must be checked before this is called
exploitable** — that is the experiment attached to this hypothesis.

**The fix, if you want it, is small and low-risk:** replace `extractDomain`'s hand-rolled parse with
`hodos::OriginFromUrl(mainFrameUrl)` and fail closed on empty (mirroring `ResolveIpcOrigin`'s
sentinel). ⛔ **Do not write a third parser.**

> **Recommendation: fix in beta.3 — but measure the reachability first, in that order.**
> The change is "call the already-tested helper", which is about as safe as a security fix gets.
> But it also alters what `extractDomain` returns for iframes and odd URLs on a path that every
> wallet HTTP request crosses, so I would want the LNA/userinfo reachability answer before landing
> it, not after. If reachability turns out to be blocked, this drops to a defence-in-depth cleanup
> and can ride to Phase 5 with item 4 — the two are the same predicate.

**The C1 banner claim has been corrected in code** (`simple_handler.cpp`, the C1/C2 banner) —
it said *"ONE derivation, applied ONCE"*, which was false and actively harmful, because it tells the
next reader to stop looking.

---

## 2. ✅ DECIDED + FIXED (`c8558dc`) — `/wallet/reveal-mnemonic` on the no-PIN branch

Returns the **BIP39 recovery phrase to page context** after one Allow click.

**This conflicts with Invariant #1 as written.** CLAUDE.md states the phrase is the
one deliberate exception to "keys never in JS", shown once at creation and thereafter
"only through PIN re-verification in the wallet overlay", and that it is "never
reachable from web content". On the no-PIN branch that last clause does not hold.

> **Recommendation: fix in beta.3 — make the endpoint first-party only.**
> Of everything in this document, this is the one I would not ship. It is key
> exfiltration, the blast radius is the entire wallet including funds not yet
> received, and it is unrecoverable. The fix is the same shape as the §4k gate
> (refuse when `X-Requesting-Domain` is present) and needs no schema change.
> If you would rather not touch it this late, the alternative is to require PIN on
> **every** branch — but that changes a shipped UX flow, so it is your call.

---

## 3. ✅ DECIDED + FIXED (`c8558dc`) — `POST /wallet/settings` is page-callable

Rewrites `default_per_tx_limit_cents`, `default_per_session_limit_cents`,
`default_rate_limit_per_min` — the values **every future approval inherits** — plus
`default_identity_key_disclosure_allowed`.

Same class as §4k, and the same payload, **without needing the encoding trick**.

> **Recommendation: fix in beta.3 — it is one line in the gate I just landed.**
> `is_permission_surface()` in `main.rs` already matches subtrees; adding
> `/wallet/settings` to it is a two-word change with the identical test shape.
> ⚠️ I did **not** do it because it is explicitly your call, but I want to flag that
> leaving it open substantially undoes the value of the §4k fix: a site that cannot
> raise its own caps directly can still raise the defaults every later grant copies.
> **Check first:** whether the wallet UI writes settings header-free. If it goes
> through the IPC bridge it is header-free and unaffected; if any overlay does a
> direct fetch carrying the header, that path breaks and needs the same self-scoped
> carve-out I used for `disconnect()`.

---

## 4. Any page on any loopback port gets full first-party IPC trust

Phase 5's `IsWalletOrigin()` territory, but the panel rates it above "follow-up".

Real exposure: a developer running anything on `127.0.0.1` — a docs site, a test
server, a scratch React app — grants that page the full ungated IPC surface,
`send_transaction` included. Our own users are disproportionately developers.

Related and separate: intercepting `127.0.0.1:8080` **hijacks the user's own dev
server** on the single most common local port, and the `/health` arm makes a
collision likely.

> **Recommendation: defer to Phase 5, but treat it as Phase 5's headline item, not a
> cleanup.** A correct predicate is "the wallet UI origin", not "loopback" — that is a
> real design change (it has to survive the dev server on 5137, the overlays, and the
> release-mode local file handler), and doing it properly needs the Phase 5 endpoint
> allowlist alongside it. Rushing it into beta.3 risks breaking every overlay.

---

## 5. The two-phase action lifecycle cannot be gated at all

**Verified structural.** `sign_action` has signature `(state, body)` — **no
`HttpRequest` parameter**, so it cannot read `X-Requesting-Domain` and has no gate.
Same for `/processAction`, `/abortAction`, `/internalizeAction`,
`/wallet/broadcast-nosend`.

The panel flagged the **modality**, not a proven exploit: whether
`PENDING_TRANSACTIONS` references are domain-bound, and whether `spends` or
`options.noSend` can change the effective spend between phase 1 and phase 2, is
**unexamined by anyone**, including me.

> **Recommendation: do not fix blind — spend one session MEASURING it first.**
> This is the largest unexamined surface left in the phase and the one I would most
> expect to hide a real defect, precisely because createAction's *first* phase has
> been audited repeatedly and its second phase never has. Adding an `HttpRequest`
> parameter to five handlers is mechanical; knowing what the gate should then *decide*
> is not. ⛔ Do not let "it's structurally ungateable" become "therefore it's
> exploitable" without a measurement — that is the CODE_READING trap this phase keeps
> paying for.

---

## 6. ✅ ANSWERED — YES, beta.3 ships macOS ⇒ the SSRF is a BLOCKER

**This is a direct question, and it changes the severity of a finding.** The
curl-userinfo SSRF on the macOS `wallet_call` path (an arbitrary-URL fetch from the
browser process with the response body returned to the page, no approval) is a
**follow-up if beta.3 is Windows-only, and a BLOCKER if macOS ships.**

Also unaddressed: CLAUDE.md invariant #9 requires macOS parity verification per
change. `cef_browser_shell_mac.mm`, the `Create*OverlayMacOS` roster and
`InstallClickOutsideMonitor` were **not reviewed by panel #2 at all**, and I have not
touched them. The Rust fixes I landed are platform-neutral; the C++ `PaymentCost.h`
change is header-only pure logic and compiles on both, but **has not been built on
macOS this session**.

> **Recommendation: answer this before Task 4.** If macOS ships, the panel-#3 scope
> and the remaining work both grow, and the SSRF has to be fixed first. If it does
> not, say so in the contract so the next session stops re-deriving the question.

---

## Summary — what I would do

| # | Item | Status |
|---|---|---|
| 2 | `reveal-mnemonic` to page context | ✅ **FIXED** `c8558dc`, RED + GREEN + first-party control |
| 3 | `POST /wallet/settings` page-callable | ✅ **FIXED** `c8558dc`, RED rewrote the defaults 1000→999999 |
| 6 | macOS scope | ✅ **ANSWERED: ships.** SSRF promoted to BLOCKER, relayed as E1 |
| 1 | `extractDomain` userinfo bypass (was filed as `IsInternalOrigin("")`) | ⬜ **OWED — measure reachability, then fix.** Sharper than filed: the IPC half already fails closed; the HTTP half treats `https://127.0.0.1:31301@evil.com/` as wallet-internal. C1 banner claim corrected in code. |
| 5 | Two-phase action lifecycle | ⬜ **OWED — measure before fixing.** Biggest unexamined surface |
| 4 | Loopback-port trust | ⬜ **OWED — Phase 5 headline**, same predicate as #1's port half |
