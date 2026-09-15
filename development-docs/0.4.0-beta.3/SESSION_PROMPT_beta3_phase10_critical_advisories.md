# Session prompt — beta.3 Phase 10: critical advisories (+ Phase 13 research in parallel)

Paste the block below into a fresh session. Everything above the line is context for whoever is assembling it;
the prompt itself starts at **PROMPT**.

**Written 2026-09-15**, after the Phase 9 close (`07f9fcf`), the dependency bumps (`ec6353e`) and the owner's
re-plan of Phases 10–13. Base commit: the tip of `origin/0.4.0` that carries the re-plan (`git log --oneline -1`
shows "docs(beta.3): re-plan Phases 10–13").

---

## PROMPT

Start beta.3 **Phase 10 — critical advisories**, and run the **Phase 13 research half** alongside it. Phase 10 is
three money-path defects from `development-docs/0.4.0-beta.3/CRITICAL_UPDATES.md`, present in the shipped
beta.29 by code reading, run as sub-phases **10a CU-3 (+CU-6) → 10b CU-1 (+CU-8, CU-9) → 10c CU-2**. The plan,
the owner's decisions and the harness requirements are in
`development-docs/0.4.0-beta.3/phase-10-critical-advisories/README.md`; the three contract stubs are in its
sub-folders. Read `../SPRINT_PLAN.md` §4/§4.1 for how 10–13 were re-cut.

### Standing rules — read before anything else

1. `CLAUDE.md` (root) top to bottom: the six working rules, the phase kickoff workflow, the negative-control rule,
   the Dev Runbook. ⛔ **Never stop a Hodos process by image name** — `scripts/stop-dev.ps1`, or a
   `Where-Object { $_.ExecutablePath -like '*rust-wallet\target\release*' }`-style filter. The owner's **installed**
   wallet and adblock (`%LOCALAPPDATA%\HodosBrowser`, ports **31301 / 31302**) run all the time and are never
   touched. Dev is `HODOS_DEV=1`, ports 31401 / 31402, CDP **9322** — and since Phase 9 the dev exe must be
   launched **without** `--remote-debugging-port` on the command line (that switch binds CDP regardless of the
   settings gate); `--profile=Default` only, then poll `/json/version`.
2. `development-docs/0.4.0-beta.3/HARNESS.md` — every acceptance row is `GREEN | RED | SUBJECT`; a skip is never a
   pass; rule 6 (the instrument is not edited by the change it measures — `REGRESSION_SET.md`'s new
   `R-ONE-CLICK-ONE-SPEND` row lands in its **own** commit after 10b). ⛔ **This phase's REDs are the defects
   themselves** and must be *observed* before each fix: a fabricated PeerPay credited (unit), two txids from one
   click (real cents), a 10× paymail broadcast in `noSend`. A fix whose defect was never seen is a fix for a
   hypothesis. The boundary regression's **T2 halves run this time** (Phase 9 recorded them INCOMPLETE; 10b
   rewrites the prompt/resume path that `R-INTEXT` and `R-COUNT` measure).
3. Memory: start at `MEMORY.md`'s first entry (P9) and read `project_p9_release_readiness_2026_09_14`,
   `feedback_never_kill_by_image_name`, `reference_vite_hmr_fakes_negative_controls`,
   `project_beta3_windows_mac_deconfliction_protocol`, `reference_arc_tx_status_ladder`,
   `feedback_own_work_is_the_weakest_link`. ⛔ `NOTES_parallel_work.md` is untracked on purpose — read it, never
   commit it. Never `git checkout` / `switch` here; `git branch --show-current` (must be `0.4.0`) before any writing
   git command. `development-docs/X402_INTEGRATION.md` may carry the owner's uncommitted edits — leave it out of
   every commit.
4. **Relay rule**: every commit that touches `cef-native/**` C++ gets a row in the current relay round naming the
   files (10b touches the shared `HttpRequestInterceptor.cpp`). Open a new Windows round at the top of
   `MAC_RELAY_BETA3.md`; the 2026-09-15 round already told Mac what is coming. `git fetch` before every push and
   rebase over Mac's commits if any (Mac is being caught up now — expect pushes).
5. Rig: dev wallet (`.\dev-wallet.ps1` or the release binary with `HODOS_DEV=1`), Vite, dev exe as above. 10b needs a
   **local test dApp** on a scratch port that the wallet gate treats as *external* (check `IsInternalOrigin` and the
   G12 matchers first — Phase 5's lesson is that a matcher miss leaves traffic *trusted*). 10c needs a **stub paymail
   host**; nothing from it is ever broadcast (`noSend`). Real money: 10b's A1/A3 and 10a's A5 are cents, recorded in
   `PAYMENT_TEST_BATCH.md`. Hard-reload before any React measurement. Stop the dev browser by path before
   `cmake --build`. At session end: `scripts/stop-dev.ps1` **and** the Vite node process, then confirm 31301/31302.

### ⛔ Kickoff first — no code until it is handed back

Per the root `CLAUDE.md` kickoff workflow: read `CRITICAL_UPDATES.md` **whole**, verify every cited symbol on today's
tree for all three sub-phases (line numbers rot), do the reuse-first audit, fill the `D-n` deltas and the
`Result` cells' *planned runs* in the three contract stubs, and hand back a tight summary with open questions
before the first commit. Things to expect:

- The owner has **already decided** (README, "Owner decisions"): no CU-3 stopgap (auto-accept stays on); order
  CU-3 → CU-1 → CU-2; bursts fixed at **every** arrival path; the user told about a burst **once**; CU-4/5/7
  re-read at this kickoff and scheduled then. Do not re-litigate; do report anything the tree contradicts.
- **The one open design question is 10b §2a** — how an over-limit burst is presented (Queue / Deny / Summarise).
  Bring a recommendation *with the tree evidence* and ask; the invariant either way is *a signature exists only
  for an amount the user saw on the modal that produced the click*.
- 10c: read the **bsvalias P2P payment destination spec** before coding — confirm "outputs sum to the requested
  amount" is the spec's rule (rule 4). Paymail is live in the UI; treat it as shipped.
- The kickoff also decides whether CU-6 truly shares CU-3's parser fix (`beef.rs` callers), and lists every
  caller of `store_derived_utxo` and `from_atomic_beef_bytes`.

### Phase 13 in parallel — research only, no code, no rig conflict

`development-docs/0.4.0-beta.3/phase-13-bot-detection/README.md` steps 1–3: enumerate the vendors and their public
demo pages, enumerate the signals they score (docs + literature; log `PRIOR_ART.md` rows), then build and run the
**matrix** — Chrome on the same machine as the positive control, Hodos dev with farbling/adblock toggled as the
negative controls, three runs per cell, and ⛔ one cell **made** to fail on purpose so the matrix is seen to
detect a bot verdict. Deliverable `P13-M1`. Fixes are **not** this session; the matrix sizes them. Use idle time
while 10a/10c builds run. 🍎 Mac will run the same matrix — write it so they can.

### Order

1. Kickoff (both halves) → hand back → wait.
2. **10a** (Rust): unit REDs seen → fix → `P10a-A1..A6`; A5 is the live two-wallet PeerPay (ask the owner / Mac
   for the second wallet). Own commit, `preflight -Full`, push.
3. **10b** (C++ shared + React + Rust): test dApp up → `P10b-A1` RED with real cents (two txids from one click,
   recorded) → fix per the §2a decision → all eight rows → own commits → `R-ONE-CLICK-ONE-SPEND` in its **own**
   commit → relay row (shared C++, modal change) → push.
4. **10c** (Rust): stub host → `P10c-A1` RED in `noSend` → fix → rows → commit → push.
5. **Adversarial panel** over all three (`HARNESS.md` §6; money path ⇒ fan-out), four questions in writing.
6. Boundary regression **with T2 halves**; contract sign-offs; `SPRINT_PLAN.md` marks 10 done; relay round;
   memory (`project_p10_critical_advisories_<date>` + the `MEMORY.md` one-liner); dev env stopped; 31301/31302
   confirmed; closing summary lists what Mac owes and the Phase 13 matrix result.

### Stop points — the owner's, not yours

- The 10b §2a presentation decision (Queue / Deny / Summarise).
- Any change to crypto, signing, derivation or the DB schema (invariants #2/#3) — the fixes here should need none;
  if one seems to, stop.
- Anything that needs the private repo's Actions minutes.
- Any real-money row beyond the cents the contracts name.
- The ARC key: the owner is rotating it; if the new key is available as an env var / secret by the time you start,
  the small commit that moves `arc_taal.rs` to `option_env!` is in scope; if not, leave the file alone and say so.
