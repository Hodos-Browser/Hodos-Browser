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

### ⭐ #3 SOLVED at the desk — one root cause, two symptoms, and the code already wants the right thing

**Clarified symptom (owner, 2026-08-17):** clicking the *pinned* taskbar icon reads **"Hodos Browser"**
correctly. Right-clicking the **running** button reads **"HodosBrowser.exe"**. And launching creates a
**separate taskbar button** instead of grouping with the pinned icon — which should only happen for a
second/third *profile*.

**Both symptoms are one cause: AppUserModelID identity mismatch.** Not the version resource —
`kFileDescription` is already `"Hodos Browser"` and is present in the **shipped** beta.2 binary
(verified by extracting the exe from the released portable zip).

Two halves, both confirmed in the tree:

**(a) Production single-profile never sets an AUMID.** `cef_browser_shell.cpp`:

```cpp
if (!g_picker_mode && (hodos::IsDevEnv() || ProfileManager::GetInstance().GetAllProfiles().size() > 1)) {
    std::wstring aumid = hodos::IsDevEnv() ? L"HodosBrowser.Dev" : L"HodosBrowser";
    if (profileId != "Default") aumid += L"." + pw;
    SetCurrentProcessExplicitAppUserModelID(aumid.c_str());
}
```

For the ordinary user — production, one profile — `IsDevEnv()` is false and `size() > 1` is false, so
**the branch never runs**. The comment says so outright: *"prod keeps its existing
(set-only-when-multi-profile) behavior."* That deliberately-preserved legacy behaviour **is the bug**.

**(b) The shortcuts declare no AUMID at all.** `installer/hodos-browser.iss` `[Icons]`:

```
Name: "{group}\Hodos Browser"; Filename: "{app}\HodosBrowser.exe"
Name: "{autodesktop}\Hodos Browser"; Filename: "{app}\HodosBrowser.exe"; Tasks: desktopicon
```

No `AppUserModelID:` parameter. Windows therefore derives the shortcut's identity from the target
path, while the running process has either no explicit identity or a different derived one —
especially fragile under the **bootstrap model**, where `HodosBrowser.exe` is CEF's `bootstrap.exe`
loading `HodosBrowser.dll` and spawning children.

⇒ Windows cannot match window → shortcut, so it makes a **new button** *(symptom b)*, and the
unmatched button has no registered display name, so the jump list falls back to the **exe filename**
*(symptom a)*.

**Fix shape** — the per-profile suffix logic is already written and correct; only the gate and the
shortcut side are wrong:

1. Set the explicit AUMID **always in production**, not only when multi-profile.
2. Add a matching `AppUserModelID:` to **both** Inno `[Icons]` entries. The two must be byte-identical
   or the mismatch persists.
3. Keep the per-profile suffix — that is exactly the owner's expected behaviour (one button normally,
   extra buttons only for additional profiles), and it is how Chrome behaves.

⚠️ **Verification must include the pinned case.** An existing pinned shortcut carries the *old*
identity, so testing only a fresh install would hide a regression for current users. Test:
fresh install, upgrade-over-existing, **and** a shortcut pinned before the change.

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

### WS1 — Overlay input & DPI correctness · items 1, 2, 7 · **Phase 1** (was FIRST; see §4)

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

### WS1b — Logging & synchronous-I/O practices review · **SPLIT: (a) is Phase 0, (b) is Phase 2**

Opened 2026-08-17 after the owner's installed **beta.1** went unresponsive — balances, the advanced
wallet, local DB reads **and ordinary web pages** all stalled together, recovering only after a
second restart. Two shipping defects were found while investigating; the incident's own cause is
**not** established.

- 🎫 **`TICKET_production_debug_logging_unbounded.md`** — `Logger` has **no level gate and no
  rotation**, so production writes DEBUG forever: **1.58 GB** on the owner's machine since 2026-07-06,
  containing **every URL visited, in plaintext**, surviving the user clearing their own history. On a
  privacy browser.
- 🎫 **`TICKET_stray_log_in_install_root.md`** — **44** raw `ofstream("debug_output.log")` writes
  across 5 shipped files use a **relative path**, landing a log **inside `{app}`** — the one place
  `cef-native/CLAUDE.md` forbids, because it already broke the silent-update backup hash once.
  Confirmed live: 9,865 bytes in `%LOCALAPPDATA%\HodosBrowser\` from the fresh **beta.2** install.
  `WalletService.cpp` is byte-identical between beta.1 and beta.2, so **both ship it**.

**The wider review this earns** (the owner's framing): a general pass over logging and **synchronous
I/O on the browser UI thread**. `getBalance` alone does 4 open/write/close cycles per call and a
synchronous wallet HTTP request; the incident window logged **417 balance calls in 52 minutes** with
uniform **2.0 s** gaps that look like a timeout rather than contention.

⛔ **The incident cause remains OPEN and must be reproduced, not inferred.** Established: the price
was fine throughout (`bsvPrice: 15.145`; the fallback chain is intact), and the failure was
specifically `/wallet/balance` returning no balance from 15:18:02 onward. **Unexplained:** why
*web pages* stalled too. One experiment settles it — stub `/wallet/balance` to hang and see whether
page loads stall with it.

### WS2 — Window / instance / focus identity · items 5, 3

`WindowManager` + Win32 shell integration. **Windows-only** — macOS Spaces is a different mechanism
and #3 has no macOS analogue. #5 needs a genuine deep dive; the reported inconsistency across
same-desktop instances suggests focus resolution keyed on a global rather than the active window.

### WS3 — Tab & peripheral parity · item 6

Tab context-menu parity (mute tab/site, new tab to right, duplicate, close others, reload, pin,
bookmark) plus mic/camera. Menu work is cross-platform and uses the existing custom
`MENU_ID_USER_FIRST` context-menu machinery. Mic/camera splits: Windows is reported working; **macOS
is unverified and suspected** (see §2).

### WS5 — Loopback routing & trust boundary · **SPLIT: (a) is Phase 0.5, (b) is Phase 5**

Filed 2026-08-18 as `TICKET_loopback_host_form_wallet_routing.md`, opened by an external interop
failure against the BRC App Lab. Verified against the tree the same day — verification log in that
ticket's §13. **The ticket splits, and the halves belong in different places.**

⭐ **The load-bearing insight, and it is counter-intuitive:** the C++ interception layer is not only a
permission gate — it is the component that *marks traffic as untrusted*. `domain_trust_mw`
(`rust-wallet/src/main.rs:65-75`, verified verbatim) treats a **missing** `X-Requesting-Domain` as an
internal, fully-trusted call. So a gate that fails to match does not leave traffic *ungated*; it
leaves it *trusted*. Every narrowing of a matcher in this area is a privilege change, and the
ticket's §5.1 is right that the structural fix done naively would be a **regression**.

- **(a) — Phase 0.5.** Three small, independent defects on or beside the money path, none depending
  on the routing rewrite: `send_transaction` takes no request context (ticket §7.1); the CORS
  backstop does not block on origin mismatch (§6.2); and three `:5137` substring gates admit any URL
  merely *containing* the string (§7.3, escalated — see below). All Rust/C++, all local, no CI, no
  Mac dependency for the fix itself.
- **(b) — Phase 5.** The compatibility fix the ticket was opened for — W0 + W1 + W2' + W3: a parsed
  `IsWalletOrigin()` predicate replacing the six-term substring gate, `/health` added to
  `isWalletEndpoint`, and instrumentation. Also closes a live **cross-wallet routing hole**: verified
  by `netstat` on this machine, MetaNet Client is `LISTENING` on `127.0.0.1:3321` **and** `:2121`
  (PID 37360), so an App Lab request inside Hodos today falls past our gate and is answered by a
  different vendor's wallet — different identity key, no Hodos gate, no indication to the user.

⛔ **W4 / W6 / W7 / W8 are beta.4**, driven by W3's instrumentation. Do not take the whole plan into
beta.3 — it rewrites the routing predicate for every network request in the browser.

### WS6 — QR scanner rejects `bsv:` payment URIs · **Phase 0.6**

Filed 2026-08-18 from an owner report against a live payment page:
`TICKET_qr_bsv_uri_scheme_rejected.md`. **Root-caused from the owner's own production log — the
decoded payload was already in it.**

The scanner is not broken. quirc read the code perfectly on the first attempt, three times:

```
QR payload: bsv:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?amount=0.11828417&label=PaiyBit%20media
```

Our classifier then discarded it, because it tests `^bitcoin:` and this is `bsv:`. The address,
amount and label are all shapes we already accept — only the four characters of the scheme were
rejected. The DOM path failed identically, from the same rule spelled a second time.

⭐ **Verdict on the owner's question: fix ours, do not ask PaiyBit to change.** There is no BRC
mandating `bitcoin:` for BSV, `bsv:` is arguably the less ambiguous scheme on this chain, and we
already declined to switch our *own* receive QRs to `bitcoin:` for compatibility reasons
(`QR_SCAN_OVERVIEW.md` §Phase 3). Demanding conformity to an unwritten preference is the wrong move.

⛔ **The rule is spelled four times and two of them hardcode `slice(8)`** — the byte length of
`"bitcoin:"`. Widening only the regex there truncates the address, fails closed, and looks exactly
like today's symptom while appearing fixed. Split on the first `:`, as the two C++ sites already do.

⚠️ **A second, separate money-path question rides along:** BIP21 `amount` is in whole coins
(`0.11828417` = BSV, not satoshis). Confirm what `TransactionForm` expects before calling this done,
or the first successful scan pre-fills a send eight decimal places wrong.

### WS4 — Chrome import · item 4

Standalone, research-heavy, security-sensitive. Scope against §2's wall **before** design.
⚠️ Chrome on macOS uses **Keychain**, not DPAPI — assume no symmetry; Mac researches its own half.

## 4. Order

**WS1b(a) → WS5(a) → WS6 → WS1 → WS1b(b) → WS2 → WS2(cont.) → WS3 → WS5(b) → WS4.**
*(phases 0 → 0.5 → 0.6 → 1 → 2 → 3 → **3.5** → 4 → 5 → 6 → **7 → 8 → 9 → 10**;
3.5 added 2026-08-30; 7–10 are the ticket consolidation added 2026-08-31 — see §4.1)*
**✅ 0 · 0.5 · 0.6 · 1 · 2 · 3 · 3.5 complete** (3.5 landed `4f87f78`, 2026-08-31 — ⬜ regression set still owed at the 3.5 → 4 boundary). **⬜ 4 · 5 · 6 · 7 · 8 · 9 · 10.**

⭐ **Changed 2026-08-18 (second revision), after `TICKET_loopback_host_form_wallet_routing.md` was
filed and verified.** WS1b splits and its first half stays at the front; WS5 splits and its first
half slots in behind it.

| Phase | What | Why here |
|---|---|---|
| **0 — WS1b(a)** | **Delete the 52 stray `{app}` log writes + A1/A2/A3** | 🚨 The mnemonic is written in plaintext into the install root (see the ticket's §0). Cheap — it is deleting debug scaffolding — and it is the one phase that also removes a key-material disclosure. ⚠️ Its *original* "silently aborts auto-update" rationale did **not** survive verification; corrected in place in the ticket, not deleted. |
| **0.5 — WS5(a)** | Money path + trust boundary: `send_transaction` request context, `block_on_origin_mismatch`, the three `:5137` substring gates | 🚨 All three are **live, verified, and shipping**. `send_transaction` honours `sendMax` with no per-call approval and no payment cap. Small, Rust/C++, local, no dependency on the routing rewrite — so there is no reason for it to wait behind a multi-day workstream. |
| **0.6 — WS6** | QR scanner: accept `bsv:` payment URIs | 🚨 A real BSV payment QR on a live site cannot be scanned. Root cause is four characters of a regex, already fully evidenced from the owner's production log — no investigation left. Smallest fix in the sprint, and it unblocks an actual payment. |
| **1 — WS1** | Overlay input & DPI | Money-path correctness — cursor offset in the wallet overlay during a send. |
| **2 — WS1b(b)** | Logger level gate, rotation, retention, sync-I/O review | The 1.58 GB plaintext-history problem. Serious but **not** self-blocking. |
| **3 — WS2** | Window / instance / focus identity | ⛔ **REVISED 2026-08-30 after the kickoff — the row below is the original and did not survive verification.** ~~#3 is solved at the desk (~1 day). #5 is an unbounded deep dive — take #3, defer #5.~~ **#3 was NOT solved at the desk**: half (a) is refuted as the cause (the owner has 2 profiles, so the branch already runs — 61 log lines prove it), and the real cause is that our identity changed under the user on 2026-07-06 and matches no shortcut. The fix is **three** parts, not two — gate + shortcuts + AUMID display-name registration — and it must ship as one change or it orphans pinned icons. **#5 is neither "unbounded" nor "two call sites"**: the mechanism is confirmed, the owner observed a **second** symptom (video fullscreen crossing windows), and the real shape is a half-finished multi-window migration. See `phase-3-window-identity/`. |
| **3.5 — WS2 (cont.)** | Layout is window-scoped | ⭐ **ADDED 2026-08-30 by owner decision.** The densest cluster of #5's defect — `ShellWindowProc`'s layout arms + 12 overlay `ScalePx` sites — where one coherent change and one test story cover many sites. ⛔ Explicitly **not** `SaveSession`/`ShutdownApplication`, which are correct as global. The scattered remainder goes to beta.4, held by the `P3-G11` ratchet. See `phase-3.5-layout-window-scoping/`. |
| **4 — WS3** | Tab & peripheral parity | Menu work is cross-platform; mic/camera is Mac-blocked on B1. |
| **5 — WS5(b)** | Loopback compatibility: W0 + W1 + W2' + W3 | Fixes the user-visible interop bug **and** closes the cross-wallet routing hole (MetaNet Client answering for us — verified live). Placed after the reported-defect work because it rewrites a predicate every request passes through. |
| **6 — WS4** | Chrome import | Last: most able to balloon, and its value is capped by a constraint we do not control. **First candidate to cut.** |
| **7 — consent surface** | The permission/consent UX tickets | ⭐ **ADDED 2026-08-31** — see §4.1 |
| **8 — money-path correctness** | Asset-safety tickets | ⭐ ADDED 2026-08-31 — see §4.1 |
| **9 — release readiness** | Promotion blockers + DevOps hygiene | ⭐ ADDED 2026-08-31 — see §4.1 |
| **10 — UI/layout leftovers** | The remaining visual defects | ⭐ ADDED 2026-08-31 — see §4.1 |

### 4.1 Phase count, how to add one, and the ticket bundles ⭐ ADDED 2026-08-31

**How many phases.** **Ten slots, seven done.** `0 · 0.5 · 0.6 · 1 · 2 · 3 · 3.5` are ✅ complete;
`4 · 5 · 6` were always planned;
`7 · 8 · 9 · 10` are the ticket consolidation added here.

**How to add a phase — three steps, no ceremony:**

1. Add a row to the order table above and to the arrow line under §4.
2. Create `phase-<n>-<slug>/PHASE_CONTRACT.md` from `PHASE_CONTRACT_TEMPLATE.md` — the seven
   sections in `HARNESS.md` §1, none optional.
3. Write a `SESSION_PROMPT_beta3_phase<n>_<slug>.md` so the phase can start in a fresh context.
   (`SCOPING_PROCESS.md`: each stage reads the previous stage's **output file**, not its reasoning.)

⭐ **The policy, in the owner's words (2026-08-31):** *"work through our current phases and then
consolidate these tickets at the end."* We add items faster than we close them; that is accepted and
is the owner's to manage. ⛔ So phases 7–10 are a **holding pattern, not a queue anyone pulls from** —
and per `0.4.0-beta.4/tickets/README.md`, a ticket is not work until the owner assigns it.

#### The bundles

| Phase | Tickets | Logic |
|---|---|---|
| **7 — consent surface** | `consent_surface_fetches_third_party_favicon` · `quiet_mode_wider_than_manifest` · `brand_remaining_permission_prompts` (21) · `site_permission_dual_store` · ❔`connect_modal_two_views_drift` · ❔`manifest_description_can_misdescribe_protocol` | All one surface: what the user is shown when deciding to trust a site. Shared test setup, and `feedback_consent_surface_needs_human_eyes` applies to every row |
| **8 — money-path correctness** | `token_outputs_destroyed_by_dust_paths` 🚨 · `placeholder_resolution_failure_broadcasts_anyway` · `bridge_single_slot_callbacks_race` (remainder) | Asset safety. ⛔ All three need the same care as Phase 0.5 and none may be done casually |
| **9 — release readiness** | `appcast_missing_minimum_system_version` 🚦 · `farbling_gate_engine_binding` · `engine_pins_are_branches_not_tags` · `dependency_freshness_review` · `cdp_port_open_in_release` · `stray_log_in_install_root` (owed T2/T3) | Everything that gates **promotion** rather than behaviour. Mostly cheap; several are one sitting together |
| **10 — UI/layout leftovers** | `modal_buttons_unclickable_small_screen` · ❔`chrome_ui_scales_but_its_window_does_not` · ❔`longlived_surfaces_snapshot_state_at_startup` · ❔`disable_features_autofill_is_a_noop` · Phase 1's overlay dead strip · the DPI matrix overlay section | The visual residue. ⚠️ Two of these may collapse into Phase 3.5 instead — see below |

#### ⛔ Four that should NOT wait for the end

Asked for explicitly. These have a reason to move, and the reason is not "it feels important":

| Ticket | Where it belongs instead | Why |
|---|---|---|
| 🚨 `token_outputs_destroyed_by_dust_paths` | **Before or alongside Phase 4** | Its **path 1 is an automatic daily task**. It needs no user action to permanently destroy a 1-sat asset. Every day it waits is a day the task runs. The owner already pulled it into beta.3 for this reason; leaving it in Phase 8 quietly undoes that |
| 🚦 `appcast_missing_minimum_system_version` | Phase 9, but **must close before promotion**, not before the sprint ends | It is a **promotion blocker**. If 0.4.0 ships without it, the sprint's output cannot be released — the phase order is irrelevant to that constraint |
| `stray_log_in_install_root` (owed T2/T3) | ⇒ **`INSTALL_TEST_BATCH.md` row I5** | Both need a **real install**, and Phase 3 *changed the installer*. Superseded by the batching decision below — it now runs once, with every other install-dependent row |
| `modal_buttons_unclickable_small_screen` + ❔`chrome_ui_scales_but_its_window_does_not` | **Phase 3.5**, not Phase 10 | Same subsystem (layout/DPI) and the *same T3 setup* — two windows, two monitors, the mixed-DPI matrix cell. `P3.5-A3` already requires that rig. Doing them in Phase 10 means building it twice |

⭐ **The pattern worth noticing:** three of the four move for the same reason — **they share a test
rig with work already scheduled.** The expensive part of this sprint is not the code, it is standing
up a human at two monitors with a real install. Group by rig, not by topic.

#### 🗂️ Install-dependent rows are BATCHED — decided 2026-08-31

> *"Lets just wait on the install tests, I know that is not best practices but we can keep track of it
> and do everything that requires install tests at the end to save time."* — owner

⇒ **`INSTALL_TEST_BATCH.md`** is the register. Build → install → test → uninstall is the sprint's most
expensive loop and several phases each owe one or two rows of it; batching turns N installs into one.

⛔ **This is only safe because the register exists.** The failure mode is silent loss — a row deferred
with no home never runs. Rules: every deferral is written down **with the phase that owes it**, keeps
its negative control, and is **OWED, never waived**. A phase does not get to look finished by moving a
row there; its sign-off still cites the row and its state stays visibly owed.

⚠️ **The batch cannot slip past the RC.** `P3-A6` and R-UPDATE are about *upgrading users who already
have Hodos installed* — the one population that cannot be re-tested after shipping.

⇒ Affects `P3-A4`, `P3-A6`, `P3-A7`/R-UPDATE, the `{app}`-cleanliness assertion,
`stray_log_in_install_root`'s T2/T3, `appcast_missing_minimum_system_version`, and the uninstall sweep.

#### 🙋 One thing to confirm

⚠️ **Phase 3.5 is contracted, inventoried, and not started**, and the owner has said they will kick
off **Phase 4** next. That is a legitimate priority call — Phase 4 is user-facing parity work and 3.5
is internal correctness — but it should be a **decision, not a drift**. Either is fine; 3.5's contract
and site inventory keep indefinitely. ⛔ What must not happen is 3.5 being *assumed* done because
Phase 3 closed.

**Why 0 and 0.5 lead.** Phase 0 removes a secret from disk; Phase 0.5 restores the approval gate to a
fund-moving endpoint that currently has none. Both are small, both are independent of everything
else, and neither is reversible-by-accident later.

⛔ **WS5's W4 / W6 / W7 / W8 are explicitly beta.4**, driven by W3's instrumentation. The routing
rewrite is a two-release plan and must not be pulled forward whole.

⚠️ **This ordering was recommended by the assistant and confirmed by the owner on 2026-08-18**, then
extended the same day when WS5 arrived. It is reversible — 0 is a deletion, 0.5 is three small
additive gates.

## 5. Mac tasking

⛔ **Do not hold all Mac work to the end.** Three items are **inputs to our design, not outputs** —
deferring them buys rework.

| When | Mac item | Why it cannot wait |
|---|---|---|
| **Now** | **Sparkle 2.9.6 verification** | Bumped, ships on macOS, never run. Blocks promotion. Already relayed. |
| **Now** | **Big Sur / `minimumSystemVersion` call** | Product decision with a macOS-shaped answer. |
| **Now** | **Mic/camera diagnosis (#6)** | If it is the helper-plist/TCC theory, it changes **what we build**, not just what we test. |
| **Now** | **Do WS1's symptoms reproduce on macOS?** (items 1, 7) | macOS uses borderless `NSWindow` + `InstallClickOutsideMonitor`, **no `WH_MOUSE_LL`**. Designing a Windows-shaped fix first risks a structurally wrong answer for Mac. One cheap question de-risks the workstream. |
| **Now** | **WS5: is the cross-wallet routing hole live on macOS?** | Does MetaNet Client (or any wallet) listen on `127.0.0.1:3321` / `:2121` on the Mac? On Windows it does — verified — which means a dApp inside Hodos is being answered by another vendor's wallet. Changes how loudly we treat WS5(b). |
| **Now** | **WS5: does a resource handler take over `https://` loopback pre-TLS on macOS?** | Ticket §8.1 / §11 open question. If TLS validation fires first, W1 must not match `:2121` and the design changes on both platforms. Cheap to observe once, expensive to discover late. |
| Later | WS5(b) W7 overlay coverage | wallet / wallet_panel / settings / backup overlays must still reach Rust before and after the predicate swap. Ticket §8.6 — the plan had no macOS acceptance criteria at all. |
| Later | Tab context menu port | Build once on Windows, then port. |
| Later | Chrome-import macOS half (Keychain) | Parallel research once WS4 starts. |
| **Never** | Items 3, 5 | Windows-only by nature. |

⭐ **WS5(a) needs nothing from Mac.** `send_transaction` and the CORS backstop are Rust — one binary,
both platforms. The three `:5137` gates are in cross-platform C++ and are equally wrong on macOS, so
the fix lands once. Only WS5(b) has a macOS-shaped unknown.

## 6. Decisions owed

1. **WS4 scope** — "bookmarks + history + passwords-via-CSV, same machine, one button", or a full
   research pass first?
2. **Chrome-import UX** — auto-detect the local profile, folder-picker, or offer at first-run? (And
   whether importing live sessions into a wallet browser is acceptable at all.)
3. **Cut line** — with WS5 added, does WS4 slip out of beta.3 entirely?
4. Big Sur users: nothing, a pinned final 0.3.x, or an in-app message?
5. ⭐ **NEW — disclosure posture.** Phase 0 and Phase 0.5 are both *shipping* defects with a security
   character: the recovery phrase written to disk, and a fund-moving endpoint with no approval gate.
   Does anything need saying to existing users, or is fixing them in beta.3 sufficient? Not an
   engineering call. *(Depends on Phase 0's reproduction result — see the ticket's §0.)*

## 7. Also in this sprint, already filed

`TICKET_appcast_missing_minimum_system_version.md` (**now a beta.3 prerequisite**) ·
`TICKET_loopback_host_form_wallet_routing.md` (**now WS5 — split across Phase 0.5 and Phase 5**) ·
`TICKET_farbling_gate_engine_binding.md` · `TICKET_cdp_port_open_in_release.md` ·
`TICKET_engine_pins_are_branches_not_tags.md` · `TICKET_dependency_freshness_review.md`
