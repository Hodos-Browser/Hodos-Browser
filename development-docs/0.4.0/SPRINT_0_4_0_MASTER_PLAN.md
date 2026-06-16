# Sprint 0.4.0 — Unified Master Plan

> **Status:** 🚧 DRAFT — design + deep-research session in progress. No source code written yet.
> **Branch context:** authored on `feature/brc121-phase1`; Phase 2.6 closed (permission/payment decisions are Rust-authoritative); **Phase 3 (ordinals) intentionally deferred until AFTER this sprint.**
> **Date started:** 2026-06-15
> **Owner:** Matthew Archbold (Marston Enterprises / Hodos)

This is the single top-level plan unifying three work-streams: **HelicOps audit remediation**, **0.4.0 features/refactors**, and the **build/test/release pipeline (DevOps-CICD)**. It is designed to be **extensible** — add new work items to the inventory table with a new `ID`, wire their dependency edges, and slot them into the phase ordering.

---

## 0. How this document was built (provenance)

- **Kickoff verification:** every load-bearing claim in the source docs was checked against current source (docs are known-stale — see §8). Where a doc and the code disagreed, **the code wins** and the divergence is logged in §8.
- **Internal read:** 14 parallel reader agents covered all ~37 sprint docs, each returning a structured summary + source-verification.
- **Synthesis:** one synthesis pass produced the inventory, dependency edges, overlap matrix, and ordering hypothesis below.
- **External research:** 7 deep-research fans (industry best practice) — §7. **In progress, one at a time.**

**How to extend:** new work item → add a row to §3 with a stable `ID` → add any dependency edges to §4 → place it in a phase in §6 → if it needs external best-practice, add a research bundle to §7.

**Status legend:** `done` (verified in source) · `in_progress` · `pending` · `stale` (doc claim contradicted by source) · `unknown`.
**Size legend:** XS / S / M / L / XL (rough effort).

---

## 1. Goals

1. **Close the real security gaps** from the HelicOps audit — starting with the confirmed-live secret-to-disk leaks (Phase 0).
2. **Ship the 0.4.0 features** — farbling-in-source (B1), header→C++ (B2), bookmarks UI (B3), extensions direction (B4) — re-scoped against the verified STAY-ON-CEF reality.
3. **Stand up a real pipeline** — PR-time CI that actually runs the tests, a test gate before signing installers, mac+windows signing/notarization, and true silent auto-update (today is notify-only).
4. **Do the cross-cutting work ONCE** — testing especially (the audit's coverage gaps and the pipeline's CI gate are the same problem).

---

## 2. The three work-streams at a glance

| Stream | What it is | Headline reality after verification |
|--------|-----------|--------------------------------------|
| **Audit** (HelicOps) | Fix the real findings from the security audit | **"Adjudicated" ≠ "fixed."** All F1–F9 are still `pending`; **both criticals (F1 mnemonic→disk, F2 cert-key→disk) are LIVE in source today.** |
| **Features** (0.4.0) | B1 farbling-in-source, B2 header→C++, B3 bookmarks, B4 extensions | A4 verdict = **stay on CEF**. B1 unblocked (Blink patches). **B3 backend already built — UI only.** **B4 needs re-scope, not "unblock"** (real extensions infeasible on CEF). **B2 is a 31-line stub.** |
| **Pipeline** (DevOps-CICD) | Real CI tests, mac+win signing, silent auto-update | **No `ci.yml` exists.** `release.yml` signs money-handling installers with **zero tests run**. Release build+sign+notarize itself works. Updater is built but **notify-only**; Windows still on **deprecated DSA**. |

---

## 3. Verified work-item inventory (33 items)

### 3.1 Audit stream

| ID | Item | Status | Size | Source evidence | Key risk |
|----|------|--------|------|-----------------|----------|
| **AUDIT-F1** | Mnemonic → disk via `std::cout` in createWallet | `pending` | XS | `cef-native/src/core/WalletService.cpp:440` `std::cout << "🔑 Mnemonic: " << response["mnemonic"]` — VERIFIED live | Crown-jewel leak; `std::cout` is **NOT** protected by Rust `prod=warn`. Patch-release candidate. |
| **AUDIT-F2** | 32-byte cert-field symmetric key logged hex+base64 (+ plaintext value adjacent) | `pending` | XS | `rust-wallet/src/handlers/certificate_handlers.rs:~1729` | Second critical; leak broader than tracker documents. |
| **AUDIT-F3** | ~13 crypto-fragment log sites (ECDH secrets, HMAC scalars, master-key halves, child privkey) | `pending` | S | `brc2.rs:73-75,111-112,150,277,281`; `certificate/verifier.rs:310,316,324`; `handlers.rs:7346`; `certificate_handlers.rs:1464,2280-2284` | `verifier.rs:324` logs **full** HMAC scalar; flexi_logger persists it. |
| **AUDIT-F4** | `std::sync::Mutex` poison-cascade DoS → migrate to `parking_lot` | `pending` | L | `main.rs:194`; `parking_lot` absent from Cargo.toml (VERIFIED); ~253 `.lock().unwrap()` sites | Touches **AppState** (invariant #4); needs own kickoff; serialize vs permission work. |
| **AUDIT-F5** | macOS command injection via raw `profileId` in `system()` | `pending` | S | `cef-native/src/core/ProfileManager.cpp:435-437` | The one genuine cmd-injection; macOS-only; Windows `CreateProcessW` already safe. |
| **AUDIT-F6** | Cross-context JS injection in BRC-100 auth overlay (`escapeJsonForJs` incomplete) | `pending` | M | `simple_render_process_handler.cpp:52-81` (single-quote-only encoder) | Real gaps: `</script>`, U+2028/2029. Shared render handler — cross-platform. |
| **AUDIT-F7** | Path traversal + un-gated `/wallet/backup` | `pending` | M | `handlers.rs:~12513` no `check_domain_approved`; `backup.rs:1643-1667` raw `fs::copy` | Any local process on :31301 can exfiltrate the encrypted-mnemonic DB. Fix = canonicalize **AND** gate. |
| **AUDIT-F8** | Remove `extract_master_key.rs` debug binary + tree-sweep + durable CI grep-gate | `pending` | S | `rust-wallet/src/bin/extract_master_key.rs` (VERIFIED present) | Durable mitigation depends on **PIPE-CI** existing. |
| **AUDIT-F9** | `fields.as_object().unwrap()` per-request panic on malformed body | `pending` | XS | `certificate_handlers.rs:~1688` | Only Low item; optional for 0.4.0. |
| **AUDIT-META** | Package PROC-2 (deliberate-design/safeguard manifest) + PROC-3 (collapsed output, stable anchors) as next-audit-prep | `in_progress` | S | HelicOps FEEDBACK.md + META.md; `cef-native/src/core/CLAUDE.md` exists | REC-1..9 are vendor-directed (external HelicOps team), not Hodos work. |
| **AUDIT-ADJ-DROPPED** | Document why 2 tool-"criticals" (format! DROP TABLE w/ internal value; deliberate TAAL key) fell out of backlog; decide TAAL key rotation | `pending` | XS | `arc_taal.rs:16` key live; DROP TABLE `handlers.rs:2938` | Tool severity is **inverted** (rated DROP-TABLE critical, seed-to-disk merely high). |

### 3.2 Features stream

| ID | Item | Status | Size | Source evidence | Key risk |
|----|------|--------|------|-----------------|----------|
| **FEAT-B1-CUR** | B1 baseline (shipped): JS-injection farbling, per-session token, Canvas/WebGL-readPixels/Navigator/Audio, no screen-res, auth-domain exempt | `done` | L | `FingerprintProtection.h:22-31,40-69`; `FingerprintScript.h:11-15`; inject `simple_render_process_handler.cpp:586-632`; commit b514c30 | **Workers leak raw values** (OnContextCreated never fires for workers) — the headline detection vector. |
| **FEAT-B1-WORKER** | Worker-context farbling hook (quick win before full patch set) | `pending` | M | `B1-farbling-design.md:111-113` | CEF 136 `OnWorkerContextCreated` availability unverified. |
| **FEAT-B1-SEED** | Persistent **per-profile** seed (login fix): HMAC-SHA256(profile_seed, eTLD+1), stored in C++ profile data, passed via cmd-line switch | `pending` | M | `B1-farbling-design.md:11-31`; current source is per-session | **Privacy trade-off:** sacrifices cross-session unlinkability for login stability — needs sign-off. |
| **FEAT-B1-SUPP** | `HodosSessionCache` as Blink `Supplement<ExecutionContext>` (covers workers for free) | `pending` | L | `B1-farbling-design.md:33-40`; **NO patch infra in repo** (VERIFIED) | Greenfield CEF patch toolchain — larger than "refactor." |
| **FEAT-B1-PATCH** | ~5-8 Blink `.patch` files (Canvas2D readback, WebGL getParameter, WebAudio, Navigator) | `pending` | XL | `B1-farbling-design.md:73-96` | **CONFLICT:** design re-adds WebGL vendor/renderer + navigator hwConcurrency/deviceMemory that the JS impl deliberately removed as detectable — reconcile first. License landmine (Brave MPL-2.0 file-copyleft, Bromite GPL-3 forbidden). |
| **FEAT-B1-VERIFY** | CreepJS/browserleaks acceptance + cross-session login test (Win+macOS) | `pending` | S | `B1-farbling-design.md:103-109` | Cross-session login is the load-bearing criterion. |
| **FEAT-B2** | **DECIDED 2026-06-16: KEEP REACT, NO native port.** Header stays a separate CEF subprocess (V8 isolation kept). Scope = React-side optimization + correctness polish (B2-* below). | `pending` | M | rationale §7.4; header `MainBrowserView.tsx` | Native rewrite scrapped — avoids per-OS re-draw + gold-pill fidelity risk. Production build path EXISTS (dev-server URL is dev-only) — no win there. |
| ~~FEAT-B2-INV~~ | ~~Visual-token inventory for native port~~ — **DROPPED** (no native port → no pixel re-reproduction) | `dropped` | — | — | n/a |
| **FEAT-B2-PILL** | Gold-pill payment safeguard — **stays as-is in React** (no longer at risk; native port scrapped) | `done` | — | `TabComponent.tsx:204-232`; `useTabManager.ts:136-189` | No rewire needed. (Sweep stale "green-dot" comments under DOC-STALE-SWEEP.) |
| ~~FEAT-B2-OVL~~ | ~~Overlay-trigger-from-native~~ — **DROPPED** (triggers stay in React) | `dropped` | — | — | n/a |
| **FEAT-B2-MEASURE** | Measure-first: CEF trace to split subprocess-spawn vs bundle-parse vs mount vs IPC, **before** spending on B2-SLIM/B2-WARM | `pending` | S | CEF `CefBeginTracing`/`CefEndTracing` → Perfetto | Tells us whether slim-MUI (parse) or pre-warm (spawn) is the real lever. |
| **FEAT-B2-SLIM** | Slim MUI out of the **header's** critical path — replace its handful of MUI controls + ~11 icons with lightweight CSS/inline-SVG equivalents. **Keep ALL functionality; keep MUI in overlays.** | `pending` | M | header imports `MainBrowserView.tsx:2-23`; MUI chunk `vite.config.ts:12-18` | Risk = visual parity — pin each control's styling exactly + before/after diff. Cuts cold V8 parse cost. |
| **FEAT-B2-WARM** | Pre-warm the header subprocess (create early / keep warm) — security-neutral (same subprocess). **Cross-window shared context is OUT unless a security review clears it** (one subprocess per window). | `pending` | M | header create `simple_app.cpp:176-197` | Separate **pre-warm (safe)** from **context-sharing (security-gated)**; do former, gate latter. |
| **FEAT-B2-FILL** | Header React content doesn't fully fill its CEF window — small gap vs the webview. Likely CSS (html/body margin, container ≠100%/box-sizing, stray border) **or DPI-rounding**. Investigate WITH FEAT-DPI. | `pending` | S | `MainBrowserView.tsx` layout + header HWND `cef_browser_shell.cpp:~3528` | May be a DPI-rounding artifact at non-100% scale. |
| **FEAT-DPI** | Multi-monitor DPI — overlay **mouse-offset** after moving between differently-scaled monitors (resize improved, hit-testing still off). **Deep-research item** (tried before, not to standard). | `pending` | L | overlay mouse mapping `SendMouseClickEvent`/`SendMouseMoveEvent`; `WM_DPICHANGED`; macOS NSPanel backing-scale | Suspects: Per-Monitor-V2 awareness, `device_scale_factor` to CEF, overlay screen-rect→DIP math, CEF mouse-event coordinate space. Cross-platform. Run a focused medium-weight research pass at this phase. |
| **FEAT-B3** | Bookmarks **UI only** (backend 100% built: SQLite `BookmarkManager` + ~10 IPC + JS bridge + import). **Plan:** (1) add a **bookmark/star icon to the header toolbar**; (2) build **`BookmarksOverlayRoot.tsx`** as a dropdown overlay (search + folders + tags + add/edit/delete) — copy the **`DownloadsOverlayRoot` pattern**; (3) add the C++ show/hide overlay HWND + handler (inherits mouse-hook close); (4) **un-stub the menu action** at `simple_handler.cpp:2542-2544` to open it; (5) **"star current page"** toggle via `bookmark_is_bookmarked`/`add`/`remove`; (6) reuse `ProfileImporter` for import/export. *Optional/stretch:* bookmark bar (`showBookmarkBar` setting already exists — ties to B2-FILL/DPI). **Styling:** survey Chrome/Brave/Vivaldi/Edge bookmark UX → make it **familiar & intuitive** (a "normal" bookmarks experience), with a tasteful unique-Hodos touch (e.g. a custom gold star/bookmark mark). Do NOT build a store. | `in_progress` | M (easy end — pure UI wiring) | `BookmarkManager.h:30-119`; IPC `simple_handler.cpp:5880-6102`; TODO stub `:2542-2544`; bridge `initWindowBridge.ts` + `types/bookmarks.d.ts`; zero management-UI consumers | Doc STALE ("check for store" already answered yes). Lowest-risk feature — good early win. Bookmark **bar** rendering touches header layout → coordinate with FEAT-B2-FILL/FEAT-DPI. |
| **FEAT-B4** | Extensions — **DEFER to a dedicated future sprint (next-quarter+), its own initiative.** Spike `w8wzfq63e` (2026-06-16) verified: target = **"Vivaldi model"** — run the **full Chrome runtime** (extension *subsystem* comes free: MV3 SW, permissions, declarativeNetRequest, content scripts) **+ patch the extension-*UI* layer** (`chrome/browser/ui`: ToolbarActionsModel, ExtensionsToolbarContainer, action popups, `chrome://extensions` WebUI) to render in OUR header. ⚠️ **CORRECTION:** this does NOT cheaply ride the farbling patch toolchain — the UI patches are an **order of magnitude bigger + far more rebase-fragile** (churny `chrome/browser/ui` every milestone) = a **standing per-bump maintenance job** (Vivaldi runs a dedicated team). A **curated allowlist** (`component_extensions_auto_granted=false`) cuts the compat-matrix + security/testing surface but **NOT the one-time UI-patch build cost**. **0.4.0 stance: NO third-party extension hosting** (also moots competing BSV wallet plugins — can't install; `window.yours` lock authoritative). | `deferred` | XL (fork-level) | spike `w8wzfq63e`; §7.5; `CWIShimScript.h` lock | **Security:** a content script w/ host perms can read/modify the page → could intercept/spoof our wallet provider — needs curation + hardening. Heavy compat testing. EIP-6963 only if EVM coexistence ever in scope (BSV has none). |
| **LOG-CONSOLIDATE** | Unify Rust + CEF C++ + frontend log sinks; replace `std::cout`→debug_output.log | `pending` | L | doc Part 5 #2; no consolidation code | Touches CefSettings/CEF lifecycle (invariant #8). Do AFTER F1. |
| **LOG-POLICY** | Error-logging specificity convention + prod log policy + optional structured/JSON logging | `pending` | L | doc Part 5 #3-5; doc-only | Largest raw audit cluster (~245 findings) — scope-creep risk. Pairs with F4. |

### 3.3 Pipeline stream

| ID | Item | Status | Size | Source evidence | Key risk |
|----|------|--------|------|-----------------|----------|
| **PIPE-CI** | Create `ci.yml` PR/push gate: rust check/test/clippy, adblock test, cpp ctest, frontend e2e, cargo audit + npm audit | `pending` | L | **VERIFIED no ci.yml** in `.github/workflows/`; release.yml test-grep = 0 | **#1 pipeline gap** — no PR-time CI at all. C++ tests need network for FetchContent. |
| **PIPE-TESTGATE** | Add test gate to release.yml (`needs:`/workflow_call) so installers can't ship on red | `pending` | M | release.yml build-windows has no `needs:` | The convergence of audit-coverage-gaps and CI-gates — ONE strategy. |
| **PIPE-A7** | Test strategy + census + trust/auditability (census stale; pyramid inverted) | `pending` | L | TRIAGE.md:74-80; verified ~71 integration + 409 inline Rust, 39 cpp (2 files), 23 adblock, 54 frontend e2e, **0 vitest** | Decide build-Vitest-or-retire. tarpaulin on Windows unreliable. |
| **PIPE-RELEASE** | Tag-triggered dual-platform build+sign+publish (works) | `done` | L | `.github/workflows/release.yml` (only workflow) | Ships signed money-handling binaries with **ZERO test gate**. Bad appcast auto-deploys to live feed. |
| **PIPE-VERSION** | Single-source version bump across 5 manual sources | `pending` | S | AboutSettings + .iss = 0.3.0-beta.15; Cargo.toml = 0.3.0; CMakeLists = 0.2.0-dev | 3 versions live simultaneously — proves drift. |
| **PIPE-DSA** | Windows DSA→EdDSA migration for appcast signing (3 sites) | `pending` | M | `AutoUpdater.cpp:94-120`; `generate-appcast.py:59`; `release.yml:177-193` | macOS already EdDSA, Windows still DSA (deprecated) for a wallet app. Needs WinSparkle 0.9.x. |
| **PIPE-SILENT** | A6 silent auto-update (the goal) — install-on-quit, silent installer | `in_progress` | M | `AutoUpdater.cpp/.mm` + WinSparkle in CI; no `/SILENT` in .iss; no `SUAutomaticallyUpdate` | Per-user install dir done (.iss:26,36). Settings UI offers only on/off. |
| **PIPE-APPCAST** | Fix appcast: `sparkle:channel` beta regression (added 24b2522, dropped 2eda476), decouple generate/push from build | `pending` | S | `generate-appcast.py` emits no channel; `release.yml:893-907` push is `\|\| true` | Broken channel filter would mis-target beta users. |
| **PIPE-IDENTITY** | Align signing identity to org; harden secret-missing failure | `pending` | S | `release.yml:493,661` "Matthew Archbold"; `:184,:715` warn-and-skip on unset secret | Personal-name identity for a company product; misconfigured secret → unsigned release without hard fail. |
| **PIPE-A1** | CEF self-build runbook + caching (sccache) + latest-stable sourcing + drift detection | `pending` | XL | `A1_BUILD_STRATEGY.md` research-only; sccache grep = 0; `build_hodos_cef.bat:60 --branch=7103` (~6mo behind M136) | GitHub runners can't do full Chromium build (disk + 6hr). The 2-week-build core pain unsolved. **Highest-value missing process.** |
| **PIPE-A4** | Brave-fork feasibility — **DECIDED: stay on CEF** | `done` | S | `BRAVE_FORK_FEASIBILITY.md`; DevOps README:49; build scripts confirm proprietary_codecs=true | B4 surfaces assuming extension support built on false premise. |
| **OPS-WSL** | WSL hybrid workspace migration (execute POST-sprint) | `pending` | L | `WSL_HYBRID_WORKSPACE.md`; no `.workspace-role` file | Doc-gated to post-sprint. Credential/secret storage unresolved (git-crypt vs out-of-repo). |

### 3.4 Cross-cutting

| ID | Item | Status | Size | Source evidence | Key risk |
|----|------|--------|------|-----------------|----------|
| **LOG-INFRA** | flexi_logger persistent file logging + rotation + prod=warn (LANDED) | `done` | M | `Cargo.toml:50` flexi_logger=0.29; `main.rs:133-159`; commit 5c4a61b | prod=warn **hides** but does NOT remove secrets; **widened F2/F3 blast radius**. |
| **DOC-STALE-SWEEP** | Documentation reconciliation pass (see §8) | `pending` | S | see §8 | Stale docs drove false "fixes done" / "CI gate exists" impressions. |
| **TEST-STRATEGY** | Canonical cross-stack testing strategy doc (the audit↔CI overlap, done once) — census, pyramid, CI gating, coverage, anti-gaming, secret-log gate | `done` | S | `DevOps-CICD/TESTING.md` (created 2026-06-16) | Home for PIPE-A7; the CI workflow it defines = PIPE-CI / PIPE-TESTGATE. |
| **TEST-HARNESS** | Capped test-wallet harness for **agent-run live e2e** — `HODOS_DEV` + tiny test wallet + **low Rust-enforced spending caps** + domain allowlist + gold-pill audit; agents run the Playwright harness, **not** raw wallet access | `pending` | M | design in `DevOps-CICD/TESTING.md` §9; reuses existing domain-permission/spending caps | Defense-in-depth: cap at wallet **+** harness **+** allowlist. Confirm testnet vs mainnet-small. Enables safe agent-driven live verification. |

---

## 4. Dependency + overlap map

### 4.1 Dependency diagram (ASCII)

```
PHASE 0 (urgent, pre-sprint patch)
  F1 ─┐
  F2 ─┼─► F8 (sweep + binary removal) ──needs──► PIPE-CI (durable grep-gate)
  F3 ─┘
  F1 ──must-precede──► LOG-CONSOLIDATE

AUDIT (parallel)            PIPELINE FOUNDATION (start early, overlaps audit)
  F5  F6  F7  F9              PIPE-A7 ──► PIPE-CI ──enables──► PIPE-TESTGATE ──► PIPE-RELEASE
  F4 (parking_lot)            PIPE-DSA ─┐
   └─shares─ LOG-POLICY       PIPE-SILENT ┼─ (one updater+signing pass)
                              PIPE-APPCAST┘
                              PIPE-IDENTITY ──must-precede──► PIPE-RELEASE
                              PIPE-VERSION (quick win)

FEATURE DECISIONS            HEAVY INFRA (longest lead, last)
  FEAT-B2-MEASURE ──► FEAT-B2 ──shares──► FEAT-B4    PIPE-A1 ──blocks──► FEAT-B1-PATCH
       (joint B2+B4 session)                          FEAT-B1-SEED ─► FEAT-B1-SUPP ─► FEAT-B1-PATCH ─► FEAT-B1-VERIFY
  FEAT-B3 (UI only — ship anytime)                    PIPE-A4 ──enables──► {B1-PATCH, B4, B2}
  FEAT-B1-WORKER (quick win)                          ⚠ F4 and B1-PATCH are both L/XL cross-cutting — DO NOT run concurrently
```

### 4.2 Key dependency edges

- `F8` **must-precede-by** `F1/F2/F3` (sweep catches the sink shape + sibling re-introductions).
- `PIPE-CI` **enables** `F8`'s durable grep-gate (regression guard is blocked on CI existing).
- `LOG-INFRA` **shares-work-with** `F2/F3` (persistent logging *is* the amplifier; deletion is the cure).
- `PIPE-A4` **enables** `FEAT-B1-PATCH`, **re-scopes** `FEAT-B4`, **frames** `FEAT-B2`.
- `PIPE-A1` **blocks** `FEAT-B1-PATCH` (no Blink patch can build without the CEF patch toolchain).
- `FEAT-B2-MEASURE` **must-precede** `FEAT-B2`; `FEAT-B2` **shares-work-with** `FEAT-B4` (toolbar ownership) and `FEAT-B3` (bookmark bar).
- `PIPE-A7` **must-precede** `PIPE-CI` **enables** `PIPE-TESTGATE` **must-precede** `PIPE-RELEASE`.
- `F4` (parking_lot) and `FEAT-B1-PATCH` are both L/XL cross-cutting — **serialize** vs each other and vs residual permission work.

### 4.3 Overlap matrix — work to do ONCE

| Overlap topic | Items | Do-once recommendation |
|---------------|-------|------------------------|
| **Testing (headline)** | PIPE-CI, PIPE-TESTGATE, PIPE-A7, AUDIT-F8 | Tests EXIST but nothing enforces them. Build **one reusable test workflow** (`workflow_call`): rust test/clippy + adblock + cpp ctest + frontend e2e + cargo/npm audit + **the F8 grep-gate as a job inside it**. Run at PR time AND as a `needs:` gate before release signs. The audit's "no secret-log regression guard" and the pipeline's "no test gate" are the **same** missing CI. |
| **Secret-to-log scrub** | F1, F2, F3, F8, LOG-INFRA | F1+F2+F3 line-deletions = ONE commit, then F8 sweep+binary-removal, BEFORE any logging-architecture work. F1 (`std::cout`) is highest priority (no log-level protection). **Phase 0 patch.** |
| **Logging arch vs audit error-handling** | LOG-POLICY, AUDIT-F4 | Define the error-handling convention as part of the F4 parking_lot kickoff so the two passes don't double-edit the same call sites (~245-finding cluster). |
| **Farbling-in-source ⟷ CEF toolchain** | FEAT-B1-PATCH, FEAT-B1-SUPP, PIPE-A1, PIPE-A4 | B1's in-source approach **IS** the A1 CEF patch toolchain. Stand it up ONCE under A1; B1 patches are the first consumer. Budget the rebase cadence once. |
| **Brave-fork verdict ⟷ B1/B4** | PIPE-A4, FEAT-B1-PATCH, FEAT-B4 | Same spike, two consequences: unblocks B1, re-scopes B4. Carry both explicitly into the B1 and B4 kickoffs. |
| **Code-signing ⟷ auto-update** | PIPE-DSA, PIPE-SILENT, PIPE-IDENTITY, PIPE-APPCAST, AUDIT-ADJ-DROPPED | One updater+signing pass: bump WinSparkle 0.8.1→0.9.x (needed for BOTH EdDSA and silent), switch Win to EdDSA, fix appcast channel, align identity to Marston, harden secret-missing fail, decide TAAL rotation. |
| **B2 header ⟷ B4 extensions** | FEAT-B2, FEAT-B4, FEAT-B2-MEASURE | CEF Chrome-runtime (only real-extension path) forces Chrome's toolbar, colliding with a custom header. **Single joint B2+B4 decision session**, fed by measure-first. |
| **Documentation reconciliation** | DOC-STALE-SWEEP + §8 | Fold each doc fix into the stream that first touches that doc; track so misreadings don't persist. |

---

## 5. Phase 0 — pre-sprint secret-scrub patch (decided)

**Decision (2026-06-15): ship a secret-scrub patch release AHEAD of 0.4.0.**

Both criticals are confirmed live in current source (total wallet compromise). Scope is tiny and independent:

1. **F1** — delete the `std::cout << "🔑 Mnemonic:"` line at `WalletService.cpp:440` + sweep createWallet for sibling response-field leaks.
2. **F2** — delete the cert-key hex/base64 log lines at `certificate_handlers.rs:~1729` (+ adjacent plaintext).
3. **F3** — delete/compile-gate the ~13 crypto-fragment log sites.
4. **F8** — remove `extract_master_key.rs` + tree-sweep for siblings.
5. **AUDIT-ADJ-DROPPED** — document why DROP-TABLE/TAAL fell out of backlog; decide TAAL rotation.

> Durable regression guard (CI grep-gate) lands later with PIPE-CI; the deletions ship now regardless.

---

## 6. Proposed phase ordering

> **Your hypothesis (audit → features → pipeline) is PARTIALLY VALIDATED.** It holds only for the secret-scrub. The big revision: **pipeline (CI test gate) must start EARLY and in parallel** — it enables F8's durable guard, gates safe releases, and shares the testing strategy with the audit's coverage gaps. And the biggest features (B1, B2) **depend on** pipeline/build infra (A1), so part of "pipeline" must precede part of "features."

| Phase | Contents | Parallelism |
|-------|----------|-------------|
| **0 — Secret-scrub patch** (urgent) | F1+F2+F3 (one commit) → F8 + binary removal → AUDIT-ADJ-DROPPED + TAAL decision | Tiny, parallel-safe; pre-empts 0.4.0 |
| **1 — Audit remediation** (mostly parallel) | F5, F6, F7, F9 in parallel (independent files). F4 parking_lot as its OWN kickoff (AppState invariant), serialized; pair LOG-POLICY into it. | F5/F6/F7/F9 parallel; F4 serial |
| **2 — Pipeline foundation** (START EARLY, overlaps Phase 1) | PIPE-A7 census → PIPE-CI reusable test workflow → PIPE-TESTGATE on release.yml. Bundle release-crypto pass (PIPE-DSA, PIPE-SILENT, PIPE-APPCAST, PIPE-IDENTITY) + PIPE-VERSION. | Runs concurrently with Phase 1 (different layers) |
| **3 — Feature decisions + early wins** | FEAT-B2-MEASURE → **B2 React optimization (B2-SLIM + B2-WARM)** + correctness polish (B2-FILL, FEAT-DPI deep-research). B4 = EIP-6963 slice (defer hosting). FEAT-B3 (UI only) ships anytime. FEAT-B1-WORKER + FEAT-B1-SEED quick wins. | B3 parallel with everything; B2 is now React-only (no native port) |
| **4 — Heavy infra** (longest lead, last) | PIPE-A1 CEF toolchain + caching + sourcing → FEAT-B1-SUPP → FEAT-B1-PATCH → FEAT-B1-VERIFY. PIPE-SILENT completion. (B2 native header REMOVED — decided keep-React.) | A1→B1 is a serial chain |
| **5 — Continuous** | Silent-update security tests; DOC-STALE-SWEEP folded into each touching stream; OPS-WSL post-sprint | — |

**Serial constraints:** F4 (AppState mutex) ⟂ B1-PATCH ⟂ residual permission work — never concurrent. A1→B1 patches is the longest-lead chain.

---

## 7. External best-practice research (deep-research fans)

> 7 bundles, run **one at a time** (decided 2026-06-15 after rate-limit throttling on the first attempt). Each bundle's findings + citations land here.

| # | Bundle | Status |
|---|--------|--------|
| 1 | CI/CD test-gate architecture + unit/integration/e2e strategy (reusable workflows, hermetic GoogleTest, coverage tooling, secret-log gate, frontend pyramid, test trust) | ✅ **Done** (§7.1). Claims gathered from primary sources; adversarial-vote stage rate-limited (artifact, not refutation) so synthesized from cached claims + primary-source knowledge. |
| 2 | Code-signing + notarization + silent auto-update (WinSparkle EdDSA/0.9.x, Sparkle CVE pins, Omaha vs Sparkle, feed signing) | ✅ **Done** (§7.2) — medium-weight (1 researcher + 8 verifiers, primary sources) |
| 3 | CEF self-build (sccache+MSVC on Chromium 136, version-sourcing/drift detection, remote/cloud build economics) | ✅ **Done** (§7.3) |
| 4 | B2 toolbar rendering prior art (Chrome/Brave/Firefox/Vivaldi; OSR-chrome surface; paint-latency decomposition methodology) | ✅ **Done** (§7.4) |
| 5 | B4 extension feasibility on current CEF (chrome.* subset, MV3 SW status, Chrome-runtime toolbar cost, EIP-6963) | ✅ **Done** (§7.5) |
| 6 | Farbling techniques (per-profile vs per-session threat model, fingerprint-chromium model, readPixels, Blink-level detectability, licensing) | ✅ **Done** (§7.6) |
| 7 | Misc (parking_lot poison cure, per-frame origin/CORS in CEF, single-source version-bump tooling, WSL secret storage) | ✅ **Done** (§7.7) |

**All 7 research fans complete.** Method note: fan 1 = synthesized from cached primary-source claims (heavy harness's vote stage was rate-limited); fans 2–7 = medium-weight (1 bounded researcher + skeptic-verified load-bearing claims, max-3 concurrency). Total research cost ≈ 3.3M tokens across fans 2–7 — a fraction of the heavy harness's failed runs.

### 7.1 Fan 1 — CI/CD + testing strategy

> Sources: GitHub Actions reusable-workflows docs (primary), github.blog reusable-workflows (primary), gitleaks repo (primary), taiki-e/cargo-llvm-cov + xd009642/tarpaulin + rustprojectprimer (primary/tool docs), plus Earthly/Incredibuild best-practice blogs. Verification panel was rate-limited; claims are from primary sources and corroborated by my own knowledge.

**Q1 — Test-gate architecture (reusable workflow, shared PR↔release).**
Yes — the canonical "installers can't ship on red" pattern is **one reusable test workflow** (`on: workflow_call`) consumed by BOTH a PR-time `ci.yml` and the tag-triggered `release.yml`, with the build/sign jobs declaring `needs: [test]`. Don't duplicate the test jobs. Wiring shape:

```yaml
# .github/workflows/_tests.yml  (reusable — the single source of truth)
on: { workflow_call: {} }      # callable; can also add pull_request to self-trigger
jobs:
  rust:     { runs-on: ubuntu-latest, steps: [cargo test, clippy -D warnings] }
  adblock:  { runs-on: ubuntu-latest, steps: [cargo test] }
  cpp:      { strategy: { matrix: { os: [windows-latest, macos-latest] } }, steps: [ctest -DHODOS_BUILD_TESTS=ON] }
  frontend: { runs-on: ubuntu-latest, steps: [vitest run, playwright test] }
  security: { runs-on: ubuntu-latest, steps: [cargo audit, npm audit, gitleaks, secret-log grep-gate] }

# ci.yml (PR gate)
on: { pull_request: {} }
jobs: { test: { uses: ./.github/workflows/_tests.yml, secrets: inherit } }

# release.yml (tag → build only if tests pass)
jobs:
  test:           { uses: ./.github/workflows/_tests.yml, secrets: inherit }
  build-windows:  { needs: [test], ... sign ... }   # never runs if test job failed
  build-macos:    { needs: [test], ... sign+notarize ... }
```
Notes: reusable workflows are invoked at **job level** via `uses:`, take `with:` inputs + `secrets: inherit`, expose results via `needs.<job>.outputs.*`; nesting capped at 4 deep / 20 calls. Make the test workflow a **required status check** on the protected branch so it can't be skipped.

**Q2 — Hermetic C++ GoogleTest.** This project **already uses vcpkg** (release.yml installs nlohmann/sqlite3 via vcpkg). Best practice = **add GoogleTest through vcpkg with a pinned `vcpkg.json` baseline** (fully hermetic, version-locked, cached) rather than network FetchContent at configure time. If staying on FetchContent: pin an exact `GIT_TAG` (not a branch) + `actions/cache` the `_deps` dir + set `FETCHCONTENT_FULLY_DISCONNECTED=ON` in CI after first cache. Trade-off: vcpkg = one dependency story already in the repo (preferred); pinned-FetchContent = less infra but still hits network on cache-miss.

**Q3 — Rust crypto coverage.** **`cargo-tarpaulin` is Linux-x86_64-centric** — its ptrace backend is Linux-x86_64-only and Windows support is unreliable/absent in practice. Use **`cargo-llvm-cov`** (LLVM source-based via `-C instrument-coverage`; works on Windows MSVC + macOS + Linux; precise line/region/branch; native gating via `--fail-under-lines/regions/functions`). Recommended: run coverage on a **Linux runner** for stability and gate there; the Win/macOS matrix runs the tests themselves. Thresholds: **crypto/signing/key-derivation = very high (≥90%, ideally near-100% line+branch)**, general code **~70–80%**. Coverage is a *signal*, not a target — pair with mutation spot-checks on the crypto modules to avoid coverage theater.

**Q4 — Secret-in-logs gate.** Use **both tools, both layers**:
- **gitleaks** with a custom `.gitleaks.toml` `[[rules]]` (regex for key/seed/mnemonic/privkey near `log::`/`println!`/`std::cout`) — official `gitleaks/gitleaks-action` for CI + `.pre-commit-config.yaml` rev-pinned for local.
- **plus a custom ripgrep gate** (cheap, project-specific) targeting the exact sink shapes (e.g. `std::cout.*mnemonic`, `log::info!.*private`). This is the F8 durable mitigation.
- **Pre-commit is bypassable** (`SKIP=` env) → it's a convenience, **the CI job is the enforceable gate** (required check). Use `.gitleaksignore` fingerprints / inline `#gitleaks:allow` for false positives.
- **Compile-time:** gate crypto-debug logging behind a dedicated cargo **feature flag** (e.g. `crypto-debug-logs`) that is **off by default and never enabled in release**, or `#[cfg(debug_assertions)]`. Feature flag is preferred (explicit, greppable, can't be flipped by an optimized debug build).

**Q5 — Frontend pyramid.** For a **thin UI that delegates all logic to the Rust backend**, an e2e-heavy posture is defensible — but the right move is a **thin Vitest layer for the pure logic that does live in the frontend** (formatters, validators, `DomainPermissionForm` validation, hooks), which is cheap and fast, while keeping Playwright for flows. Don't chase coverage on presentational components. Modern guidance (Testing Trophy) favors integration-leaning tests over isolated unit tests for UI; the "inverted pyramid" is acceptable *only* when business logic genuinely isn't in the client. **Decision for PIPE-A7:** add a small Vitest layer for the handful of logic-bearing modules; formally scope-down (not retire) `DevOps-CICD/TEST_PLAN.md` §3 to those.

**Q6 — Test trust / anti-gaming.** Required status checks on protected branches (tests can't be skipped to merge); **ban `continue-on-error`/silent retries** on security-critical jobs; **fail CI on skipped/ignored tests** in crypto paths (e.g. deny `#[ignore]` in those modules via a grep gate); quarantine flaky tests **visibly** (tracked issue + dashboard) rather than auto-retry-until-green; treat coverage as a signal subject to Goodhart, backed by mutation testing on crypto. This mirrors how Bitcoin Core / Brave gate (mandatory CI, deterministic/reproducible builds, multi-reviewer requirements). **No silent caps:** if CI samples or skips anything, it must be logged.

### 7.2 Fan 2 — Code-signing + notarization + silent auto-update

> Method: medium-weight (1 bounded researcher, ≤5 fetches, primary sources; 8 load-bearing claims each independently skeptic-verified). Sources: WinSparkle NEWS/README/winsparkle.h (primary), Sparkle customization/publishing docs (primary), NVD + GitHub Security Advisories (primary), Microsoft SmartScreen/code-signing docs (official), Inno Setup + MS Restart Manager docs (official). Verifier verdicts noted inline.

**Windows EdDSA migration (PIPE-DSA) — confirmed, it's a must.**
WinSparkle **0.8.1 is DSA-only**; EdDSA (Ed25519) was added in **0.9.0** (latest **0.9.3**). DSA is explicitly **deprecated and "will be removed in a future version."** → **Upgrade WinSparkle to 0.9.3**, call `win_sparkle_set_eddsa_public_key()`, generate/sign with the bundled `winsparkle-tool`. For a money-handling app this is a security requirement, not optional. *(Both claims verifier-CONFIRMED against WinSparkle NEWS + README.)*

**Windows silent update (PIPE-SILENT) — sharper than the doc assumed.**
⚠️ Verifier correction: `win_sparkle_check_update_without_ui()` (exists since **0.4**, not 0.6) **is NOT fully UI-less — it still shows the "update available" window.** WinSparkle has **no native install-on-quit** (open feature request, issue #21). True zero-UI Windows update therefore requires:
- run the Inno installer with **`/VERYSILENT /SP- /SUPPRESSMSGBOXES`** as the update arguments;
- configure Inno **`AppMutex` + `CloseApplications=yes`** so the running CEF browser **and all child subprocesses** are terminated (via Windows Restart Manager) before file replacement — otherwise a locked EXE forces a reboot;
- **per-user install** (already configured) correctly avoids UAC, which is essential for unattended updates. *(Verifier CONFIRMED the Inno/Restart-Manager mechanism; the WinSparkle-flag overstatement is the corrected part.)*

**macOS silent update (PIPE-SILENT) — native, but gated by an unpatched CVE.**
Set `SUAutomaticallyUpdate=YES` + `SUEnableAutomaticChecks=YES` in Info.plist; Sparkle downloads in background + installs on quit. Keep `SUVerifyUpdateBeforeExtraction` on (needs EdDSA). *(Verifier CONFIRMED.)*
- ✅ **CVE-2025-0509** (pre-2.6.4 signature-bypass): current **2.9.0 is safe** (fixed in 2.6.4) — keep ≥2.6.4. *(CONFIRMED via NVD.)*
- 🚨 **CVE-2026-47122** (AppInstaller XPC accepts unvalidated connections → spoofed appcast-item injection): **affects ≤ 2.9.1 and "Patched versions: None" as of the 2026-05-19 advisory.** *(Verifier CONFIRMED via official Sparkle GHSA-g3hp-f6mg-559v; also debunked a web snippet that falsely claimed 2.9.0 fixed it.)* **→ Enabling macOS silent install widens the blast radius of an unpatched installer-stage CVE. Gate: wait for / monitor the patch, or accept-with-mitigation, before flipping `SUAutomaticallyUpdate=YES`.** New open question (§9).

**Updater choice — confirmed: stay on Sparkle/WinSparkle.** Omaha 4 is not realistic for a small team (weeks of C++ fork work, or ~€12k/OS + ~€399/mo commercial). *(CONFIRMED.)*

**Code signing — current setup is best-in-class; one identity action.**
- **Windows:** EV **no longer buys SmartScreen reputation** (Microsoft removed EV code-signing OIDs from Trusted Root, Aug 2024) — don't buy an EV cert. **Azure Trusted Signing** gives **instant** SmartScreen reputation and is Microsoft's recommended path — **keep the existing setup, no migration.** *(CONFIRMED.)*
- **macOS:** Developer ID + `notarytool` + staple + DMG is correct (notarize+staple is mandatory for silent updates to relaunch without quarantine prompts).
- 🟠 **PIPE-IDENTITY is time-sensitive:** sign under the **organization** identity (Marston Enterprises), **not "Matthew Archbold," BEFORE wide distribution** — the signer's legal name is shown to users (wallet-trust), and **changing the signing identity later RESETS accrued SmartScreen/Gatekeeper reputation.** Do this before scaling. *(Reputation-reset = medium confidence; org-vs-person display = confirmed.)*

**Appcast feed security (PIPE-APPCAST).** The appcast XML is **not** document-signed; security rests on **HTTPS + per-enclosure EdDSA verified client-side** (a tampered/unsigned enclosure is rejected; a wrong-version appcast can't inject malware without a forgeable signature). Therefore: **CI must fail-closed** — an unsigned/failed-signature build must not be promotable to the live feed (today's `|| true` non-fatal appcast push violates this). Enforce **`sparkle:channel` discipline** — an **unchannelled beta item auto-deploys to ALL stable users** (this is exactly the regression at `generate-appcast.py` 24b2522→2eda476). *(CONFIRMED.)*

### 7.3 Fan 3 — CEF self-build: build time, latest-stable sourcing, drift detection (PIPE-A1 / PIPE-CEF-LATEST)

> Method: medium-weight, primary sources (CEF branches doc, Chromium build docs + commits, mozilla/sccache, chromium-dev PSA, EngFlow). Verifier flagged one numeric error (drift is worse than stated).

- **Drift is worse than the doc said.** Current CEF stable = **branch 7827 / Chromium M149** (beta 7871/M150). Hodos at **7103/M136** is **~13 milestones ≈ ~12 months behind** (the synthesis's "~6 months" was verifier-**REFUTED** — at the 4-week cadence it's ~12mo), **and M136 predates the M138 LTS program entirely → zero current security-patch coverage.** Red-line drift test: *branch past its Chromium stable-exit date AND outside any LTS window* (track via Chromium Dash schedule).
- **🔑 Pin to a CEF LTS branch, don't chase stable.** CEF now ships **LTS branches** — every 6th (M138, M144, **M150**, …) gets ~8 months of security fixes after exiting stable, feature churn only every 6 months. For a small team carrying Blink patches this is the strategic answer to A2: **target M150 (LTS)**, cutting rebase frequency from ~13–26/yr to ~2/yr.
- **⚠️ Chromium → 2-week stable cycle in Sept 2026** (from 4-week), doubling upstream churn — makes the LTS strategy essential, not optional.
- **🔑 reclient is being REMOVED (~Sept) and replaced by Siso.** Any remote-build investment must target **Siso + a third-party REAPI backend** (EngFlow / BuildBuddy free tier / NativeLink) — **not reclient**, and Google's hosted RBE is off-limits to non-Googlers.
- **sccache works on MSVC** via `cc_wrapper="sccache"` + `chrome_pgo_phase=0` (the toolchain then auto-drops the `/Brepro` + `/showIncludes:user` flags that otherwise make objects un-cacheable). Supports an **S3-backed shared cache** for team/CI reuse. **BUT** the famous "3× speedup" is a **warm-cache/incremental** figure (2021 Electron report) — **a cold from-scratch build (the ~2-week pain) gets no benefit from caching alone.**
- **Local levers:** `is_component_build=true` + `symbol_level=0` + `is_debug=false` is the highest-leverage no-cost dev speedup — *but component build is a dev-only layout, not a shippable single-binary release.*
- **GitHub-hosted runners cannot build Chromium** (disk + 6h cap). Lowest-cost realistic path: **self-hosted runner/beefy VM for the cold build + shared sccache for incrementals**; paid RBE buys parallelism at per-compute cost.
- **Rebase treadmill = bump-frequency × patch-depth** (no per-bump hours figure exists). The lever is pin-to-LTS (~2 bumps/yr) + minimize Blink-patch surface.
- ⚠️ **Not verified:** AWS Windows-spot / EC2-Mac / MacStadium $ figures — price directly before any go/no-go.

### 7.4 Fan 4 — B2 toolbar rendering prior art + measure-first (FEAT-B2)

> Method: medium-weight; the only public *measured* numbers on web-rendered browser chrome come from Vivaldi's engineering blogs (primary). All claims verifier-confirmed.

- **Industry split:** Chrome & Brave draw the toolbar/tab-strip with **native Views** (WebUI only for page-like surfaces: settings/history/downloads). **Brave RETREATED from a custom web UI to Chromium-native in 2018 specifically to cut maintenance.** Firefox post-XUL is DOM/Web-Components — *but in-process, not a spawned renderer subprocess.* **Hodos's pain is the CEF subprocess spawn + V8 init + IPC warmup, not DOM rendering per se.**
- **🔑 Vivaldi is the existence proof + the playbook.** Its entire UI is React on Chromium. Its headline new-window speedups (**37%/64%**) came from **"Portal Windows": ONE shared script context across all windows of a profile instead of a fresh document/context per window** — this maps **1:1** to Hodos's per-subprocess-spawn cost. Separately, culling no-op store re-renders gave **2× tab-open** (the MUI-re-render analog).
- A dedicated lightweight OSR surface (Preact, no MUI) shrinks **bundle parse / first paint** but **does NOT fix spawn/IPC** — context reuse does.
- **Measure-first gate (mandatory):** use CEF tracing (`CefBeginTracing`/`CefEndTracing`) → chrome://tracing / Perfetto to split **(1) subprocess/renderer spawn, (2) V8 context + JS bundle parse, (3) React mount + first paint, (4) IPC warmup.** The split dictates which approach helps.
- **🔑 Verdict — favor (c) optimize-React + context-reuse over a native rewrite.** Native draw (a) is the highest fidelity-*risk* for a small team (the **gold pill must be re-drawn twice — Views/Win + Cocoa/Mac**), which is exactly why Brave retreated and why Chrome keeps it native only with a full team. Keeping web chrome makes **pixel-identical gold-pill preservation trivial (it stays CSS)**. Hodos's "keep popups as React overlays, question only the always-on toolbar" instinct matches the mainstream native-frame-+-web-panels split.

### 7.5 Fan 5 — B4 extension feasibility on CEF + EIP-6963 (FEAT-B4)

> Method: medium-weight, primary CEF issues/forum + Chrome extension docs + EIP-6963 spec. One verifier **refutation refines** the A4 framing.

- **A4 confirmed, with an important correction.** Alloy bootstrap removed in **M128** → CEF 136 leaves the **Chrome runtime as the only extension path**, and *"the Chrome extension API is supported with Chrome-style browsers/windows only."* No programmatic load API (only `--load-extension` / `chrome://extensions`).
- **⚠️ Verifier refutation:** the claim "CEF lacks MV3 service workers" is **mis-framed**. The CEF Chrome runtime **embeds Chromium's real extension subsystem and CAN run MV3 service workers** — the actual blocker is that extensions require **Chrome-*style* windows (Chrome's toolbar/URL-bar UI)**, which **collides head-on with the B2 custom header** (verified: custom toolbar/URL-bar/menu approaches break under the Chrome runtime and must be rebuilt via Views). **So B4's real blocker is the B2/Chrome-UI collision, not a missing runtime.** This *tightens* the B2↔B4 coupling.
- **EIP-6963 is the high-value, low-cost win.** FINAL status; pure window-event injection (`announceProvider`/`requestProvider` with uuid/name/icon/rdns); race-free; preserves `window.ethereum` back-compat; the canonical fix for "Hodos provider fighting MetaMask." Slots directly into Hodos's existing `CWIShimScript`/`window.yours` injection layer; dApp-side libs (wevm/mipd, wagmi) handle interop. **BSV caveat:** Hodos injects `window.yours` (BSV) while 6963 is EVM/EIP-1193-shaped — confirm whether Hodos needs EVM-wallet coexistence or only a BSV-equivalent discovery convention before investing.
- **🔑 0.4.0 slice:** ship **EIP-6963 deconfliction at the injection layer + curated native first-party integrations** (the Rust adblock already mirrors the `declarativeNetRequest` declarative model); **DEFER actual extension hosting** until a B2-compatible Chrome-runtime header strategy exists, if ever.

### 7.6 Fan 6 — Farbling techniques (FEAT-B1) — resolves the B1 conflict

> Method: medium-weight, primary Brave wiki/source + fingerprint-chromium repo. All claims verifier-confirmed.

- **Per-profile seed trade-off is well-scoped & defensible.** It weakens **only cross-SESSION linkability** (vs bulk trackers — exactly what Brave's per-session reset defends) and **NOT cross-SITE, provided Hodos still hashes `master_seed || eTLD+1 || storage_area` per surface.** The lost property never defended targeted adversaries anyway. Given Hodos's "UX wins ties" + login-stability requirement, this is a sound call. **Recommend: persistent per-profile master seed, still per-domain-mixed.**
- **🔑 The B1 "conflict" is resolved by the layer + value-constraint:**
  - **deviceMemory / hardwareConcurrency: SAFE to re-add at Blink/C++** via compile-time getter replacement (Brave's `chromium_src` pattern) — **no JS `toString` tell** (the "detectable" objection was a *JS-injection artifact*, not inherent). **MUST constrain to the standard valid set** (deviceMemory ∈ {2,4,8,16,32}; concurrency to plausible counts) — an out-of-spec value is itself a fingerprint.
  - **WebGL `UNMASKED_VENDOR/RENDERER`: DANGEROUS even natively** — randomized strings are *more* unique than the truth (Brave saw ~1-in-223k). If re-added, map to a **small set of common real GPU strings**, never noise. **Hodos's original instinct to DROP it was correct.**
- **🔑 Worker leak + readPixels fix = go to the Blink layer.** Patch the **shared bitmap-readback path (`static_bitmap_image.cc`)** → covers `toDataURL` + `getImageData` + `readPixels` in one place. Bind farbling state to **`Supplement<ExecutionContext>`** (Brave's `BraveSessionCache`) → automatically reaches `WorkerGlobalScope`, fixing the worker leak that exists *because* JS `OnContextCreated` injection never fires for workers. Budget worker edge cases (seed across the thread boundary; null-safe top-frame-origin lookup).
- **Licensing:** **fingerprint-chromium = BSD-3** (safe text base) — **but its WebGL-metadata path is Linux-only and Chrome 144 removed the flags, so Hodos (Win/Mac) must re-implement that part.** **Brave = MPL-2.0** (file-level copyleft → re-implement the *mechanism*, don't copy text). **Bromite = GPL-3 (forbidden)** for proprietary Hodos.

### 7.7 Fan 7 — Misc: parking_lot, per-frame origin, CORS, version-bump, WSL secrets

> Method: medium-weight, primary tokio/parking_lot/CEF/ts-sdk sources. Two verifier refutations (both refine, neither overturns the recommendation).

- **F4/AUDIT-F4 — parking_lot is the right poison cure.** Non-poisoning by design (`lock()` returns the guard, no `Result`). ⚠️ Verifier refutation: poison *does* have one designed use — **cancellation-by-panic-unwind** (rust-analyzer/chalk) — so "poison's only value is bug-detection" was overstated. **But Hodos's wallet DB mutex is not a cancel-by-unwind design, so it loses nothing.** Action: confirm no code path intentionally panics-while-locked as a signal, then swap. **Keep `tokio::sync` ONLY for locks held across `.await`;** the synchronous SQLite wallet-DB lock → parking_lot. (`std::sync::nonpoison` is coming on nightly — parking_lot is forward-compatible.)
- **🔑 B2 iframe-origin-confusion FIX (open question #4).** CEF's `GetResourceRequestHandler` already receives **`request_initiator`** — the origin (scheme+domain) of the page that initiated the request, **populated by Chromium's network stack, not JS-spoofable.** Gate wallet routing on `request_initiator` per request, **NOT `frame->GetURL()`** (which returns the iframe's own document URL and mis-attributes). A per-frame IPC bridge is unnecessary. Don't treat direct-fetch as fully accepted-risk; only an opaque/null initiator is the residual case.
- **CORS narrowing (open question #4).** Replace `Access-Control-Allow-Origin: *` with **echo-the-Origin** (optionally allowlisted), answer the **OPTIONS preflight** (POST + `application/json` triggers it), and **do NOT set `Access-Control-Allow-Credentials: true`**. ⚠️ Verifier refutation: @bsv/sdk's browser credentials mode defaults to **`same-origin`**, not "omit" — but the conclusion is unchanged (no cross-origin credentials are sent, so echo-origin works for every dApp). Treat CORS as defense-in-depth; the real auth boundary is the `request_initiator` gate above.
- **🔑 Version single-sourcing (PIPE-VERSION).** **git tag = source of truth.** `cargo-release` owns the bump+tag event (Cargo.toml `[package] version` can't be derived at build time). Everything else derives from `git describe` at build: **CMake** via `andrew-hardin/cmake-git-version-tracking` (generates the APP_VERSION header), **Rust** via `shadow-rs`, and a small **CI step injects the tag** into the Inno `.iss` (`-D`) and the TS constant. No file hand-edited.
- **WSL secrets (OPS-WSL).** `git-crypt` encrypts *contents only* (AES-256-CTR) — **leaks filenames, commit messages, timestamps; Windows support is WSL-only; key-loss = permanent data loss.** Adequate only if metadata isn't sensitive and the key is backed up out-of-band; otherwise prefer out-of-repo or self-hosted git. **WSL ext4 canonical-repo backup must run from *inside* WSL** (git push to a private remote + `restic`/`tar` of the ext4 tree) — Windows backup tools can't reliably read `\\wsl$` ext4.

---

## 8. Staleness flags (doc-vs-source reconciliation backlog)

These are the doc claims found contradicted by current source during the kickoff. Fold each fix into the stream that first touches the doc (DOC-STALE-SWEEP).

1. **MEMORY/tracker "audit fixes done"** — FALSE. All F1–F9 `pending`; both criticals live. "Adjudicated" = analysis done, NOT remediation.
2. **AUDIT_FIX_TRACKER F2/F9 line numbers** drifted ~+19; F2 leak broader than documented. Trust tracker paths, not line numbers.
3. **findings.jsonl line numbers** reference a scan-time `/tmp` clone — universally drifted. Trust category+file only. (TAAL key relocated to `arc_taal.rs:16`; DROP-TABLE to `handlers.rs:2938`.)
4. **18 "weak randomness" findings are FALSE POSITIVES** (`rand::thread_rng` IS a CSPRNG). Exec-summary "predictable key material" headline is misleading.
5. **`cef-native/include/core/CLAUDE.md` farbling row** claims WebGL/navigator farbling that `FingerprintScript.h:11-15` deliberately REMOVED. Doc factually wrong vs source.
6. **0.4.0 README** still says "research phase / no source yet" — stale (A4 landed, B1/B3 backends exist). B3 doc's "check for store" premise already answered yes.
7. **`cef-native/tests/CLAUDE.md`** references the DELETED `permission_engine_test.cpp` in 3+ places (removed commit 1d7de47); its SessionManager roadmap is moot (deleted 2.6-H.2). Also in **root CLAUDE.md Key Files**.
8. **Test census inflated everywhere** — `TEST_PLAN.md` (was UNIT_TESTING.md) "780+" vs ~480 Rust real; "73 Playwright" vs 54; "~46 C++" vs 39; adblock "0" vs 23. *(Reconciled 2026-06-16: UNIT_TESTING.md renamed to `DevOps-CICD/TEST_PLAN.md` with verified census in §2.0.)*
9. **BUILD_AND_RELEASE.md §5.1/5.2** describe a `ci.yml` that doesn't exist; §5.3 a release "test-gate" job that doesn't exist. `TEST_PLAN.md` (was UNIT_TESTING.md) §7 "PR can't merge if tests fail" is fiction.
10. **BUILD_AND_RELEASE.md drift** — separate appcast-mac.xml (reality: single appcast.xml); timestamp URL; signing identity "Marston Enterprises" vs real "Developer ID Application: Matthew Archbold"; Dependabot "enabled" but no `dependabot.yml`.
11. **generate-appcast.py REGRESSION** (code) — `sparkle:channel` beta added 24b2522 then dropped 2eda476; channel filtering may be silently broken.
12. **AUTO_UPDATE_IMPLEMENTATION_PLAN.md** "Planning" status stale — updater substantially implemented; only the SILENT path + Windows EdDSA pending.
13. **Bug-hunt B1** ("record_spending has zero callers") STALE — FIXED in 582ce22. **B2 iframe-origin-confusion genuinely OPEN** (CORS wildcard `*` at `HttpRequestInterceptor.cpp:1416` confirmed; lockdown reverted at ccf7c6d).
14. **B2-header-to-cpp.md is a 31-line stub**; gold pill still called "green-dot" in 3+ C++ comments.
15. **DevOps README/TRIAGE** undersell Linux (release.yml has an ubuntu job at :802). Version sources skewed (0.2.0-dev / 0.3.0 / 0.3.0-beta.15).
16. **browser-extensions/ deep-dive docs UNTRUSTED** — assume an infeasible path (A4 proved real extensions impossible on CEF). Do not anchor sizing on the "15-21 weeks" estimate.

---

## 9. Open questions / decisions needed before execution

> **Research-resolved (now ratify-or-override, recommendation in §7):** Q2 (parking_lot — §7.7), Q4 (CORS + iframe-confusion — §7.7), Q5 (B2 measure-first + optimize-over-rewrite — §7.4), Q6 (B4 = EIP-6963 + defer hosting — §7.5), Q7 (B1 spoof conflict resolved at C++ layer within valid set; drop WebGL vendor/renderer — §7.6), Q8 (B1 per-profile seed defensible if eTLD+1-mixed — §7.6). **Still genuinely open / your call:** Q3, Q9, Q10, Q11, Q12, Q13, Q14, Q15. Plus new research-driven decisions below (Q16–Q18).

1. ✅ **DECIDED** — F1/F2 ship as a Phase-0 patch ahead of 0.4.0.
2. **F4 parking_lot timing** — when does its dedicated kickoff run relative to residual permission work (253 AppState edits = conflict risk)? Does any handler rely on poison-as-circuit-breaker?
3. **Effective prod log level** — is Option A (`warn`) actually suppressing F2/F3 `log::info!` lines in prod builds? (F1 is `std::cout`, unprotected regardless.)
4. **CORS wildcard** — `Access-Control-Allow-Origin: *` is verified at `HttpRequestInterceptor.cpp:1416` (the B2 iframe-confusion enabler). The audit's "CORS-locked, so DoS low-sev" assumption is partly false — confirm the actual bind/allow-list posture.
5. **B2 measure-first** — React mount vs CEF spawn vs IPC warmup? And how to reconcile a native header with the root-CLAUDE.md "NEVER add panels to MainBrowserView" rule (V8 isolation intent)?
6. **B4 direction** (joint with B2) — curated first-party / CEF Chrome-runtime / defer? Re-scope to EIP-6963 deconfliction + keep `window.yours` lock as the 0.4.0 slice?
7. **B1 conflict** — does native-level patching make WebGL vendor/renderer + navigator hwConcurrency/deviceMemory safe to spoof (no JS `toString` tell), or is the design re-introducing known breakage?
8. **B1 privacy trade-off** — sign off on persistent per-profile seed (login stability vs cross-session unlinkability)?
9. **Windows DSA→EdDSA** — migrate to meet A6's own EdDSA-on-both bar (needs WinSparkle 0.9.x)?
10. **TAAL key** — rotate as part of 0.4.0, or accept (rotates monthly at build time)?
11. **CI test strategy** — reusable `workflow_call` shared between ci.yml and release.yml, or duplicate? Where do C++ GoogleTest (network FetchContent) and crypto coverage (tarpaulin Win vs Linux) run hermetically?
12. **Frontend pyramid** — build the missing Vitest unit layer or formally retire `DevOps-CICD/TEST_PLAN.md` §3?
13. **Appcast safety** — decouple appcast generate/push from build so a bad appcast can't auto-deploy? Soften Apple notarization hard-fail (currently blocks ALL releases on an Apple outage)?
14. **macOS silent-update vs 2026 Sparkle CVEs** — *RECONCILED in `DevOps-CICD/AUTO_UPDATE.md` §4:* CVE-2026-47122 (appcast-item injection) + CVE-2026-47121 (delta traversal) are unpatched through 2.9.1 but **LOCAL-only** (need existing code execution; EdDSA+HTTPS block remote) → **not a hard blocker.** Mitigate with **binary-deltas OFF** + monitor for a patch. Remaining call: confirm deltas-off is acceptable and proceed with silent.
15. **Branch/sprint strategy** — how to sequence/branch this unified sprint vs `feature/brc121-phase1`; confirm Phase 3 ordinals stays deferred.
16. **🆕 CEF branch strategy (from §7.3)** — adopt the **CEF LTS line (target M150)** instead of chasing stable, and treat any future distributed-build work as **Siso + third-party REAPI (not reclient)**? This reframes PIPE-A1/PIPE-CEF-LATEST. Note Hodos's current M136 has **zero security-patch coverage** today — does the CEF bump get pulled earlier in the sprint as a result?
17. ✅ **DECIDED 2026-06-16** — **B2 = keep React, NO native port.** Header stays a separate CEF subprocess. Scope: FEAT-B2-MEASURE → FEAT-B2-SLIM (slim MUI, keep all functionality) + FEAT-B2-WARM (pre-warm only; cross-window context sharing gated behind a security review) + FEAT-B2-FILL (header gap) + FEAT-DPI (multi-monitor mouse-offset, deep-research). Gold pill stays as-is.
18. **🆕 B1 farbling design (from §7.6)** — ratify the resolved design: **move to Blink-layer patches** (`Supplement<ExecutionContext>` + shared bitmap-readback) to fix the worker leak + readPixels; **re-add deviceMemory/hardwareConcurrency at C++ within the standard valid set**; **keep WebGL vendor/renderer dropped**; **persistent per-profile seed still mixed with eTLD+1**; license via re-implementation (no Brave/Bromite text)?

---

## 10. Agent-tasking plan (parallel vs sequential)

- **Parallelizable now:** F5/F6/F7/F9 (independent files); PIPE-A7 census + PIPE-VERSION + the release-crypto pass; FEAT-B3 bookmarks UI.
- **Sequential / own-kickoff:** F4 parking_lot; A1 CEF toolchain → B1 patch chain.
- **Decision-gated:** B2-MEASURE before B2-SLIM/B2-WARM; FEAT-DPI deep-research before fix.

---

## 11. Build Pipeline — Process & Procedures (living doc)

> **STANDING INSTRUCTION (applies to the whole sprint, not just the pipeline):** **Always document lessons learned.** Whenever a build/dep/release step surprises us, breaks, or teaches us something, write it down AND update the relevant Process & Procedures below (and the canonical runbooks: `DevOps-CICD/CEF_BUILD_RUNBOOK.md`, `BUILD_AND_RELEASE.md`). The goal of this sprint is as much about **repeatable procedures a small team can run** as it is about code. Treat P&P as code: keep it current or it rots.

### 11.1 CEF self-build bump procedure
> Reframes PIPE-A1 / PIPE-CEF-LATEST / PIPE-CEF-DEPBUMP into a repeatable checklist.

1. **Pin to a CEF *LTS* branch** (target M150), **not** newest stable — Chromium goes to a 2-week stable cadence Sept 2026; LTS = ~6-month feature churn + ~8-month security fixes.
2. **Cadence — two distinct rebases:**
   - **Quarterly (4×/yr): pull the latest *security point-release* of the pinned LTS branch.** Cheap — our Blink patches rarely touch files that change inside a milestone, so re-apply is usually trivial.
   - **~Every 6 months: *milestone jump* to the next LTS** (e.g., M150→M156). The expensive one — budget patch-rework time.
3. Fetch via `automate-git.py` against the pinned branch (it resolves Chromium's *internal* deps automatically).
4. Run **§11.2 dependency verification** for Hodos's *own* deps.
5. **Re-verify codecs every bump** (§11.3) — flags persist but behavior must be smoke-tested.
6. **Re-apply local patches** (farbling, §11.4) via the patch toolchain; fix any that no longer apply.
7. Build Debug+Release, smoke-test, then promote.

### 11.2 Dependency-verification checklist (run every CEF bump)
> The hard part is **Hodos's own deps staying compatible**, not Chromium's internal ones. For **each** dependency, answer in writing:
- **What is it + current version?** (vcpkg: nlohmann/sqlite3/OpenSSL/quirc; the `libcef_dll_wrapper`; frontend React/Vite/TS; Rust crates.)
- **Is it compatible with the new CEF/Chromium ABI + toolchain?** (compiler/CRT/C++ std version match?)
- **Is this the right version — and *why* this one?** (pinned for a reason? transitive constraint?)
- **What else does bumping it affect?** (ripple to other deps, APIs, behavior.)
- **Any conflict?** (two deps wanting different versions of a shared lib.)
- **Record the answer** so next bump starts from a known baseline (not from scratch).

### 11.3 Codec re-verification (every bump)
- Confirm GN flags still set: `proprietary_codecs=true`, `ffmpeg_branding=Chrome`.
- Smoke-test real playback: video (YouTube), audio, images, common formats.
- **DRM (Widevine) is NOT covered by self-build** — protected content (Netflix/Spotify-premium-class) is a separate licensing path; track as a known gap, don't assume it works.
- If build flags must change for any reason, **document why** (§11 standing instruction).

### 11.4 Farbling-in-source gating sequence (FEAT-B1)
1. Stand up the **patch-management toolchain** (`patch/patches/`, `patch.cfg`, automate-git integration) — greenfield; **this IS the PIPE-A1 work and B1 is its first consumer.**
2. Implement Blink-layer farbling by **re-implementing Brave's *method*** (MPL-2.0 = don't copy text; Bromite GPL-3 = forbidden; fingerprint-chromium BSD-3 = safe base but re-do its Linux-only WebGL path for Win/Mac): `Supplement<ExecutionContext>` (reaches workers) + shared bitmap-readback (`static_bitmap_image.cc`) for canvas/getImageData/readPixels.
3. Re-add deviceMemory/hardwareConcurrency at C++ **within the standard valid set only**; **keep WebGL vendor/renderer dropped**. Persistent **per-profile** seed, still hashed with eTLD+1 per domain.
4. **VERIFY** (FEAT-B1-VERIFY): CreepJS/browserleaks + **cross-session login test** on the auth basket, Win+macOS.
5. **Only after verification passes:** remove the JS-injection farbling from CEF-Native (`FingerprintProtection.h`/`FingerprintScript.h`/render-process injection) — never leave a no-protection gap.
6. **Then** test whether the OAuth/auth-domain **exclude list can shrink/be removed** (verify-then-decide; per-profile + in-spec values make this likely but not guaranteed).
7. **Each build is the test loop** — a failed farbling change means a new CEF build, so this is time-consuming; batch changes and lean on incremental/cached builds (sccache) where possible.

---

## 12. Tentative release sequencing (2026-06-16 — owner's working plan)

> Branch/remote model: see **CLAUDE.md → "Branch & Remote Workflow"** + `DevOps-CICD/README.md`. All code lands in `origin` (feature → `staging` → `main`); the signed **public** build happens on the `release` remote (holds the signing keys). Phase-3 ordinals (1Sat) is pulled to sit **between** an internal test build and the **public** 0.4.0 release.

1. **Push this sprint's work to `origin`** — `origin/staging` → `origin/main` (clean fast-forward; `0.4.0` already contains the Sigma work).
2. **0.4.0 sprint EXECUTION (all in origin)** — implement everything: Phase-0 secret-log fixes, audit fixes, features (header opt, bookmarks, B1), **and the full CI/CD pipeline + unit tests.**
3. **Internal beta build through the NEW pipeline** — build the *entire* thing through the new CI/CD + unit tests, version it **`0.3.x-beta`**, and **fetch it on our machines for testing only — NOT public** (don't make it the newest GitHub release). **This is the FIRST real exercise of the new pipeline.**
4. **1Sat ordinals (Phase 3)** — implemented + tested here (the 0.4.0 sprint work is already done + privately validated by now).
5. **Public 0.4.0 release** — run the pipeline again (unit tests + everything; may **reuse the CEF binaries**, no full CEF rebuild), tag it **`0.4.0-beta.0` / `-beta.1` (public BETA)**, push `main` → `release` for the signed public build. **SECOND exercise of the new pipeline.** (0.4.0 goes out as a public beta, not a GA release.)

**Notes:** (a) **Phase-0 secret-log fixes (F1/F2/F3): do early as code hygiene, but NO urgent patch release needed** — no public users yet, so the live-leak urgency is low (owner's call 2026-06-16). (b) Corrected: the internal beta (step 3) runs through the **NEW** pipeline (built in step 2), so it **does** validate the new CI/CD — both step 3 and step 5 are real exercises, with ordinals between. (c) Pulling ordinals before the public 0.4.0 release is an intentional scope choice.

---

> Next action: branch model + open questions (§9) + extensions / BROADCAST_AND_EXPLORER_REVIEW / bug-hunt review, then a concrete execution plan. No source code until those are settled. (Header ✅ decided, bookmarks ✅ scoped, farbling/pipeline ✅ captured, DevOps consolidation ✅ done, testing strategy ✅ done.)
