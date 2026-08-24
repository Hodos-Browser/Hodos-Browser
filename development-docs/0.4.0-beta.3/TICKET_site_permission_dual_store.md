# Site permission decisions are written to two stores, and the wrong one governs

**Opened 2026-08-24**, found while wiring the loopback prompt in Phase 0.9.
**Status:** 🔵 OPEN. Affects **location, notifications, clipboard** — shipped today.
Camera/mic are **not** affected. Loopback/local-network were fixed in 0.9.

## The defect

Answering a Hodos permission prompt writes **both** stores:

- our `site_permissions.db` (`SitePermissionStore`), and
- **Chromium's own content setting**, written as a side effect of
  `CefPermissionPromptCallback::Continue(ACCEPT|DENY)`.

Chromium then consults **its** setting before it ever calls `OnShowPermissionPrompt` again. So once
a decision exists, our row is never read.

The Site controls panel (`SiteInfoOverlayRoot.tsx` → `site_permissions_set` /
`site_permissions_reset`) writes **only our SQLite**. Therefore:

- Setting a site to **Block** in the panel changes a row nobody consults. The panel shows "Block"
  and the site keeps working.
- **Reset** is likewise inert.

That is worse than a missing control: it is a control that reports success and does nothing, on a
surface CLAUDE.md names as a load-bearing UX safeguard ("right-click → Manage Site Permissions").

## Evidence

Paired rows in the dev profile, same decision in both stores:

| Store | Row |
|---|---|
| Chromium content settings | `notifications  https://www.youtube.com  BLOCK  2026-08-10 15:38:14` |
| Hodos `site_permissions.db` | `www.youtube.com  Notifications  Block` |

And the asymmetry that proves the mechanism — a media-path decision writes **only** our store:

| Store | Row |
|---|---|
| Chromium content settings | *(no `media_stream_mic` entry for x.com)* |
| Hodos `site_permissions.db` | `x.com  Microphone  Allow` |

Camera/mic ride `OnRequestMediaAccessPermission`, whose `Continue()` persists nothing — so our
store really is authoritative there, and those toggles work.

**Directly measured:** with a stored Chromium BLOCK on `example.com`, a loopback request produced
**0** `OnShowPermissionPrompt` calls. Chromium never asked us. That is the whole defect in one line.

## The fix

Phase 0.9 already did it for the two network types — copy that:

`simple_handler.cpp :: MirrorNetworkPermissionToChromium` uses
`CefRequestContext::SetContentSetting(originUrl, originUrl, <type>, ALLOW|BLOCK|DEFAULT)`.
`CEF_CONTENT_SETTING_TYPE_*` exists for geolocation, notifications and clipboard.
`SitePermissionState::Ask` maps to `CEF_CONTENT_SETTING_VALUE_DEFAULT` (Chromium deletes the
exception rather than storing ASK).

Extend the mirror to Location / Notifications / Clipboard in `site_permissions_set` and
`site_permissions_reset`. Leave camera/mic alone.

⚠️ Note the header's own warning: incorrect use of `SetContentSetting` can destabilise Chromium.
Scope it to the types listed here.

## Negative control

Set a site's Notifications to Block in the panel, then have that site call
`Notification.requestPermission()`. Before the fix it is still allowed and no prompt appears —
**that** is the red. After the fix the request is blocked. Assert the site's actual behaviour, not
what the panel displays; the panel displaying "Block" is precisely the lie under test.

## Related

- `development-docs/0.4.0-beta.3/TICKET_prompt_denials_should_not_persist.md` — same three types.
- `phase-0.9-chromium-prompt-branding/PHASE_CONTRACT.md` §7.3 for the precedent.
