# Profile Picker vs Taskbar — Why It Shows on One Windows Machine but Not Another

**Status:** INVESTIGATION / DESIGN INPUT ONLY. No code changes. Fix deferred (see Part D and
`PROFILE_PICKER_SAME_PROCESS_PLAN.md`).

**Question:** Both users launch Hodos Browser from the Windows taskbar, yet the profile picker
appears on every launch on one machine and never on the other. Why is this non-deterministic,
and how does Chrome make it deterministic?

**One-line answer:** Our picker shows **iff the launch command line has no `--profile=` flag**.
A Windows taskbar pin of a *running* Hodos window bakes the live process command line —
including `--profile="X"` — into the pin, because we set an explicit per-profile AppUserModelID
(AUMID) but register **no relaunch command** and ship **no per-profile Start-Menu shortcut** for
Windows to resolve the pin against. So whether a machine's taskbar entry carries `--profile=`
depends entirely on *how/when that pin was created*, not on "launching from the taskbar."

---

## Part A — How Chrome does it

### A1. The picker: single process, `--profile-directory`

- Chrome's profile picker runs **in a single process**, inside a minimal internal "system
  profile." On selection it constructs the chosen `Profile` + `BrowserWindow` **in the same
  process** and closes the picker — no exe re-spawn, no cold boot. (This is the model our
  `PROFILE_PICKER_SAME_PROCESS_PLAN.md` targets.)
- **When it shows vs. goes straight to a profile:** by default the picker is *not* shown when
  only one profile exists; with multiple profiles it shows at startup unless the user unchecks
  "Show on startup." The `ProfilePickerOnStartupAvailability` enterprise policy can force it on
  (value `Forced`/2) even with a single profile, or disable it.
- **`--profile-directory=`** on the command line selects a specific profile and **bypasses the
  picker** — the analogue of our `--profile=`. A shortcut that does *not* specify a profile
  launches Chrome with the **most-recently-used** profile (Chrome's fallback) rather than a
  picker, unless the picker-on-startup setting says otherwise.
  Sources:
  - https://chromeenterprise.google/policies/profile-picker-on-startup-availability/
  - https://chromium.googlesource.com/chromium/src/+/HEAD/docs/windows_shortcut_and_taskbar_handling.md

### A2. Taskbar behavior on Windows (the important part)

From the Chromium doc *Windows Shortcut and Pinned Taskbar Icon handling*
(https://github.com/chromium/chromium/blob/main/docs/windows_shortcut_and_taskbar_handling.md):

- **Per-window AUMI.** Chrome sets each window's AppUserModelID in
  `BrowserWindowPropertyManager::UpdateWindowProperties`. Format:
  `<BaseAppId>[browser_suffix][.profile_name]`, where **profile_name is only appended when it is
  not the default profile.** Windows groups windows with the same AUMI under one taskbar button.
- **The primary pin is argument-free.** Chrome's installed shortcuts "do **not** specify a
  profile, so they launch Chrome with the most recently used profile." The main taskbar pin
  therefore carries no `--profile-directory`.
- **Per-profile shortcuts are a deliberate, separate artifact.** "When the user has more than
  one profile, the shortcuts are renamed to include the profile name, e.g., `Chrome.lnk` becomes
  `<profile name> - Chrome`," and the icons "are badged with their profile icon." These
  profile-specific shortcuts (which *do* carry `--profile-directory=`) are created by Chrome for
  the profile flow — they are distinct `.lnk` files with a matching profile-specific AUMI, so a
  running profile window resolves/pins to the *correct* profile shortcut rather than to a raw
  process command line.
- **How Chrome keeps a transient profile arg out of the primary pin:** because each window
  carries an explicit AUMI *and* Chrome maintains a matching Start-Menu shortcut for each AUMI
  (base AUMI → argument-free `Chrome.lnk`; profile AUMI → `<profile> - Chrome.lnk`), Windows
  resolves a pin-of-running-window **to the matching shortcut** instead of synthesizing one from
  the live process command line. The base window pins to the clean shortcut; a profile window
  pins to the profile shortcut. Neither bakes a stray `--profile-directory` into the *base* pin.

### A3. The Windows rule that ties it together

Microsoft's `System.AppUserModel.RelaunchCommand` reference
(https://learn.microsoft.com/en-us/windows/win32/properties/props-system-appusermodel-relaunchcommand):

> "This property is used only if a window has an explicit Application User Model ID
> (AppUserModelID)… **If the window does not have an explicit AppUserModelID, this property is
> ignored and the window is grouped and pinned as if it were part of the process that owns it.**"

Set via `SHGetPropertyStoreForWindow` → `IPropertyStore` → `PKEY_AppUserModel_RelaunchCommand`
(formatID `9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3`, propID 2). It **must be set together with**
`System.AppUserModel.RelaunchDisplayNameResource` — if either is missing, neither is used.

**Practical resolution order when Windows pins a *running* window:**
1. Window has explicit AUMI **and** a Start-Menu shortcut with a matching AUMI exists → pins
   that shortcut (clean command line). ← *Chrome's case.*
2. Window has explicit AUMI **and** a `RelaunchCommand`/`RelaunchDisplayNameResource` pair is set
   → pins using that command. ← *the lightweight fix in Part D.*
3. Window has explicit AUMI but **no matching shortcut and no relaunch command** → Windows
   synthesizes the pin from the **live process command line (exe + args)**. ← **Hodos's case:
   `--profile="X"` gets baked in.**
4. No explicit AUMI → pinned as the owning process (also the raw process command line).

### A4. Key takeaway

Chrome's picker is deterministic because the **primary taskbar pin is guaranteed to launch a
clean, argument-free command line** — Chrome ensures a matching argument-free shortcut exists for
the base AUMI, so pinning a running window can never bake in a transient `--profile-directory`.
Profile-specific launches live in *separate, clearly-labeled* profile shortcuts. Launch intent
("show picker / MRU profile" vs "open profile X directly") is therefore fixed by *which* pin the
user clicks, and every pin's command line is stable regardless of what was running when it was
created.

---

## Part B — Our code (read-only)

### B1. The showPicker decision (why the flag is load-bearing)

`cef-native/cef_browser_shell.cpp:4417-4437`:

- `ParseProfileArgument(GetCommandLineW())` returns `""` when there is **no `--profile=` flag**
  (the taskbar/desktop/Start no-arg launch). `ProfileManager::ResolveStartup(argProfile, …)` then:
  - explicit valid `--profile` → that profile (**picker suppressed**);
  - no-arg + exactly 1 profile → that profile (no picker);
  - no-arg + >1 profiles + picker setting on → **picker mode**;
  - no-arg + >1 profiles + picker setting off → the default (starred) profile.
- `ParseProfileArgument` is the only gate: `cef-native/src/core/ProfileManager.cpp:652-690`
  (wide-string) / `:642-650` (argv). It literally scans for `--profile=`.
- A C3 diagnostic line already logs the exact inputs (`profileCount / pickerSettingOn /
  defaultId / showPicker`) at `cef_browser_shell.cpp:4430-4436`.

So: **the picker's appearance is a pure function of whether `--profile=` is on the command line**
(given >1 profiles + setting on). Nothing about "the taskbar" enters the decision — only the
argv the taskbar entry supplies.

### B2. Selecting a profile → two-process spawn with `--profile=`

`cef-native/src/core/ProfileManager.cpp:525-563` (`LaunchWithProfile`, Windows branch):

```
std::wstring cmdLine = L"\"" + exePath + L"\" --profile=\"" + profileId + L"\"";
CreateProcessW(NULL, cmdLine, …);
```

Picking a profile in the picker spawns a **new** `HodosBrowser.exe --profile="X"` and the picker
process closes (`simple_handler.cpp` `profiles_switch` → `LaunchWithProfile` → `WM_CLOSE`). That
child process now *has* `--profile=` on its command line — and it is that child window a user
would right-click-pin.

### B3. AUMID is set, but with NO relaunch command and NO per-profile shortcut

`cef-native/cef_browser_shell.cpp:4503-4516`:

```
if (!g_picker_mode && (hodos::IsDevEnv() || GetAllProfiles().size() > 1)) {
    std::wstring aumid = hodos::IsDevEnv() ? L"HodosBrowser.Dev" : L"HodosBrowser";
    if (profileId != "Default") aumid += L"." + profileId;   // e.g. HodosBrowser.Profile_2
    SetCurrentProcessExplicitAppUserModelID(aumid.c_str());
}
```

- We set an **explicit, per-profile AUMID** (`HodosBrowser.Profile_2`, etc.) whenever there is
  >1 profile (or dev). Picker mode is skipped (base AUMID, `:4508`).
- **Confirmed by grep across `cef-native/`:** the *only* AUMID-related call in the codebase is
  `SetCurrentProcessExplicitAppUserModelID` at `:4514`. There is **no**
  `SetAppUserModelRelaunchCommand`, no `PKEY_AppUserModel_RelaunchCommand`, no
  `SHGetPropertyStoreForWindow`, no `IPropertyStore`. (`RelaunchCommand` string matches exist
  only in `cef_browser_shell.cpp` comments and `WINDOW_INSTANCE_DECONFLICTION.md`, not in code.)
- `SetupTaskbarProfile` (`cef-native/src/core/TaskbarProfile.cpp:326-350`, called at
  `cef_browser_shell.cpp:4984-4985`) only sets an **overlay badge icon** via `ITaskbarList3::
  SetOverlayIcon`. It does **not** set any relaunch/pin metadata.
- **Installer ships no per-profile shortcut and no AUMID on its shortcut.**
  `installer/hodos-browser.iss:80-83`:
  ```
  Name: "{group}\Hodos Browser";     Filename: "{app}\HodosBrowser.exe"
  Name: "{autodesktop}\Hodos Browser"; Filename: "{app}\HodosBrowser.exe"; Tasks: desktopicon
  ```
  Both are **argument-free** with **no `Parameters:` and no `AppUserModelID:`**. So the only clean
  shortcut Windows knows about has *no* explicit AUMID, and it does **not** match the running
  window's explicit `HodosBrowser.Profile_2` AUMID.

### B4. What this means for pinning (mapping B3 onto A3)

Take a running multi-profile Hodos window. Its window/process has explicit AUMID
`HodosBrowser.Profile_2` (set at `:4514`). When the user pins that running window:

- There is **no Start-Menu shortcut whose AUMID matches** `HodosBrowser.Profile_2` (the installer
  shortcut has no explicit AUMID) → resolution step A3-1 fails.
- We set **no `RelaunchCommand`** → step A3-2 fails.
- → Windows falls through to A3-3 and **synthesizes the pin from the live process command line**,
  which for a profile window spawned by B2 is `…\HodosBrowser.exe --profile="Profile_2"`.

Additionally, **because the AUMID embeds the profile name, each profile is a *separate* taskbar
button** that can be pinned independently — and each such pin captures *its* process command line
(with `--profile=`). There is no "neutral" running button while multi-profile: every live window
already carries a profile-specific AUMID. The *only* argument-free pin obtainable is one made from
the installer's Start-Menu/desktop shortcut, or one made **before a second profile existed** (when
the `>1 profiles` gate was false, no explicit AUMID was set, and the raw command line was the bare
exe).

---

## Part C — Reconciling "both launch from the taskbar"

"Launching from the taskbar" is not one thing: the two machines have **taskbar `.lnk` files with
different `Target`/`Arguments`**. Three concrete hypotheses, each with an exact check.

> **Where the taskbar pins live (all checks):**
> `%APPDATA%\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\*.lnk`
> Read a pin's target + args in PowerShell:
> ```powershell
> $s = New-Object -ComObject WScript.Shell
> Get-ChildItem "$env:APPDATA\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\*.lnk" |
>   ForEach-Object { $l = $s.CreateShortcut($_.FullName); "$($_.Name)  =>  $($l.TargetPath) $($l.Arguments)" }
> ```
> GUI equivalent: right-click the taskbar icon → in the jump list right-click **"Hodos Browser"**
> → **Properties** → look at the **Target** field for a trailing `--profile="…"`.

### Hypothesis 1 — Pin created from a running `--profile` window vs. from the installer shortcut

- *Machine that shows the picker:* its pin was created from the **installer's argument-free
  shortcut** (`{group}\Hodos Browser` or the desktop icon) — Target = `…\HodosBrowser.exe`, no
  args → no `--profile=` → picker shows (B1).
- *Machine that never shows the picker:* its pin was created by **right-click → "Pin to taskbar"
  on a running profile window** (e.g. after picking a profile, per B2), so Windows baked
  `--profile="X"` into the `.lnk` (B4) → picker suppressed.
- **Check:** compare the `Arguments` field of the TaskBar `.lnk` on each machine (script above).
  Picker-machine should be empty; no-picker-machine should contain `--profile="…"`.

### Hypothesis 2 — Pin captured while single-profile vs. after a second profile existed

- If a machine was pinned **while it had only one profile**, the `>1 profiles` gate at
  `:4508` was false → no explicit AUMID was set → even a pin-of-running-window captured the bare
  exe (no `--profile`). Later adding a 2nd profile makes the picker start showing on that machine.
- The other machine may have had its pin **(re)created after the 2nd profile existed**, off a
  profile-specific window, capturing `--profile=` (B4).
- **Check:** on each machine, `ProfileManager` / `profiles.json` count *and* the pin's args. A
  clean-args pin that now shows the picker = pinned pre-second-profile; a `--profile=` pin =
  pinned post-second-profile from a profile window. (`%APPDATA%\HodosBrowser\profiles.json`.)

### Hypothesis 3 — Two different `.lnk`s / jump-list relaunch entries with different targets

- The user may have **multiple** Hodos entries (a clean installer pin *and* a profile-specific
  pin created separately because the profile AUMID gives it its own taskbar button, A2/B4). The
  two machines may simply have *different ones of these* pinned. A jump-list "relaunch" of a
  profile-badged button re-runs its captured `--profile=` target.
- **Check:** enumerate **all** `.lnk`s in the TaskBar folder (script above lists every one) and
  note how many reference `HodosBrowser.exe` and which carry `--profile=`. Also check
  `…\User Pinned\StartMenu\` and the desktop for stray profile shortcuts. Confirm on the
  no-picker machine that the *pinned* icon (not another copy) is the one with `--profile=`.

> Any one check (the `.lnk Arguments` field) settles it. Expected finding: the picker machine's
> pin has empty args; the no-picker machine's pin has `--profile="…"`. That fully explains the
> divergence with **no code difference between the two machines** — only pin provenance differs.

---

## Part D — Fix options (design only; do NOT implement here)

### Option 1 — Lightweight: register a clean `RelaunchCommand` (+ optionally a matching shortcut)

Set, on each real (non-picker) window after creation, via `SHGetPropertyStoreForWindow` →
`IPropertyStore`:

- `PKEY_AppUserModel_RelaunchCommand` = argument-free `"<app>\HodosBrowser.exe"` (no `--profile`),
- `PKEY_AppUserModel_RelaunchDisplayNameResource` = a display name (**must be set together** per
  A3, or neither is honored).

Effect: pinning a running window resolves via A3-2 to the **clean** command → the pin launches
no-arg → picker shows deterministically (given >1 profiles + setting on), matching the
picker-machine behavior everywhere.

**Trade-offs / cautions:**
- **This deliberately makes a per-profile pin *stop* reopening that profile directly** — the pin
  becomes "open Hodos (→ picker)", not "open Profile 2." That is the *intended* determinism, but
  it removes the (accidental) "pinned straight to my profile" convenience some users may rely on.
  Chrome's answer to keep both is *separate* profile shortcuts (A2) — see the "full" note below.
- `RelaunchDisplayNameResource` wants a resource (`"path,-id"`); a plain string may render oddly.
  Needs testing on Win10 and Win11 (pin-resolution behavior differs subtly across builds).
- Must be applied per-window (multi-window) and **not** in picker mode (picker keeps base AUMID).
- Existing *already-baked* `--profile=` pins won't change retroactively — users would need to
  unpin/re-pin. A relaunch command only governs *future* pins.
- To preserve BOTH behaviors like Chrome (clean base pin *and* working per-profile pins), we would
  additionally need to (a) write per-profile `.lnk`s with matching profile AUMIDs and
  `--profile=` args, and (b) put the base AUMID + argument-free target on the installer shortcut.
  That is more than "lightweight" and edges toward Chrome's full shortcut-management machinery.

### Option 2 — Full same-process picker refactor (Chrome model)

Adopt `PROFILE_PICKER_SAME_PROCESS_PLAN.md`: picker-first startup in a single process; on
selection, build the header/tab browsers for the chosen profile **in-process** (per-profile
`CefRequestContext` with its own `cache_path`) instead of spawning `HodosBrowser.exe --profile=`.

Because there would be **no `--profile=` self-spawn**, the "pinned a running profile window and
baked in `--profile=`" failure mode largely disappears — the running process's command line stays
clean regardless of which profile is active. This is the architecturally-correct fix but is a
large, high-risk effort (CEF cache-path is process-global and set before `CefInitialize`; wallet
per-profile handoff; render `--profile` propagation; update-gate accounting). See that doc's
danger areas and phased spike plan. **Note C3** (picker "shows once then default forever") is a
*separate* diagnostic issue, not this refactor.

### Recommendation

- **Immediate determinism, low risk:** Option 1's `RelaunchCommand` pair (accepting that pins
  become "open → picker"). Pairs naturally with the C3 startup-log diagnosis already in place.
- **Correct long-term:** Option 2. If we want Chrome-identical UX (clean base pin *plus* working
  per-profile pins), that requires Chrome-style shortcut management (per-profile `.lnk`s + base
  AUMID on the installer shortcut) layered on either option.

---

## Code anchors (read-only, verified this session)

| What | Location |
|------|----------|
| showPicker decision (`ParseProfileArgument`→`ResolveStartup`→`g_picker_mode`) | `cef-native/cef_browser_shell.cpp:4417-4437` |
| C3 diagnostic startup log | `cef-native/cef_browser_shell.cpp:4430-4436` |
| AUMID set (per-profile, no relaunch cmd) | `cef-native/cef_browser_shell.cpp:4503-4516` |
| `SetupTaskbarProfile` call (badge only) | `cef-native/cef_browser_shell.cpp:4984-4985` |
| `SetupTaskbarProfile` / overlay badge | `cef-native/src/core/TaskbarProfile.cpp:326-350` |
| `LaunchWithProfile` (spawns `--profile=`) | `cef-native/src/core/ProfileManager.cpp:525-563` |
| `ParseProfileArgument` (wide / argv) | `cef-native/src/core/ProfileManager.cpp:652-690` / `:642-650` |
| Installer shortcuts (argument-free, no AUMID) | `installer/hodos-browser.iss:80-83` |
| No relaunch-command API anywhere | grep: only `SetCurrentProcessExplicitAppUserModelID` at `cef_browser_shell.cpp:4514` |

## Sources (Chrome / Windows)

- Chromium — *Windows Shortcut and Pinned Taskbar Icon handling*:
  https://github.com/chromium/chromium/blob/main/docs/windows_shortcut_and_taskbar_handling.md
- MS Learn — *System.AppUserModel.RelaunchCommand*:
  https://learn.microsoft.com/en-us/windows/win32/properties/props-system-appusermodel-relaunchcommand
- MS Learn — *System.AppUserModel.ID*:
  https://learn.microsoft.com/en-us/windows/win32/properties/props-system-appusermodel-id
- Chrome Enterprise — *ProfilePickerOnStartupAvailability*:
  https://chromeenterprise.google/policies/profile-picker-on-startup-availability/
</content>
</invoke>
