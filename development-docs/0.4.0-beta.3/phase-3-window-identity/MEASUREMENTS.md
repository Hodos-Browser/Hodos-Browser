# Phase 3 — measurements

Every number in `PHASE_CONTRACT.md` is cited from here. Each entry is labelled
📏 **MEASURED** (a run that produced this output) or 🧠 **CLAIM** (a code reading).
⛔ Per `HARNESS.md` §6 Q4 and the M2.1 lesson from Phase 2, mislabelling one as the other is the
error this column exists to prevent.

**Taken:** 2026-08-26, Windows 11 Pro 26200, at `0540832`.
⛔ **The owner's installed browser and its wallet on 31301 were never touched.** Everything below is
either a file read, a registry-free shell property read, or an out-of-process window property read.
No process was started, stopped or signalled. Matching was by **exe path**, never process name.

---

## M1 — #3 half (a): the AUMID gate, verified current 🧠 CLAIM (code read)

`cef-native/cef_browser_shell.cpp:5148-5161` — unchanged in shape from `SPRINT_PLAN.md` §2:

```cpp
// Set process AUMID early (before window creation) so Windows gives dev vs
// prod (and multi-profile) DISTINCT taskbar buttons. ... prod keeps its existing
// (set-only-when-multi-profile) behavior. The picker owns no profile -> keep the base AUMID.
if (!g_picker_mode && (hodos::IsDevEnv() || ProfileManager::GetInstance().GetAllProfiles().size() > 1)) {
    std::wstring aumid = hodos::IsDevEnv() ? L"HodosBrowser.Dev" : L"HodosBrowser";
    if (profileId != "Default") {
        std::wstring pw(profileId.begin(), profileId.end());
        aumid += L"." + pw;
    }
    SetCurrentProcessExplicitAppUserModelID(aumid.c_str());   // :5159
    LOG_INFO("AUMID set: " + std::string(hodos::IsDevEnv() ? "dev " : "") + profileId);  // :5160
}
```

The prompt's cited line ~5159 is **exact**. Two notes:

- ✅ **`LOG_INFO` confirmed**, not assumed — so the line survives Phase 2's production INFO gate
  (Phase 2 §8 Q2). The prompt asked for this to be confirmed rather than assumed; it is.
- 📜 History (`git log -L 5153,5161`): the gate was `size() > 1` alone at `af62fbc`, widened to add
  `IsDevEnv()` at `f9408fd` ("scope single-instance pipe + AUMID + DevTools port to dev/prod").
  ⇒ **The prod arm has never been reached by a single-profile production install.**
- ⚠️ `SetupTaskbarProfile` does **not** set the AUMID, despite `TaskbarProfile.h:7` saying "Sets
  per-profile AUMID, overlay icon badge, and badged window icon". The body only sets an
  `ITaskbarList3` overlay badge and is itself gated on `profiles.size() > 1`
  (`TaskbarProfile.cpp:326-330`). **The header comment is stale** — a doc fix, not a code defect.

## M2 — 📏 MEASURED: (a) does **not** explain the owner's symptom

The refutation, in three readings.

**M2.1 — the owner has 2 profiles, since before the bug report.**
`%APPDATA%\HodosBrowser\profiles.json`:

```json
{ "defaultProfileId": "Default", "lastUsedProfile": "Profile_1",
  "profiles": [ { "id": "Default",   "name": "Archie", "createdAt": "2026-04-18T20:43:49Z" },
                { "id": "Profile_1", "name": "Hodos",  "createdAt": "2026-07-06T21:41:06Z" } ] }
```

⇒ `GetAllProfiles().size() == 2` since **2026-07-06**, six weeks before the 2026-08-17 report.
⚠️ A third directory `Profile_2/` exists on disk (created 2026-04-18) but is **absent from
`profiles.json`** — an orphan. Phase 1's sweep (`84997eb`) owns it; out of scope here (§7).

**M2.2 — the branch demonstrably ran, 61 times, on the shipping build.**
`%APPDATA%\HodosBrowser\logs\debug_output.log` (2.56 GB, the owner's real production log):

```
$ grep -a -c "AUMID set" debug_output.log
61
```

Last twelve, verbatim:

```
[2026-08-13 09:30:34.941] [MAIN] [INFO] AUMID set: Default
[2026-08-14 06:10:50.707] [MAIN] [INFO] AUMID set: Default
[2026-08-17 15:19:45.701] [MAIN] [INFO] AUMID set: Default     <-- report day
[2026-08-17 15:24:42.421] [MAIN] [INFO] AUMID set: Profile_1
[2026-08-17 15:26:35.704] [MAIN] [INFO] AUMID set: Default
[2026-08-17 15:26:51.956] [MAIN] [INFO] AUMID set: Default
[2026-08-17 15:46:07.813] [MAIN] [INFO] AUMID set: Default
[2026-08-17 15:48:00.977] [MAIN] [INFO] AUMID set: Default
[2026-08-17 15:54:58.242] [MAIN] [INFO] AUMID set: Default
[2026-08-18 07:55:19.883] [MAIN] [INFO] AUMID set: Profile_1
[2026-08-21 10:55:00.226] [MAIN] [INFO] AUMID set: Profile_1
[2026-08-26 08:26:36.026] [MAIN] [INFO] AUMID set: Profile_1
```

🎯 **This log line is emitted by the very branch under investigation** — it is the branch's own
output, so it is self-proving evidence that the branch executed. And the SUBJECT proves itself
twice over:

- the file lives in `%APPDATA%\HodosBrowser` (**production** data dir — a `HODOS_DEV=1` build writes
  to `HodosBrowserDev`), and
- the message has **no `"dev "` prefix**, which the code emits iff `IsDevEnv()`. ⇒ the run took the
  **`size() > 1` prod arm**, not the dev arm.

**M2.3 — the conclusion.** On 2026-08-17, six times, the owner's production process ran with an
explicit AUMID of `HodosBrowser` and **still** showed "HodosBrowser.exe" on right-click and **still**
made a separate taskbar button.

⇒ 🧠 **Inference (clearly labelled):** half (a) is a real latent defect for genuinely single-profile
users, but it **cannot be the cause of the reported symptom**. `SPRINT_PLAN.md` §2's causal chain
— *"⇒ Windows cannot match window → shortcut, so it makes a new button … and the unmatched button
has no registered display name"* — was never measured, and its (a) leg is now refuted.

## M3 — 📏 MEASURED: the shortcuts on disk, and the finding nobody had looked for

Probe: `lnkaumid.ps1` (in this folder), reading `System.AppUserModel.ID` via `Shell.Application`
`ExtendedProperty` plus target path via `WScript.Shell`.

**Hodos's three shortcuts:**

| `.lnk` | Target | AUMID |
|---|---|---|
| `User Pinned\TaskBar\Hodos Browser.lnk` | `…\AppData\Local\HodosBrowser\HodosBrowser.exe` | 🎯 **`Chromium.OYX4LNTP4DA5GSWHROKEK3C7BM`** |
| `Start Menu\Programs\Hodos Browser\Hodos Browser.lnk` | same | **`<NONE>`** |
| `Start Menu\…\Uninstall Hodos Browser.lnk` | `…\unins000.exe` | **`<NONE>`** |

⇒ **#3 half (b) is confirmed on disk**, not merely in `hodos-browser.iss:81,83` — the installed
Start Menu shortcut really does carry no AUMID.

**The new finding.** The **pinned** shortcut carries a Chromium-computed identity. Every other
pinned browser on this machine carries a clean, short one:

```
Brave.lnk              AUMID=Brave
Google Chrome.lnk      AUMID=Chrome
File Explorer.lnk      AUMID=Microsoft.Windows.Explorer
Outlook.lnk            AUMID=Microsoft.Office.OUTLOOK.EXE.15
Slack.lnk              AUMID=com.squirrel.slack.slack
Hodos Browser.lnk      AUMID=Chromium.OYX4LNTP4DA5GSWHROKEK3C7BM
```

🧠 **CLAIM:** `Chromium.<26-char base32>` is the shape Chromium's own `ShellUtil::GetBrowserModelId`
produces for an **unbranded, user-level** install, hashing the install directory — here
`C:\Users\archb\AppData\Local\HodosBrowser`. Labelled a claim: the shape and the branded
counter-examples (`Chrome`, `Brave`) are measured, the derivation is not verified against source.

🧠 **Inference:** Windows stamps a pinned `.lnk` with the **effective AUMID of the window being
pinned**. This `.lnk` therefore records that, *at pin time*, the effective identity of the Hodos
window was `Chromium.OYX…` — **not** the `HodosBrowser` our process set.

**Two sub-hypotheses — and M7 settles them.** They were: (1) **stale pin**, created while
single-profile so Chromium's default applied; (2) **Chromium overrides us** at runtime so
`HodosBrowser` never takes effect. ⇒ **M7 measures (1) true and refutes (2).**

## M4 — 📏 MEASURED: we set no per-window AUMID; Chrome and Brave do

Probe: `winaumid.ps1` (in this folder) — `EnumWindows` → `GetWindowThreadProcessId` →
`Get-Process().Path` (⛔ **path match, never process name**) → `SHGetPropertyStoreForWindow` →
`PKEY_AppUserModel_ID` (`{9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}`, pid 5).

| PID | Window | AUMID | Path |
|---|---|---|---|
| 35468 | Hodos Browser | **`<NONE>`** | `…\AppData\Local\HodosBrowser\HodosBrowser.exe` |
| 35468 | Hodos Browser | **`<NONE>`** | same |
| 26640 | Hodos Browser | **`<NONE>`** | same |
| 28124 | …— Brave | `Brave` | `…\BraveSoftware\Brave-Browser\Application\brave.exe` |
| 15928 | …— Chrome | `Chrome` | `…\Google\Chrome\Application\chrome.exe` |
| 15928 | …on X: … | 🎯 `Chrome.UserData.Profile1` | same |

⭐ **The instrument is trustworthy because it distinguishes.** It was pointed at Chrome and Brave
**first**, as positive controls, and returned correct, differing values — including Chrome's
**per-profile** form `Chrome.UserData.Profile1`, which is precisely the pattern our
`HodosBrowser.Profile_1` imitates. A probe that returned `<NONE>` for everything would have faked a
green; this one does not, so `<NONE>` for Hodos is a **reading**, not an instrument failure.

⚠️ **What this does and does not prove.** `SHGetPropertyStoreForWindow` reports the **window-level**
property. `<NONE>` means Hodos stamps no per-window AUMID — it does **not** prove the process-wide
explicit AUMID is absent or ineffective. ⛔ Do not cite M4 as proof that our AUMID failed; that is
exactly the over-claim `P3-A2` exists to avoid.

🧠 **What it does establish:** Chrome and Brave set identity **per HWND**; we set it **process-wide
only**, once, before any window exists. Two windows of two different profiles in **one** process
(which M6 shows is possible) therefore cannot have different identities under our design — but
**can** under Chrome's.

## M5 — #5: the global lookup, confirmed 🧠 CLAIM (code read, not measured)

`SPRINT_PLAN.md` §3 offered *"focus resolution keyed on a global rather than the active window"* as
a hypothesis to falsify. The code says it is right.

`simple_handler.cpp:396`:

```cpp
CefRefPtr<CefBrowser> SimpleHandler::GetHeaderBrowser() {
    auto* win = WindowManager::GetInstance().GetPrimaryWindow();   // <-- a global
    return win ? win->header_browser : nullptr;
}
```

Its own comment (`:390`) says these 18 accessors are *"backwards-compat shims for window 0"* and
that *"Cross-browser IPC within a handler should use `GetOwnerWindow()` instead"* —
`GetOwnerWindow()` being `WindowManager::GetWindow(window_id_)` (`:342`), the window that owns
**this handler**.

The Ctrl+F arm, `simple_handler.cpp:9072-9090`:

```cpp
if (event.windows_key_code == 'F' && role_.find("tab_") == 0) {
    if (event.modifiers & EVENTFLAG_CONTROL_DOWN) {
        CefRefPtr<CefBrowser> header = SimpleHandler::GetHeaderBrowser();   // <-- primary window
        if (header) { ...SendProcessMessage(PID_RENDERER, "find_show"); }
        if (header) { header->GetHost()->SetFocus(true); }                  // <-- raises THAT window
        return true;
    }
}
```

⇒ Ctrl+F in a non-primary window sends `find_show` to the **primary** window's header and calls
`SetFocus(true)` on it — which activates that window, and if it sits on another virtual desktop,
Windows switches desktops. **That is the reported symptom exactly.**

**Audit of `OnPreKeyEvent`'s arms** (`:8985` onward) — the good news, and the reason §6 calls this
bounded:

| Uses | Arms |
|---|---|
| ❌ global `GetHeaderBrowser()` | **2** — Ctrl+F (`:9080`), Ctrl+L (`:9101`) |
| ✅ correct `GetOwnerWindow()` | F5/Ctrl+R, Ctrl+W, Ctrl+J, Ctrl+Shift+A |
| ⚠️ global `TabManager::GetActiveTab()` | **4** — same defect class; `GetActiveTabForWindow(int)` already exists (`TabManager.cpp:625` vs the global at `:334`). **Out of scope** (§7) |

⇒ The correct API exists, siblings in the same function already use it, and the reported defect is
**two call sites**.

## M6 — 📏/🧠: "two instances" is one process with two windows — for the same profile

🧠 **CLAIM (code read).** `cef_browser_shell.cpp:5199-5235`: a launch calls
`SingleInstance::TryAcquireInstance(profileId)`; if that fails, `SendToRunningInstance(profileId,
launchUrl)` forwards over a named pipe and the new process **exits** (`:5225` *"SingleInstance:
Forwarded to running instance, exiting"*). The running process's listener posts `new_window`
(`SingleInstance.cpp:201`) and creates a second window **in the same process**
(`cef_browser_shell.cpp:1583` *"SingleInstance: Creating new window"*).

⚠️ **The pipe is keyed on `profileId`.** So:

| Configuration | Processes | `GetPrimaryWindow()` misroutes? |
|---|---|---|
| Two windows, **same** profile | **1** (forwarded) | ✅ **yes** — both windows share one `WindowManager` |
| Two windows, **different** profiles | 2 | ❌ **no** — each has its own `WindowManager` and its own primary |

⭐ **This explains the "mixed and inconsistent" same-desktop results** in the original report
(`SPRINT_PLAN.md` §1 item 5): same-profile pairs misbehave, different-profile pairs behave.

🎯 **It is also a falsifiable prediction**, and `P3-A8` carries it: pre-fix, two windows of
**different** profiles must **not** show the bug. If they do, M5/M6 is wrong and the row is void.

---

## M7 — 🎯 📏 MEASURED: the root cause, closed. Three identities for one app.

Two further readings, both non-destructive, settle M3's open question.

**M7.1 — the pin predates the AUMID.**

```
Name              CreationTime          LastWriteTime
Hodos Browser.lnk 4/27/2026 6:39:01 AM  4/28/2026 3:35:16 PM
```

**M7.2 — the branch had never run before the second profile existed.** The *first* `AUMID set` line
in the entire 2.56 GB log:

```
[2026-07-06 15:41:14.096] [MAIN] [INFO] AUMID set: Default
```

`profiles.json` gives `Profile_1.createdAt = 2026-07-06T21:41:06Z` = **15:41:06 local**. The first
AUMID line is **8 seconds later**. ⇒ The branch began running the instant the second profile
existed, and **never once before it**.

**M7.3 — Windows identifies our Start Menu entry by exe path.** `Get-StartApps`:

```
Name                    AppID
Brave                   Brave
Google Chrome           Chrome
Hodos Browser           C:\Users\archb\AppData\Local\HodosBrowser\HodosBrowser.exe
```

⇒ Because the shortcut declares no AUMID, Windows falls back to the **target path** as its identity.
Brave and Chrome declare real ones.

### 🎯 The root cause, as a measured sequence

| When | What | Evidence |
|---|---|---|
| 2026-04-18 | Only profile `Default` exists | `profiles.json` |
| **2026-04-27** | User **pins** Hodos. `size() > 1` is false ⇒ **no explicit AUMID** ⇒ Chromium's default is the effective identity ⇒ the pin is stamped `Chromium.OYX…` | M7.1 + M7.2 (no AUMID line before 2026-07-06) + M3 |
| **2026-07-06 15:41** | User creates `Profile_1`. 8 s later the branch runs for the first time; process identity becomes **`HodosBrowser`** | M7.2 |
| 2026-07-06 → now | Running window (`HodosBrowser`) **≠** pinned shortcut (`Chromium.OYX…`) ⇒ **separate taskbar button**. And `HodosBrowser` matches **no** shortcut on the system ⇒ no registered display name ⇒ Windows falls back to the exe filename ⇒ **"HodosBrowser.exe"** | M2.2, M3, M7.3 |

⇒ **Both reported symptoms, one cause, fully measured**: *adding a second profile silently changed
the application's identity, orphaning the user's pin — and the new identity is one that no shortcut
on the system declares, so Windows has no name for it.*

### ⛔ Sub-hypothesis 2 is refuted

If Chromium overrode our AUMID at runtime, the running window would carry `Chromium.OYX…`, would
**match** the pinned shortcut, and would **group** with it. The owner reports it does not group
(`SPRINT_PLAN.md` §2). ⇒ Our `HodosBrowser` **is** effective. 🧠 Labelled an inference: it is sound
given the owner's report, and `P3-A2` still confirms it cheaply — but it no longer *gates* the code.

### 🚨 The consequence for the proposed fix — the desk diagnosis had it backwards

`SPRINT_PLAN.md` §2 fix #1 is *"Set the explicit AUMID **always** in production."* Applied **alone**,
that does to every remaining single-profile user exactly what 2026-07-06 did to the owner: moves
them off the Chromium-derived identity **their pin already carries and currently matches**, and
orphans the pin.

⭐ **This is no longer a speculative risk — the owner's own machine is the natural experiment, and
the outcome was the bug being fixed.** ⇒ fix #1 is only safe **together with** fix #2 (shortcuts
declaring the same AUMID) **and** a decision about existing pins, which M7 now forces rather than
leaves open (§8 Q4).

---

## M8 — 📏 MEASURED: the machine is *already* in the configuration both defects need

Taken 2026-08-26 while the owner's browser was running. ⛔ Read-only (`Win32_Process`), matched by
`ExecutablePath`, filtered to non-`--type=` (i.e. browser-process, not CEF children):

```
ProcessId : 35468   ...\HodosBrowser.exe --profile="Default"   --picker-handle 5316
ProcessId : 26640   ...\HodosBrowser.exe --profile="Profile_1" --picker-handle 5260
```

Cross-referenced with M4's window enumeration: **PID 35468 has two visible windows**, PID 26640 has
one.

⇒ Right now, on the owner's machine:

| What is live | Why it matters |
|---|---|
| **PID 35468 (`Default`) has two windows in one process** | This is exactly M6's misrouting configuration. `P3-A8`'s positive arm can be run **without launching anything** |
| **PID 26640 (`Profile_1`) is a separate process** | This is `P3-A8`'s **negative control** — M6 predicts Ctrl+F here behaves correctly. Also live, also free |
| Two processes ⇒ AUMIDs `HodosBrowser` and `HodosBrowser.Profile_1`, plus an orphaned pin on `Chromium.OYX…` | ⇒ **three** distinct taskbar identities should be visible at once. Directly observable, and it confirms or refutes the whole M7 model in one glance |
| A `HodosBrowser.Profile_1` button exists on screen | ⇒ **Open question 2** (does a non-`Default` AUMID still fall back to the exe name?) is answerable by one right-click, with **no build and no install** |

⭐ **Consequence for the phase plan:** the three cheapest and most load-bearing observations need
none of the infrastructure §4.1 describes. They need the owner to look at a taskbar that is already
in the right state. The second Windows user account (§4.1 route 1) is still required for `P3-A4`'s
genuinely-single-profile arm, but it is no longer on the critical path to confirming the diagnosis.

---

## M9 — 👤 OWNER-OBSERVED, 2026-08-26: #3 confirmed, #5 confirmed, and a second symptom found

### M9.0 ⛔ Correction: the profile names were stated backwards in the owner-facing ask

`profiles.json` maps **`Default` → name "Archie"** and **`Profile_1` → name "Hodos"**. The kickoff ask
inverted them. `Profile_2` is an **orphaned directory**, absent from `profiles.json`, and nothing
runs from it. Corrected mapping:

| Running | Profile id | Display name | Desktop |
|---|---|---|---|
| PID 35468, **two windows** | `Default` | **Archie** | 1 |
| PID 26640, one window | `Profile_1` | **Hodos** | 2 |

### M9.1 — #3 confirmed on screen, and it matches M7 exactly

> *"If I right click the running instance it says hodosbrowser.exe, if I right click the generic pin
> not running it just says Hodos Browser."*

⇒ Exactly M7's prediction. The **pin** is named by its own `.lnk`; the **running** windows carry an
AUMID (`HodosBrowser` / `HodosBrowser.Profile_1`) that matches **no** shortcut, so Windows has no
registered display name and falls back to the exe filename. The pinned icon appears on both virtual
desktops and is **not** a running instance — it is the orphan.

### M9.1a — 🟢 Open question 2 ANSWERED: the per-profile AUMID loses its name too

Owner, 2026-08-29, right-clicking the **Hodos** (`Profile_1`) window's taskbar button: **yes**, it
also reads "hodosbrowser.exe".

⇒ Both `HodosBrowser` **and** `HodosBrowser.Profile_1` are unnamed. 🚨 **Design consequence:** adding
`AppUserModelID:` to the two Inno `[Icons]` entries **cannot fix the non-`Default` profiles** — a
per-profile AUMID matches no shortcut by construction, and a shortcut per profile does not exist.
Naming a shortcut-less AUMID needs one of:

| Option | Mechanism | Cost |
|---|---|---|
| **A — register the AUMID** ⭐ | `HKCU\Software\Classes\AppUserModelId\<AUMID>` with `ApplicationName` + `ApplicationIcon`. This is exactly what that key is for | One registry write per profile, at profile creation and at startup. No Start Menu clutter |
| B — per-profile shortcut | A Start Menu `.lnk` per profile carrying its AUMID. What Chrome does (`Chrome.UserData.Profile1`) | Clutters the Start Menu; must be created/removed with the profile |

⇒ **Recommend A**, with B noted as the precedent that proves the pattern works.
⚠️ Option A is a **registry write** — new surface for this project. It is per-user (`HKCU`), not
per-machine, and must be removed on uninstall and on profile deletion or it becomes orphaned state
(cf. `TICKET_deleted_profile_id_reused_over_orphaned_data`).

### M9.2 — #5 confirmed 👤 OBSERVED (positive arm)

> *"When I am in the second Archie it opens the search in the first instance."*

Owner, restating it precisely (2026-08-29):

> *"I run an 'Archie' and then I run another 'Archie' and I do ctrl+f while in the second Archie —
> the find search bar opens in the first Archie. **Nothing happens in the second one.**"*

⇒ M5's mechanism, observed. Two windows of the **same** profile = one process = one
`GetPrimaryWindow()`, and Ctrl+F drives the primary. *"Nothing happens in the second one"* is the
signature detail: the event is **consumed** (`return true`) in the window that received it and acted
on elsewhere — not duplicated, **redirected**.

### M9.2a — ⛔ RETRACTED: the "different-profile" negative control was a badly-posed test

The kickoff asked the owner to press Ctrl+F in the **Hodos** window as a control, predicting it would
behave correctly. The owner pushed back that the question made no sense — **and they were right.**
Hodos runs a **single** window, so its only window *is* its primary; Ctrl+F working there is the
ordinary single-window case and discriminates almost nothing. It would have produced a green that
meant nothing — the exact failure this harness exists to prevent, authored by the harness's own user.

⭐ It is also already answered: *"The problem is on multiple instances of the same profile"* is the
owner's own statement that the cross-profile case does not reproduce.

**Replaced by two controls that actually discriminate** (see `P3-A8`):

1. **Ordinal, not adjacent.** M5 predicts find always lands on the **primary** (first-created)
   window — not on "the previous one". With **three** same-profile windows, Ctrl+F in window 3 must
   open in window **1**, not window 2. ⛔ If it opens in window 2, the mechanism is wrong.
2. **Same-binary revert.** Post-fix, restore the `GetHeaderBrowser()` call on the same binary → the
   redirect returns. This is the standard control and the one that actually gates the fix.

### M9.3 — 🆕 A SECOND SYMPTOM, and a worse one: HTML5 video fullscreen crosses windows

> *"If I click the full screen on a video in the first instance, it makes the second instance go full
> screen. And vice versa, they both go full screen either way. Not the maximize but a full screen on
> a video makes the header section disappear on both if I have one up on a second monitor."*

**Root cause, code-read, `cef_browser_shell.cpp:273 :: HandleFullscreenChange(bool fullscreen)`.**
`SimpleHandler::OnFullscreenModeChange(browser, fullscreen)` (`simple_handler.cpp:1067`) **receives
the `browser` and discards it** — it calls `HandleFullscreenChange(fullscreen)` with no window
context at all. The function then operates on **five process-globals**:

| Global | Effect |
|---|---|
| `g_is_fullscreen` | One `bool` for the whole process — two windows **cannot** hold different fullscreen states |
| `g_hwnd` | Geometry is read from the **primary** window's client rect, whichever window went fullscreen |
| `g_header_hwnd` | Hides/shows the **primary** window's header ⇒ *"the header section disappears on both"* |
| `TabManager::GetAllTabs()` | Resizes **every tab in every window** in the process |
| `SimpleHandler::GetHeaderBrowser()` | Primary again, on the restore path |

⇒ Fullscreening a video in *any* window resizes *all* tabs in *all* windows to the *primary*
window's dimensions. ⭐ That is why it is visibly wrong on a **second monitor**: the other window's
tabs are sized to a rect that belongs to a different display.

🚨 **This is more damaging than the reported Ctrl+F defect** and it is the same root cause —
*window-scoped work performed against process-globals* — with the correct context **available and
thrown away**.

### M9.4 — 📏 MEASURED: the defect class is a half-finished migration, not two call sites

⛔ **This refutes the kickoff's own "bounded to two arms" framing.** That audit covered only
`OnPreKeyEvent` — the function the session prompt pointed at — and the owner found a defect outside
it within minutes. ⭐ `feedback_own_work_is_the_weakest_link`: a negative control only proves the
property you thought of, and the audit scope was the thing not questioned.

Counts across `cef-native/src` + `cef_browser_shell.cpp`:

| Global | Uses | Window-scoped counterpart |
|---|---|---|
| `g_hwnd` | **77** (54 handler + 23 shell) | `GetOwnerWindow()->hwnd` |
| `g_header_hwnd` | **25** | `GetOwnerWindow()->header_hwnd` |
| `TabManager::GetAllTabs()` | **16** | per-window tab list |
| `TabManager::GetActiveTab()` | **19** | `GetActiveTabForWindow(int)` — **18** existing correct uses |

⇒ 19 global vs 18 correct on the tab axis: the multi-window migration is **roughly half done**, and
a half-done migration is precisely the shape that produces the *"mixed and inconsistent"* results in
`SPRINT_PLAN.md` §1 item 5.

### M9.4a — ⛔ CORRECTION (2026-08-30): the counts above are RAW LINES and overstate the defect

Owner challenged the "137" figure. Re-counted, separating declarations from logic:

| Global | Raw lines (above) | **Real uses** | The gap |
|---|---|---|---|
| `g_hwnd` | 77 | **52** | 25 lines are `extern HWND g_hwnd;` forward declarations |
| `g_header_hwnd` | 25 | **25** | — |
| `GetAllTabs()` | 16 | **15** | one is the function definition |
| `GetActiveTab()` | 19 | **18** | one is the function definition |
| **Total** | **137** | **~110** | |

⛔ **"137" was a mechanical `grep -c` reported as a defect count.** That is the same error class this
harness exists to catch — a number that looks measured but counts the wrong thing. Corrected
everywhere it appeared.

### M9.4b — ⛔ CORRECTION: the 12 `ScalePx` sites were deferred to a phase that is CLOSED

The kickoff said the 12 `ScalePx(x, g_hwnd)` sites "belong with the Phase 1 DPI work". ⛔ **Phase 1
is finished.** A deferral pointing at a closed phase is a defect that quietly disappears.

Re-examined: `ScalePx(cssPx, hwnd)` (`LayoutHelpers.h`) calls `GetDpiForWindow(hwnd)`. Passing
`g_hwnd` takes DPI from the **primary** window. Phase 1 fixed DPI *within* one window
(`0a7d43b`, OSR overlay mouse input); it never opened two. So this is a **multi-window** DPI defect
Phase 1 could not have caught, not a Phase 1 regression.

⇒ Correct home: **Phase 3.5** (they are overlay layout). 📖 Code reading — ⛔ **never reproduced**;
`P3.5-A3` is written to reproduce it before fixing it.

### M9.4c — 📏 The `GetAllTabs()` sites split three ways, and one group must NOT be converted

| Function | Sites | Verdict |
|---|---|---|
| `HandleFullscreenChange` | L290, L331 | Phase 3 — reported symptom |
| ⛔ `SaveSession`, `ShutdownApplication` | L365, L578, L640 | 🚫 **CORRECT AS GLOBAL.** Session save and shutdown legitimately mean *all tabs in all windows* |
| ✅ `ShellWindowProc` | L1175, L1244, L1538, L1557, L1719 | Phase 3.5 — per-window layout |

⭐ **The correct pattern is already inside the buggy function.** `cef_browser_shell.cpp:1206`:

```cpp
HWND thisHeaderHwnd = bw ? bw->header_hwnd : g_header_hwnd;   // correct
// …siblings, same function: :1157, :1277, :1299, :1301, :1323, :1325, :1347, :1349, :1377 use the global
```

`ShellWindowProc` takes `HWND hwnd` — the window the message is for — as its first parameter, and
discards it in those arms. Same shape as `OnFullscreenModeChange` (M9.3): **the context is in hand
and thrown away.** ⇒ This is why the owner's "surely it's a pattern" instinct is right *for this
subsystem* and wrong for the scattered remainder.

---

## M10 — 🎯 📏 MEASURED: what actually names a taskbar button. Two candidates refuted.

The single most useful sequence in this phase, because **two of the three candidates were things
this project already believed**, and one of them I had implemented and committed.

| # | Candidate | Verdict | How it was settled |
|---|---|---|---|
| 1 | The exe's **version resource** (`FileDescription`) | ⛔ **REFUTED** | 📏 Measured on **both** binaries: dev `FileDescription = "Hodos Browser"`, installed `= "Hodos Browser"`, `chrome.exe = "Google Chrome"`. Taskbar still read **"HodosBrowser.exe"**. ⇒ Windows falls back to the exe **FILENAME**, not its description. `SPRINT_PLAN.md` §2's "⛔ it is NOT the version resource" was right; this is the first time there is a *measurement* behind it rather than an assertion |
| 2 | **Registry** — `HKCU\Software\Classes\AppUserModelId\<aumid>` with `ApplicationName` | ⛔ **REFUTED, and it was already committed** | 📏 Key verified **absent** before launch, **present** after, with `ApplicationName = "Hodos Browser"` and an icon path. 👤 Owner right-clicked the dev taskbar button: **still "HodosBrowser.exe"**. That key drives **toast notifications**, not the taskbar. Implementation removed |
| 3 | A **shortcut** declaring the same AUMID | ✅ **CONFIRMED** | 📏 Created `.lnk` with `System.AppUserModel.ID = HodosBrowser.Dev` named **"Hodos Browser DEVTEST"**, restarted the browser on the same binary. 👤 Owner: button read **"Hodos Browser DEVTEST"** |

⭐ **Why the test string was deliberately ugly.** Naming the test shortcut "Hodos Browser" would have
produced a result consistent with **all three** candidates and proved none of them — the classic
false green. "Hodos Browser DEVTEST" exists nowhere else on the machine, so the button could only
have got it from the shortcut. Cheap discipline, worth reusing.

⭐ **Confirmed a second time, on the real path.** With the code change in, launching `--profile=Profile_1`
wrote `Hodos Browser - Test_1.lnk` carrying `HodosBrowser.Dev.Profile_1` (verified equal to the
process AUMID in the log), and 👤 the owner read **"Hodos Browser - Test_1"** off the taskbar.

### 🔧 Correction to M3: the hash is per-USER, not per-path

M3 said `Chromium.OYX4LNTP4DA5GSWHROKEK3C7BM` was *"`Chromium` + base32 hash of the install
directory"*. 📏 **Wrong.** The owner's Start Menu contains:

```
Maxthon.lnk   AUMID=Maxthon_Id.OYX4LNTP4DA5GSWHROKEK3C7BM
```

**Same suffix, different application, different install path.** ⇒ it is Chromium's *user-specific*
registry suffix, derived from the Windows account SID, not from the path. M3 was labelled 🧠 CLAIM,
which is the only reason this is a correction and not a defect — but it is a reminder that a
plausible derivation is not a measured one. The root-cause narrative in M7 is **unaffected**: the pin
still carries a Chromium-era identity the process no longer claims.

## M11 — 👤 The generated shortcut is inert in a dev build, and the owner found it

👤 The owner clicked the newly-created `Hodos Browser - Test_1` shortcut and got a *"profile in
use"*-style error dialog, and asked whether it was a regression.

📏 **It was not.** No `debug_output-*.log` was created for that launch at all — the newest log
predated the click — so the process died **before `Logger::Initialize`**. Exactly one thing runs that
early: `AppPaths::EnforceDevSafeguard`, whose caller shows a `MessageBoxA` and returns 1
(`cef_browser_shell.cpp:4920`).

⇒ A `.lnk` **cannot carry an environment variable**. A shortcut to `build\bin\Release\HodosBrowser.exe`
therefore launches without `HODOS_DEV=1`, and the safeguard correctly refuses rather than let a dev
build open the production database.

⚠️ Checked and cleared first, rather than assumed: the shortcut passes `--profile="Profile_1"` **with
quotes**, and `ProfileManager::ParseProfileArgument` handles the quoted form
(`ProfileManager.cpp:888-893`). Not the cause — and the process never reached the parser anyway.

⇒ **Fix: `EnsureProfileShortcut` now returns early when `IsDevEnv()`.** Guarded inside the function,
not at the call site, so a future caller cannot reintroduce it. 📏 Verified: relaunching
`--profile=Profile_1` created **no** shortcut and logged *"Dev build — skipping profile shortcut …
(a .lnk cannot set HODOS_DEV=1, so it could never launch)"*.

⭐ **This is a defect the owner found by using the thing**, in a path no test covered — the shortcut
was written by dev builds, for dev builds, and could never work.

---

## Open questions carried into the contract

| # | Question | Status | Answered by |
|---|---|---|---|
| 1 | Is our process AUMID effective, or does Chromium override it? | 🟢 **Answered by M7** — effective; sub-hypothesis 2 refuted. Confirmation only, no longer a gate | `P3-A2` |
| 2 | Does a non-`Default` AUMID (`HodosBrowser.Profile_1` = **Hodos**), which matches no shortcut, still fall back to the exe name? | 🟡 **Probably yes (M9.1)** — but the owner's report did not separate Archie from Hodos | `P3-A5` |
| 3 | Does the M5/M6 mechanism survive its different-profile prediction? | 🔴 **OPEN — the negative control was not run** (M9.2) | `P3-A8` |
| 4 | Do we migrate existing pins, or tell users to re-pin once? | 🔴 OPEN — but M7 proves pins **will** break without action | §8 Q4 |
| 5 | 🆕 How far does the fullscreen fix go — the one function, or the class (M9.4)? | 🔴 OPEN | §8 Q3 |
