# Phase 0.5 — money path & trust boundary · PHASE CONTRACT

**Workstream:** WS5(a) · **Ticket:** `../TICKET_loopback_host_form_wallet_routing.md` §6.2, §7.1, §7.3
**Status:** ⬜ NOT STARTED · **Opened:** 2026-08-18 · **Platforms:** both (Rust = one binary; the C++ gates are cross-platform)
**Standard:** `../HARNESS.md`.

---

## 1. Goal

A fund-moving request from a web page is subject to the same approval engine as every other payment,
while a send the user initiates in their own wallet UI continues to complete **without any prompt**.

## 2. Done means

- [ ] `send_transaction` takes `HttpRequest` and routes external callers through `dispatch_payment`,
      exactly as `create_action` does
- [ ] An internal caller (no `X-Requesting-Domain`) is **unchanged** — no modal, no new latency
- [ ] `sendMax` from an external origin is subject to the per-tx and per-session caps
- [ ] `.block_on_origin_mismatch(true)` on the CORS layer
- [ ] The three `:5137` **trust-boundary** substring gates use a prefix/origin match
- [ ] T0 gate `G2` baseline driven **5 → 2**, with both residuals named in §6
- [ ] `IsInternalOrigin("")` decided — it is now load-bearing

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| **`R-INTEXT`** | **Internal never prompts, external always gates** | 🚨 **This is the phase's central risk.** Gating `send_transaction` naively — e.g. requiring `X-User-Approved` unconditionally — makes the user's own send start prompting. The discriminator must remain header-presence, the same one everything else uses |
| `R-PERIM` | The four privacy-perimeter gates | Touches `dispatch_payment`'s call surface |
| `R-COUNT` | Per-session counters | A newly-gated endpoint now increments them; it did not before |
| `R-GOLD` | Gold pill fires | A newly silent-approved path must still emit the pill, or a payment goes visually unannounced |

## 4. Evidence table

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P0.5-R1` | User send from the wallet UI → **no modal**, tx broadcasts | Stub the internal branch to require approval → a modal appears | Frame URL `127.0.0.1:5137`; Rust log shows **no** `X-Requesting-Domain` | T2 | ⬜ |
| `P0.5-R2` | External page, over-cap send → **202 + modal** | Revert the `dispatch_payment` wiring → the send completes silently | Rust log shows `X-Requesting-Domain: <exact page host>` | T2 | ⬜ |
| `P0.5-R3` | External `sendMax:true` is capped | Same revert → full balance sweeps | Scratch wallet, funded with a token amount. ⛔ **Never the production wallet** | T2 | ⬜ |
| `P0.5-R4` | Gold pill fires on a newly silent-approved send | Stub the emit → no pill | Correct tab via `TabManager::GetTabIdForBrowserIdentifier` | T2 | ⬜ |
| `P0.5-C1` | Cross-origin simple POST no longer executes the handler | Remove `block_on_origin_mismatch` → the handler runs despite the browser hiding the response | **Server-side effect**, not the browser's error. A blocked read is not a blocked write | T2 | ⬜ |
| `P0.5-G1` | `https://<origin>/?x=127.0.0.1:5137` is served **nothing** from disk | ⛔ **Pre-fix this must SUCCEED** — if it does not, §7.3 is refuted and this row is withdrawn | **Release-shaped** build: `IsFrontendAvailable()` is true in production (`{app}\frontend\`), so this is not a dev-only defect | T2 | ⬜ |
| `P0.5-G2` | The same page gets **no** `window.hodosBrowser.identity` | Pre-fix it must be **defined**. Control: the same page *without* the substring must get neither | Renderer for **that page's** frame — not an overlay. `type:"page"` over CDP is not proof of which browser | T2 | ⬜ |
| `P0.5-G3` |  `preflight.ps1` gate `G2` passes at baseline **2** (from 5) | Add one new `find("127.0.0.1:5137")` → gate **exits non-zero** even at a non-zero baseline | `preflight.ps1 -NegativeControl` | T0 | ⬜ |
| `P0.5-E1` | `IsInternalOrigin("")` behaves per the decision taken | Feed an origin-less frame → observe the decided outcome, not today's silent `true` | The Rust-side gate outcome | T1 | ⬜ |

**Pairing:** `R1`/`R2` are the two halves of `R-INTEXT` and are each other's control. **Neither may be
signed off alone.** A fix that satisfies one by breaking the other is the specific failure this
phase is most likely to produce.

## 5. Blast radius

- `rust-wallet/src/handlers.rs :: send_transaction` (9612–9951) — signature change; every caller is
  `WalletService::sendTransaction` via `simple_handler.cpp:5740`, i.e. first-party today.
- `rust-wallet/src/main.rs` — CORS builder (`:922-930`). One line, affects every route.
- `cef-native/src/handlers/simple_handler.cpp:7962` — frontend-from-disk gate.
- `cef-native/src/handlers/simple_render_process_handler.cpp:539,541` — the privileged V8 surface
  (`hodosBrowser.identity`, `.navigation`, history, `WALLET_CALL_BRIDGE_SCRIPT`).
- ⭐ **Reuse, do not re-spell:** `IsInternalFrontendUrl()` (`simple_handler.cpp:137-142`) is already
  the correct predicate. It lives in the browser process, so the render-process pair needs a shared
  header or the same four lines — **not a fourth spelling**.

## 6. Out of scope

- **W0/W1/W2'/W3** — the parsed `IsWalletOrigin()` predicate and `/health`. That is **Phase 5**.
- **W4/W6/W7/W8** — beta.4.
- Residual `G2` violations, knowingly allowed at target **2**, listed so they are not mistaken for
  oversights:
  - `TabManager.cpp:168` and `TabManager_mac.mm:191` — `find(...) == npos` used to *exclude* internal
    URLs from history. Sloppy, and it means an external URL merely containing the string would be
    skipped from history — a nuisance, not a trust boundary. Goes with W7 in beta.4.
  - *(`simple_handler.cpp:1153` was on this list until the gate was written: `find(X) != 0` **is** a
    prefix check, so `G2` now excludes it by construction along with `rfind(X, 0) == 0`. Correct code
    should not be flagged.)*
- `extractDomain()`'s scheme-blindness and main-frame-URL sourcing — filed, deliberately deferred.

## 7. Rollback

Three independent reverts: the Rust handler signature, the one-line CORS change, and the C++ gates.
Each stands alone; none shares a commit with another.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] ⛔ `P0.5-G1` / `P0.5-G2` pre-fix reproduction attempted and its result recorded **either way** —
      a refuted premise is a valid outcome and must be written down, not quietly dropped
- [ ] `scripts/preflight.ps1` + `-NegativeControl` recorded
- [ ] `../REGRESSION_SET.md` in full at the 0.5 → 1 boundary, `R-INTEXT` **both halves**
- [ ] Adversarial review — ✅ **workflow panel** per `../HARNESS.md` §6, distinct lenses:
      *can I reach a money endpoint un-stamped* · *can I forge an origin* · *does the internal path still stay silent*
- [ ] `G2` 5 → 2 in `../HARNESS.md` §9
- [ ] Commits cite row IDs

| Item | Result | Date | By |
|---|---|---|---|
| preflight | | | |
| preflight -NegativeControl | | | |
| regression set (0.5 → 1) | | | |
| adversarial review (panel) | | | |
