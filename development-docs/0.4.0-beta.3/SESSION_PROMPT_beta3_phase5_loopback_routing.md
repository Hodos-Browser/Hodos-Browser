Start beta.3 Phase 5 — loopback routing & trust boundary (WS5b). ⛔ **Kickoff has NOT been run.
Run it before any code** — and expect to delete scope, not add it (see §3).

# 0. Read first, in this order

1. Auto-loaded `MEMORY.md`, then `project_p4_tabmenu_landed_2026_09_01.md` (Phase 4 just closed).
2. ⭐ **`TICKET_loopback_host_form_wallet_routing.md`** — the ticket this phase exists for, including
   its §13 verification log. **`SPRINT_PLAN.md` §WS5 is the scope statement, but read the ticket
   first**; the plan compresses it and §3 below shows the compression has gone stale.
3. `HARNESS.md` (tiers, ratchets, §4, §8, §9) and `REGRESSION_SET.md` — **especially `R-INTEXT`**,
   which this phase can break more directly than any phase so far.
4. `phase-4-tab-peripheral-parity/MEASUREMENTS.md` M11 (what stayed manual) — the same T3 rig and the
   same CDP limits apply here.

⛔ **Do not create `phase-5-*/` and start writing.** There is no phase folder yet, deliberately: the
first deliverable is a contract that says what is *left*.

# 1. State

✅ **Phase 4 CLOSED and pushed** (`0b68727` menu, `420b722` mute, `21db88e` relay, `8de7e78` ticket
move). `HEAD` == `origin/0.4.0` at `8de7e78`. ⛔ **`git fetch` and read `git log HEAD..origin/0.4.0`
yourself — the Mac side pushes to this branch.**

✅ Dev env last left **running** (wallet 31401, Vite 5137, dev CDP 9322). ⚠️ The owner's **installed**
browser and its wallet (31301) are also running — see §7.

# 2. ⭐ The load-bearing insight — read this twice

⛔ **The C++ interception layer is not only a permission gate. It is the component that marks traffic
as UNTRUSTED.**

`domain_trust_mw` (`rust-wallet/src/main.rs`, ~line 63) reads `X-Requesting-Domain`; a **missing**
header means *internal, fully trusted, ungated*. C++ is what attaches that header, and it only does so
for traffic it recognises as coming from a page.

⇒ **A matcher that fails to match does not leave traffic ungated. It leaves it TRUSTED.**

🚨 Every narrowing of a predicate in this phase is therefore a **privilege change**, and the naive
structural fix — "replace the loose substring test with a strict parsed one" — is a **regression** in
the safe direction *only if* you also prove nothing that used to be gated has stopped being gated.
⭐ That is the whole difficulty of this phase. Plan the evidence for it before writing the predicate.

# 3. 🚨 MEASURED 2026-09-01 — the plan's scope is partly ALREADY DONE. Verify before building.

`SPRINT_PLAN.md` §WS5(b) lists "W0 + W1 + W2' + W3: a parsed `IsWalletOrigin()` replacing the
six-term substring gate, `/health` added to `isWalletEndpoint`, and instrumentation." 📏 Two of those
landed in **Phase 0.5** (2026-08-19) and the plan was never updated:

| Plan says | 📏 Measured in the tree, 2026-09-01 |
|---|---|
| *"`/health` added to `isWalletEndpoint`"* | ⛔ **ALREADY THERE.** `HttpRequestInterceptor.cpp :: isWalletEndpoint` opens with a `/health` arm, host-scoped via `hodos::IsWalletHostPort`, with a comment explaining it must stay host-qualified |
| *"the six-term substring gate"* | ⛔ **Already three-quarters replaced.** `simple_handler.cpp` (~9074) now reads `IsWalletHostPort(url) \|\| IsLoopbackHostPort(url,"3321") \|\| IsLoopbackHostPort(url,"2121") \|\| IsLoopbackHostPort(url,"8080")` — the bare `url.find("localhost:3321")` literals are gone |
| — | ⭐ **What IS still a bare substring: `url.find("messagebox.babbage.systems")` and `url.find("/.well-known/auth")`.** Unanchored ⇒ `https://evil.example/?x=messagebox.babbage.systems` matches. **That is the live remnant of the defect class** |

⇒ **This is the fourth sprint doc in a row whose claims did not survive measurement** (after P3.5's
contract, K7, and P4's two §WS3 claims). ⛔ **Inventory against the tree before believing the plan**,
and write the delta into the contract as `§0`, the way Phase 4's contract does.

⚠️ **Do not treat "already done" as "phase cancelled".** Real work remains: the two unanchored terms,
the `IsWalletOrigin()` consolidation, **W3 instrumentation**, and §4.

# 4. 🎯 The claim that must be MEASURED, not inherited

The plan asserts a live **cross-wallet routing hole**: *"an App Lab request inside Hodos today falls
past our gate and is answered by a different vendor's wallet — different identity key, no Hodos gate,
no indication to the user."*

📏 **Half-refuted already, and you must settle it first** because it decides whether this phase is
urgent or tidy-up:
- ✅ **MetaNet Client is genuinely running on this machine** — `127.0.0.1:3321` **and** `:2121`,
  `LISTENING`, PID 37360, re-verified 2026-09-01.
- ⛔ **But we now intercept both ports** (§3), and `IsLoopbackHostPort` checks *both* host spellings.
  So the specific hole the plan describes **may already be closed**.

🎯 **SUBJECT: what the wallet actually received**, not what C++ logged it intended. Drive a real
`fetch('http://127.0.0.1:3321/getVersion')` **and** the `https://127.0.0.1:2121` form from a real
external page, and read **our** Rust log for the request. If it never arrives, find out who answered.
⭐ The negative control is free and decisive: **stop MetaNet Client** and see whether the behaviour
changes. If it does, we were not intercepting.

📌 Datapoint from the Phase 4 close-out: `chaintap.utxoengineer.com` pays successfully through
`window.CWI` → our bridge → our wallet. So the **shim** path is healthy; this phase is about the
**HTTP-probe** path, which is a different discovery mechanism. Do not generalise one to the other.

# 5. ⛔ Scope fence

**IN:** the two unanchored substring terms · `IsWalletOrigin()` consolidation · `/health` **verify,
not add** · W3 instrumentation · settling §4.

**OUT — `SPRINT_PLAN.md` is explicit:** ⛔ **W4 / W6 / W7 / W8 are beta.4.** *"Do not take the whole
plan into beta.3 — it rewrites the routing predicate for every network request in the browser."*
⛔ macOS: relay, never claim.

# 6. Evidence — where this phase is unusually exposed

⚠️ **`R-INTEXT` is the standing check this phase can break, and its RED has never been run** — both
prior boundaries recorded *"the injected RED was not re-run; it needs a code change."* **This is the
phase that will already be changing that code.** ⇒ ⭐ **budget for finally running R-INTEXT's real
RED**: force internal-as-external (stub the origin derivation to always emit the header) and
external-as-internal (suppress it), and observe both. Closing that is arguably worth more than the
refactor.

⚠️ `IsInternalOrigin("")` returns **`true`** (`HttpRequestInterceptor.cpp`). A frame URL with no
`://` collapses to *internal*. It is the one path by which external silently becomes trusted — and
this phase touches exactly that neighbourhood. Put it in the contract's blast radius.

# 7. 🚨 Machine state — and a rule that was learned the hard way

⛔ **The owner's INSTALLED browser and wallet are running** (~71 processes, wallet on **31301**).

⛔⛔ **NEVER stop a Hodos process by image name.** All three — `HodosBrowser.exe`, `hodos-wallet.exe`,
`hodos-adblock.exe` — share their name between the dev and installed builds.

⭐ **Use the script:** `.\scripts\stop-dev.ps1` (add `-WhatIf` to preview). It is path-matched and was
negative-controlled: with both builds live it spared **73** installed processes and targeted 25 dev
ones. The rule is in `CLAUDE.md`'s Dev Runbook.

> 🚨 Written because on 2026-09-01 a name-matched `Stop-Process` on `hodos-wallet.exe` — run to free
> a file lock for `cargo build` — killed the **owner's production wallet**. Their installed browser
> stayed up and told every dApp *"no wallet"* for ~4 hours, and they lost real time diagnosing a site
> that was never broken. ⚠️ `cargo build` fails *"Access is denied"* and the linker fails `LNK1104`
> while dev processes run — that pressure is exactly what produces a hasty kill.

**To bring the dev env up:** `.\dev-wallet.ps1` → `cd frontend && npm run dev` → `Start-Process` the
dev exe with `$env:HODOS_DEV='1'` and `--profile=Default` (CDP then on 9322). ⛔ A detached bash `&`
launch comes up minimized.

# 8. The rig — reuse it

`phase-3.5-layout-window-scoping/` — `p35drive.py` (CDP: `list` · `key` · `send` · `eval`),
`winprobe.ps1 -WatchSeconds`. ⚠️ Two headers share `http://127.0.0.1:5137/`; disambiguate by
**target-id diff across Ctrl+N**, never list order.

⛔ **CDP reaches React's handlers but not the OSR mouse path** — `SendInput` clicks are dropped in the
agent environment. Mostly irrelevant here (this phase is network-level), but it is why anything
needing a real click goes to the owner.

# 9. Deliverable

1. A **contract** whose `§0` records the plan-vs-tree delta from §3 — written *before* code.
2. §4 settled with a measurement and a named negative control.
3. The two unanchored terms closed, with a RED **observed** for each (a URL that merely *contains*
   the string must stop matching, and a legitimate one must still match).
4. ⭐ `R-INTEXT`'s injected RED finally run, both directions.
5. preflight + `-NegativeControl`; the 5 → 6 regression boundary.
6. Mac relay entry. What stays manual, said plainly.

# 10. Carried, not this phase

- 🎫 `TICKET_wallet_backend_death_is_silent_and_unrecovered.md` — 👤 owner moved it into **beta.3**,
  Phase 8 the natural home. 📏 Cross-platform. ⛔ Not Phase 5.
- 🎫 `TICKET_token_outputs_destroyed_by_dust_paths` — 👤 owner: **stays in Phase 8.** ⭐ Deadline is an
  event: must close before beta.4's 1Sat Ordinals sprint. ⛔ Do not re-escalate.
- 🔴 Phase 4 owner items still open: **O2** (click-outside dismiss), **O3** (focus half of `P4-A4`),
  **O5** (macOS), **O6** (actually *hear* a muted tab). ⭐ **O1 and O4 are CLOSED** — O1 by the owner's
  own mute test, O4 by a real 4-cent payment that produced the first observed
  `payment.auto_approved` audit line and confirmed the R-GOLD identity translation
  (`cefBrowserId=17 → tabId=5`).
- ⛔ `G11` stays at **60** and cannot be lowered — reason in `HARNESS.md` §4.

# 11. Working style

⭐ Walk the owner through anything they must click: what to do, what they should see, **and what a
wrong result would mean.**
⭐⭐ **Check your own work before sending them.** Every defect this sprint found late was found by a
two-minute owner test of something already reported as done.
⛔ **Ask before narrowing a predicate.** §2 means a matcher change is a privilege change; if two
readings of "should this still be gated" are defensible, present both and ask — do not pick silently.
