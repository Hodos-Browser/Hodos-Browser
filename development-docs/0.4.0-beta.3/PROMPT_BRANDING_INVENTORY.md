# Every prompt Chromium can show a Hodos user — inventory and branding status

**Built 2026-08-24** from source, not memory. Phase 0.9 branded exactly one surface (the loopback
permission); this is the map of everything else, so the remaining work can be scoped rather than
discovered one bug report at a time.

**Sources of truth used here**
- `cef-binaries/include/internal/cef_types.h :: cef_permission_request_types_t` — every permission Chromium can raise
- `cef-native/include/handlers/simple_handler.h` — which CEF hooks we actually override
- `chromium/src/chrome/browser/ui/page_action/page_action_icon_type.h` — Chromium's own list of omnibox-anchored bubbles
- `libcef/` grep — proof of which surfaces CEF exposes no hook for at all

**Legend:** ✅ Hodos-branded · ⬜ stock Chrome, hook available · ⛔ stock Chrome, **no CEF hook**
(patch-only) · 🖥️ OS dialog, not ours to brand

---

## Group A — Permission prompts (`CefPermissionHandler::OnShowPermissionPrompt`)

Every one of these is interceptable **today** with no patching. We already own the mechanism:
`PendingPermissionManager` parks the callback, the notification overlay shows a Hodos prompt, and
`permission_response` resolves it. Adding a type is a stable id + a mapping line + prompt copy.

| Bit | Permission | Status | Notes |
|---|---|---|---|
| 2 / 1 | Camera (+ pan-tilt-zoom) | ✅ | PTZ folds into Camera |
| 12 | Microphone | ✅ | |
| 8 | Geolocation | ✅ | |
| 15 | Notifications | ✅ | |
| 4 | Clipboard | ✅ | |
| 26 | Local network | ✅ | Phase 0.9 |
| 27 | Loopback network | ✅ | Phase 0.9 |
| 0 | AR session | ⬜ | WebXR |
| 21 | VR session | ⬜ | WebXR |
| 9 | Hand tracking | ⬜ | WebXR |
| 3 | Captured surface control | ⬜ | scroll/zoom a shared tab |
| 5 | Top-level storage access | ⬜ | |
| 20 | Storage access | ⬜ | |
| 6 | Disk quota | ⬜ | |
| 7 | Local fonts | ⬜ | **fingerprinting-relevant** |
| 10 | Identity provider | ⬜ | FedCM |
| 11 | Idle detection | ⬜ | **privacy-relevant** |
| 13 | MIDI sysex | ⬜ | |
| 14 | Multiple downloads | ⬜ | |
| 16 | Keyboard lock | ⬜ | |
| 17 | Pointer lock | ⬜ | |
| 18 | Protected media identifier | ⬜ | DRM; **device-identifying** |
| 19 | Register protocol handler | ⬜ | |
| 22 | Web app installation | ⬜ | |
| 23 | Window management | ⬜ | **screen-layout fingerprinting** |
| 24 | File system access | ⬜ | **high risk — read/write real files** |
| 28 | Sensors | ⬜ | **fingerprinting-relevant** |
| 25 | Local network access (deprecated) | n/a | Does not exist in our build and libcef never emits it — see contract §2 |

**7 of 28 branded.** The rest render as stock Chrome bubbles.

⭐ If we brand more, the ones that matter for a privacy browser are **File system access**, **Window
management**, **Sensors**, **Local fonts**, **Idle detection** and **Protected media identifier** —
each is either a real capability grant or a fingerprinting vector, and each currently looks like
generic Chrome.

---

## Group B — Chrome-UI bubbles. **No CEF hook exists.**

`grep` over `libcef/` and `libcef_dll/` for `password_manager|ManagePasswords|PasswordBubble` returns
**zero hits**. There is no client callback for any of these. They are Chrome's own UI, drawn by
Chrome's browser-window layer, which our chrome-style CEF browsers inherit.

From Chromium's `PageActionIconType` enum — the bubbles anchored in the location bar:

| Bubble | Status | Note |
|---|---|---|
| **Save / update password** (`kManagePasswords`) | ⛔ | The one the owner specifically asked about |
| Save address (`kAutofillAddress`) | ⛔ | |
| Save card / virtual card enrol / save IBAN | ⛔ | `kSaveCard`, `kVirtualCardEnroll`, `kSaveIban` |
| Mandatory reauth (`kMandatoryReauth`) | ⛔ | |
| Translate (`kTranslate`) | ⛔ | |
| PWA install (`kPwaInstall`) | ⛔ | |
| Intent picker (`kIntentPicker`) | ⛔ | "Open in app?" |
| File system access (`kFileSystemAccess`) | ⛔ | the *status* bubble, distinct from the Group A permission |
| Cookie controls (`kCookieControls`) | ⛔ | we have our own cookie panel; this is Chrome's |
| Zoom (`kZoom`) | ⛔ | |
| Sharing hub, price insights, discounts, Lens, AI mode, reading mode | ⛔ | mostly Google-service UI |

### What it would take

Three options, in ascending cost:

1. **Suppress** — turn the feature off so the bubble never appears. Zero patch.
   For passwords: `CefRequestContext::SetPreference("credentials_enable_service", false)`.
   `pref_helper.cc :: CanSetPreference` has no allowlist, so this should work. ⚠️ **UNMEASURED.**
2. **Replace with our own feature** — e.g. a Hodos password manager. That is not branding; it is
   encrypted storage + form detection + fill. An order of magnitude larger than this phase.
3. **Patch Chromium to expose a hook** — a new CEF client interface (public header + libcef impl +
   `libcef_dll` cpptoc/ctocpp regeneration + `cef_api_versions.json` rehash) plus a Chromium hunk
   diverting e.g. `ManagePasswordsUIController::UpdateBubbleAndIconVisibility`. Roughly 300–500 LOC
   across two repos, **re-verified on every Chromium bump**, and it yields only a dialog.

✅ **ANSWERED 2026-08-24 by the owner: the save-password bubble DOES appear in Hodos today.**
Owner-observed over normal use, not instrumented. Worth one confirming look to be sure it is Chrome's
`ManagePasswords` bubble rather than another surface, but **treat Group B as live, not theoretical.**

This matches the mechanism: chrome-style CEF puts the WebContents into a real `Browser` tab strip
(`chrome_browser_host_impl.cc:129`), which attaches `ManagePasswordsUIController`. And it kills the
assumption that we had already disabled this — see
`TICKET_disable_features_autofill_is_a_noop.md`: the `--disable-features=Autofill` switch names a
feature that does not exist, so it disables nothing.

---

## Group C — Other prompts CEF *does* expose. Cheap wins live here.

| Surface | CEF hook | Status |
|---|---|---|
| **JavaScript `alert()` / `confirm()` / `prompt()`** | `CefJSDialogHandler::OnJSDialog` | ⬜ **NOT OVERRIDDEN.** We inherit the interface and return ourselves from `GetJSDialogHandler()`, but only `OnBeforeUnloadDialog` is implemented — so every `alert()` on every site draws Chrome's stock dialog. Probably **the most-seen unbranded surface in the browser.** |
| **HTTP Basic / proxy auth** | `CefRequestHandler::GetAuthCredentials` | ⬜ **NOT IMPLEMENTED on SimpleHandler.** (The only `GetAuthCredentials` in the tree belongs to a throwaway `CertFieldResponseHandler`.) Stock Chrome credential dialog, on a **password-entry** surface. |
| **Client certificate picker** | `OnSelectClientCertificate` | ⬜ Not implemented. Rare. |
| `beforeunload` "Leave site?" | `OnBeforeUnloadDialog` | ✅ Suppressed by design (trap prevention) |
| Certificate error | `OnCertificateError` | ✅ `CertErrorPage.tsx` |
| Downloads | `CefDownloadHandler` | ✅ `DownloadsOverlayRoot.tsx` |
| Context menu | `CefContextMenuHandler` | ✅ custom menu ids |
| Find in page | `CefFindHandler` | ✅ `FindBar.tsx` |
| Wallet / payment / cert-disclosure consent | our own | ✅ `BRC100AuthOverlayRoot.tsx` |

⭐ **`OnJSDialog` and `GetAuthCredentials` are the two highest-value items in this whole document.**
Both are pure branding, both use machinery we already have (park a callback, show the notification
overlay, resolve on response — exactly the Group A pattern), and one of them is a password field.

---

## Group D — OS dialogs. Not ours.

| Surface | Status |
|---|---|
| File open/save picker | 🖥️ Native. `OnFileDialog` returns false deliberately; it only sets `g_file_dialog_active` so overlays survive the focus steal. Branding would mean building our own file browser — no. |
| Print dialog | 🖥️ Native |
| OS credential / biometric prompts | 🖥️ Native |

---

## Suggested order, if we do a branding phase

1. **`OnJSDialog`** — highest visibility, zero risk, reuses the Group A pattern.
2. **`GetAuthCredentials`** — a stock Chrome password box inside a Hodos window is the worst
   look/trust mismatch on the list.
3. **Measure Group B** — 5 minutes: does the save-password bubble actually appear? Then decide
   suppress vs. build vs. patch, with evidence instead of assumption.
4. **Group A privacy set** — File system access, Window management, Sensors, Local fonts, Idle
   detection, Protected media identifier. Mechanical: one stable id + one mapping line + copy each.
5. Everything else in Group A, as they come up.

⛔ Not before all of the above: Group B interception by patching Chromium. It is the largest item on
the page and buys the least per unit of maintenance.

---

## Standing rules from Phase 0.9 that apply to any new prompt

- **No "Allow this time"** on `OnShowPermissionPrompt`-path types — CEF has no grant-once, so the
  button would lie. Media-path types (camera/mic) may keep it.
- **Prompt denials are temporary** (`DISMISS`, nothing stored); **overt Site-controls actions
  persist**. Owner standard, 2026-08-24.
- **Browser logo, not wallet logo** — these are browser-level asks about sites and the device.
- **New types get new stable `SitePermissionType` integers**, never a CEF bit value.
- **Anything added to `kSitePermCaps` must write through to Chromium's content setting**, or the
  Site controls toggle silently does nothing (`TICKET_site_permission_dual_store.md`).
