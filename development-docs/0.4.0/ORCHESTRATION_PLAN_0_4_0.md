# 0.4.0 Sprint — Orchestration & Execution Plan

> **Created 2026-06-17.** This is the **execution boot-doc** for the 0.4.0 sprint. A fresh context should read this first, then `SPRINT_0_4_0_MASTER_PLAN.md` (the what/why), then the per-item phase docs.
> Companion: `MACOS_PORT_0_4_0.md` (the running macOS-delta log, filled continuously, pulled at Mac-sprint time).

---

## 0. Boot instructions (for a fresh context)

1. Read this doc + `SPRINT_0_4_0_MASTER_PLAN.md` (esp. §3 inventory, §9 decisions, §11 P&P, §12 release sequencing).
2. **Docs drift** — re-verify every cited `file:line` against current source before trusting it (kickoff workflow, CLAUDE.md).
3. Start at **Wave 0**. Run **every** chunk through the §1 harness lifecycle.
4. **CEF target = stable M149** (`149.0.3` / Chromium `149.0.7827.115`), verified live from `cef-builds.spotifycdn.com/index.json` on 2026-06-17. Re-verify the exact build from the CDN at bump time — never from a wiki.
5. **Windows-first.** Capture every macOS delta into `MACOS_PORT_0_4_0.md` as you go (§6).
6. **No rush.** Task agents *narrowly*. Adversarial-verify before and after coding (§1, §8).

---

## 0.5 Release strategy — RESHAPED 2026-06-19 (read before the wave graph in §3)

The sprint is now split into **two release milestones**. Rationale: the current public build has real bugs (incl. big ones); we've since landed a large pile of fixes (security F-track, profiles, history isolation). Ship those to users **now** in a fast, cheap build instead of holding them behind the ~10–12 h Chromium rebuild + farbling work.

**Terminology (decided 2026-06-19):**
- **Shell release** = rebuild Hodos's C++/Rust/React (the "CEF Shell" layer) on the **CURRENT, already-built CEF binaries**. Fast (~35 min CI). = the runbook's "Tier 2". This is the maintenance release.
- **Engine release** = rebuild **Chromium + CEF from source** (CEF M149 bump + Blink farbling patches). Slow (~10–12 h), rare. = the runbook's "Tier 1". This is the CEF long-pole.

**Milestone 1 — Shell release (current engine), CROSS-PLATFORM (Win + Mac).** Readiness checklist (rough order):
1. **macOS parity first** (cross-platform decision): port the pre-window picker to an NSWindow path; apply the history-isolation fix in the mac render helper (`process_helper_mac.mm` / `cef_browser_shell_mac.mm` — same hardcoded-`Default` pattern). Plus the user's manual macOS smoke. (See `MACOS_PORT_0_4_0.md`.)
2. **Header / Omnibox UX pass** (expanded 2026-06-19) — a cohesive header rework; **kick off with a scoped research+design WORKFLOW** (Chrome/Brave/Vivaldi, win+mac) that returns one comprehensive design + impl plan + macOS-parity analysis, THEN implement feature-by-feature through the harness. Features:
   - **(a) Bookmarks** (B3, backend done): a non-star bookmark BUTTON (Brave-style) near the address bar → `BookmarksOverlayRoot` dropdown with the current page + toggle-star at top and a searchable list below; Ctrl+D toggles; un-stub menu action (`simple_handler.cpp:~2542`). No horizontal bookmarks bar; defer HTML import/export.
   - **(b) Site-info button** (left, inside the address bar) — Brave-style "view site information": cookies/site-data, site settings, connection/cert state. Backends mostly exist (cookie managers, per-site privacy toggles ≈ Privacy Shield, cert state); a full cert viewer needs CEF SSL plumbing. ⚠️ Delineate vs the existing right-side Privacy Shield to avoid redundancy.
   - **(c) Wallet permissions in the site-info dropdown** — reuse the existing modal (`MENU_ID_MANAGE_PERMISSIONS`); KEEP the right-click entry; **rename "Manage Site Permissions" → "Manage Wallet Permissions"** (clearer it's wallet, not too long).
   - **(d) Downloads auto-hide + animation** — restore the prior behavior: no downloads icon until a download starts; animate it into the header with a progress bar; complete-indicator; click to open/clear; cleared → icon disappears (Chrome-like).
   - **(e) Tab-list dropdown** — small caret at the tab-strip edge → open tabs + recent history (TabManager + HistoryManager). ⚠️ macOS: traffic-light buttons sit top-left → inset the caret to their right (study how Chrome positions it on mac).
   - **⚠️ Interaction:** this adds several header/omnibox icons while `B2-FILL` (header doesn't fill its window) and `FEAT-DPI` (multi-monitor hit-testing) are unresolved — the design must map the full header icon inventory + placement (win+mac) DPI-correctly; consider pulling `B2-FILL`/`DPI` investigation alongside.
3. **Header** `B2-FILL` (the visible "React doesn't fill the header window" gap) + `B2-MEASURE`→`B2-SLIM`/`B2-WARM` (slow header load). Header stays React (no C++ port — confirmed).
4. **FEAT-DPI** — multi-monitor mouse-offset / hit-testing across differently-scaled monitors. **Deep-research item** (medium-weight research pass first); coupled with `B2-FILL` (the gap may be DPI rounding).
5. **Auto-update completion** — keystone, must ship IN this release so the NEXT release validates A→B updating: Windows DSA→EdDSA (WinSparkle 0.9.x), silent install-on-quit, decouple appcast publish from build. (`AUTO_UPDATE.md`: PIPE-DSA / PIPE-SILENT / PIPE-APPCAST.)
6. **CI test-gate (Track E)** — wire the reusable `_tests.yml` (rust+adblock+**cpp** `hodos_tests`+frontend+security) into `release.yml` so the Shell release is test-gated. **cpp-CI wiring DEFERRED to this Track E push** (needs CEF binaries + vcpkg in the runner; tests pass locally meanwhile, 19 green).
7. **Ship Shell release** (push `main` → `release` repo, tag, signed CI build).

**Milestone 2 — Engine release (later):** CEF M149 bump → Blink farbling (B1) → B1-VERIFY → ship. This is the existing "CEF long-pole" (§3), now explicitly *after* the Shell release.

> This re-sequences the original §3 wave graph: Wave-2 items (B2 optimization, auto-update) and the macOS parity work are pulled **forward** into the Shell release; the CEF long-pole is pushed **after** it. The §3 graph below is retained for the dependency detail but is superseded on *ordering* by this section.

---

## 1. The implementation harness (per-chunk lifecycle)

Every work-chunk — large or small — runs the same rails. Steps 2 and 5 are the verification gates.

1. **Kickoff review** — re-read the phase doc; **re-verify every cited `file:line` against current source** (catches drift).
2. **Design → adversarial DESIGN review** *(gate 1 — ALWAYS)*. A skeptic agent challenges the approach *before any code*. Cheapest place to be wrong. Highest ROI gate.
3. **Narrow implementation** — one tightly-scoped agent or interactive. No broad/ambiguous scopes.
4. **Build + unit tests** written *with* the code (cargo / GoogleTest / Vitest).
5. **Adversarial CODE review** *(gate 2 — proportional to risk, see tiers below)*. Independent reviewer + skeptic; re-verify claims against source; reuse/simplify pass. *Before commit.*
6. **macOS-parity capture** — append the Mac delta to `MACOS_PORT_0_4_0.md` (§6).
7. **Smoke** if browser-core changed (minimal basket: youtube / x / github).
8. **Commit.**

### Code-review risk tiers (gate 2 sizing)
| Tier | Examples | Gate-2 depth |
|------|----------|--------------|
| **Heavy** | crypto/signing, CEF lifecycle, auto-update signing, B1 farbling, B2 origin attribution, anything touching money/keys | Multi-skeptic adversarial (3+ independent, refute-by-default) + live smoke |
| **Medium** | new wallet endpoints, permission gates, IPC paths | 1 reviewer + 1 skeptic |
| **Light** | bookmarks UI, doc edits, mechanical refactors, log-level changes | 1 reviewer pass |

> **Design review (gate 1) is ALWAYS run regardless of tier** — it's the cheap one that saves the rework loop.

---

## 2. Workflows vs. interactive — hybrid

- **Use `Workflow` for:** the verification gates (reviewer/skeptic/referee), independent fixes across separate files, mechanical sweeps, audits, the mac-parity sweep. Fan-out, no human-in-loop needed. Use `isolation: worktree` when parallel agents mutate files.
- **Keep in the interactive main loop:** stateful, build-gated implementation — CEF lifecycle, crypto, the B1 patch→rebuild→test loop. These need live build/test/debug iteration + owner judgment.
- **Rule:** workflows are the *verification spine + parallel chunks*; the interactive loop carries the *load-bearing sequential work*. Never expect a workflow to autonomously implement CEF correctly.
- **Cost/throttle:** bounded/medium concurrency (the ~100-agent deep-research harness hits server-side throttling). Resumable.

---

## 3. Dependency graph + waves

```
WAVE 0 (first, fast)
  └─ Phase-0 secret fixes: F1 (std::cout mnemonic), F2/F3 (log::info secrets)

WAVE 1 (parallel tracks — run concurrently)
  ├─ Track A: Audit fixes F5/F6/F7/F9        (independent files → parallel agents)
  ├─ Track B: B2 iframe-origin fix           (self-contained C++; smoke now, re-verify post-bump)
  ├─ Track C: FEAT-B3 bookmarks UI           (frontend, independent)
  ├─ Track D: FEAT-B2-MEASURE                (header perf) ──gates──> B2-SLIM/WARM (Wave 2)
  ├─ Track E: CI/CD pipeline scaffolding     (shared workflow_call, pin GoogleTest,
  │                                            tarpaulin-on-Linux, Vitest setup)
  └─ ⚠ F4 parking_lot                        (SOLO, SEQUENTIAL — 253 AppState touch-points;
                                               do NOT run parallel with other Rust edits)

CEF LONG-POLE (start EARLY, parallel with Wave 1; mostly sequential within)
  A1 patch toolchain (greenfield)
     └─> CEF bump → stable M149 (Q16; closes M136 zero-patch gap + targets B1's patch base)
            ├─ build-config/file-manifest DRIFT AUDIT (CEF_BUILD_RUNBOOK Step 5.5; automate via A1)
            └─> FEAT-B1 Blink farbling
                   └─> B1-VERIFY (CreepJS/browserleaks + cross-session login, Win first)
                          └─> remove JS-injection farbling
                                 └─> shrink auth exclude-list (verify-then-decide)

WAVE 2 (gated)
  ├─ FEAT-B2-SLIM / B2-WARM / B2-FILL / FEAT-DPI   (after B2-MEASURE)
  └─ Auto-update: WinSparkle 0.9.x EdDSA (Q9) + appcast-decouple/Apple-soften (Q13)
                                                   (after Track E exists — wires into release.yml)

SEPARATE TRACK (own kickoff)
  └─ Broadcast/Explorer: VERIFICATION REVIEW FIRST (not blind) → fresh TAAL key → implement + Arcade
       ⏰ TAAL ARC key re-check trigger lives here
       🕹 Live Arcade v2 endpoints found (2026-06-17 Slack) + batch=NON-atomic contract → see BROADCAST_AND_EXPLORER_REVIEW "Live Arcade endpoints + batch semantics"

══════════════ CONVERGENCE GATE (synchronous) ══════════════
  ALL of the above green → run the WHOLE thing through the NEW pipeline
     → INTERNAL BETA build  0.3.x-beta  (PRIVATE; first real pipeline exercise)
        → PHASE 3 ordinals (1Sat)
           → PUBLIC  0.4.0-beta  (second pipeline exercise; main → release remote)
```

Legend: everything above is **Windows-first**. macOS deltas captured continuously into `MACOS_PORT_0_4_0.md`.

---

## 4. Parallel / sequential / gated — explicit

| Item | Can run parallel with | Blocked by / gates |
|------|----------------------|--------------------|
| Phase-0 F1/F2/F3 | each other | nothing (do first) |
| Audit F5/F6/F7/F9 | each other, B/C/D/E, CEF track | nothing |
| B2 iframe fix | A/C/D/E | re-verify after CEF bump |
| FEAT-B3 bookmarks | A/B/D/E | nothing |
| FEAT-B2-MEASURE | A/B/C/E | **gates** B2-SLIM/WARM |
| CI/CD scaffolding (Track E) | A/B/C/D | **gates** internal-beta build + auto-update wiring |
| **F4 parking_lot** | **nothing** (solo) | own kickoff; conflicts with Rust edits |
| A1 patch toolchain | Wave 1 | **gates** CEF bump |
| CEF bump → M149 | Wave 1 | A1 toolchain; **gates** B1 |
| FEAT-B1 farbling | (nothing — long pole) | CEF bump |
| B1-VERIFY | — | B1 implemented |
| remove JS farbling | — | B1-VERIFY **passes** (never leave a no-protection gap) |
| Auto-update (Q9/Q13) | Wave 1 features | Track E |
| Broadcast/Arcade | Wave 1/2 | verification review + fresh TAAL key |
| Internal beta build | — | **ALL** fixes+features+tests green + pipeline functional |
| Phase 3 ordinals | — | internal beta validated |
| Public 0.4.0-beta | — | ordinals done |

---

## 5. Test placement

- **Per chunk:** unit tests with the code (cargo test / C++ GoogleTest / Vitest) + gate-2 adversarial review.
- **Per browser-core change:** minimal smoke (youtube / x / github).
- **B2:** cross-origin-iframe smoke — approved page embeds a cross-origin iframe firing raw `fetch` to `localhost:31301/createAction`; confirm the **iframe's** origin is attributed, not the top page.
- **B1:** CreepJS / browserleaks + **cross-session login** test on the auth basket (Win first, Mac in Mac-sprint).
- **Convergence:** full new pipeline (unit + integration + coverage) + **standard→thorough** smoke basket (all categories).
- **Per-CEF-bump regression (standing P&P):** re-verify B2 frame/origin attribution + codecs (§11.3) + farbling patches re-apply.

---

## 6. Windows-first + macOS plan

- **`MACOS_PORT_0_4_0.md`** (repo-committed): every Windows chunk's step 6 appends its Mac delta — *"Windows changed X at `file:fn`; Mac equivalent `file_mac.mm:fn` needs Y."* Grounded in the prior macOS-parity review (which caught the `TabManager_mac::CloseTab` gap).
- **"macOS watcher" pattern (honest framing):** no truly always-on agent. Simulated by (a) mandatory mac-parity capture in every chunk + (b) a **periodic mac-parity sweep workflow** that reviews recently-landed Windows changes and fills the port doc.
- **Mac sprint:** pull `MACOS_PORT_0_4_0.md`; an agent implements straight from it, then Mac smoke + Mac B1-VERIFY.

---

## 7. CEF target (Q16 resolved 2026-06-17)

- **Target: CEF stable `149.0.3` / Chromium M149** (`149.0.7827.115`). Verified live from `cef-builds.spotifycdn.com/index.json`, 2026-06-17.
- **CEF has NO LTS / Extended-Stable channel** — only `stable` + `beta`. The §11.1 "pin to CEF LTS (M150)" recommendation rested on a false premise; **corrected**. Conservatism comes from pinning a specific stable milestone + pulling its security point-releases, *not* from an LTS channel that doesn't exist.
- **Why pulled early:** M136 has **zero security-patch coverage today**, and B1's Blink patches must target the build we ship — don't author them against M136 then redo them.
- **Optional future:** if extended-stable conservatism is ever wanted, self-build CEF from a *Chromium* extended-stable branch (extra toolchain work — note only, not now).
- **Distributed-build (Siso + third-party REAPI, not reclient)** — deferred; not 0.4.0.

---

## 8. Adversarial verification standard (applies to both gates)

- **Refute-by-default** — skeptics try to *kill* the finding/design; survive only if it withstands.
- **Ground every claim in source** — cite `file:line`, re-verify it exists *now*. No hallucinated/drifted locations.
- **Root-cause collapse** — "N instances → 1 fix (mechanism)", never N line-items.
- **Severity from reachability**, not pattern.
- **Declare coverage** — state what was *not* checked; no silent caps.
- **Owner makes the final cut** — the harness never auto-drives the sprint.

---

## 9. Open / deferred

- **Q16 sub-part (distributed build / Siso+REAPI)** — deferred beyond 0.4.0.
- **Broadcast/Explorer** — verification review BEFORE any implementation (see `BROADCAST_AND_EXPLORER_REVIEW.md` "Owner notes 2026-06-17"); add Arcade this sprint. ⏰ Re-check TAAL ARC API + get a fresh key when this track starts.
- **F4 parking_lot kickoff timing** (master-plan Q2/item-2) — confirm no handler relies on poison-as-circuit-breaker before porting.
- **Reorder freely** — this plan is a living doc; adjust waves as reality demands and record why (Invariant #12).
