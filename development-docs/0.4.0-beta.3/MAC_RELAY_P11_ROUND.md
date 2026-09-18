# 🍎 Mac relay — beta.3 Phase 11, omnibox cluster (items 2, 3, 4)

**From:** Windows · **Opened:** 2026-09-18 · **Standard:** `HARNESS.md`
**Why this file exists:** root `CLAUDE.md` build rule — *nothing compiles C++ on push*, so every commit
touching `cef-native/**` names its files here and the other platform rebuilds after its next rebase.

---

## ⚠️ Rebuild required — shared C++ touched

| File | Platform split? | Item | What changed |
|---|---|---|---|
| `cef-native/src/handlers/simple_handler.cpp` | ❌ none — pure CEF, no `#ifdef` added | 3 | New `omnibox_navigated` IPC arm, next to the existing `omnibox_hide` arm. Forwards the clicked URL to the **owning window's** `header_browser`. ⛔ Deliberately **not** `SimpleHandler::GetHeaderBrowser()`, which resolves the **primary** window's header — in a second window the URL would land in the wrong address bar |
| `cef-native/src/handlers/simple_render_process_handler.cpp` | ❌ none | 3 | Relays `omnibox_navigated` to the header document as `window.postMessage({type:'omnibox_navigated', url})`, same shape as the existing `omnibox_autocomplete_update` relay |

| `cef-native/src/handlers/simple_app.cpp` | ❌ none — CEF preference API, cross-platform | 9 | 🚨 `--disable-features=Autofill` named a feature that **does not exist** (verified against the Chromium source: `BASE_FEATURE(feature, name, …)` takes the name as its 2nd arg, and none declares `"Autofill"`), while autofill was live and recording form input to `<profile>/Default/Web Data`. Token removed; `DisableChromiumAutofill()` now sets `autofill.profile_enabled` and `autofill.credit_card_enabled` to false in `OnContextInitialized`. ⛔ `GlicActorUi` kept — it is the CEF 150 crash fix |

Both files are shared and carry **no** `#ifdef _WIN32` / `#elif defined(__APPLE__)` in the new code,
so the macOS build needs nothing but a rebuild.

🍎 **macOS should re-run item 9's evidence on its own profile**: the `autofill` table lives at
`~/Library/Application Support/HodosBrowserDev/Default/Default/Web Data`. Type a probe value into any
form, submit, quit the browser, and confirm no row is added. ⚠️ And check whether the **installed**
macOS build has already accumulated rows, as the Windows one has.

## 🍎 What macOS should check when it gets here

⚠️ **The measured cause of item 3 is Windows-specific in one respect, and the fix is not.**

The defect: clicking an omnibox suggestion leaves the header's address `<input>` effectively in edit
mode, so the address bar never shows the URL that was clicked. On Windows that is because the omnibox
overlay's WndProc returns **`MA_NOACTIVATE`** on `WM_MOUSEACTIVATE` (deliberate — the dropdown must
not steal the caret from the address bar). 📏 Measured: the header input blurs for ~18 ms and
re-focuses, so the tab-sync effect gets **exactly one run, with the pre-navigation URL**, and is
blocked from then on.

macOS overlays are borderless `NSWindow`s, not `WS_POPUP`, and click-outside is handled by
`InstallClickOutsideMonitor()` — ⬜ **whether the same focus flicker happens there is unmeasured.**
Either way the fix is the right one on both platforms (it is what Chrome does: the bar shows the
destination before the page arrives), and it is in shared code, so macOS gets it for free.

⬜ **Owed from macOS:** re-run the item-3 row on a Mac with the probe's equivalent —
`development-docs/0.4.0-beta.3/phase-11-ui-leftovers/omniboxprobe.py` is Windows-only (it reads
`IsWindowVisible` on the `CEFOmniboxOverlayWindow` HWND via `user32`), so the HWND-layer half of
item 2 needs a macOS analogue before item 2's row can be called green there.

⚠️ Items 2 and 4 are **React-only** (`MainBrowserView.tsx`, `OmniboxOverlayRoot.tsx`) and relay with
the frontend — no rebuild needed for those, but they ride in the same branch.
