# Session prompt — beta.3 Phase 9: release readiness

Paste the block below into a fresh session. Everything above the line is context for whoever is
assembling it; the prompt itself starts at **PROMPT**.

**Written 2026-09-14**, at the close of Phase 8d (Windows). Base commit: `8e39abc` on `origin/0.4.0`,
pushed and verified with `git fetch` (0 ahead / 0 behind at the time of writing).

---

## PROMPT

Start beta.3 **Phase 9 — release readiness**, and work through **all of it** in the order below.
Phase 9 is "everything that gates *promotion* rather than behaviour": promotion blockers plus DevOps
hygiene (`SPRINT_PLAN.md` §4.1). Owner has assigned the whole bundle (2026-09-14). Base: `8e39abc`.

### Standing rules — read before anything else

1. `CLAUDE.md` (root) top to bottom: the six working rules, the phase kickoff workflow, the negative-control
   rule, the Dev Runbook. ⛔ **Never stop a Hodos process by image name** — `scripts/stop-dev.ps1`, or a
   `Where-Object { $_.ExecutablePath -like '*rust-wallet\target\release*' }`-style path filter. The owner's
   **installed** wallet and adblock (`%LOCALAPPDATA%\HodosBrowser`, ports **31301 / 31302**) run all the time
   and are never touched. Dev is `HODOS_DEV=1`, ports 31401 / 31402, CDP 9322.
2. `development-docs/0.4.0-beta.3/HARNESS.md` — every acceptance row is `GREEN | RED | SUBJECT`; a skip is
   never a pass; rule 6 (the instrument is not edited by the change it measures — preflight baselines and
   `REGRESSION_SET.md` move in their own commit, with `-NegativeControl` re-run).
3. Memory: start at `MEMORY.md`'s first entry (P8d) and read `project_p8d_wallet_supervision_2026_09_14`,
   `reference_cef_file_thread_ids_are_one_thread`, `feedback_never_kill_by_image_name`,
   `reference_vite_hmr_fakes_negative_controls`, `project_beta3_windows_mac_deconfliction_protocol`.
   ⛔ `NOTES_parallel_work.md` is untracked on purpose — read it, never commit it. Never `git checkout` /
   `switch` in this directory; `git branch --show-current` (must be `0.4.0`) before any writing git command.
4. **Relay rule** (root `CLAUDE.md`, Branch & Remote Workflow): no CI compiles C++ on push. Every commit that
   touches `cef-native/**` C++ gets a row in the current relay round naming the files, with `#ifdef` splits and
   any `*_mac.*` touch called out. Write it in **`MAC_RELAY_BETA3.md`** (newest round FIRST, at the top) —
   this phase opens a **new round, `2026-09-1x (Windows)`**. `git fetch` before every push and rebase over
   Mac's commits if any; the deconfliction memory says Mac has not pushed since `5710742`.
5. Rig, when a row needs the browser: dev wallet (`.\dev-wallet.ps1` or `rust-wallet` + `HODOS_DEV=1`),
   Vite (`cd frontend && npm run dev`), dev exe with `HODOS_DEV=1 --profile=Default --remote-debugging-port=9322`
   from `cef-native\build\bin\Release`. Hard-reload before any React measurement. Stop the dev browser (by
   path) before `cmake --build` or the link fails `LNK1104`. At session end: `scripts/stop-dev.ps1` **and** the
   Vite node process (match its command line on `Hodos-Browser\frontend`), then confirm 31301/31302 still up.

### ⛔ Kickoff first — no code until it is handed back

Per the root `CLAUDE.md` kickoff workflow: read every ticket below **whole**, verify each cited symbol on
today's tree (line numbers rot; symbols survive), do the reuse-first audit, write
`development-docs/0.4.0-beta.3/phase-9-release-readiness/PHASE_CONTRACT.md` from
`PHASE_CONTRACT_TEMPLATE.md` (all seven `HARNESS.md` §1 sections; `D-n` for every plan-vs-tree delta;
one evidence row per ticket with GREEN / RED / SUBJECT named), and **hand back a tight summary with the
open questions before the first commit.** Two things already known to be stale, so expect deltas:

- `TICKET_dependency_freshness_review.md` says `frontend/package.json` has no `browserslist` and no
  `engines`. **It now has both** — verify what they pin and whether the Vite target is tied to the shipped
  Chromium (`CEF_VERSION` in `cef-binaries/include/cef_version.h`, read it, do not quote it).
- `TICKET_stray_log_in_install_root.md` is **not Phase 9 work**: its owed T2/T3 rows are batched as
  `INSTALL_TEST_BATCH.md` row I5. Do not do it here; just confirm the row is still accurate.

### The order, and why

| # | Ticket | Why here | Shape |
|---|---|---|---|
| 1 | `TICKET_cdp_port_open_in_release.md` | **Security, shipping, decision already approved** (`0.4.0/DEVTOOLS_SECURITY_DESIGN.md` D2, 2026-08-04, never built). A release Hodos on the Default profile listens on `127.0.0.1:9222` and CDP sees the wallet and auth overlays as ordinary pages | `cef_browser_shell.cpp :: RunHodosMain` (the `settings.remote_debugging_port` block near "Remote debugging port:") — `IsDevEnv()` only *offsets* the port, it does not gate. Gate it: **dev keeps it (the rig needs 9322), release binds nothing.** ⚠️ The same block exists in `cef_browser_shell_mac.mm` — **Mac mirrors it (relay item below), do not edit the `.mm`.** GREEN: dev build still answers on 9322. RED for the release half: the design's §6 test plan; a release-shaped run is an **install-batch row** (add it to `INSTALL_TEST_BATCH.md`, next to I4/I5) — say so in the contract rather than claiming it measured. Also honour the design's note that TESTING.md §14.6's smoke harness uses the **dev** port and must keep working |
| 2 | `TICKET_engine_pins_are_branches_not_tags.md` | Minutes, removes a silent-failure mode on the shipping engine's source ref | On the fork clone `C:\cef\cef150\chromium\src\cef` (remote `origin` = `Hodos-Browser/cef`): tag `pin-9ccef04/7871` and `pin-7dd0357/7871` at the SHAs the branches point to (verify with `git rev-parse` first; the ticket lists them), push the tags. ⚠️ **Pushing to the fork needs the owner's GitHub auth on this box** — if the push is refused, stop and hand the two exact `git tag` / `git push origin <tag>` commands to the owner to run with `!`. Then the convention note in `cef-native/CLAUDE.md` + `CEF_BUILD_RUNBOOK.md` ("a pin is a tag"). ⛔ Do **not** delete the branches in this phase |
| 3 | `TICKET_farbling_gate_engine_binding.md` | The release gate binds a verdict to "Chromium ≥ 150", which every engine satisfies; a token from the wrong engine passes | `development-docs/0.4.0/chromium-rebuild/farbling_seed_rotation_check.py`: `require_engine()` (line ~649) exists and has **zero call sites**; the token line carries `engine_version()` (Chromium version) instead of the `+g<sha>` engine id. Wire `require_engine`, put the engine id in the token, and make `promote.yml`'s re-derive step parse and compare **that** — not `MAJOR`. GREEN: a token from the dev browser carries `g9ccef04`-style id and the parse accepts it. RED: the same token with the id edited to another engine is **rejected** by the same parse (run the step's shell locally; no CI minutes on the private repo). `--negative-control` must still invert the exit code. Rule 6 applies: `promote.yml` is an instrument — its own commit, reason in `FARBLING_RELEASE_GATE.md` |
| 4 | `TICKET_dependency_freshness_review.md` | Process gap: pinned well, never re-evaluated since DEP-1 (2026-08-03) | One review pass now (`cargo audit` / `cargo outdated` on `rust-wallet` and `adblock-engine`, `npm audit` on `frontend`, vcpkg baseline for OpenSSL / sqlite3 / nlohmann) + the symbol-coexistence re-measure the ticket describes (export count of the shipped `libcef.dll`) + a written cadence in `DevOps-CICD/` (which file: the dependency-verification checklist the root `CLAUDE.md` invariant 12 names). ⛔ **Report** upgrades; do not bump a dependency in this phase without the owner — a money-handling binary is not upgraded as a side effect of a review |
| 5 | Phase close | — | Contract sign-off table filled; `SPRINT_PLAN.md` §4 marks 9 done; relay round written; memory updated; dev env stopped |

⭐ Every C++ change here is **Windows-only or shared** (`cef_browser_shell.cpp`); nothing in this phase
touches Rust or the schema. Each ticket its own commit, own `scripts/preflight.ps1 -Full` (bare is not a
pass — it skips the frontend build), own push.

### 🍎 Mac — task them in the relay round, in this order

Write these into the new Windows round at the top of `MAC_RELAY_BETA3.md` (they already have three items
queued from the 2026-09-14 round — do not re-list those, reference them):

1. **`TICKET_appcast_missing_minimum_system_version.md` — THEIRS, and it is the promotion blocker.**
   `scripts/generate-appcast.py` never emits `<sparkle:minimumSystemVersion>`; the macOS floor moved 11.0 → 12.0
   with CEF 150, and a Big Sur user on 0.3.x would be offered 0.4.0, install it, and be left with a browser
   that will not launch. The script is platform-neutral Python but the **verification is Sparkle on a Mac**:
   emit `12.0` for the macOS item (from `MACOSX_DEPLOYMENT_TARGET`, not a literal), regenerate the draft
   appcast, and prove a Sparkle client below the floor is *not* offered the update. CLAUDE.md invariant #13:
   the ticket was filed for approval rather than fixed — the owner's Phase 9 assignment **is** that approval.
   🚦 It must close before promotion; that is a harder constraint than phase order.
2. **`cdp_port` mirror** — after Windows lands ticket 1, apply the same gate to the identical block in
   `cef_browser_shell_mac.mm` (near "Remote debugging port:"), keep dev on 9322, verify with `lsof` that a
   non-dev launch binds nothing. Small; the Windows commit's relay row will name the exact shape.
3. Their standing three from the 2026-09-14 round, unchanged in order: 8c M7 column → 8c M8 → 8d `P8d-A8`.

### Stop points — the owner's, not yours

- Any push to the `Hodos-Browser/cef` fork (ticket 2) that the box's auth refuses.
- Any dependency **bump** (ticket 4 reports; it does not upgrade).
- Anything that would need the private repo's Actions minutes — run gate logic locally instead.
- If ticket 1's gate cannot keep the dev rig's CDP working, stop and present the options from the design
  doc §5 Q3 (pipe mode / one-time flag) rather than choosing.

When the phase is done: contract status line ✅, relay round complete, memory (`project_p9_release_readiness_<date>`
+ the `MEMORY.md` one-liner), `stop-dev.ps1` + Vite stopped, installed 31301/31302 confirmed untouched,
and a closing summary that lists what Mac still owes.
