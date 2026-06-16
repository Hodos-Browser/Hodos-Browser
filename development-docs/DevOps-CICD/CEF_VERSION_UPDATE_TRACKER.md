# CEF Version Update Tracker

Track features, fixes, and investigations to research when updating the CEF build.

**Current CEF version:** 136 (built from source with `proprietary_codecs=true ffmpeg_branding=Chrome`)

---

## Must Investigate on Next CEF Update

### FedCM (Federated Credential Management) Support
- **Priority:** HIGH
- **Why:** Google made FedCM mandatory for "Sign in with Google" as of August 2025. CEF 136 does not implement the browser-level UI (account chooser dialog) that FedCM requires. This breaks "Sign in with Google" on any site that migrated to FedCM-only (no popup/redirect fallback).
- **What to check:**
  - Does the new CEF version include `CefPermissionHandler` methods for FedCM?
  - Is there a `navigator.credentials.get({identity: ...})` handler we can implement?
  - Check Chromium commit history for FedCM-related CEF changes
  - Test: Go to any site with "Sign in with Google" — does the account chooser appear?
- **Workaround (current):** Sites that still support OAuth popup/redirect fallbacks work. Sites that went FedCM-only do not show the Google sign-in button at all.
- **References:**
  - https://developer.chrome.com/docs/identity/fedcm/overview
  - https://developers.google.com/identity/gsi/web/guides/fedcm-migration
  - CEF issue tracker: search "FedCM" or "Federated Credential Management"
- **Added:** 2026-05-01

### Permissions API Updates
- **Priority:** MEDIUM
- **Why:** CEF 136 handles some permissions natively via Chrome bootstrap. Newer CEF versions may add `CefPermissionHandler` methods for notifications, geolocation, camera/mic that we should implement.
- **What to check:**
  - New `CefPermissionHandler` methods
  - Permission persistence APIs
  - Test: Check if notification permissions, camera access work
- **Added:** 2026-05-01

### CefResponseFilter Stability
- **Priority:** LOW
- **Why:** We use `CefResponseFilter` for YouTube ad-key stripping (`AdblockResponseFilter`). This API has had stability issues in some CEF versions.
- **What to check:**
  - Verify YouTube ad blocking still works (response filter streaming)
  - Check if API changed or was deprecated
- **Added:** 2026-05-01

---

## Nice to Have / Research

### Web Bluetooth / Web USB
- CEF may add support for these APIs in newer versions
- Currently not available in CEF 136
- Low priority for a browser focused on BSV/Web3

### COOP/COEP Header Handling
- Cross-Origin-Opener-Policy affects OAuth popup `window.opener` preservation
- Newer Chromium versions have `restrict-properties` mode
- Verify our popup handling still works with stricter COOP defaults

### Codec Updates
- We build CEF from source with proprietary codecs
- Check if build flags changed for H.264/AAC/H.265 support
- Verify media playback on YouTube, Twitch after update

---

## Process for CEF Version Updates

1. Check this document for investigation items
2. Build from source with `proprietary_codecs=true ffmpeg_branding=Chrome`
3. Run full test suite (Minimal + Standard site verification from CLAUDE.md)
4. Specifically test: Google Sign-In, OAuth flows, media playback, ad blocking, fingerprint protection
5. Update this document with findings
6. Update `CLAUDE.md` x.com media section if codec situation changes
