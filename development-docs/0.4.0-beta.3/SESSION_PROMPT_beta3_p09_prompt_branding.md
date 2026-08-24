# Session prompt — beta.3 **Phase 0.9: Hodos branding on Chromium's own prompts**

Written 2026-08-23, after P0.8 closed (`6fc35d2`) and the Phase 1 prompt was written
(`ea55abf`). The owner chose to run **0.9 before Phase 1**.

⛔ CLAUDE.md's **mandatory phase kickoff workflow applies.** The contract already exists
(`phase-0.9-chromium-prompt-branding/PHASE_CONTRACT.md`) and is unusually complete — the
kickoff's job is to **verify its cited code is still current**, not to re-plan it.

---

Paste the block below to open the session.

---

Start **beta.3 Phase 0.9 — Hodos branding on Chromium's own prompts**. ⛔ **Kickoff first;
no code until I sign off.**

This came from the bitgenius.net connect test: a Chromium loopback prompt appeared looking
like stock Chrome, standing in front of a wallet on `127.0.0.1`.

## 0. Read first

1. **Auto-loaded MEMORY.md** — especially `project_p08_consent_surface_round3_2026_08_23`
   (the consent-surface defect class, and what P0.8 left unverified) and
   `feedback_consent_surface_needs_human_eyes`.
2. `development-docs/0.4.0-beta.3/phase-0.9-chromium-prompt-branding/PHASE_CONTRACT.md` —
   **the whole thing.** §3 scope groups, §4 done-means, §5 evidence table with its REDs.
3. `cef-native/include/core/PendingPermissionRequest.h` and `SitePermissionStore.h` — the
   mechanism this phase **adds cases to**; it does not add machinery.
4. `development-docs/DevOps-CICD/DPI_RESOLUTION_TEST_MATRIX.md` — cells #4/#6/#9, needed for
   `P0.9-A5`.
5. `development-docs/0.4.0-beta.3/TICKET_modal_buttons_unclickable_small_screen.md` — ⛔ read
   its **2026-08-23 correction**; the original "overlays are OSR" premise was false on Windows.

## 1. ⛔ Task 1 is a RESEARCH question, and it gates a promise

The owner specifically asked about **save-password**. Per §3 that is **Group B** — a Chrome-UI
bubble, not a permission request — and whether CEF 150 surfaces it **is unknown**.

**Answer that in writing before scoping anything into the phase.** The honest answer may be
*"only by patching Chromium"*, which we can do (we build our own CEF — see the CLAUDE.md note
about not reasoning "CEF won't let us") but is a completely different size of job.

Three acceptable verdicts: **interceptable**, **patch-only** (with a size estimate), or **not
at all**. ⛔ Do not let "probably possible" stand in for any of them.

## 2. What is actually in scope

**Group A — permission prompts.** `SimpleHandler` already implements `CefPermissionHandler`,
and `PendingPermissionManager` already parks a CEF callback while a Hodos overlay shows,
resolving it via the `permission_response` IPC. Camera, mic, location, notifications and
clipboard already ride it. Verified present in `cef-binaries/include/internal/cef_types.h`:

```
CEF_PERMISSION_TYPE_LOCAL_NETWORK_ACCESS = 1 << 25
CEF_PERMISSION_TYPE_LOCAL_NETWORK        = 1 << 26
CEF_PERMISSION_TYPE_LOOPBACK_NETWORK     = 1 << 27
```

⚠️ Re-verify those constants still exist with those names in the pinned CEF before building on
them — that is exactly what kickoff step 2 is for.

## 3. The three traps the contract already names — do not rediscover them the hard way

- ⛔ **Browser logo, NOT wallet logo.** `frontend/public/` has both
  (`Hodos_Gold_Browser_Icon.svg` vs `Hodos_Gold_Wallet_Icon.svg`). These are browser-level
  prompts about sites and the device. Getting it backwards teaches users that *the wallet* is
  asking — precisely the confusion an attacker wants. `P0.9-A4` tests the **rendered image**,
  not the source path.
- ⛔ **`SitePermissionType` is deliberately DECOUPLED from CEF's bitflag enum**, so a Chromium
  bump renumbering `cef_permission_request_types_t` cannot corrupt stored rows. New types get
  **new stable integers**, mapped only at the callback boundary. **Never store a CEF bit value.**
- ⛔ **NO AUTO-ALLOW.** Owner-agreed 2026-08-21. Not for loopback, not for the wallet's own
  origin, not "just for `127.0.0.1:31301`". A correct "this is the wallet UI" predicate is
  Phase 5's `IsWalletOrigin()` work; hardcoding one here would be a **fourth derivation of a
  security value** — the exact mistake `extractDomain` made. ⚠️ This will be tempting, because
  an un-auto-allowed loopback prompt appears in front of our own wallet UI. Resist it and say
  so in the contract.

## 4. ⚠️ A scheduling interaction you should know about

`P0.9-A5` requires the prompt to be **clickable at cells #4/#6/#9** — but **Phase 1 (WS1,
overlay input & DPI) has not run yet**, and its item 7 is an unresolved *mouse-offset* bug in
exactly this overlay machinery.

So if `A5` fails, **do not assume it is a 0.9 defect.** It may be the Phase 1 bug showing up
through a new surface. ⭐ The contract already anticipates this: A5's RED requires the same
prompt to be **seen working at 100%**, so the test can distinguish "DPI problem" from "broken
prompt". Honour that — it is the whole value of the row.

⭐ Useful pre-finding from 2026-08-23 (MEASURED by grep, cause NOT reproduced): Windows
overlays are `SetAsPopup` — **windowed** CEF browsers — yet `cef_browser_shell.cpp` still
hand-forwards mouse input at **48 `GET_X_LPARAM` sites with zero DPI conversion** under
`PER_MONITOR_AWARE_V2`. If A5 misbehaves, that is the first place to look, and it belongs to
Phase 1, not here.

## 5. Standards

- ⛔ **Negative control on every acceptance test.** §5's table already specifies each RED —
  use them, do not invent softer ones. ⭐ `A2`'s is the sharp one: *"a blocked read is not a
  blocked write"* — assert the request **did not arrive** in the wallet log, not merely that
  the page got an error.
- ⭐ **A control that cannot prove it injected anything is worth nothing.** Assert the
  injection actually changed the code/state. A P0.8 probe silently no-op'd after a restyle and
  would have reported a meaningless result.
- ⚠️ **Assert the right SUBJECT.** The header and ~14 overlays are separate CEF browsers that
  CDP reports uniformly as `type:"page"`. Driving the wrong one faked a bug here before.
- ⚠️ **`SendInput` clicks are dropped in the agent environment** (moves work). Plan for CDP +
  unit tests + me at the keyboard.
- **macOS parity from the start** (CLAUDE.md #9, and §4 of the contract) — not deferred to a
  relay round. New overlays need a macOS creation path; `MAC_RELAY_BETA3.md` is the channel.

## 6. Deliverable

Kickoff summary first — open questions, assumptions, and the **Group B verdict** — then, after
I sign off: the implementation, the evidence table filled in with **measured** results, updated
`PHASE_CONTRACT.md`, a `MAC_RELAY_BETA3.md` round, and memory.

## 7. Machine state

- Dev stack may still be running: wallet **31401 (schema V25)**, Vite **5137**, dev browser
  from `cef-native/build/bin/Release`. Re-check.
- ⛔ My **installed** browser (`AppData\Local\HodosBrowser`, ~61 procs) and its wallet on
  **31301** must never be touched. Match by **exe path**, never by process name.
  ⛔ `cargo build` fails "Access is denied" while the dev wallet runs — stop that PID by path.
- ⚠️ The notification overlay is **keep-alive**: after a frontend change, restart the dev
  browser or it keeps serving the old JS.
- ⛔ Never commit `development-docs/X402_INTEGRATION.md` or
  `development-docs/Onchain-Backup-and-Sync/*` — my parallel work. Stash / pop around a rebase.
- The P0.8 demo tunnel and fixture server are **stopped**; that hostname is dead. If this phase
  needs a real HTTPS origin, the recipe is in `test-fixtures/manifest-dapp/README.md` and **I**
  have to start the tunnel — the sandbox blocks it.

## 8. Working style

⭐ Walk me through anything I need to click **step by step**. I am doing the clicking. Tell me
what to do, what I should see, and what a wrong result would mean.
