# Profile Manager Review — Windows + macOS (0.4.0)

**Created:** 2026-06-18 · **Status:** Research complete — recommendations awaiting implementation decision · **Type:** research only (no code changed)

> **How this was produced.** A scoped multi-agent workflow (`profile-manager-review`, 9 agents): 4 parallel research agents (Hodos current-state by source-reading; Chromium process model; Firefox model; cross-browser UX), 1 synthesis, then a 3-lens adversarial review (architecture-fit / regression-risk / research-accuracy) + reconcile. Several synthesis premises were **falsified** during the adversarial pass (see Rejected). The DB/lock foundation was then **hand-verified against current source** by the reviewer (see Verified Foundation).
>
> Fuller per-agent reasoning lives in the workflow transcript (`…/workflows/wf_c081928c-884`). This doc is the reconciled, decision-ready output.

---

## Verified foundation (hand-checked against source)

- **WAL + `busy_timeout(5000)` are already set** on all four SQLite DBs — `BookmarkManager`, `CookieBlockManager`, `HistoryManager`, `PaidContentCache` (each does `PRAGMA journal_mode=WAL` + `sqlite3_busy_timeout(…,5000)` and `wal_checkpoint(RESTART)` on `CloseDatabase`). → Adding more `busy_timeout` is redundant (R4 rejected).
- **`ReleaseProfileLock()` is called at three sites:** `cef_browser_shell.cpp:530` and `:3585` (Windows — *two* release paths is itself a smell) and `cef_browser_shell_mac.mm:4940` (macOS).
- **`AcquireProfileLock` already retries** 6 × 500 ms ("previous instance may still be shutting down"); a separate `SingleInstance.cpp` mechanism also exists.
- **Conclusion:** the "DB held on quick restart" pain is a **shutdown-ordering** problem — DB checkpoint/close and lock release don't reliably complete (in the right order) within the new instance's ~3 s retry window — *not* a missing-retry or missing-PRAGMA problem. This is exactly what R2 + R3 target.

---

## Recommendations (reconciled)

Legend — disposition: **keep** / **modify** (refined by the adversarial review) / **reject** (premise falsified or unsafe).

### Fix-bug + security (the priority cluster)

| ID | Title | Disp. | Pain point | Anchor | Risk/Effort | Final recommendation |
|----|-------|-------|-----------|--------|-------------|----------------------|
| **R1** | posix_spawn/execv + validate profileId (**F5**) | keep | F5-security | `ProfileManager.cpp:437` (`system()`); IPC `simple_handler.cpp` `profiles_switch` | low / small | Replace `system()` with `posix_spawn`/`execv` (argv, no shell); add `IsValidProfileId` (modern + legacy `Default`/`Profile_N`) at the 3 boundaries (`profiles_switch`, `LaunchWithProfile`, `--profile` parse). Defense-in-depth. |
| **R2** | DB `CloseDatabase` cascade **after** browsers force-closed | modify | db-held-on-restart | shutdown path `cef_browser_shell.cpp` / `_mac.mm`; the 4 managers' `CloseDatabase` | med / med | Cascade-close all SQLite managers **after** CEF force-close + thread-drain (not before); null-guarded + idempotent. Residual contention is `SQLITE_BUSY`, **not** corruption (WAL). |
| **R3** | `ReleaseProfileLock` **after** the DB cascade | modify | db-held-on-restart | `cef_browser_shell.cpp:530`/`:3585`, `_mac.mm:4940` | med / med | Release the profile lock **after** R2's cascade completes; make the lock release atomic w.r.t. acquire so a quick relaunch never sees a false "in use." Reconcile the two Windows release sites. |
| **R7** | Validate `--profile`; **coherent** Default fallback | keep | profileId-mgmt | `ParseProfileArgument` / startup, before SQLite init | low / small | An unknown/garbage `--profile` id must fall back to `Default` **coherently** (UI + data-dir agree) **before** SQLite init — today UI and data-dir can disagree. (Rejected: injecting the picker as the fallback.) |
| **R5** | Stop the **boot-time rewrite** of `profiles.json` | keep | profileId-mgmt | `ProfileManager` (registry write) | med / med | Persist `lastUsedProfile` only on an actual user switch, not on every boot; protect the registry with a real cross-process (RAII) lock. The current per-process mutex can torn-write the shared registry. **Design first.** |

### Shutdown robustness

| ID | Title | Disp. | Pain point | Anchor | Risk/Effort | Final recommendation |
|----|-------|-------|-----------|--------|-------------|----------------------|
| **R8** | Narrow the **macOS** shutdown gaps | modify | shutdown | `cef_browser_shell_mac.mm` (~`:4940`) | med / med | Original premise ("mac doesn't force-close browsers") was **FALSE** — it already does. *Real* gaps: missing DB cascade + late lock release. ⚠️ Touches CEF shutdown lifecycle → **CLAUDE.md invariant #8: ASK before changing.** |
| **R9** | Bounded-timeout on server-stop join | modify | shutdown | wallet daemon stop (`WalletService` stop / shutdown) | low / small | Join is already parallel and the wallet's SQLite lives in the *Rust* process — the real risk is a **second daemon racing to rebind :31301** on quick restart. Add a bounded-timeout join + port-rebind guard. |

### Correctness

| ID | Title | Disp. | Pain point | Anchor | Risk/Effort | Final recommendation |
|----|-------|-------|-----------|--------|-------------|----------------------|
| **R6** | JSON-build `profiles_get_all` via nlohmann | keep | other (correctness) | `simple_handler.cpp` `profiles_get_all` | low / small | Build the payload with `nlohmann::json`, not hand-concatenation, so a profile **name** containing quotes/specials can't break the picker. **Note:** this is *not* `escapeJsonForJs` (F6) — that's for interpolating into a JS string literal; here we're building a JSON *document*, so the JSON library is correct. Grep for sibling hand-built sites. |

### UI/UX enhancements (later)

| ID | Title | Disp. | Anchor | Risk/Effort | Final recommendation |
|----|-------|-------|--------|-------------|----------------------|
| **R10** | Colored identity chip | keep | React (`MainBrowserView`/header) | low / small | Render a profile chip using existing `ProfileInfo.color`. Pure React. Land R6 first (clean profile data). |
| **R11** | Per-profile window color accent | keep | header-browser creation + CSS | low / small | CSS accent from profile color; flash-free requires passing the color at header-browser creation time. |
| **R13** | Always-visible picker buttons + `Ctrl+Shift+<n>` | keep | `ProfilePickerOverlayRoot` + keyboard handler | low / small | Add always-visible create/switch buttons; `Ctrl+Shift+<n>` spawns a **new window** for profile N (in-window profile swap is unsupported in our model). Additive. |
| **R14** | Domain→profile **hint** toast | keep | new C++-side store + notification overlay | med / med | Opt-in hint (e.g. "this site is usually opened in <Work>") — **never auto-switch**. New small C++-side store (NOT the Rust `domain_permissions` table); reuse the notification overlay. |

### Rejected

| ID | Title | Why rejected |
|----|-------|--------------|
| **R4** | Widen acquire window via `busy_timeout` | WAL + `busy_timeout(5000)` already present on all four DBs (verified). Its useful half (release-ordering) folds into **R3**. |
| **R12** | Startup-picker opt-out toggle | Premise falsified — the `>= 2 profiles` gate already exists (`ShouldShowPickerOnStartup`, ~`:1242`); default is already opt-in. |

---

## Recommended implementation sequence

1. **R1** — F5 security fix (already scoped; smallest, highest-severity).
2. **R6, R7** — cheap correctness/validation wins (JSON-built picker payload; coherent `--profile` fallback).
3. **R2 + R3 (atomic)** — the **DB-held-on-quick-restart** core: cascade DB close *then* release lock, *after* browser force-close. This is the pain point that's been hurting.
4. **R5** — registry write hardening (design-first; cross-process lock).
5. **R9** — server-stop bounded-timeout + :31301 rebind guard.
6. **R8** — macOS shutdown gaps (⚠️ invariant #8 — ASK before touching CEF shutdown lifecycle).
7. **R10 / R11 / R13 / R14** — UX layer, last.

## Open questions for decision

- **Scope of this push:** land just the bug/security cluster (R1, R2, R3, R6, R7) in 0.4.0 and defer R5/R8/R9 + all UX (R10–R14) — or take more?
- **R2/R3/R8 touch the CEF shutdown lifecycle** (invariant #8). Each needs its own kickoff + explicit go before code.
- **R5** changes registry-write timing — confirm the desired `lastUsedProfile` semantics before designing.
- **R14** introduces a new C++-side domain→profile store — confirm that's wanted vs. out-of-scope for 0.4.0.
