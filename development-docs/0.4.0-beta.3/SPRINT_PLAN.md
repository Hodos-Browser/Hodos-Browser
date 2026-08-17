# beta.3 sprint — plan

**Opened:** 2026-08-17 · **Status:** 🚧 SCOPING — workstreams agreed in shape, cut line not yet set.

> **Decided:** `v0.4.0-beta.2` will **NOT** be promoted. It is kept as a draft and run locally as a
> soak build over the coming week. **beta.3 is the release users get**, carrying everything below
> plus the 0.4.0 engine work. That makes the appcast `minimumSystemVersion` fix a **hard
> prerequisite for beta.3**, not a nice-to-have — see `TICKET_appcast_missing_minimum_system_version.md`.

> **Engine:** no Blink change is expected, so **no CEF rebuild** is anticipated. If that changes it is
> a ~5 h rebuild plus a new asset, and it reshapes the whole schedule — flag it early.

> **CI:** the dev fork's Actions minutes are exhausted until ~2026-09-01. All work below is
> local-first by necessity. Anything needing a tag build queues behind the reset.

---

## 1. The raw list, as reported

Kept verbatim in substance so nothing is lost in the re-grouping below.

| # | Reported |
|---|---|
| **1** | Overlays close on click-outside correctly, **except directly below the modal** — clicking just under it does nothing; further below, or left/right, works. Suspected overlay window sizing vs React component fill. |
| **2** | **Profile picture cannot be selected.** Edit → choose file opens Explorer, but the profile overlay **closes as soon as the user navigates in the file dialog**. Selecting an image anyway does not populate. Worked before. |
| **3** | Taskbar right-click → new instance shows **"HodosBrowser.exe"**; should read "Hodos Browser". |
| **4** | **Import Chrome data** — profiles, passwords, cookies, history — so a user's setup "just works" as on their other computer. Open questions on UX: auto-detect vs folder-picker vs prompt at first-run. |
| **5** | **Windows virtual desktops:** two Hodos instances on two desktops; `Ctrl+F` in the second **switched desktops** and opened find in the *other* instance. Same-desktop multi-instance results were mixed and inconsistent. |
| **6** | **macOS mic never tested** — Twitter Spaces did not work on Mac. Windows mic tested, Mac not. Also: **tab context menu parity** — mute tab/site, new tab to right, duplicate, close, reload, pin, bookmark. |
| **7** | **DPI / mouse offset.** After dragging the window to the smaller screen, the cursor is off inside the **wallet overlay**: hovering a button does not highlight, but slightly above it does. Affects all modals on the secondary screen; correct again on the primary. (Also two unreproducible notes: downloads "Show in Folder" — now works; an un-clickable Approve — no repro.) |

## 2. Pre-flight findings — these change the shape of three items

Checked against the code before planning, so the phases are built on facts rather than the reported symptom.

### ⚠️ #3 is NOT a missing version resource

`cef-native/tools/stamp_win_resources.cpp` already sets `kFileDescription = "Hodos Browser"`, and the
string **is present in the shipped beta.2 binary** (verified by extracting `HodosBrowser.exe` from the
released portable zip and searching its UTF-16 resource data).

⇒ The taskbar name is therefore **not** coming from `FileDescription`. The likely source is the
**AppUserModelID / jump-list identity**, which Windows takes from the shortcut or an explicitly-set
AUMID. Investigation should start at the Inno `[Icons]` shortcut and whether we set an AUMID at all —
**not** at the resource stamper.

### ⚠️ #6's media plumbing is present, which makes the Mac failure more interesting

- `cef-native/mac/entitlements.plist` carries **both** `com.apple.security.device.microphone` and
  `…device.camera`.
- `cef-native/Info.plist` carries **both** `NSMicrophoneUsageDescription` and `NSCameraUsageDescription`.
- `SimpleHandler::OnRequestMediaAccessPermission` **is implemented** and inspects the audio / video /
  desktop-capture flags.

⛔ **But `cef-native/mac/helper-Info.plist.in` has NEITHER usage string** — and on macOS capture runs
in a helper process. That is the prime suspect and it is **only diagnosable on a Mac**.

### 🚨 #4 hits a hard external wall — decide scope against it

Chrome encrypts cookies and passwords with **DPAPI bound to the Windows user account**, and since
Chrome 127 with **App-Bound Encryption** tied to the Chrome binary itself.

| Data | Same machine | **Different machine** |
|---|---|---|
| Bookmarks, history | ✅ portable (we already import these — `ProfileImporter.h`) | ✅ |
| Passwords | ⚠️ DPAPI/ABE | ⛔ **only** via Chrome's user-initiated CSV export |
| Cookies / sessions | ⚠️ DPAPI/ABE | ⛔ **effectively impossible** |

⇒ *"works as they had it on their other computer"* is **not achievable** for encrypted data by any
route Chrome itself does not provide. Chrome's own answer is account sync. ⚠️ Separately: importing
another browser's **live logged-in sessions into a wallet browser** is a real security-surface
decision, not just a feature.

⭐ **Existing code to extend, not duplicate:** `cef-native/include/core/ProfileImporter.h` already does
Chrome/Brave/Edge **bookmarks and history**. A Firefox stub exists and is never called.

## 3. Workstreams

The seven items are four pieces of work.

### WS1 — Overlay input & DPI correctness · items 1, 2, 7 · **FIRST**

All three are the same defect class: **the overlay's model of where things are ≠ where they are.**
Overlay HWND geometry vs React fill, hit-testing, OSR mouse-coordinate translation, and the
close-path guards. One investigation, one test rig.

⛔ **Why this is first, and it is not a UX argument.** #7 is a **correctness problem in the money
path** — a cursor offset inside the wallet overlay during a send means the user hovers one control
and activates another. That outranks every feature on this list.

Leads already in hand:
- **#2:** CLAUDE.md documents `g_file_dialog_active` as the *synchronous C++* guard that spares
  overlays during a native file dialog. The profile panel is a **mouse-hook** overlay
  (`ProfilePanelMouseHookProc`); the guard is documented on the `WM_ACTIVATEAPP` path. If the hook
  does not consult it, that is exactly the reported symptom.
- **#2 (second trap):** hidden `<input type="file">` triggered by `.click()` is the known-broken CEF
  pattern; **visible** file inputs work. `WalletPanelPage.tsx` has a working reference implementation.
- **#1:** if the overlay HWND is taller than the rendered React content, the empty strip below the
  visible modal is still "inside" the window — clicks there would not close it. Matches the symptom
  precisely (below = dead, further below = outside = closes).
- **#7:** overlays are **OSR**; mouse events are forwarded manually with explicit coordinates. A
  per-monitor DPI scale that is not re-resolved for the monitor the overlay is on produces exactly
  this offset.

⚠️ Load-bearing safeguards in blast radius — audit every one: the **gold pill** payment indicator,
`g_wallet_overlay_prevent_close`, `g_file_dialog_active`, and the four privacy-perimeter gates.
Verification must include the **DPI & resolution matrix** cells #4/#6/#9.

### WS2 — Window / instance / focus identity · items 5, 3

`WindowManager` + Win32 shell integration. **Windows-only** — macOS Spaces is a different mechanism
and #3 has no macOS analogue. #5 needs a genuine deep dive; the reported inconsistency across
same-desktop instances suggests focus resolution keyed on a global rather than the active window.

### WS3 — Tab & peripheral parity · item 6

Tab context-menu parity (mute tab/site, new tab to right, duplicate, close others, reload, pin,
bookmark) plus mic/camera. Menu work is cross-platform and uses the existing custom
`MENU_ID_USER_FIRST` context-menu machinery. Mic/camera splits: Windows is reported working; **macOS
is unverified and suspected** (see §2).

### WS4 — Chrome import · item 4

Standalone, research-heavy, security-sensitive. Scope against §2's wall **before** design.
⚠️ Chrome on macOS uses **Keychain**, not DPAPI — assume no symmetry; Mac researches its own half.

## 4. Order

**WS1 → WS2 → WS3 → WS4.** WS4 last because it is the item most able to balloon, and the only one
whose value is undermined by an external constraint we do not control.

## 5. Mac tasking

⛔ **Do not hold all Mac work to the end.** Three items are **inputs to our design, not outputs** —
deferring them buys rework.

| When | Mac item | Why it cannot wait |
|---|---|---|
| **Now** | **Sparkle 2.9.6 verification** | Bumped, ships on macOS, never run. Blocks promotion. Already relayed. |
| **Now** | **Big Sur / `minimumSystemVersion` call** | Product decision with a macOS-shaped answer. |
| **Now** | **Mic/camera diagnosis (#6)** | If it is the helper-plist/TCC theory, it changes **what we build**, not just what we test. |
| **Now** | **Do WS1's symptoms reproduce on macOS?** (items 1, 7) | macOS uses borderless `NSWindow` + `InstallClickOutsideMonitor`, **no `WH_MOUSE_LL`**. Designing a Windows-shaped fix first risks a structurally wrong answer for Mac. One cheap question de-risks the workstream. |
| Later | Tab context menu port | Build once on Windows, then port. |
| Later | Chrome-import macOS half (Keychain) | Parallel research once WS4 starts. |
| **Never** | Items 3, 5 | Windows-only by nature. |

## 6. Decisions owed

1. **WS4 scope** — "bookmarks + history + passwords-via-CSV, same machine, one button", or a full
   research pass first?
2. **Chrome-import UX** — auto-detect the local profile, folder-picker, or offer at first-run? (And
   whether importing live sessions into a wallet browser is acceptable at all.)
3. **Cut line** — is all four workstreams in beta.3, or does WS4 slip?
4. Big Sur users: nothing, a pinned final 0.3.x, or an in-app message?

## 7. Also in this sprint, already filed

`TICKET_appcast_missing_minimum_system_version.md` (**now a beta.3 prerequisite**) ·
`TICKET_farbling_gate_engine_binding.md` · `TICKET_cdp_port_open_in_release.md` ·
`TICKET_engine_pins_are_branches_not_tags.md` · `TICKET_dependency_freshness_review.md`
