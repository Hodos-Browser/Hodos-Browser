# Mac relay — beta.3 Phase 7c (quiet mode) + the `stop-dev.ps1` fix

**Commits:** `118c2af` · `bddd312` · `292dc8d` · `930245d` · `c7ed703` · `9989935` · `f3c56fa`
**Base:** `c0b0672` (the end of the 7a+7b relay). **Branch:** `0.4.0`.
**Written 2026-09-08.** Predecessor: `MAC_RELAY_P7_ROUND.md` (7a + 7b).

⭐ **The short version, if you read nothing else.** 7c is **Rust + React only**. No C++, no
platform-conditional code, no new overlay, no HWND/NSWindow work. `git pull` and rebuild — there is
nothing to port. The Mac-relevant content below is about **what to re-measure**, not what to write.

---

## M1 — What changed

Quiet mode (`domain_permissions.bundled_scope_grant`) used to make **every** `ProtocolUse` and
`BasketAccess` from an approved site silent — declared or not, ticked or not — because
`matrix_c.rs :: decide_scoped_grant` consulted it *before* `scoped_grant_exists`. The per-item list
on the connect screen was a preview, not a limit.

It now silences nothing. `scoped_grant_exists` (the V18 child tables) governs alone.

⛔ **Two things the plan said that were false, recorded so nobody re-derives them on Mac:**

1. *"a single reordering of two `if` statements"* — a **provable no-op**. Both arms returned
   `Silent`, so all four input cells are identical either way. The arm had to go, not move.
2. *"narrow to the V24 `domain_manifest_snapshots` table"* — **forbidden**. That table's own header
   says *informational only, never a decision input* (`R-SNAPSHOT` / `P0.8-A11`, owner-approved
   2026-08-22). A site could otherwise widen its own grants by republishing after approval.

**No schema change. No migration.** V22/V25 columns remain, unread.

## M2 — 🎯 There is no Mac-shaped risk in this one, and here is why that claim is safe

The 7a+7b relay's M2 flagged `FaviconStore` because it was initialised in `cef_browser_shell.cpp`
(Windows entry) and would fail **silently and plausibly** on Mac. Nothing in 7c has that shape:

| Layer | 7c touches | Mac risk |
|---|---|---|
| `hodos_permission_engine` (Rust) | ✅ the decision itself | none — one crate, one binary |
| `permission_service` (Rust) | ✅ the override plumbing | none |
| `BRC100AuthOverlayRoot.tsx` | ✅ two connect views | none — same bundle both platforms |
| `DomainPermissionForm.tsx`, `ApprovedSitesTab.tsx` | ✅ dead controls removed | none |
| **C++** | ❌ **nothing** | — |

⚠️ The one thing worth an eyeball rather than an assumption: the connect modal lost a checkbox and a
callout, so it is **shorter** now. Phase 7a's viewport work was about a modal too *tall* to show its
buttons. Shorter cannot reintroduce that, but confirm the layout still looks deliberate on a Mac
display rather than leaving a gap where the callout was.

## M3 — How to reproduce, no special rig

The subject must be a site with `bundled_scope_grant = 1` and **zero** V18 grants. On Windows all 8
quiet-mode rows were that shape.

```bash
# 1. find a subject
sqlite3 ~/Library/Application\ Support/HodosBrowserDev/wallet/wallet.db \
  "SELECT dp.domain, dp.bundled_scope_grant,
     (SELECT COUNT(*) FROM domain_protocol_permissions p
       WHERE p.domain_permission_id=dp.id AND p.revoked_at IS NULL)
   FROM domain_permissions dp WHERE dp.bundled_scope_grant=1;"

# 2. one call shape, two domains — the pair is the whole test
curl -s -o /dev/null -w '%{http_code}\n' -X POST http://127.0.0.1:31401/createHmac \
  -H 'Content-Type: application/json' -H 'X-Requesting-Domain: <QUIET_SITE>' \
  -d '{"data":[1,2,3],"protocolID":[2,"p7c probe"],"keyID":"1","counterparty":"self"}'
```

**Expected: `202`** (prompt) for a quiet site with no grants; **`200`** (silent) for a scope that
site was actually granted. ⭐ Run **both** — three sites prompting is equally consistent with *"the
engine is broken and everything prompts now."*

⛔ `counterparty` is **required** by `CreateHmacRequest`. Omit it and serde returns **400 after the
gate has already run** — which is not a silence, and reading it as one manufactures a green. That
happened on Windows.

## M4 — 🚨 What to re-measure on Mac, specifically

| # | Check | Why it is not inherited from Windows |
|---|---|---|
| 1 | The two probes in M3 | Confirms the Mac wallet binary carries the engine change |
| 2 | `cargo test -p hodos_permission_engine` | 42 tests; 4 are new. Cheap and decisive |
| 3 | **Connect modal has no "Quiet mode" checkbox**, and the per-item ticks are **live** (not greyed) | The greying was `disabled={manifestAllowBundledScope}`; a stale bundle would still grey them |
| 4 | **Manage Site Permissions** has no Quiet mode toggle | Different component (`DomainPermissionForm`) |
| 5 | **Wallet settings** has no *"Start new sites in quiet mode"* | Third component again (`ApprovedSitesTab`) |
| 6 | A scoped prompt's **Always allow** writes `key_id = '*'` | The keyID wildcard (D3). Check the DB, not the screen |

## M5 — ⚠️ What a Mac reviewer should be most suspicious of

⭐ **Four instruments produced false or vacuous results on Windows in this phase.** Do not repeat them:

1. **`cargo build --release` passed clean while 8 test call sites were broken.** They are
   `#[cfg(test)]`, so the release build never compiled them. **Use `cargo test --workspace`.**
2. **Bare `preflight.ps1` reported `INCOMPLETE`, not a pass** — `T1d` skips without `-Full`, and the
   script says *"0 failed, 1 skipped. This is NOT a pass."* A skip is never a pass.
3. **A `400` was read as a silence** — see M3.
4. **`-SkipHttpErrorCheck` is PowerShell 7 only**; on 5.1 every row printed `OTHER ()`. Vacuous, and
   caught only because it *looked* vacuous. ⚠️ Related and subtler: `Invoke-WebRequest` treats **202
   as success**, so a success/catch split labels a **prompt** as a **silence**. Branch on
   `StatusCode`. (Both are Windows-shaped; the *lesson* — assert your probe's trigger fired — is not.)

## M6 — 🍎 The one item that is genuinely Mac-owed

`scripts/stop-dev.ps1` was fixed (`9989935`) — `$PSScriptRoot` is empty at parameter-binding time
under `powershell -File`, so the documented no-argument invocation died before the body ran.

⛔ **There is no macOS equivalent of this script.** The 2026-09-01 incident it guards against — a
name-matched kill taking out the owner's **production** wallet — is not Windows-specific:
`hodos-wallet` and `HodosBrowser` share their names with the installed build on macOS too, and
`pkill -f hodos-wallet` is exactly the shape that caused it.

⇒ **Recommend a `scripts/stop-dev.sh`** that matches on executable path (`rust-wallet/target/release`,
`cef-native/build`, `adblock-engine/target/release`) and refuses anything outside the repo. Not
written; flagged. Acceptance is in
`TICKET_stop_dev_script_cannot_be_run_as_documented.md` — note especially that the negative control
is *"the installed wallet survives"*, not *"the script ran"*.

## M7 — Carried, still Mac-owed from earlier rounds

Everything in `MAC_RELAY_P7_ROUND.md` §M6 is unchanged and still owed, including the `FaviconStore`
init/teardown check (M2 there), `T1g` on macOS, the missing `CreateTabContextMenuOverlay`, and the
Phase 4 owner items.

## M8 — Not in this relay

- **Phase 7d is not written yet** — the session prompt exists
  (`SESSION_PROMPT_beta3_phase7d_management_surface.md`) but no code. Its dual-store row **will** be
  platform-relevant: `SetContentSetting` behaviour and the three unmirrored types need macOS
  verification when it lands.
- `R-INTEXT` end-to-end and the 7c adversarial review are **owed on both platforms** before the
  beta.3 release boundary.
- `TICKET_wallet_quiet_detector_blind_to_long_polls.md` — filed, not fixed. ⛔ Its negative control
  needs a **first** connect to a manifest site with a slow auth step; a test on an already-approved
  site *cannot fail*, which is how it survived.
