# Phase 9 — release readiness · PHASE CONTRACT

**Workstream:** promotion blockers + DevOps hygiene (`../SPRINT_PLAN.md` §4.1, Phase 9 bundle)
**Tickets:** `../TICKET_cdp_port_open_in_release.md` · `../TICKET_engine_pins_are_branches_not_tags.md` ·
`../TICKET_farbling_gate_engine_binding.md` · `../TICKET_dependency_freshness_review.md` ·
(🍎 Mac, relayed) `../TICKET_appcast_missing_minimum_system_version.md` ·
(⛔ not here — `INSTALL_TEST_BATCH.md` I5) `../TICKET_stray_log_in_install_root.md`
**Status:** 🚧 IN PROGRESS — kickoff `fba4c7a`; 👤 owner answered §8 2026-09-14 (all seven, recorded there).
**Opened:** 2026-09-14 · **Owner:** Matthew Archbold · **Platforms:** Windows for every commit here; macOS owes the two
mirror items in the relay (appcast floor, CDP-port `.mm` block)
**Standard:** `../HARNESS.md`. **Base:** `8e39abc` (+ `4a5c275`, the session prompt) on `origin/0.4.0`,
fetched 2026-09-14: 0 ahead / 0 behind, no Mac push since `5710742`.

> ⚠️ Kickoff method: every claim below is a **code read on today's tree** (symbols, not line numbers)
> except where marked 📏 measured. Nothing has been built or pushed.

---

## 0. Plan-vs-tree delta — what the four tickets say vs what the tree says today

### 0.1 ✅ `D-1` — the CDP port is open exactly as ticket 1 describes, and it is open on this machine right now

`cef_browser_shell.cpp :: RunHodosMain`, the block under the comment *"Remote debugging port: each profile
gets a unique port…"*: `g_picker_mode → 0`, `"Default" → 9222`, `Profile_N → 9222+N`, then `if
(hodos::IsDevEnv() && port != 0) port += 100`. `IsDevEnv()` (`include/core/PortConfig.h`) is the
`HODOS_DEV=1` env check — it **offsets**, it does not gate. 📏 `Get-NetTCPConnection -State Listen` on
2026-09-14: **9222 is bound by `%LOCALAPPDATA%\HodosBrowser\HodosBrowser.exe`** — the owner's installed
production browser, today. That is the defect, measured, not inferred.

⭐ Two facts that make the fix safe, both **read in the CEF source** (rule 4): `libcef/common/chrome/
chrome_main_delegate_cef.cc` only appends `--remote-debugging-port` when the value is in `[1024, 65535]`, so
`= 0` never reaches Chromium (the existing picker-mode path already relies on this, and the block's own
comment documents it). And nothing in `cef-native/`, `frontend/src`, `scripts/` or `.github/` references
9222/9322 except the two shell entry points — the only consumers of the port are the Python harnesses under
`development-docs/`, all of which default to **9322** (`--port` default in `farbling_seed_rotation_check.py`
and siblings) and drive **dev** builds.

### 0.2 🚨 `D-2` — the design doc's D3 premise is false today: dev tooling DOES depend on `--remote-allow-origins=*`

`DEVTOOLS_SECURITY_DESIGN.md` §2 says *"Nothing in this repo depends on either"* (the port or the flag) and
D3 says drop `--remote-allow-origins=*`. That was true on 2026-08-04. It is not true now:

- `simple_app.cpp :: OnBeforeCommandLineProcessing` still appends `remote-allow-origins=*` unconditionally,
  and this file is **shared** — no `#ifdef`, so it is both platforms' flag.
- The farbling harnesses connect with **`websocket-client` 1.9.0** (`websocket.create_connection(...)`),
  which sends `Origin: http://127.0.0.1:<port>` unless `suppress_origin` is passed (read in
  `websocket/_handshake.py`), and Chromium's `content/browser/devtools/devtools_http_handler.cc ::
  OnWebSocketRequest` **403s any Origin-bearing upgrade** not in `--remote-allow-origins` (read in the local
  7871 tree). ⇒ without the `*` the seed-rotation gate's own harness cannot attach in dev.

The design already carries the answer: *"If a dev workflow turns out to need it, re-add it inside the dev
branch only."* So D3's shape here is **gate the append on `IsDevEnv()`** — dev unchanged, release drops it.
This is a one-line change in the same commit as D2 (design §7: *"D2 + D3 are small and independent — one
commit"*), but it is a **scope question for the owner** (§8 Q1) because the session prompt names only the port
block.

### 0.3 ⚠️ `D-3` — ticket 1's acceptance list includes D4 (no Inspect Element on overlays), which is a behaviour change with a T3 row

The ticket's acceptance has *"wallet overlay offers no Inspect Element and refuses DevTools if invoked"*,
which is design decision **D4** (route all four `ShowDevTools()` entry points through
`SimpleHandler::ShowOrFocusDevTools()` and gate on `role_.rfind("tab_", 0) == 0`; stop adding
`MENU_ID_DEV_TOOLS_INSPECT` in the non-tab context-menu branch). The design says D4 *"deserves its own
commit plus the overlay smoke"*. It is not in the session prompt's description of ticket 1. §8 Q1 asks.

### 0.4 ⛔ `D-4` — the release half of ticket 1 cannot be measured on this box; it is an install-batch row

`AppPaths::EnforceDevSafeguard` refuses to start any binary under a build directory without `HODOS_DEV=1`
(`cef_browser_shell.cpp`, *"DEV SAFEGUARD"*), and the only way around it — copying the build tree elsewhere and
launching without `HODOS_DEV` — would run against the **production** data directory and the owner's live
wallet on 31301. ⛔ Not doing that. The release-shaped proof (nothing on 9222, F12 still works, log says
`Remote debugging port: 0`) is **`INSTALL_TEST_BATCH.md` row I8**, added by this phase, owed by this phase.
What *is* measurable locally, and is: the dev gate keeps 9322 (`P9-A1`), and the `= 0` mechanism the
release branch will use produces "nothing bound" on this exact binary (picker mode, `P9-A1`'s RED).

### 0.5 ✅ `D-5` — ticket 2's refs are exactly as listed; the tag name is constrained and the existing tag is lightweight

📏 On the fork clone `C:\cef\cef150\chromium\src\cef` (remote `origin` = `Hodos-Browser/cef`), `git ls-remote
origin`: `refs/heads/pin-9ccef04/7871` and `refs/heads/hodos/7871` both at `9ccef044…`,
`refs/heads/pin-7dd0357/7871` at `7dd0357392…`, and the only tag is `refs/tags/pin-c636546/7871`
(**lightweight** — `cat-file -t` = `commit`; the new tags match that kind). The clone is on a detached HEAD at
`9ccef044…` with the automate-git patch set applied (1,409 modified files) — tagging a SHA touches neither.
📏 `gh auth status`: logged in as `BSVArchie`, and `gh api repos/Hodos-Browser/cef` reports
`push: true, admin: true` — the push should not be refused, but the ticket's stop point stands if it is.

⭐ **The tag name is load-bearing**, per `CEF_BUILD_RUNBOOK.md` *"From the 2026-08-10 pin tagging"*:
`cef/tools/cef_version.py` derives the branch field from the commit's **decoration**, taking the last
path component, so the tag must be `pin-<sha>/7871` — last component exactly `7871`. 📏 `python
tools/cef_version.py current` on the clone today prints `150.0.43-7871.3576+g9ccef04+chromium-150.0.7871.187`;
the same command after tagging is `P9-B1`'s GREEN. ⚠️ A tag and a branch with the same name are legal
(different ref namespaces) but make the short name ambiguous — every command uses the full
`refs/tags/pin-…/7871` refspec, and the branches are **not** deleted here (owner's instruction).

### 0.6 ✅ `D-6` — ticket 3's three findings hold verbatim, and the Windows half of `require_engine` is thinner than its docstring

`farbling_seed_rotation_check.py`: `require_engine()` is defined and has **zero call sites in this file**
(the five sibling harnesses — psl-linkability, realm matrix, vector matrix, worker probe, worker residual —
all call it behind `--expect-cef`); the measured dict returns `"engine": engine_version(args.port)` (the
CDP `Browser` string); the usage docstring says CEF_VERSION is *"printed as `engine=`"*, which is false.
`promote.yml`'s re-derive step takes `grep -oE '[0-9]+' | head -n1` of `engine=` and checks `≥ 150`.

📏 One thing the ticket does not say: `engine_identity()` on **Windows** only reads the staging header
(`cef-binaries/include/cef_version.h`) and sets `why = "non-darwin: use md5(app libcef.dll) == md5(distrib)"`
— the md5 half is a comment, not code. The memory `reference_engine_identity_not_chromium_version` records
why the header alone false-greens (restage without rebuilding). §8 Q3 proposes adding the ~10-line md5 half.

⭐ The expected engine for the gate already has a single definition: `release.yml` `env.CEF_ASSET`
(`cef-binaries-windows-150.0.43-g9ccef04.zip` / `…-macos-150.0.43-g9ccef04.tar.bz2`), and `promote.yml` checks
out **the tag's source** before the gate runs, so the gate can derive `g<sha>` from those two lines (must
agree) with no new workflow input. `release.yml` already does the same parse for its own artifact assertion.

### 0.7 🚨 `D-7` — ticket 4 is more than half done already; two of its headline claims are stale

`DEPENDENCY_VERIFICATION.md` carries a **"Freshness review — 2026-08-17 (the first one)"** the ticket
predates: Node **20 → 22** (EOL fix) on both `release.yml` arms, Sparkle 2.9.3 → 2.9.6, WinSparkle tool
0.9.3 → 0.9.4, and the frontend engine binding — `frontend/vite.config.ts` `build.target: 'chrome150'`
(the load-bearing setting, measured live: −11,430 bytes) plus `package.json` `browserslist: ["Chrome >= 150"]`
and `engines: { node: ">=22" }`. So the ticket's *"no browserslist and no engines"* and *"Node 20"* are
both stale, as the session prompt warned. The Vite target is tied to the shipped engine in prose only
(`CEF_VERSION` in `cef-binaries/include/cef_version.h` is the `+chromium-150.…` engine; `target` is
`chrome150`); nothing derives one from the other.

What the ticket asks for that is **not** yet done:

| Ticket item | Tree today | Left for this phase |
|---|---|---|
| review pass with `cargo audit` / `cargo outdated` / `npm audit` | 2026-08-17 review queried upstream *latest* for the C++/tool pins only; **no advisory run recorded anywhere**; `test.yml`'s audits are triple-neutered and CI has been dark since 2026-08-14 | run all three locally, record the table |
| symbol-coexistence re-measure | 247 / 240 / 1 measured 2026-08-17 on P4f, method not scripted; the shipped `libcef.dll` is byte-identical between `cef-binaries/Release` and `build/bin/Release` (292,293,632 B) | script it, re-measure, record |
| written cadence | *"Method — repeat at every engine bump and quarterly"* exists; tracker step 3 says *"re-run DEPENDENCY_VERIFICATION.md"* | one policy paragraph (hold/bump table per review) + the checkpoint sentence in the tracker |
| macOS float accepted or escalated | `Brewfile` says *"tracked in DEPENDENCY_VERIFICATION.md"*; the doc has **no decision** | 👤 owner (§8 Q5) |

📏 Tooling: `cargo audit` and `cargo outdated` are **not installed** on this box (CI installs `cargo-audit`
via `taiki-e/install-action`); `dumpbin` exists at the VS 2022 BuildTools path. Node here is v23.10.0.

### 0.8 ✅ `D-8` — `stray_log_in_install_root` is correctly parked in `INSTALL_TEST_BATCH.md` I5

Ticket status *"CODE COMPLETE (Phase 0) — T2/T3 rows owed"*, sprint row *"⇒ INSTALL_TEST_BATCH.md row I5"*,
I5 reads *"owed by Phase 0 (code complete), same install as I4"*. Consistent; nothing to do here.

### 0.9 🍎 `D-9` — the appcast blocker is Mac's, and the human queue already lists it as C2

`scripts/generate-appcast.py` has no `minimumSystemVersion` argument or emit path; `release.yml` sets
`MACOSX_DEPLOYMENT_TARGET: "12.0"` and `-DCMAKE_OSX_DEPLOYMENT_TARGET=12.0`, and its `minos` guard reads the
framework's floor with `vtool`. `HUMAN_TEST_QUEUE.md` C2 already carries the Big Sur call as human-bound on
Mac. Relayed, not owned here (§5).

---

## 1. Goal

A promoted 0.4.0 has no unauthenticated control channel open on users' machines, its engine source is
pinned by an immutable ref, its farbling gate refuses a token from any engine but the one being shipped, and
its dependency pins have a written review cadence and one recorded review.

## 2. Done means

- [ ] `P9-A1`..`A3` green with reds observed; I8 written into the install batch and owed by Phase 9
- [ ] both `pin-*` tags visible in `git ls-remote --tags origin` on `Hodos-Browser/cef`; version string unchanged
- [ ] the rotation token carries `+g9ccef04+`; the gate's shell refuses an edited-engine token and a garbage
      token and accepts the real one — all three **run**, locally
- [ ] `DEPENDENCY_VERIFICATION.md` has the 2026-09 review table + the cadence paragraph; the tracker's bump
      checklist names the review; the export-count script exists and its number is recorded
- [ ] relay round written; sprint plan marks 9 done; memory updated; dev env stopped, 31301/31302 untouched

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-INTEXT` | internal never prompts, external always gates | D4 (if in scope) touches `simple_handler.cpp` near the context-menu and IPC arms; the `wallet_call` origin derivation lives in the same file. Nothing here changes it; the boundary regression is run anyway |
| `R-UPDATE` | the update path still applies | ticket 3 edits `promote.yml` (an instrument on the promotion path); ticket 4 touches nothing that ships. `R-UPDATE` stays T1-only at this boundary as at every prior one (I3 owed) |
| `R-GOLD` / `R-CLOSE` / `R-COUNT` / `R-PERIM` / `R-DUST` | — | no code path in this phase reaches them; run at the boundary because the rule says every boundary |
| `T0 G1–G5, G11, G12` | static gates at baseline | ticket 1 adds lines in `cef_browser_shell.cpp` (outside G11's paths) and `simple_app.cpp` (inside them — the gate pattern matches `GetPrimaryWindow()`/`GetActiveTab()` only). ⛔ Rule 6: baselines untouched |

## 4. Evidence table

⛔ No empty RED or SUBJECT cells. A green result is reported with its red half or not at all.

| ID | 🟢 GREEN — must be true | 🔴 RED — must be *seen* to fail, and how | 🎯 SUBJECT — proves the right thing was measured | Tier | Result |
|---|---|---|---|---|---|
| `P9-A1` | Dev rig (`HODOS_DEV=1 --profile=Default`): `Get-NetTCPConnection -LocalPort 9322 -State Listen` finds the port and `/json/version` answers; log line `Remote debugging port: 9322` | Same build, **picker mode** (no `--profile`): probe on 9322 **and** 9222 finds nothing; log says `Remote debugging port: 0`. This exercises the exact `= 0` path the release branch takes, on this binary — the probe is shown able to read "nothing bound" | owning PID's `ExecutablePath` = `cef-native\build\bin\Release\HodosBrowser.exe` (never the installed 9222 process, which is live on this box); `/json/version` reports `Chrome/150.0.7871.187` | T2 | ✅ 2026-09-14 GREEN: 9322 bound by pid 48824 = `…\cef-nativeuildin\Release\HodosBrowser.exe`, `/json/version` = `Chrome/150.0.7871.187`, 11 page targets, log `debug_output-48824.log`: `Remote debugging port: 9322`. RED: picker launch (pid 13808) ⇒ log `Remote debugging port: 0`, **no** 9322 listener, 9222 owned only by `%LOCALAPPDATA%\HodosBrowser\HodosBrowser.exe` (pid 34432, the installed build). ⚠️ SUBJECT trap: launch with **no** `--remote-debugging-port` switch — the P8d rig launcher passed one, which binds CDP regardless of this gate |
| `P9-A2` | (D3, if Q1 = yes) dev browser's child command lines still carry `--remote-allow-origins=*`; `websocket.create_connection` to a page target succeeds with the default `Origin` header | The same connect against a process **without** the flag 403s (`"Rejected an incoming WebSocket connection from the … origin"`) — ⛔ cannot be produced locally without a release run: **carried by I8**, stated here, not claimed | `Get-CimInstance Win32_Process` filtered on the dev exe path; the 403 text is Chromium's own (`devtools_http_handler.cc`) | T2 / I8 | ✅ 2026-09-14 GREEN: `SystemInfo.getInfo` over the browser socket ⇒ effective command line contains `--remote-allow-origins=*` (and `--remote-debugging-port=9322`, appended by CEF from settings); `websocket-client` page attach with default `Origin` **and** with `origin="http://evil.example"` both accepted, `Runtime.evaluate 1+1 → 2`. RED: **I8** (owed, written) |
| `P9-A3` | (D4, if Q1 = yes) right-click on the wallet overlay offers **no** Inspect Element; F12 with the overlay focused opens nothing; right-click / F12 / menu / IPC on a web-page tab all open DevTools | Revert the role gate ⇒ the overlay's context menu shows Inspect Element again and F12 opens DevTools on it | `SimpleHandler::role_` logged at the gate: `wallet` / `brc100auth` refused, `tab_<id>` allowed; a tab navigated to `/settings-page` is **still inspectable** (accepted by design) | T3 | ⬜ |
| `I8` (install batch, added by this phase) | Installed release: nothing listens on 9222; `Remote debugging port: 0` in its log; F12 / Ctrl+Shift+I / menu / Inspect all open DevTools on a web page; (D3) a WebSocket upgrade with an `Origin` header is 403'd; (D4) the overlay refuses | Dev build on the same machine: 9322 **is** bound — proving the probe can see a bound port at all. A "nothing listening" from a browser that failed to start is not a pass | owning `ExecutablePath` = `%LOCALAPPDATA%\HodosBrowser\HodosBrowser.exe`; `/json/version` absent (port closed) but the log line and the install path identify the binary | T2/T3 | ⬜ **OWED** |
| `P9-B1` | After tagging, `git ls-remote --tags origin` on the fork shows `pin-9ccef04/7871` and `pin-7dd0357/7871` at the two SHAs; `python tools/cef_version.py current C:\cef\cef150\chromium\src` still prints `150.0.43-7871.3576+g9ccef04+…` | Before tagging the same `ls-remote --tags` shows **only** `pin-c636546/7871` (📏 seen 2026-09-14) — the check is seen absent before it is seen present. The version-string trap (`hodos/7871-c636546` ⇒ a plausible wrong string; no decoration ⇒ `150.0.0-HEAD`) is the runbook's **2026-08-10 measurement**, cited, not re-run | the query is against the **remote** (`ls-remote`), not the local clone's refs; SHAs compared to `CEF_COMMIT_HASH` in `cef-binaries/include/cef_version.h` (`9ccef044…`) | T2 | ✅ 2026-09-14 — before: only `pin-c636546/7871`; after push + re-query: all three at the listed SHAs; `cef_version.py current` = `150.0.43-7871.3576+g9ccef04+chromium-150.0.7871.187` both before and after; branches unchanged; push accepted (no auth stop) |
| `P9-C1` | The rotation harness (dev rig, `--dev --port 9322`) prints `engine=150.0.43-7871.3576+g9ccef04+chromium-150.0.7871.187`; `--negative-control` still exits 0 only when its assertions fail (inverted exit) | `--expect-cef g7dd0357` ⇒ **REFUSED before launch** (`require_engine` exit); the negative-control run goes RED on the farbled vectors | `require_engine` prints `CEF_VERSION=` from the staging header **and** (Q3) md5 of `build\bin\Release\libcef.dll` == md5 of `cef-binaries\Release\libcef.dll`; `check_role_in_log` still asserts the farbled host was served to a `tab_` role | T2/T4 | ✅ 2026-09-14 GREEN (exit 0): `engine: CEF_VERSION=150.0.43-7871.3576+g9ccef04+…`, `app libcef.dll == staged distrib (md5 8c761ddc…)`, all seed-rotation contracts hold, token `engine=150.0.43-7871.3576+g9ccef04+chromium-150.0.7871.187 exempt=53225ec8×3 large=0cdc9b48×3 farbled=0e4e6251/c355cddd/0e4e6251`. RED: `--expect-cef +g7dd0357` ⇒ `REFUSED: CEF_VERSION … does not contain expected '+g7dd0357'` before launch; exe copied next to a foreign `libcef.dll` ⇒ `REFUSED: app libcef.dll md5 1adf1e85… != staged 8c761ddc…`; `--negative-control` ⇒ **NEGATIVE CONTROL PASSED** — RED on canvas/webgl/audio/navigator with farbling off, exit 0 only because it went red. 📏 The green run's `--log` pointed at the pre-P2 fixed filename so its `SUBJECT:` line was absent; the harness now accepts the logs directory and the NC run shows `SUBJECT: shell served example.com to role=tab_1 OK` ×3 |
| `P9-C2` | The gate's re-derive shell, extracted verbatim from `promote.yml` and run locally with `bash` against a `release.yml` checkout: the real `P9-C1` token **passes** | Same shell, same token with `g9ccef04` edited to `g7dd0357` ⇒ **exit 1** (`engine … does not match the engine release.yml builds against`); token with `engine=Chrome/150.0.7871.187` (today's shape) ⇒ **exit 1**; garbage `engine=` ⇒ exit 1; `MAJOR < 150` still refused | the script is the step's `run:` body byte-for-byte (diffed against the workflow file), `TOKEN` fed through the same env indirection, expected sha derived from the checked-out `release.yml` — not typed in | T0 (local) | ✅ 2026-09-14, step body extracted from `promote.yml` by regex and run under Git Bash with `TOKEN` via env: **real token exit 0** (`engine binding OK … carries +g9ccef04+ (release.yml env.CEF_ASSET)`); `+g7dd0357+` edit **exit 1**; legacy `engine=Chrome/150.0.7871.187` **exit 1**; `engine=banana` **exit 1** (floor); `136.0.1-…+g9ccef04+…` **exit 1** (floor). ⭐ The first local run caught a bug in my own gate (`tr -d '-.'` parsed as an option ⇒ every token refused, fail-closed) — fixed to `sed`, re-run. Runner: scratchpad `p9_gate_local.py` |
| `P9-D1` | `cargo audit` ×2, `cargo outdated --root-deps-only` ×2, `npm audit` recorded as a hold/bump table in `DEPENDENCY_VERIFICATION.md` with dates and advisory ids; the export-count script prints the three numbers for `cef-binaries/Release/libcef.dll` | Export script pointed at `HodosBrowser.dll` (which statically links our OpenSSL/SQLite) shows a **different** shape — proving the count is of the file named, not a constant; `cargo audit` run against a scratch `Cargo.lock` carrying a known-yanked/advisory crate reports it (the tool is seen to find something) | script output names the file path + size + md5 of the DLL measured; the shipped DLL is the one `release.yml`'s asset asserts (`+g9ccef04+`) | T0/T2 | ⬜ |

**Two-sided rows:** `P9-A1` (dev keeps the port) and `I8` (release binds nothing) are each other's control;
`P9-C2`'s accept and refuse halves are one script.

## 5. Blast radius

- `cef-native/cef_browser_shell.cpp` — one condition added to the port block. Windows entry point only; the
  `.mm` block is Mac's (relay item 2). The picker-mode `= 0` path and the `Profile_N` offset are untouched.
- `cef-native/src/handlers/simple_app.cpp` (D3, if in scope) — **shared file, both platforms**; gating one
  `AppendSwitchWithValue` on `IsDevEnv()`. Relay row names it.
- `cef-native/src/handlers/simple_handler.cpp` (D4, if in scope) — four DevTools entry points + the non-tab
  context-menu branch. Same file as the `wallet_call` IPC arm and the context-menu IDs; `R-INTEXT` and the
  right-click *Manage Site Permissions* item are in the boundary run.
- `development-docs/0.4.0/chromium-rebuild/farbling_seed_rotation_check.py` — token content and one new
  refusal before launch. Every consumer of the token is `promote.yml`; **Mac produces tokens too** (relay).
- `.github/workflows/promote.yml` — instrument; own commit (rule 6); reason in `FARBLING_RELEASE_GATE.md`.
- Fork `Hodos-Browser/cef` — two new tag refs; no branch moves, no deletions, no working-tree change.
- Docs: `DEVTOOLS_SECURITY_DESIGN.md` status, `TESTING.md` §14.7 note, `INSTALL_TEST_BATCH.md` I8,
  `cef-native/CLAUDE.md` + `CEF_BUILD_RUNBOOK.md` convention, `FARBLING_RELEASE_GATE.md`,
  `DEPENDENCY_VERIFICATION.md`, `CEF_VERSION_UPDATE_TRACKER.md`, `PRIOR_ART.md` (one row), `SPRINT_PLAN.md`,
  `MAC_RELAY_BETA3.md`.

## 6. Out of scope

- Deleting the `pin-*` **branches** on the fork (ticket 2 step 2) — owner said not this phase.
- Any dependency **bump**, including `cargo update`, `npm update`, vcpkg baseline, Inno 7.x — ticket 4 reports.
- Flipping `test.yml`'s audits from informational to blocking, or the dev fork's Actions quota — CI is dark;
  reported, not touched. Nothing here runs on the private repo's minutes.
- The design's Q3 pipe-mode spike and Q4 authenticated CDP — carried, not built.
- Widening the token beyond canvas (the harness docstring's explicit follow-up).
- `cef-binaries-backup-gc636546/` disposition — owner (§8 Q4).
- `stray_log_in_install_root` T2/T3 — I5.
- The macOS mirror edits (`cef_browser_shell_mac.mm` port block; `generate-appcast.py` verification on Sparkle).

## 7. Rollback

Each ticket is its own commit(s): revert the shell commit to reopen the port; `git push origin
:refs/tags/pin-…` to remove a tag (the branches still hold the SHAs); revert the harness commit and the
`promote.yml` commit independently (the old `≥150` gate accepts the new token shape, so either order is
safe); the ticket-4 commit is docs + one script.

---

## 8. Open questions for the owner — ✅ answered 2026-09-14

👤 **Q1 (b) now, D4 as the phase's last commit · Q2 yes · Q3 yes · Q4 keep and record · Q5 accept in writing, and put it in the Mac relay so Mac can say whether they want the tap · Q6 yes · Q7 mark 7/8/9 done, or "pending Mac updates" where a macOS half is still owed.**

| # | Question | My recommendation |
|---|---|---|
| **Q1** | **Scope of ticket 1.** (a) D2 only — gate the port block, as the session prompt says. (b) D2 + D3 — also gate the `--remote-allow-origins=*` append on `IsDevEnv()` (one line, shared file, same commit per the design's §7; `D-2` above corrects the design's "nothing depends on it" — dev needs it, release does not). (c) (b) + **D4** as its own commit — no Inspect Element / DevTools on overlays, role-only gate, the ticket's third acceptance line, a T3 row (`P9-A3`) | **(b) now; (c) yes but as the last commit of the phase**, because the ticket's acceptance names it and the design argues it is the defence against the "paste this into the console" attack that closing the port does not remove. If you want D4 deferred, say so and `P9-A3` moves to a ticket note |
| **Q2** | **`I8` is the release-shaped proof** for ticket 1 (`D-4`): the dev safeguard makes a local release-shaped run impossible without pointing a binary at your production wallet. Accept an owed install-batch row? | Yes — same rig as I1–I7, and the ticket's own negative control needs a real install anyway |
| **Q3** | **Add the Windows md5 half to `require_engine`** (`D-6`): compare `md5(<exe dir>\libcef.dll)` to `md5(cef-binaries\Release\libcef.dll)` so the token's engine claim is tied to the binary the app loaded, not the header it was staged next to. ~10 lines, refuses before launch. Not in the ticket's text; it is in the memory that motivated the ticket | Do it — the header-only check has already false-greened once on this project, and this is the harness whose token a release gate consumes |
| **Q4** | Ticket 2 acceptance item 4: fate of `cef-binaries-backup-gc636546/` (the only built copy of the `c636546` engine; source is tagged and rebuildable at ~5 h) | Keep it, record it in `cef-native/CLAUDE.md`'s pin section as "local-only built copy, no CI asset"; decide deletion at 0.4.0 release, not here |
| **Q5** | Ticket 4: **macOS dependency float** — accept in writing (release notes' reproducibility section states OpenSSL/sqlite3/nlohmann float to Homebrew's current formula) or escalate to a `brew extract` Hodos tap | Accept in writing for 0.4.0; the tap is real work on Mac's plate and no macOS build has broken on drift |
| **Q6** | Ticket 4 tooling: install `cargo-audit` and `cargo-outdated` on this box (`cargo install`, build-host tools, not project dependencies) | Yes — CI already installs `cargo-audit`; nothing in the repo changes |
| **Q7** | `SPRINT_PLAN.md` §4's status line still reads `⬜ 7 · 8 · 9 · 10` although 7 and 8 closed (7d `f0c9282`, 8d `17f9e28`). When marking 9 done, also mark 7 and 8? | Yes — same line, same edit, both facts verifiable from the phase contracts |

**Assumptions I will proceed on unless told otherwise:** the fork tags are lightweight (matching
`pin-c636546/7871`); each ticket is one commit (+ the separate instrument commit for `promote.yml`), each with
its own `preflight.ps1 -Full` and push; the relay round `2026-09-14b (Windows)` opens at the top of
`MAC_RELAY_BETA3.md` and tasks Mac in the prompt's order (appcast floor → CDP `.mm` mirror → their standing
three, referenced not re-listed); Mac's rotation tokens change shape with ticket 3, so the relay says so.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed (I8 owed, visibly)
- [ ] `scripts/preflight.ps1 -Full` run — result + date recorded below
- [ ] `scripts/preflight.ps1 -NegativeControl` run — every T0 gate seen to fail
- [ ] `../REGRESSION_SET.md` run in full at this boundary — result recorded
- [ ] Adversarial review complete, four questions answered in writing
- [ ] No baseline moved (`../HARNESS.md` §4) — confirm, do not assume
- [ ] Commit messages cite the row IDs they satisfy

| Item | Result | Date | By |
|---|---|---|---|
| preflight -Full | | | |
| preflight -NegativeControl | | | |
| regression set | | | |
| adversarial review | | | |
