Start beta.3 Phase 6 — Chrome import (WS4). ⛔ **Kickoff has NOT been run. Run it before any code.**
⚠️ This phase is different from every other one in the sprint: **the first deliverable may be a
recommendation to cut it.** Read §2 before you plan anything.

# 0. Read first, in this order

1. Auto-loaded `MEMORY.md`, then `project_p5_loopback_landed_2026_09_02.md` (Phase 5 just closed).
2. ⭐ **`SPRINT_PLAN.md` §2 "#4 hits a hard external wall"** — the constraint table. Then §WS4 (four
   lines) and §6 "Decisions owed" items **1, 2 and 3**. ⛔ Three of the five open decisions in this
   sprint belong to this phase and **none of them are engineering calls.**
3. `HARNESS.md` (tiers, ratchets, §4, §8, §9) and `REGRESSION_SET.md`.
4. `cef-native/include/core/ProfileImporter.h` + `src/core/ProfileImporter.cpp` — **the thing you are
   extending.** It already imports Chrome/Brave/Edge bookmarks and history.

⛔ **Do not create `phase-6-*/` and start writing.** The first deliverable is a contract, and its §0
has to answer whether this phase should exist at all.

# 1. State

✅ **Phase 5 CLOSED and pushed.** Sprint order `0 · 0.5 · 0.6 · 1 · 2 · 3 · 3.5 · 4 · 5` complete.
⛔ **`git fetch` and read `git log HEAD..origin/0.4.0` yourself — the Mac side pushes to this branch.**

⚠️ Landed outside the phase sequence on 2026-09-02: `047c3bb` fixed `/signAction` to return BRC-100's
`tx` (AtomicBEEF bytes) instead of `rawTx` hex, after a live `beta.zanaadu.com` failure. Two
follow-ups were filed and **not** fixed — `sendWith` silently ignored, and a fatal broadcast still
returning `200`. Both are Phase 8 candidates. See `TICKET_signaction_response_not_brc100_shape.md`.

# 2. ⭐ The load-bearing fact — read this before scoping

⛔ **The goal as originally written is not achievable, and the reason is external to us.**

Item 4's stated aim is *"import Chrome data — profiles, passwords, cookies, history — so a user's
setup **just works** as on their other computer."*

Chrome encrypts cookies and passwords with **DPAPI bound to the Windows user account**, and since
Chrome 127 with **App-Bound Encryption tied to the Chrome binary itself**:

| Data | Same machine | Different machine |
|---|---|---|
| Bookmarks, history | ✅ portable — **we already do this** | ✅ |
| Passwords | ⚠️ DPAPI/ABE | ⛔ only via Chrome's own user-initiated CSV export |
| Cookies / sessions | ⚠️ DPAPI/ABE | ⛔ effectively impossible |

⇒ *"as on their other computer"* cannot be delivered for encrypted data by any route Chrome itself
does not provide. **Chrome's own answer is account sync.**

🚨 **And there is a second, independent objection that is ours, not Chrome's:** importing another
browser's **live logged-in sessions into a wallet browser** is a security-surface decision, not a
feature. `SPRINT_PLAN.md` flags it explicitly. Do not treat it as a UX detail.

⇒ ⭐ **The honest scope is much smaller than the ask**, and the plan already says so: WS4 is *"last:
most able to balloon, and its value is capped by a constraint we do not control. **First candidate to
cut.**"*

# 3. 🎯 What must be settled BEFORE any code — and it is not yours to settle

`SPRINT_PLAN.md` §6 owes three decisions on this phase. Present them **together, with a
recommendation, in one message** (`CLAUDE.md`'s scoping rule), then wait.

| # | Decision | Why it cannot be defaulted |
|---|---|---|
| **1** | Scope: *"bookmarks + history + passwords-via-CSV, same machine, one button"* — or a full research pass first? | These are different sizes and different risks. The CSV path is the only lawful password route |
| **2** | UX: auto-detect the local profile, folder-picker, or offer at first-run? **And whether importing live sessions into a wallet browser is acceptable at all** | The second half is a security posture question |
| **3** | **Cut line — does WS4 slip out of beta.3 entirely?** | 👤 Owner call. Phases 7–10 (consent surface, money-path, release readiness, UI leftovers) are all queued behind it, and several carry shipping defects |

⭐ **Recommend honestly.** If measurement says the deliverable is "we already import bookmarks and
history; the rest is blocked by ABE", then say that plainly and let the owner decide whether the
remaining slice earns a phase. A phase that ships a paragraph explaining why it was cut is a **good**
outcome here — it is the one item in the sprint whose value is capped by someone else's design.

# 4. 📏 Inventory against the tree first — four sprint docs in a row have been wrong

⛔ **This is the fifth kickoff. The previous four each found the plan's claims did not survive
measurement** (P3.5's contract, K7, P4's two §WS3 claims, P5's `/health` + "six-term gate"). Assume
nothing; measure and write the delta into the contract as `§0`, the way Phase 4 and Phase 5 do.

Specific things to verify rather than believe:

- **What `ProfileImporter` actually does today.** `CLAUDE.md` says Chrome/Brave/Edge bookmarks +
  history, **no cookie import**, and a Firefox stub (`GetFirefoxProfilePath()` returns `""`) that is
  never called. Confirm all of it, including whether the importer is reachable from the UI at all.
- **Whether ABE actually blocks us on this machine.** The wall is documented from research, not from
  a run. ⭐ A 20-minute experiment — try to read a Chrome cookie DB value on this box — turns a
  citation into a measurement, and it is the single highest-value thing this kickoff can do.
  📖 Chrome 127+ is the ABE cutoff; check the installed Chrome's actual version first.
- **What the CSV export path really requires** of the user, end to end.
- **macOS:** Chrome uses **Keychain**, not DPAPI. ⛔ Assume no symmetry. Relay, never claim.

# 5. ⛔ Scope fence

**IN (if the phase proceeds at all):** extending `ProfileImporter` — ⛔ **not** a parallel importer ·
the UX surface for whatever scope is agreed · the security review of session import.

**OUT:** anything requiring us to defeat DPAPI/ABE. ⛔ **Do not design around Chrome's encryption.**
If a route requires extracting keys Chrome protects, it is out — say so and stop.
⛔ macOS Keychain work: relay, never claim.

# 6. Evidence — where this phase is unusually exposed

⚠️ **`R-INTEXT`, `R-GOLD`, `R-PERIM` are not obviously at risk here** — this phase touches neither the
wallet nor the trust boundary. Say so explicitly rather than leaving the rows blank, and still run the
boundary set: ⭐ **it is the phase least likely to break them and therefore the one most likely to skip
them**, and `HARNESS.md` §8 is explicit that a skipped check is SKIPPED, never a pass.

🚨 **The real risk is different in kind:** importing history/bookmarks writes to
`HistoryManager` / `BookmarkManager` SQLite. A bad import is **data corruption in the user's own
browser data**, and there is no undo. Any acceptance row needs a restore story, and the RED for it
must be *observed*, not reasoned.

⭐ **`R-GOLD` / `R-COUNT` / `R-CLOSE` are still owed from every prior boundary** — one real payment
closes the first two. If a payment happens in this session for any reason, take it.

# 7. Machine state

⛔ **The owner's INSTALLED browser and wallet are running** (wallet on **31301**).
⛔⛔ **NEVER stop a Hodos process by image name** — all three share their name with the installed
build. ⭐ Use `.\scripts\stop-dev.ps1` (`-WhatIf` to preview). It spares the installed processes and
reports the count.

⚠️ 📏 **Known defect in that script, found 2026-09-02, NOT fixed:** it dies with
`Cannot bind argument to parameter 'Path'` when invoked as `powershell -File`, because `$PSScriptRoot`
is empty during parameter binding. **Use `& '.\scripts\stop-dev.ps1'`** — that form works. Worth its
own ticket; it is the tool that exists so nobody hand-writes a kill.

**Bring dev up:** `.\dev-wallet.ps1` → `cd frontend && npm run dev` → `Start-Process` the dev exe with
`$env:HODOS_DEV='1'` and `--profile=Default` (CDP on 9322). ⛔ A detached bash `&` launch comes up
minimized.

# 8. Rig

`phase-3.5-layout-window-scoping/p35drive.py` (CDP: `list` · `key` · `send` · `eval`).
⚠️ Its `eval` does **not** `awaitPromise` — for async work, assign to a global and poll it.
⚠️ Address targets by URL substring or `#<target-id>`; two headers share `http://127.0.0.1:5137/`.
⛔ CDP reaches React's handlers but **not** the OSR mouse path — `SendInput` clicks are dropped in the
agent environment, so anything needing a real click goes to the owner.

# 9. Deliverable

1. A **contract** whose `§0` records the plan-vs-tree delta — written *before* code.
2. §3's three decisions presented together with a recommendation, and **answered by the owner**.
3. If the phase proceeds: the agreed slice, with an evidence table whose REDs were **observed**.
4. If it is cut: a short written record of *why*, in the sprint plan, so it is a decision and not a
   drift. ⭐ **This is a legitimate outcome — do not pad the phase to justify it.**
5. preflight + `-NegativeControl`; the 5 → 6 regression boundary.
6. Mac relay entry. What stays manual, said plainly.

# 10. Carried, not this phase

- 🎫 `TICKET_wallet_backend_death_is_silent_and_unrecovered.md` — 👤 owner moved it into beta.3,
  **Phase 8**.
- 🎫 `TICKET_token_outputs_destroyed_by_dust_paths` — 👤 owner: **stays in Phase 8.** ⭐ Must close
  before beta.4's 1Sat Ordinals sprint.
- 🎫 `TICKET_signaction_response_not_brc100_shape.md` §11 + §14 — `sendWith` ignored; fatal broadcast
  returns 200. Both need an owner decision. **Phase 8 candidates.**
- 🔴 Phase 4 owner items still open: **O2** (click-outside dismiss), **O3** (focus half of `P4-A4`),
  **O5** (macOS), **O6** (hear a muted tab).
- ⛔ `G11` stays at **60**; `G12` stays at **4** until beta.4's W8 retires `LegacyWalletGateMatch`.

# 11. Working style

⭐ Walk the owner through anything they must click: what to do, what they should see, **and what a
wrong result would mean.**
⭐⭐ **Check your own work before sending them.** Every defect this sprint found late was found by a
two-minute owner test of something already reported as done.
⛔ **Do not write documents nobody asked for.** 📏 2026-09-02: a 200-line overlay reference was
written off the back of the phrase *"we should have some documentation"* — which meant *"go find the
docs we already have."* Answer the question that was asked; produce an artifact only when one is
requested.
⛔ **Say "I don't know" plainly.** The same session asserted "their overlay" three times without
evidence and had to retract it.
