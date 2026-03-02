# PS1: Global Shield Toggles

**Status**: Complete
**Complexity**: Low-Medium
**Completed**: 2026-03-01

---

## Summary

Wired the two global privacy toggles (Ad & tracker blocking, Third-party cookie blocking) in Settings > Privacy so they function as master switches over all blocking code paths. Added UI feedback in the Privacy Shield panel — per-site toggles are greyed out and disabled when the corresponding global toggle is OFF, with tooltip explanation.

---

## What Was Done

### C++ Backend — Global Ad-Block Toggle

- Added `std::atomic<bool> global_enabled_` to `AdblockCache` singleton with `SetGlobalEnabled()`/`IsGlobalEnabled()` methods (lock-free reads)
- Synced on startup: `AdblockCache::SetGlobalEnabled()` called after `Initialize()` in `cef_browser_shell.cpp`, reading from `SettingsManager::GetPrivacySettings().adBlockEnabled`
- Synced on change: `settings_set` IPC handler in `simple_handler.cpp` calls `AdblockCache::SetGlobalEnabled()` when `privacy.adBlockEnabled` changes
- Added `IsGlobalEnabled()` guard to all 5 ad-blocking code paths:
  1. Network blocking (`GetResourceRequestHandler`)
  2. Scriptlet pre-cache (`OnBeforeBrowse`)
  3. Scriptlet pre-cache (`OnLoadingStateChange` — loading start)
  4. Cosmetic CSS + scriptlet injection (`OnLoadingStateChange` — loading end)
  5. YouTube response filter (`GetResourceResponseFilter`)

### C++ Backend — Global Cookie Blocking Toggle

- Added `SettingsManager` include to `CookieBlockManager.cpp`
- Inserted early-return guard in both `CanSendCookie()` and `CanSaveCookie()` — after blocked domain check, before `IsThirdParty()` check
- When `thirdPartyCookieBlocking` is false: all non-blocked third-party cookies are allowed
- Known tracker domains (doubleclick.net, etc.) are **still blocked** regardless of toggle state

### Frontend — Privacy Shield Panel Feedback

- `PrivacyShieldPanel.tsx`: imports `useSettings` to read global toggle state
- When global adblock is OFF: tracker blocking + scriptlet rows are greyed out, switches disabled and unchecked, tooltip says "Disabled globally in Privacy Settings. Per-site toggle has no effect."
- When global cookie blocking is OFF: cookie blocking row gets same treatment
- When both globals are OFF: master toggle also disabled
- Blocked counts hidden when global is off (prevents showing stale "N trackers blocked")
- `showCount` prop (from `PrivacyShieldOverlayRoot`) ensures settings refresh on every panel open, even for the same domain

### Files Modified

| File | Changes |
|------|---------|
| `cef-native/include/core/AdblockCache.h` | `atomic<bool> global_enabled_`, `SetGlobalEnabled()`, `IsGlobalEnabled()` |
| `cef-native/src/handlers/simple_handler.cpp` | 5 guard checks + settings_set sync |
| `cef-native/src/core/CookieBlockManager.cpp` | SettingsManager include + guard in CanSendCookie/CanSaveCookie |
| `cef-native/cef_browser_shell.cpp` | Startup sync of global flag |
| `frontend/src/components/PrivacyShieldPanel.tsx` | useSettings, greyed-out UI, showCount refresh |
| `frontend/src/pages/PrivacyShieldOverlayRoot.tsx` | showCount state + prop |

---

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Global OFF overrides per-site ON? | Yes | Global is the master switch — per-site settings are preserved but ineffective |
| Shield panel reflects global state? | Yes | Greyed-out toggles + tooltip prevents confusion |
| Blocked domains still blocked when cookie toggle OFF? | Yes | Known trackers (doubleclick.net) are a separate concern from generic third-party cookies |
| Cookie toggle reads from SettingsManager directly? | Yes | Simple `GetPrivacySettings()` call — one mutex lock per cookie check is acceptable since cookie checks are less frequent than ad-block URL checks |
| Ad-block toggle uses atomic bool? | Yes | Avoids mutex lock on every network request — performance critical path |

---

## Test Results

- [x] Toggle ad-block OFF globally → youtube.com shows ads
- [x] Toggle ad-block ON globally → youtube.com ads blocked
- [x] Per-site exception works: global ON, site-specific OFF → ads appear on that site only
- [x] Toggle third-party cookies OFF → third-party cookies flow freely
- [x] Toggle third-party cookies ON → ephemeral cookie blocking resumes
- [x] Known tracker domains still blocked with cookie toggle OFF
- [x] Settings persist across restart
- [x] Shield panel toggles greyed out when global is OFF
- [x] Shield panel updates immediately when reopened after global change

---

**Last Updated**: 2026-03-01
