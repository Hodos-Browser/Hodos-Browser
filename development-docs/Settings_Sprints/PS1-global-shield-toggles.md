# PS1: Global Shield Toggles

**Status**: Not Started
**Complexity**: Low-Medium
**Estimated Phases**: 2

---

## Current State

- UI exists in `PrivacySettings.tsx` — two toggle switches under "Shields" card
- Settings persist to `settings.json` via `SettingsManager`
- **Ad & tracker blocking toggle**: `privacy.adBlockEnabled` is stored but **never read** — ad blocking is always on globally, controlled only per-site via `AdblockCache::isSiteEnabled()`
- **Third-party cookie blocking toggle**: `privacy.thirdPartyCookieBlocking` is stored but **never read** — `EphemeralCookieManager` always enforces ephemeral third-party cookies with no off switch

---

## What Needs to Happen

### Phase 1: Global Ad-Block Toggle

**Goal**: When the global toggle is OFF, disable all ad blocking (network, cosmetic, scriptlet) regardless of per-site settings.

**Changes needed**:
- [ ] Read `SettingsManager::GetAdBlockEnabled()` in the ad-block check path
- [ ] In `simple_handler.cpp` where `AdblockCache::GetInstance().isSiteEnabled()` is checked, add a guard: if global is off, skip all blocking
- [ ] In cosmetic CSS injection (`OnLoadEnd`), check global setting before injecting
- [ ] In scriptlet injection (`OnContextCreated`), check global setting before injecting
- [ ] When `settings_set` IPC fires for `privacy.adBlockEnabled`, could optionally call `AdblockCache::SetGlobalEnabled(bool)` for fast in-memory access

**Design decisions**:
- Should global OFF override per-site ON? (Recommended: yes — global is the master switch)
- Should the Privacy Shield panel reflect global state? (e.g., show "Protection disabled globally" when off)
- Should per-site settings be preserved when global is off? (Recommended: yes — just skip checking them)

### Phase 2: Global Third-Party Cookie Toggle

**Goal**: When the toggle is OFF, allow all third-party cookies (disable EphemeralCookieManager enforcement).

**Changes needed**:
- [ ] Add `IsEnabled()` / `SetEnabled(bool)` to `EphemeralCookieManager`
- [ ] Read `SettingsManager::GetThirdPartyCookieBlocking()` during initialization
- [ ] In `CookieBlockManager::ShouldBlockCookie()`, check the global setting — if off, allow all third-party cookies
- [ ] When `settings_set` IPC fires for `privacy.thirdPartyCookieBlocking`, sync to `EphemeralCookieManager::SetEnabled()`

**Design decisions**:
- When toggled off, should existing ephemeral cookies be promoted to persistent? (Probably not — just stop enforcing going forward)
- Should `blocked_domains` (known trackers) still be blocked even when third-party toggle is off? (Recommended: yes — tracker domains are a separate concern)

---

## Test Checklist

- [ ] Toggle ad-block OFF globally → visit youtube.com → ads appear
- [ ] Toggle ad-block ON globally → visit youtube.com → ads blocked
- [ ] Per-site exception still works: global ON, site-specific OFF → ads appear on that site only
- [ ] Toggle third-party cookies OFF → visit site with third-party cookies → cookies allowed
- [ ] Toggle third-party cookies ON → third-party cookies are ephemeral again
- [ ] Known tracker domains (doubleclick.net) still blocked even with third-party toggle OFF
- [ ] Settings persist across browser restart
- [ ] Privacy Shield panel reflects global state correctly

---

**Last Updated**: 2026-02-28
