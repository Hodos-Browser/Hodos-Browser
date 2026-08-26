# Browser chrome scales but its window doesn't — header content is clipped

**Found:** 2026-08-25 by the owner, during the Phase 1 (WS1) test session, on the 125% display.
**Reported:** *"the bottom of the header section disappears behind the webview"* — reproduced by
Ctrl+scroll **and independently by the Windows Text size accessibility setting.**

⛔ **Not a Phase 1 defect.** Phase 1 is overlay input and DPI-coordinate correctness. This is header
layout under content scaling. Filed rather than fixed, because the easy-looking version is a
workaround and the correct version touches the most regression-prone layout code in the tree.

---

## Measured

| | |
|---|---|
| Header window height | **96 px** (`HEADER_CSS_HEIGHT`, a hardcoded constant in `LayoutHelpers.h`) |
| Header content at 125% scale | **120 px** |
| **Clipped** | **24 px**, hidden behind the webview |

Two independent ways in, and they are **not** equivalent:

| Route | Who does it | Reversible by the user? |
|---|---|---|
| **Ctrl + mouse wheel** | a user, deliberately | yes — Ctrl+0 |
| **Windows Settings → Accessibility → Text size** | a user with poor eyesight, once | **no** — there is no per-app opt-out, and Ctrl+0 does not touch it |

⭐ **The accessibility route is the serious one.** It is not a user doing something unusual; it is a
supported OS setting, set once, that leaves our header permanently broken.

## Three separate causes, and only one of them is "zoom"

### 1. The existing zoom guard covers the keyboard and not the wheel

`simple_handler.cpp :: OnPreKeyEvent` blocks zoom for non-tab browsers:

```cpp
if (event.type == KEYEVENT_RAWKEYDOWN && role_.find("tab_") != 0) {
    if (key == 0xBB || key == 0xBD || key == '0' || ...) return true;  // don't zoom chrome
}
```

`KEYEVENT_RAWKEYDOWN` only. Ctrl+**wheel** never reaches it, and the header is a *windowed* browser
so CEF hands the wheel straight to Chromium's zoom. A tree-wide grep for wheel-with-Ctrl handling
returns nothing.

⭐ **Third instance in one session of "one path guarded, its sibling not":** the file-dialog guard was
honoured on the WndProc paths and ignored by all 9 mouse hooks; `NotifyScreenInfoChanged` was paired
with `WasResized` for overlays and not for the header. This is becoming the codebase's signature
defect and is worth naming as a review question: *"what is the sibling path, and does it have the
same guard?"*

### 2. Chrome zoom is per-ORIGIN, so all of Hodos's chrome shares one setting

The header **and all 15 overlays** are served from `http://127.0.0.1:5137` on the shared global
request context. Chromium stores zoom per origin, so zooming the header zooms **every overlay too**.
Owner-observed ("the overlays zoom"). Any fix that treats this as "the header zoomed" is
mis-scoped.

### 3. ⭐ The real defect: the header's window height is a constant

`LayoutHelpers.h` — `HEADER_CSS_HEIGHT = 96`, and `GetHeaderHeightPx()` scales it by **monitor DPI
only**. Nothing in that path accounts for page zoom or for the OS text-scale factor. So whenever the
header's content is scaled by anything other than monitor DPI, the window stays 96 CSS px and the
overflow is clipped.

**Blocking zoom does not fix this** — it removes one of the two routes and leaves the accessibility
route broken.

## Why the "obvious" fix is not the fix

`CefCommandHandler::OnChromeCommand` exists and our header **is** Chrome-style (`SetAsChild` with
`runtime_style` DEFAULT — see the comment at `simple_app.cpp` about `IsChromeStyle`), so a veto there
is plausible. ⚠️ **But Ctrl+wheel zoom is not dispatched as a Chrome command** — the keyboard
accelerators are, wheel zoom is handled lower in the input stack. So that API likely catches only the
path we already block. **Unverified: confirming it means building it.**

## Shape of the fix

Two parts, both needed:

1. **Chrome must not zoom.** Chrome and Brave do not zoom their own toolbars. Cover the wheel path,
   not just the keyboard — and remember §2: the target is "all chrome", not "the header".
2. **Chrome must follow the OS text-scale factor.** `GetHeaderHeightPx()` should derive from the
   effective content scale, not a constant. Chromium already reads the setting
   (`ui/display/win/uwp_text_scale_factor.cc`, folded into the device scale factor by
   `screen_win.cc :: GetScaleFactorForDPI`) — windowed browsers get it for free, which is exactly why
   the header grew and the overlays did not.

⚠️ **This is the header-layout path the DPI matrix records as having regressed three times** —
`7277980`, the header-UX pass, then `ff3e2ee`. Any change here is gated on running DPI matrix cells
#4/#6/#9 **plus** a text-scale pass, which the matrix does not currently cover at all.

## Related

- `DPI_RESOLUTION_TEST_MATRIX.md` — needs a **text-scale** dimension; it has none today, and this
  defect is invisible to every existing cell.
- The **unbranded Chromium zoom bubble** appears on Ctrl+scroll with working +/− controls — a Chrome
  UI surface inside Hodos. Belongs with `TICKET_brand_remaining_permission_prompts.md`, alongside the
  unimplemented `OnJSDialog` and `GetAuthCredentials`.
- Overlay sizing contract (Phase 1 item 1) — the same "window size and content size decided
  independently" family, from the other direction.
