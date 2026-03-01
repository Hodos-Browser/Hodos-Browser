# PS1: Global Shield Toggles — Detailed Implementation Plan

**Status**: Not Started  
**Complexity**: Low-Medium  
**Estimated Time**: 2-3 hours  
**Dependencies**: None

---

## Executive Summary

Wire the global privacy toggles (ad-blocking, third-party cookies) to actual behavior. Currently, these settings are saved but ignored — ad-blocking is always on globally, and third-party cookie blocking is always enforced.

---

## Current State Analysis

### What Exists

**Ad-Blocking:**
- **UI**: Toggle in `PrivacySettings.tsx` — "Block ads and trackers"
- **Persistence**: `SettingsManager::SetAdBlockEnabled()` saves `privacy.adBlockEnabled`
- **Backend**: `AdblockCache::isSiteEnabled(domain)` checks per-site toggles
- **Gap**: Global toggle (`privacy.adBlockEnabled`) is **never read**

**Third-Party Cookies:**
- **UI**: Toggle in `PrivacySettings.tsx` — "Block third-party cookies"
- **Persistence**: `SettingsManager::SetThirdPartyCookieBlocking()` saves `privacy.thirdPartyCookieBlocking`
- **Backend**: `EphemeralCookieManager` always enforces ephemeral third-party cookies
- **Gap**: Global toggle (`privacy.thirdPartyCookieBlocking`) is **never read**

---

## Phase 1: Global Ad-Block Toggle (1-2 hours)

### Design Decision

**Global OFF overrides per-site ON**: When global is disabled, all ad-blocking stops regardless of per-site settings. Per-site settings are preserved and will apply again when global is re-enabled.

### Step 1: Add Global Check to AdblockCache

**Option A (Simple)**: Check `SettingsManager` on every ad-block check.

**File**: `include/core/AdblockCache.h` — update `check()` method

```cpp
bool check(const std::string& url, const std::string& sourceUrl,
           const std::string& resourceType) {
    if (!g_adblockServerRunning) return false;
    
    // NEW: Check global setting first
    auto& settings = SettingsManager::GetInstance();
    if (!settings.GetPrivacySettings().adBlockEnabled) {
        return false; // Global ad-block disabled — allow all
    }
    
    // Existing cache lookup and backend check...
}
```

**Option B (More Efficient)**: Cache the global state in AdblockCache to avoid mutex lock on every check.

```cpp
// Add to AdblockCache class:
private:
    bool globalEnabled_ = true;

public:
    void SetGlobalEnabled(bool enabled) {
        globalEnabled_ = enabled;
        if (!enabled) {
            clearAll(); // Clear cache when disabled
        }
    }
    
    bool check(...) {
        if (!globalEnabled_) return false;
        // ... existing logic
    }
```

Then sync from IPC when setting changes:
```cpp
// In settings_set handler:
if (key == "privacy.adBlockEnabled") {
    AdblockCache::GetInstance().SetGlobalEnabled(value.GetBool());
}
```

**Recommendation**: Option B for performance — ad-block checks happen very frequently.

### Step 2: Update Cosmetic/Scriptlet Injection

**File**: `cef_browser_shell.cpp` — in `OnLoadEnd()` and `OnContextCreated()`

Before injecting cosmetic CSS or scriptlets:
```cpp
// Check global ad-block setting
auto& settings = SettingsManager::GetInstance();
if (!settings.GetPrivacySettings().adBlockEnabled) {
    return; // Skip all injections
}
```

Or use the cached state:
```cpp
if (!AdblockCache::GetInstance().IsGlobalEnabled()) {
    return;
}
```

### Step 3: Update Privacy Shield Panel (Optional UX Enhancement)

When global ad-blocking is disabled, the Privacy Shield panel could show a message like:
- "Ad blocking is disabled globally"
- Or gray out the per-site toggle with a tooltip

---

## Phase 2: Global Third-Party Cookie Toggle (1 hour)

### Step 1: Add Enabled State to EphemeralCookieManager

**File**: `include/core/EphemeralCookieManager.h`

```cpp
class EphemeralCookieManager {
public:
    // NEW: Global enable/disable
    void SetEnabled(bool enabled);
    bool IsEnabled() const;
    
private:
    bool enabled_ = true; // Default: enforcing ephemeral cookies
    // ... existing members
};
```

**File**: `src/core/EphemeralCookieManager.cpp`

```cpp
void EphemeralCookieManager::SetEnabled(bool enabled) {
    std::unique_lock lock(mutex_);
    enabled_ = enabled;
}

bool EphemeralCookieManager::IsEnabled() const {
    std::shared_lock lock(mutex_);
    return enabled_;
}
```

### Step 2: Check Enabled State in Cookie Blocking

**File**: `src/core/CookieBlockManager.cpp` — in `ShouldBlockCookie()` or `CanSaveCookie()`

```cpp
bool CookieBlockManager::ShouldBlockCookie(...) {
    // NEW: Check global setting
    if (!EphemeralCookieManager::GetInstance().IsEnabled()) {
        return false; // All third-party cookies allowed
    }
    
    // ... existing third-party cookie logic
}
```

### Step 3: Sync Setting on Change

**File**: `simple_handler.cpp` — in `settings_set` IPC handler

```cpp
if (key == "privacy.thirdPartyCookieBlocking") {
    EphemeralCookieManager::GetInstance().SetEnabled(value.GetBool());
}
```

### Step 4: Initialize on Startup

**File**: Where `EphemeralCookieManager` is initialized (likely startup code)

```cpp
auto& cookieSettings = SettingsManager::GetInstance().GetPrivacySettings();
EphemeralCookieManager::GetInstance().SetEnabled(cookieSettings.thirdPartyCookieBlocking);
```

---

## Important Design Notes

### Blocked Domains List
The `blocked_domains` (known tracker list) should **still be blocked** even when third-party cookie toggle is OFF. These are hardcoded malicious trackers.

```cpp
bool CookieBlockManager::ShouldBlockCookie(...) {
    // Always block known trackers, regardless of toggle
    if (IsKnownTracker(domain)) {
        return true;
    }
    
    // Only enforce ephemeral cookies if toggle is ON
    if (!EphemeralCookieManager::GetInstance().IsEnabled()) {
        return false;
    }
    
    // ... existing third-party logic
}
```

### What "Disable Third-Party Blocking" Means
- **Toggle ON** (default): Third-party cookies are ephemeral (deleted when site closes)
- **Toggle OFF**: Third-party cookies persist normally (like Chrome default behavior)

---

## Gaps & Questions

| Gap | Resolution |
|-----|------------|
| Cache invalidation when toggle changes | Clear AdblockCache on toggle |
| Privacy Shield UI reflection | Show "Protection disabled" when off |
| Per-site vs global precedence | Global OFF wins over per-site ON |

---

## Test Checklist

### Global Ad-Block Toggle
- [ ] Toggle OFF globally → visit YouTube → ads appear
- [ ] Toggle ON globally → visit YouTube → ads blocked
- [ ] Toggle OFF globally → per-site exceptions ignored (all ads show)
- [ ] Toggle ON → per-site OFF → ads show on that site only
- [ ] Privacy Shield shows correct state

### Global Third-Party Cookie Toggle
- [ ] Toggle ON (default) → third-party cookies are ephemeral
- [ ] Toggle OFF → third-party cookies persist across sessions
- [ ] Known tracker domains (doubleclick.net) still blocked regardless
- [ ] Toggle changes take effect immediately (no restart needed)

### Persistence
- [ ] Both settings persist across browser restart
- [ ] Settings work correctly on fresh profile

---

## Files to Modify

| File | Changes |
|------|---------|
| `include/core/AdblockCache.h` | Add `SetGlobalEnabled()`, `IsGlobalEnabled()` |
| `include/core/EphemeralCookieManager.h` | Add `SetEnabled()`, `IsEnabled()` |
| `src/core/EphemeralCookieManager.cpp` | Implement enable/disable methods |
| `src/core/CookieBlockManager.cpp` | Check global state in `ShouldBlockCookie()` |
| `cef_browser_shell.cpp` | Check global state before cosmetic/scriptlet injection |
| `src/handlers/simple_handler.cpp` | Sync settings to singletons on IPC change |

---

**Last Updated**: 2026-02-28
