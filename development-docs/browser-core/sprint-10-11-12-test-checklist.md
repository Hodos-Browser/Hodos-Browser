# Sprint 10 / 11 / 12 — Comprehensive Test Checklist

**Date**: 2026-02-25
**Sprints covered**: 10 (Scriptlet Compatibility), 11 (Menu + Settings), 12 (Fingerprint Protection)
**Test level**: Thorough (30-45 min)

---

## Pre-Test Setup

1. Build all components:
   - [ ] `cargo build --release` in `adblock-engine/`
   - [ ] `cargo build --release` in `rust-wallet/`
   - [ ] `npm run build` in `frontend/`
   - [ ] `cmake --build build --config Release` in `cef-native/`
2. Start all services:
   - [ ] Rust wallet running on port 3301
   - [ ] Adblock engine running on port 3302
   - [ ] Frontend dev server on port 5137 (or production build)
   - [ ] Launch HodosBrowserShell.exe

---

## Sprint 10: Scriptlet Compatibility System

### 10a — Exception List (hodos-unbreak.txt)

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 1 | Exception list loads | Check adblock engine startup log | `Loaded hodos-unbreak.txt` in log, no errors | [ ] |
| 2 | x.com auth works | Navigate to x.com, sign in | Login succeeds, no JS errors blocking auth flow | [ ] |
| 3 | google.com auth works | Navigate to accounts.google.com, sign in | Login succeeds, OAuth flow completes | [ ] |
| 4 | github.com auth works | Navigate to github.com, sign in | Login succeeds | [ ] |

### 10b — Per-Site Scriptlet Toggle (C++ AdblockCache JSON)

> **Note**: Per-site adblock/scriptlet settings were moved from the Rust wallet DB (port 3301) to C++ AdblockCache (`adblock_settings.json` in profile dir) during the adblock architecture refactor. The old wallet endpoints (`/adblock/scriptlet-toggle`, `/adblock/site-toggle`) no longer exist.

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 5 | Scriptlet toggle via Shield | Open Privacy Shield panel, toggle "Scriptlet injection" off for a site | Setting saved to `adblock_settings.json` in profile dir | [ ] |
| 6 | Scriptlets disabled on reload | With scriptlets off for a domain, reload the page | No scriptlet JS injected (check debug log for `script=0`) | [ ] |
| 7 | Re-enable scriptlets | Toggle scriptlet injection back on in Shield panel | Scriptlets injected on next page load | [ ] |
| 8 | Cosmetic CSS independent | With scriptlets disabled for a domain | Cosmetic CSS selectors still applied (only scriptlets disabled) | [ ] |

### 10c — Privacy Shield UI Toggle

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 9 | Scriptlet toggle visible | Open Privacy Shield panel on any site | "Scriptlet injection" row with toggle visible | [ ] |
| 10 | Toggle disabled when adblock off | Turn off "Tracker blocking" in shield panel | Scriptlet toggle becomes disabled/greyed | [ ] |
| 11 | Toggle scriptlets off | Toggle scriptlet injection off for a site | Toggle saves, refreshing page shows scriptlets disabled | [ ] |
| 12 | Toggle scriptlets back on | Toggle scriptlet injection back on | Scriptlets re-enabled, page reload injects them | [ ] |

---

## Sprint 11: Menu Button + Full-Page Settings

### 11a — Three-Dot Menu

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 13 | Menu opens | Click three-dot (⋮) button in toolbar | Menu dropdown appears with sections | [ ] |
| 14 | Menu closes on outside click | Click anywhere outside the menu | Menu closes | [ ] |
| 15 | Menu closes on Escape | Press Escape while menu is open | Menu closes | [ ] |
| 16 | New Tab | Click "New Tab" in menu | New tab opens | [ ] |
| 17 | Find in Page | Click "Find in Page" | Find bar opens (Ctrl+F equivalent) | [ ] |
| 18 | Print | Click "Print" | Print dialog opens | [ ] |
| 19 | Zoom controls | Click +/- zoom buttons in menu | Page zooms in/out, percentage updates | [ ] |
| 20 | Zoom reset | Click reset button (percentage number) | Zoom returns to 100% | [ ] |
| 21 | Bookmark Page | Click "Bookmark Page" | Page bookmarked (Ctrl+D equivalent) | [ ] |
| 22 | Downloads | Click "Downloads" | Downloads panel opens (Ctrl+J equivalent) | [ ] |
| 23 | History | Click "History" | History tab opens (Ctrl+H equivalent) | [ ] |
| 24 | Developer Tools | Click "Developer Tools" | DevTools window opens (F12 equivalent) | [ ] |
| 25 | Settings | Click "Settings" | Full-page settings opens in a new tab | [ ] |
| 26 | Exit | Click "Exit" | Browser closes | [ ] |
| 27 | Keyboard shortcuts shown | Look at menu items | Shortcut labels shown (Ctrl+T, Ctrl+F, etc.) | [ ] |

### 11a — Settings Page (Layout)

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 28 | Settings page loads | Navigate to settings (via menu or URL) | Full-page settings with sidebar navigation | [ ] |
| 29 | Sidebar sections | Look at sidebar | 5 sections: General, Privacy, Downloads, Wallet, About | [ ] |
| 30 | Section navigation | Click each sidebar item | Content area updates to show that section | [ ] |
| 31 | Deep-link URL | Navigate to `/settings-page/privacy` | Privacy section auto-selected | [ ] |

### 11a — General Settings

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 32 | Homepage setting | Set homepage to `https://example.com` | Value persists after refresh | [ ] |
| 33 | Homepage applied | Open a new tab | New tab loads the configured homepage | [ ] |
| 34 | Restore session toggle | Toggle restore session on/off | Value persists, toggle reflects state | [ ] |
| 35 | Search engine dropdown | Change search engine | Value persists | [ ] |
| 36 | Bookmark bar toggle | Toggle bookmark bar | Value persists | [ ] |

### 11a — Privacy Settings

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 37 | Ad blocking toggle | Toggle ad/tracker blocking | Value persists, adblock behavior changes globally | [ ] |
| 38 | Cookie blocking toggle | Toggle third-party cookie blocking | Value persists | [ ] |
| 39 | Fingerprint protection toggle | Toggle fingerprint protection | Value persists | [ ] |
| 40 | DNT toggle | Toggle Do Not Track | Value persists | [ ] |
| 41 | DNT header sent | Enable DNT, visit httpbin.org/headers | `DNT: 1` and `Sec-GPC: 1` headers present | [ ] |
| 42 | Clear data on exit toggle | Toggle clear data on exit | Value persists | [ ] |
| 43 | Clear browsing data button | Click "Clear browsing data now" | IPC sent, data cleared | [ ] |

### 11a — Download Settings

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 44 | Download folder displayed | Open Downloads settings | Current download path shown | [ ] |
| 45 | Download folder change | Change download path | New path persists | [ ] |

### 11b — Wallet Settings

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 46 | Auto-approve toggle | Toggle auto-approve on/off | Value persists | [ ] |
| 47 | Per-tx limit | Change per-transaction limit | Value persists (displays in cents) | [ ] |
| 48 | Per-session limit | Change per-session limit | Value persists | [ ] |
| 49 | Rate limit | Change rate limit per minute | Value persists | [ ] |

### 11a — About Section

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 50 | About info | Open About section | Shows Hodos version, CEF version, tech stack | [ ] |

### 11b — Settings Persistence

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 51 | Settings survive restart | Change multiple settings, restart browser | All changed settings persist | [ ] |
| 52 | Settings per-profile | Switch profiles, check settings | Each profile has independent settings | [ ] |

---

## Sprint 12: Fingerprint Protection

### 12c — Seed Infrastructure

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 53 | Session token generation | Start browser, check debug log | `FingerprintProtection initialized with session token` in log | [ ] |
| 54 | Domain seed consistency | Visit a site, navigate within it | Same fingerprint values within a single domain for the session | [ ] |
| 55 | Cross-domain variation | Visit two different domains, compare fingerprints | Different fingerprint values per domain | [ ] |
| 56 | Session rotation | Restart browser, visit same site | Fingerprint values change from previous session | [ ] |

### 12d — Canvas Farbling

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 57 | Canvas fingerprint changed | Visit browserleaks.com/canvas or similar | Canvas hash differs from raw Chrome/CEF | [ ] |
| 58 | Canvas still functional | Visit a site that uses canvas (e.g. Google Maps, charts) | Canvas renders correctly (not visibly broken) | [ ] |
| 59 | toDataURL affected | Run `document.createElement('canvas').getContext('2d').canvas.toDataURL()` in console | Returns a data URL (but hash differs between sessions) | [ ] |

### 12d — WebGL Spoofing

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 60 | WebGL vendor spoofed | Check `UNMASKED_VENDOR_WEBGL` via DevTools | Returns generic "Graphics" instead of real vendor | [ ] |
| 61 | WebGL renderer spoofed | Check `UNMASKED_RENDERER_WEBGL` | Returns generic "WebGL" instead of real GPU name | [ ] |
| 62 | WebGL still works | Visit a WebGL demo site | 3D rendering works correctly | [ ] |

### 12d — Navigator Overrides

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 63 | hardwareConcurrency | Run `navigator.hardwareConcurrency` in console | Returns 2, 4, 6, or 8 (not real CPU count if different) | [ ] |
| 64 | deviceMemory | Run `navigator.deviceMemory` in console | Returns 8 | [ ] |
| 65 | plugins | Run `navigator.plugins.length` in console | Returns 0 | [ ] |

### 12d — AudioContext Farbling

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 66 | AudioContext fingerprint | Visit audiofingerprint test site | Audio fingerprint differs from raw Chrome | [ ] |
| 67 | Audio still works | Play audio/video on YouTube | Audio plays correctly, no glitches | [ ] |

### 12e — Settings Integration

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 68 | Fingerprint in Privacy Shield | Open Privacy Shield panel | "Fingerprint shield" row shows as enabled | [ ] |
| 69 | Fingerprint in Settings | Open Privacy > Settings page | Fingerprint protection toggle present and on | [ ] |
| 70 | Disable fingerprint protection | Toggle off in Settings | Fingerprint values return to real values on next navigation | [ ] |
| 71 | Re-enable fingerprint protection | Toggle back on | Fingerprint farbling resumes | [ ] |

### 12 — No Screen Resolution Spoofing (Intentional)

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 72 | Screen resolution real | Run `screen.width` and `screen.height` | Returns REAL screen dimensions (not spoofed) | [ ] |
| 73 | No layout breakage | Visit responsive sites (youtube, x.com, reddit) | No layout issues from resolution mismatch | [ ] |

---

## Cross-Feature Integration Tests

| # | Test | Steps | Expected | Pass? |
|---|------|-------|----------|-------|
| 74 | Shield + Settings agree | Toggle adblock in Settings, check Shield panel | Both reflect same state | [ ] |
| 75 | Menu + Settings flow | Open menu > Settings > Privacy > toggle adblock | Setting changes, shield reflects it | [ ] |
| 76 | All protections on | Enable all shields, visit youtube.com | Ads blocked, cookies blocked, fingerprint farbled, no crashes | [ ] |
| 77 | All protections off | Disable all shields, visit youtube.com | Normal browsing, no ad/cookie/fingerprint blocking | [ ] |
| 78 | Multi-profile settings | Create profile, change settings in each | Settings independent per profile | [ ] |

---

## Regression Tests (Standard Site Basket)

| # | Site | Test | Expected | Pass? |
|---|------|------|----------|-------|
| 79 | youtube.com | Watch a video with shields on | Video plays, ads blocked, no errors | [ ] |
| 80 | youtube.com | Search, browse recommendations | All features work | [ ] |
| 81 | x.com | Sign in with shields on | Auth succeeds (exception list) | [ ] |
| 82 | x.com | Browse timeline, view profiles | Content loads, scriptlets don't break UX | [ ] |
| 83 | github.com | Sign in, browse repos | Auth works, site functions normally | [ ] |
| 84 | google.com | Sign in to Google account | OAuth flow completes | [ ] |
| 85 | reddit.com | Browse subreddits, read threads | Content loads, ads blocked | [ ] |
| 86 | amazon.com | Search products, browse pages | E-commerce features work | [ ] |
| 87 | twitch.tv | Watch a stream | Stream plays, chat works | [ ] |
| 88 | nytimes.com | Read articles | Content loads, trackers blocked | [ ] |

---

## Known Limitations

- **Fingerprint shield toggle in Privacy Shield panel is read-only** — toggle it via Settings page instead
- **Screen resolution NOT spoofed** — intentional decision (Brave removed it; breakage > entropy benefit)
- **CefResponseFilter buffering** adds minor YouTube page load latency — tracked in `ux-ui-cleanup.md`
- **Pre-existing frontend TS errors** (SendPage, BackupOverlayRoot, etc.) — not from Sprints 10-12

---

## Build Verification Summary

| Component | Command | Status |
|-----------|---------|--------|
| rust-wallet | `cargo check` | Pass (warnings only) |
| adblock-engine | `cargo check` | Pass (warnings only) |
| frontend | `npm run build` | Pass (pre-existing errors only, no Sprint 10-12 errors) |
| cef-native | `cmake --build build --config Release` | **Needs build** (user must build on Windows) |

---

## Sign-Off

- Tester: _______________
- Date: _______________
- Browser version: Hodos 1.0.0
- Overall result: [ ] Pass / [ ] Pass with notes / [ ] Fail
- Notes: _______________
