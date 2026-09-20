# Phase 3 — window / instance / focus identity · PHASE CONTRACT

**Workstream:** WS2 · **Items:** #3 (taskbar identity), #5 (Ctrl+F drives the wrong window) · **Status:** 🟡 **KICKOFF — awaiting owner sign-off. No code written.**
**Opened:** 2026-08-26 · **Owner:** Matthew · **Platforms:** Windows only (§3 of `SPRINT_PLAN.md`: macOS Spaces is a different mechanism; #3 has no macOS analogue)
**Standard:** `../HARNESS.md`. Measurements: `MEASUREMENTS.md`.

> 🍎 **AMENDED 2026-09-19 — the #5 half is no longer Windows-only.** The header above still
> reads *"Platforms: Windows only"*, and that remains correct for **#3** (taskbar AUMID has no
> macOS analogue). It is **no longer correct for #5**: the macOS port landed on 2026-09-19.
> - **Ctrl+F / Ctrl+L** needed nothing — those arms live in shared `simple_handler.cpp` and
>   already resolve `GetOwnerWindow()`, so macOS inherited the fix when Windows made it.
> - **Fullscreen** was broken on macOS in both halves and is now fixed and measured:
>   `HandleFullscreenChange` took the `BrowserWindow*` and drove process globals anyway, and
>   `ToggleMainWindowFullscreen()` always acted on `g_main_window`. 📏 Both reproduced with two
>   windows in one process and both closed with a negative control — `P3_MAC_RESULTS.md`.
> - ⭐ The per-window fields this phase added to `BrowserWindow` (`is_content_fullscreen`,
>   `is_window_fullscreen`) are what the macOS port consumed; it added only a per-window
>   pre-fullscreen frame. The note in `BrowserWindow.h` that *"macOS already models these
>   separately"* was true of the FLAGS and not of the LAYOUT they were supposed to drive.
> - ⬜ Still macOS-global, deliberately out of this port and left to the beta.4 ticket: the
>   remaining `g_main_window` / `GetActiveTab()` sites outside the fullscreen path.

---

## 0. What the kickoff changed about the phase as written

⛔ **The desk diagnosis does not survive verification.** Both halves of #3 are real code defects and
both are still present at the cited locations — but **half (a) cannot be the cause of the symptom the
owner reported**, and a third fact nobody had looked at is a better fit for every reported detail.

| # | Session prompt / SPRINT_PLAN §2 says | Kickoff found | Label | Where |
|---|---|---|---|---|
| 1 | "(a) Production single-profile never sets an AUMID" — presented as one of two halves of the root cause | **Code verified current** (`cef_browser_shell.cpp:5153`). But the owner has **2 profiles** since 2026-07-06, so `size() > 1` is true and **the branch runs on their machine**. Their production log says `AUMID set: Default` **61 times**, including six times on 2026-08-17 — the day they reported the symptom. ⇒ (a) is a real latent defect but **NOT the cause of what was observed**. | 📏 **MEASURED** | M1, M2 |
| 2 | "(b) The shortcuts declare no AUMID" | **Code verified current** (`hodos-browser.iss:81,83`) **and confirmed on disk**: the Start Menu and Uninstall `.lnk` both read `AUMID=<NONE>`. | 📏 **MEASURED** | M3 |
| 3 | (not mentioned anywhere) | 🎯 **The pinned taskbar shortcut carries `Chromium.OYX4LNTP4DA5GSWHROKEK3C7BM`** — a Chromium-computed identity, not `HodosBrowser`. Chrome's pins as `Chrome`, Brave's as `Brave`. Ours is the **unbranded-Chromium** default form, `Chromium.<base32 hash of install path>`. | 📏 **MEASURED** | M3 |
| 4 | (not mentioned anywhere) | Hodos windows carry **no per-window AUMID** (`<NONE>`). Chrome and Brave **do** — including `Chrome.UserData.Profile1`, the exact per-profile form we are trying to imitate. We set AUMID **process-wide only**; they set it **per HWND**. | 📏 **MEASURED** | M4 |
| 5 | "⇒ Windows cannot match window → shortcut" | The causal chain from (a)+(b) to the symptom was an **inference**, never a measurement, and item 1 above breaks it. | 🧠 **CLAIM, now refuted** | M2 |
| 6 | #5: "suggests focus resolution keyed on a global rather than the active window" — flagged as a hypothesis to falsify | **Confirmed by code read, not measurement.** `GetHeaderBrowser()` returns `WindowManager::GetPrimaryWindow()->header_browser` (`simple_handler.cpp:396`) — a global. The Ctrl+F arm (`:9080`) and the Ctrl+L arm (`:9101`) are the **only two** arms in `OnPreKeyEvent` still using it; their siblings already use `GetOwnerWindow()`. | 🧠 **CLAIM (code read)** | M5 |
| 7 | #5: "two Hodos instances" | A second launch **of the same profile** forwards over a named pipe and the running process opens a **second window in the same process** (`SingleInstance.cpp:201`, `cef_browser_shell.cpp:1583`). So "two instances" is one process with two windows — the exact configuration where a `GetPrimaryWindow()` lookup misroutes. Two *different* profiles are genuinely separate processes and are **not** affected. | 🧠 **CLAIM (code read)** | M6 |

| 8 | (not mentioned anywhere) | 🎯 **The root cause, closed and measured (M7).** The pin was created **2026-04-27**, while single-profile — so it carries Chromium's default identity. The **first** `AUMID set` line in 2.56 GB is **2026-07-06 15:41:14**, eight seconds after `Profile_1` was created. ⇒ *Adding a second profile silently changed the app's identity and orphaned the user's pin*, and the new identity (`HodosBrowser`) is declared by **no shortcut on the system**, so Windows has no name for it and falls back to the exe filename. | 📏 **MEASURED** | M7 |

⭐ **The headline: the root cause is neither (a) nor (b) as framed — it is that our identity changed
under the user and matches nothing.** Both symptoms follow from one measured sequence (M7).

🚨 **And the proposed fix is backwards.** `SPRINT_PLAN.md` §2 fix #1 — *"set the explicit AUMID
always in production"* — applied **alone** does to every remaining single-profile user exactly what
2026-07-06 did to the owner: moves them off the Chromium-derived identity their pin currently
matches, and orphans it. **The owner's machine is the natural experiment and the outcome was this
bug.** Fix #1 is safe only *together with* fix #2 and a decision on existing pins.

⭐ **Second headline: #5's root cause is confirmed and its scope is bigger than the kickoff first
said.** The owner observed the Ctrl+F symptom (M9.2) **and** reported a second one the kickoff had
not looked for — **HTML5 video fullscreen crossing windows** (M9.3), same root cause, more damaging.
⛔ The kickoff's own "bounded to two arms" answer was **wrong**: it audited only `OnPreKeyEvent`,
the function the prompt pointed at. The real shape is a **half-finished multi-window migration**
(M9.4). See §6 for the re-proposal.

⚠️ **Profile names were stated backwards in the owner-facing ask** — `Default` is **Archie**,
`Profile_1` is **Hodos**. Corrected in M9.0.

---

## 1. Goal

Windows knows which window is Hodos: the taskbar names it "Hodos Browser" and groups it with the
user's pinned icon, and a keyboard shortcut acts on the window the user pressed it in.

## 2. Done means

- [ ] On a **production, single-profile** install, right-clicking the running taskbar button reads
      **"Hodos Browser"**, not "HodosBrowser.exe".
- [ ] Launching groups with the pinned icon instead of creating a second, unnamed button.
- [ ] A user running a **second profile** still gets a **separate, correctly-named** taskbar button.
      ⚠️ This is the requirement that pulls against the one above; see §5.
- [ ] What happens to an **existing pinned shortcut** on upgrade is decided, implemented or
      documented, and stated in the release notes — never silently changed.
- [ ] Ctrl+F opens the find bar in the window it was pressed in.
- [ ] 🆕 Fullscreening a video in one window does **not** resize the other window or hide its header
      (M9.3) — the second, worse symptom of the same cause, reported by the owner 2026-08-26.
- [ ] The remaining global-window call sites (M9.4) are **counted, gated and ticketed**, not
      silently left — per the time-box in §6.

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| **R-UPDATE** | A staged update still applies | ⛔ Changing `[Icons]` rewrites shortcuts on upgrade. Inno rewrites `{group}` and `{autodesktop}` but **never** the pinned `.lnk` (`User Pinned\TaskBar` is user-owned). Also: `TICKET_stray_log_in_install_root` — nothing new inside `{app}`. |
| **R-CLOSE** | Overlay close guards | `WindowManager` owns primary-window tracking and overlay handles. Any change to window identity or focus resolution risks the overlay lifecycle. #5's fix touches focus. |
| **R-GOLD** | Gold pill on the correct tab | #5's fix touches window→tab resolution. `Tab::id` ≠ `CefBrowser::GetIdentifier()`; a pill on the wrong tab is a failure. `GetActiveTab()` is a **global** and `GetActiveTabForWindow(int)` already exists beside it (`TabManager.cpp:334` vs `:625`) — the same defect class as #5. |
| **R-COUNT** | Per-session counters reset on tab close | Untouched. Listed because #5's fix edits `OnPreKeyEvent`, which sits in the same file as the IPC dispatch that drives session close. |
| **R-INTEXT** | Internal never prompts, external always gates | Untouched. Listed because `simple_handler.cpp` is edited. |

## 4. Evidence table

⛔ No empty RED or SUBJECT cells. A green result is reported with its red half or not at all.

### 4.0 The instrument, and why it is trustworthy

Both probes live in this folder and were **validated against positive controls before being used on
Hodos**: `winaumid.ps1` returned `Brave` for Brave, `Chrome` and `Chrome.UserData.Profile1` for
Chrome; `lnkaumid.ps1` returned `Brave`, `Chrome`, `com.squirrel.slack.slack` for their pinned
shortcuts. ⭐ **This matters**: a probe that returned `<NONE>` for everything would have faked a
green here. Because it distinguishes, `<NONE>` for Hodos is a reading, not an instrument failure.
⚠️ `winaumid.ps1` reads the **window-level** property. It does **not** read a process-wide explicit
AUMID — that is precisely why `P3-A2` exists as a separate confirmation of M7.

| ID | 🟢 GREEN — must be true | 🔴 RED — must be *seen* to fail, and how | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P3-A1` | The AUMID **decision** is a pure function with the right answer per state: prod+1 profile → `HodosBrowser`; prod+Profile_1 → `HodosBrowser.Profile_1`; dev → `HodosBrowser.Dev*`; picker → per Q5 | 🔴 **SEEN 2026-08-30.** Reinstated the pre-fix gate (`if (!isDev && !pickerMode && profileId=="Default") return nullopt;`) in `ComputeAumid` on the same binary → **5 of 13 RED**, incl. `ProdSingleProfileGetsAnIdentity`. Reverted | `hodos_tests`, release config. ⚠️ Tests the **decision**, not the taskbar — never cite this row as evidence the taskbar is fixed | T1 | 🟢 **GREEN** — 13/13; suite 342 pass, 1 pre-existing skip |
| ~~`P3-A1b`~~ | ⛔ **VOID — the row tested a mechanism that does not work.** It asserted the registry key was *written*, which it was; it never asserted the taskbar was *named*, which it was not. ⭐ A green row that measured the wrong thing — the exact failure `HARNESS.md` §2's SUBJECT column exists to catch, committed by the author of this contract. Superseded by `P3-A5`/`P3-A5b` | See M10 | — | — | ⛔ void |
| `P3-A2` | A freshly-created pin of a running Hodos window carries the AUMID our process set — **confirming** M7's inference that our AUMID is effective | Owner unpins and re-pins the running browser; `lnkaumid.ps1` reads the new `.lnk`. Expect `HodosBrowser.Profile_1`. If it reads `Chromium.OYX…` again, **M7's refutation of sub-hypothesis 2 is wrong** and the whole §0 item 8 narrative must be reopened — that is this row's RED | The **running** installed browser, matched by exe path `…\AppData\Local\HodosBrowser\HodosBrowser.exe`. Pre-state recorded: `Chromium.OYX4LNTP4DA5GSWHROKEK3C7BM`, pin created 2026-04-27 | T3 (human) | ⬜ |
| `P3-A3` | Process AUMID, shortcut AUMID and pinned AUMID are **byte-identical** | Read all three with the two probes. Build one shortcut without the `AppUserModelID:` parameter → it reads `<NONE>` and the row goes RED. **Pre-fix RED already measured:** Start Menu `.lnk` = `<NONE>`, pinned `.lnk` = `Chromium.OYX…`, process = `HodosBrowser` (log) — three different values | The `.lnk` files on disk **and** the running window. Assert equality, not presence | T2 | ⬜ |
| `P3-A4` | Production **single-profile**: taskbar right-click reads "Hodos Browser"; launch groups with the pinned icon | Same install, pre-fix binary → reads "HodosBrowser.exe" and makes a second button. ⛔ This RED must be observed **on the single-profile install**, not on the owner's 2-profile machine where the branch already runs | 🚨 **A genuinely single-profile production install** — see §4.1. `HODOS_DEV=1` takes the branch that already works and proves nothing | T3 (human) | ⬜ |
| `P3-A5` | ⭐ **The coexistence control for `P3-A4`.** With 2 profiles, the second profile's window gets its **own** taskbar button, **correctly named** | 👤 **RED OBSERVED (M9.1a):** the `Profile_1` button read "hodosbrowser.exe". 👤 **GREEN OBSERVED (M10):** with a shortcut declaring `HodosBrowser.Dev.Profile_1`, the button read **"Hodos Browser - Test_1"**. Delete the shortcut → reverts to the exe filename | 🎯 **DEV build**, non-Default profile. ⛔ Proves the **naming mechanism**, not the production string — that needs a real install (`P3-A4`) | T3 (human) | 🟢 **GREEN (mechanism)** — production string still owed |
| `P3-A5b` | 🆕 **Two refuted mechanisms stay refuted.** Neither the exe version resource nor an `AppUserModelId` registry value names the button | 🔴 **BOTH SEEN TO FAIL (M10).** `FileDescription` is already "Hodos Browser" on both exes → still "HodosBrowser.exe". Registry key verified written → still "HodosBrowser.exe". ⭐ The confirming test used the deliberately unique name **"Hodos Browser DEVTEST"**, because "Hodos Browser" would have been consistent with all three candidates and proved none | The **taskbar button**, read by a human. ⛔ No probe can read this: `SHGetPropertyStoreForWindow` returns the *window* property, which is `<NONE>` by design since the AUMID is per-**process** | T3 (human) | 🟢 **GREEN** — both refuted, recorded in code so neither is re-tried |
| `P3-A5d` | 🔴 **OPEN — owner requirement, 2026-08-31.** The Start Menu carries **exactly one** entry, "Hodos Browser", which opens the picker; **and** a window's taskbar button still shows its profile name ("Hodos Browser - `<name>`", Default included "where needed") | ⛔ **The two halves conflict under the mechanism we proved.** Shortcuts name buttons, and per-profile shortcuts are the clutter the owner rejected. So naming must move to a **window-level** mechanism: `PKEY_AppUserModel_RelaunchDisplayNameResource` / `…RelaunchCommand` via `SHGetPropertyStoreForWindow`. 🧠 **CANDIDATE, UNVERIFIED** — the docs say `…DisplayNameResource` takes an *indirect resource reference* (`"app.exe,-101"`), not a plain string | The taskbar button of a **non-Default** window. ⛔ Must be read by a human — no probe can see it (`P3-A5b`). ⚠️ **Measure before implementing**: this phase has already been wrong twice about what names a button | T3 (human) | 🔴 **OPEN — deferred, not solved** |
| `P3-A5e` | ✅ The single Start Menu entry opens the **profile picker** when >1 profile exists | 📖 `ProfileManager::ResolveStartup` (header-only): no `--profile` arg + `existingIds.size() > 1` + `pickerEnabled` ⇒ `showPicker = true`. The installer's `{group}\Hodos Browser` passes **no** argument. RED = pass `--profile=` (then it bypasses the picker), which is what a per-profile shortcut did | ⚠️ 📖 **Code reading, not a run.** The owner has not clicked the installed Start Menu entry since this change | T2 | 🟡 **Believed GREEN by construction — unrun** |
| `P3-A5c` | 🆕 A dev build creates **no** profile shortcuts | 🔴 **SEEN (M11):** before the guard, a dev launch wrote `Hodos Browser - Test_1.lnk`; 👤 the owner clicked it and got the DEV SAFEGUARD dialog — a `.lnk` cannot carry `HODOS_DEV=1`, so it is inert by construction. Remove the `IsDevEnv()` guard → the dead shortcut returns | `Start Menu\Programs`, after launching a **non-Default** dev profile. Assert the directory, not just the log line | T2 | 🟢 **GREEN** — no shortcut written; log records the skip |
| `P3-A6` | An **existing pinned shortcut** behaves per the §8 Q4 decision, and the behaviour is in the release notes | Pin on the pre-fix build, upgrade over it, observe. Whatever happens **is** the finding — "it needs re-pinning once" is an acceptable GREEN if it is *decided and documented*, and a defect if it is discovered by a user | The pinned `.lnk` in `User Pinned\TaskBar`, before and after, read with `lnkaumid.ps1` | T3 (human) | ⬜ |
| `P3-A7` | R-UPDATE: a staged update still applies, and `[Icons]` changes reach existing installs | Corrupt the staged installer after signing → the apply must refuse and roll back. Also assert **nothing new inside `{app}`** | The **real** N−1 → N transition. ⚠️ Currently 🟡 T1-only at every boundary this sprint; this phase **changes the installer**, so the real apply is more owed here than usual | T2 | ⬜ |
| `P3-A8` | #5: Ctrl+F opens the find bar in the window it was pressed in | 👤 **RED ALREADY OBSERVED (M9.2):** Ctrl+F in the second Archie opens find in the first; *"nothing happens in the second one"*. Post-fix control = restore the `GetHeaderBrowser()` call on the **same binary** → the redirect returns. ⛔ The kickoff's original "different-profile" control was **retracted as vacuous** (M9.2a) | The window that owns the key event. ⭐ **Discriminator:** with **three** same-profile windows, Ctrl+F in window 3 must land on window **1** (the primary), not window 2. If it lands on 2, M5 is wrong and this row is void | T3 (human) | ⬜ |
| `P3-A9` | 🆕 **Fullscreen is window-scoped.** A video fullscreened in window A leaves window B's size, header and fullscreen state **untouched** | 👤 **RED ALREADY OBSERVED** (M9.3): today both windows go fullscreen either way and the header disappears on both. Post-fix, restore `HandleFullscreenChange`'s global path on the same binary → the cross-window effect returns | Two windows of the **same** profile (one process), **on different-sized monitors** — that is where the primary-window geometry is visibly wrong. Assert window B's client rect is *unchanged*, not merely that A worked | T3 (human) | ⬜ |
| `P3-A10` | Fullscreen state is **per window**: A fullscreen + B normal is a representable state | Today `g_is_fullscreen` is one process-wide `bool`, so this state **cannot exist** — that is the RED, and it is structural, not incidental | The `BrowserWindow` record, not a global. ⚠️ Pairs with `P3-A9`: A9 says *B is untouched*, A10 says *both states coexist*. A fix that simply ignores B's fullscreen entirely passes A9 alone | T2 | ⬜ |
| `P3-A11` | 🆕 The three-dot menu's fullscreen button **toggles** — it can exit, not only enter | 🔴 **Structural RED, pre-fix:** the arm read the flag and never wrote it, so the "exit" branch was reachable only while an HTML5 video happened to be fullscreen. Post-fix control = delete the two `is_window_fullscreen = …` assignments → the one-way trap returns | The **menu** fullscreen (borderless `WS_POPUP`), which is a *different state* from video fullscreen. ⚠️ Two flags now, per macOS's existing model — asserting one is not asserting the other | T3 (human) | ⬜ |
| `P3-G11` | T0 gate, **ratcheted**: window-scoped work resolved through `GetPrimaryWindow()` or the global `GetActiveTab()` does not **exceed** baseline. ⭐ Lands here, lowered by 3.5, driven to 0 in beta.4 | 🔴 **SEEN 2026-08-30:** `preflight.ps1 -Only G11 -NegativeControl` → *"detected the injected violation (61 > baseline 60)"* | `cef-native/src/{handlers,core}`, Windows only (the G8 lesson — adding `.mm` would bury the lines that matter). ⚠️ Baseline **measured by the tool**: **60**, not the ~19 M9.4 predicted by hand | T0 | 🟢 **GREEN** — baseline 60, PASS, RED observed |

**Two-sided pairings.** `P3-A4`/`P3-A5` are each other's control and are the heart of #3: A4 says
*one app, one button, correctly named*; A5 says *extra profiles still get their own*. A fix that
gives every window the same identity satisfies A4 and breaks A5 — and **A4 alone cannot see it**.
Likewise `P3-A1` (the decision is right) is paired with `P3-A2`/`P3-A3` (the decision actually
reaches Windows) — the whole point of M3/M4 is that we may be computing a correct AUMID that
nothing downstream honours.

### 4.1 🗂️ DEFERRED to the install batch — decided 2026-08-31

⛔ **`P3-A4`, `P3-A5`'s production half, `P3-A6` and `P3-A7` do not run in this phase.** The owner
batched every install-dependent row into **`../INSTALL_TEST_BATCH.md`** (rows I1–I7), to be run
once before the RC. They are **OWED, not waived** — this phase closes with them visibly outstanding
rather than by quietly reclassifying them.

The reasoning below still stands and is why they cannot be run any other way.

### 4.2 🚨 How a production single-profile build gets tested — the honest answer

The prompt is right that this is the hard part, and the honest answer is **it cannot be tested from
`build/bin/Release`**: the dev safeguard refuses without `HODOS_DEV=1`, and `HODOS_DEV=1` takes the
`IsDevEnv()` arm that already works. Three routes, in order of preference:

1. ⭐ **A second Windows user account on this machine.** A fresh `%APPDATA%` gives a genuine
   production, single-profile install with no VM and **without touching the owner's install or its
   wallet on 31301**. This is the cheapest real answer and it is a true fresh profile — ⛔ noting
   `project_p09_loopback_branding_done`'s lesson that *a fresh profile is not a fresh test*: here
   the point is the fresh **install identity**, which a new user account genuinely provides.
2. **A VM or spare machine** — same thing, slower, and the only route that also tests a clean
   first-run.
3. **`hodos_tests` for the decision only** (`P3-A1`). Necessary, not sufficient; it can never see
   M3/M4.

⛔ **What will NOT be accepted as evidence:** the owner's own machine showing a correct taskbar.
It has 2 profiles, the branch already runs there, and that is exactly the false green §0 item 1
was built out of.

## 5. Blast radius — written before the code, not after

⚠️ **Per-profile taskbar buttons work today and users rely on them.** The two requirements pull in
opposite directions, so this is how they coexist:

⭐ **The good news, and it is load-bearing:** the per-profile suffix logic is already written and
correct, and widening the gate **changes nothing for multi-profile users**. They satisfy
`size() > 1` today, take the branch today, and compute the identical string. The only population
whose behaviour changes is **single-profile production users** — precisely the target. That is
asserted from the code and is the reason `P3-A5` exists to check it rather than assume it.

| Risk | Why it is real here | Mitigation in the plan |
|---|---|---|
| 🚨 **The fix makes it worse for everyone** | 📏 **MEASURED, not speculated (M7).** Single-profile users run on Chromium's `Chromium.<hash>` identity — which is what **their pinned shortcut also carries**, so it matches today. Setting an explicit `HodosBrowser` moves them **off** it and orphans a pin that works. This already happened to the owner on 2026-07-06 | ⛔ Fix #1 never ships alone. Q2 forces #1 + #2 + the pin decision together |
| **Non-`Default` profiles may still be misnamed** | `HodosBrowser.Profile_1` matches no shortcut, so the display name may still fall back to the exe. This is the owner's own most-used profile | `P3-A5` tests the **naming** half explicitly, not just the separate-button half |
| **Pinned shortcuts are user data** | Inno never writes `User Pinned\TaskBar`. M7 proves pins **will** stop matching — now certain, not a risk | Q4 decides; `P3-A6` observes; release note either way |
| **AUMID is inherited by child processes** | CEF spawns render/GPU/utility children under the bootstrap model | Not a new risk — already true for dev and multi-profile today. Assert, don't change |
| **The picker sets no AUMID at all** | `!g_picker_mode` skips the branch entirely, though the comment says "keep the base AUMID" — the code and comment disagree | Q5 decides; the comment is corrected either way |
| **#5 touches `OnPreKeyEvent`** | Every keyboard shortcut in the browser routes through it | Bounded to 2 arms; `P3-G11` ratchets; R-CLOSE/R-GOLD listed above |
| **`GetActiveTab()` is the same defect class** | 4 further arms in `OnPreKeyEvent` use the global tab getter while `GetActiveTabForWindow(int)` exists | ⛔ **Explicitly out of scope** (§7) — recorded, not fixed, so it cannot masquerade as done |
| **macOS** | WS2 is Windows-only by §3 of the sprint plan | No macOS claims made from Windows; nothing added to the relay (§9) |

## 6. #5 — the time-box, stated up front

⛔ **The kickoff's first answer to this was wrong, and the owner disproved it in minutes.**
It said #5 was "bounded to two arms". That audit covered only `OnPreKeyEvent` — the function the
session prompt pointed at — and the owner immediately reported a **second symptom outside it**:
HTML5 video fullscreen crossing windows (M9.3), which is the same root cause and **more damaging**.
⭐ `feedback_own_work_is_the_weakest_link`: the audit scope was the thing I failed to question.

**The truth is between the sprint plan and the desk.** Not an unbounded deep dive, and not two call
sites: a **half-finished multi-window migration** (M9.4) — 77 `g_hwnd`, 25 `g_header_hwnd`, 16
process-wide `GetAllTabs()`, and 19 global `GetActiveTab()` against 18 already-correct
window-scoped calls.

**Box: 6 hours, and it buys the two *reported* symptoms only** —

1. **Ctrl+F / Ctrl+L** — `GetHeaderBrowser()` → `GetOwnerWindow()->header_browser` (2 arms).
2. **Fullscreen** — `HandleFullscreenChange(bool)` → take the `BrowserWindow*` that
   `OnFullscreenModeChange` **already has and discards**, and move `g_is_fullscreen`, `g_hwnd`,
   `g_header_hwnd` and the tab list onto it.

⇒ **Re-proposal for the owner (§8 Q3):** take **both reported symptoms**, add the `P3-G11` ratchet
so the global count cannot grow, and file the remaining migration as a beta.4 ticket. That is the
harness's own pattern — *a gate lands in the phase that discovers the problem, and the phase that
fixes it drives the baseline down* (`HARNESS.md` §4) — and it avoids both failure modes: shipping a
known-broken window model, and letting a 100+ call-site refactor eat the phase.

⛔ **Not in the box:** the other 77/25/16/19 call sites. They get the ticket, not the commit.

**If the box expires:** write the mechanism, the counts and the two candidate fixes into
`TICKET_window_scoped_work_uses_process_globals.md`, mark #5 deferred, and close on #3 alone.
⛔ Per the session prompt, do not let it eat the phase — Phase 2 lost most of a session to a
side-quest and the owner had to pull it back.

## 7. Out of scope

- 🚨 **The rest of the multi-window migration** — the other 77 `g_hwnd`, 25 `g_header_hwnd`, 16
  `GetAllTabs()` and 19 `GetActiveTab()` call sites (M9.4). Same defect class. **Counted, gated by
  `P3-G11`, and ticketed to beta.4 — not fixed here.** ⛔ This is the unbounded audit the sprint plan
  warned about, and the ratchet is what makes deferring it safe rather than silent.
- **A general window-identity refactor.** The 18 static accessors are documented backwards-compat
  shims; retiring them is its own phase.
- **macOS.** Windows-only per `SPRINT_PLAN.md` §3.
- **Registering a jump list / task list.** Adjacent to AUMID work and tempting. Not this phase.
- **The orphaned `Profile_2` directory** found during the kickoff (on disk, absent from
  `profiles.json`). Phase 1's orphan sweep (`84997eb`) owns it; noted in M2 only.
- Carried Phase 1/2 items: the overlay dead strip, the DPI matrix overlay section, P2-A7's
  unexplained torn lines.

## 8. Owner decisions — ⛔ OPEN. No code until Q2 and Q4 are answered.

⭐ **Q1 is now answered by M7** and is kept only as the confirmation step it became.

| # | Question | Options | Recommendation |
|---|---|---|---|
| **Q1** | ~~Is our process AUMID effective at all?~~ | 🟢 **ANSWERED by M7** — yes; Chromium does not override us. `P3-A2` remains as cheap confirmation (≈60 s of your time), no longer a gate | Run it opportunistically |
| **Q2** | ✅ **ANSWERED, then part #3 REVERSED by the owner 2026-08-31.** Shipped: **#1** always set the AUMID · **#2** installer `[Icons]` declare `HodosBrowser` · **#4** pins → release note (Q4). ⛔ **#3 (per-profile Start Menu shortcuts) is REMOVED** | Owner: *"the start menu should just say Hodos Browser, nothing else and then that opens the profile picker"* — and separately, *"there is not really any reason for us to have a default at all… the default can be the one already highlighted in the picker"* | ⇒ Start Menu clutter gone; picker already the front door (`P3-A5e`). ⚠️ **Per-profile taskbar NAMING is now unsolved** and deferred to `P3-A5d` |
| **Q3** | 🆕 **How far does #5 go?** The kickoff said "2 arms"; you then found the fullscreen defect, and the class is ~137 call sites (M9.4) | (i) both **reported** symptoms (Ctrl+F/Ctrl+L + fullscreen) + the `P3-G11` ratchet, rest ticketed to beta.4; (ii) Ctrl+F only; (iii) the full migration now; (iv) defer all of #5 | **(i).** Fullscreen is the more damaging of the two and you hit it unprompted. (iii) is a 100+ site refactor that would eat the phase; the ratchet stops the count growing meanwhile |
| **Q4** | 🚨 **What happens to an existing pinned shortcut?** M7 removes the option of not deciding: pins **will** stop matching. | (i) do nothing, release-note **"re-pin Hodos once after updating"**; (ii) on first run, stamp `System.AppUserModel.ID` onto any `.lnk` in the user's pinned/Start folders whose target is our exe — Chrome does exactly this ("shortcut migration") | **(ii)** if we can keep it small, because (i) silently breaks a pin the user never touched. But (ii) **writes user data** and is not `git revert`-able — so it lands last and separately (§10). Your call |
| **Q5** | Should the **picker** set the base AUMID? Code skips it; the comment claims otherwise | (i) set base; (ii) keep skipping and fix the comment | **(i)** — an unidentified picker window is the same bug, briefly |
| **Q6** | ⭐ **Fold a real payment into this session?** R-GOLD, R-COUNT and the still-unobserved `payment.auto_approved` audit line are all owed from the 2 → 3 boundary and one payment closes all three | (i) yes, while you are at the machine; (ii) keep owing them | **(i)** if you are at the machine anyway — they are the three that most directly guard the money path |

## 9. Mac relay

**Nothing is owed to the Mac side by this phase** — WS2 is Windows-only per `SPRINT_PLAN.md` §3.
⚠️ But the **existing** asks in `MAC_RELAY_P2_ROUND.md` do not go stale because this phase is
Windows-only; they remain outstanding and should be chased independently of Phase 3's progress.

## 10. Rollback

One commit reverts. The AUMID change is a widened condition plus the two `[Icons]` parameters;
the installer change is two `[Icons]` parameters; #5 is two call-site substitutions. `git revert`
restores today's behaviour.
⚠️ The **exception** is Q4(ii) if chosen: stamping a user's pinned `.lnk` writes to user data and is
not revertible by `git`. If we do it, it lands **last and separately**, like Phase 2's log cleanup.

---

## Sign-off

- [ ] Owner has answered Q2–Q6
- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1` run — result + date recorded below
- [ ] `scripts/preflight.ps1 -NegativeControl` run — every T0 gate seen to fail
- [ ] `../REGRESSION_SET.md` run in full at the 3 → 4 boundary — result recorded
- [ ] Adversarial review complete, four questions answered in writing
- [ ] Commit messages cite the row IDs they satisfy

| Item | Result | Date | By |
|---|---|---|---|
| preflight | | | |
| preflight -NegativeControl | | | |
| regression set | | | |
| adversarial review | | | |
