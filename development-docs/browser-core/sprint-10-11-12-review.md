# Sprint 10, 11, 12 Plan Review & Recommendations

**Reviewer**: Edwin (AI Assistant)  
**Date**: 2026-02-25  
**Request**: Compare Sprint 10 and 12 to Brave's implementations, provide improvement recommendations

---

## Executive Summary

All three sprint plans are **well-researched and comprehensive**. The architecture decisions are sound and align with industry best practices. Below are specific recommendations organized by sprint.

**Overall Assessment**:
| Sprint | Quality | Readiness | Concerns |
|--------|---------|-----------|----------|
| 10 (Scriptlet Compatibility) | ⭐⭐⭐⭐⭐ | Ready to start | Minor: verify `#@#+js()` syntax in adblock-rust 0.10.3 |
| 11 (Menu + Settings) | ⭐⭐⭐⭐ | Ready with notes | Scope creep risk — very large |
| 12 (Cookie + Fingerprint) | ⭐⭐⭐⭐⭐ | Ready to start | Complex, but well-planned |

---

## Sprint 10: Scriptlet Compatibility — Review

### What's Excellent

1. **Two-layer approach is correct**: Built-in exception list + user toggle matches Brave's architecture exactly
2. **Using `#@#+js()` syntax**: This is the right call — let adblock-rust handle exceptions natively rather than C++ bypass logic
3. **YouTube exception awareness**: Correctly notes that YouTube needs scriptlets for ad blocking to work
4. **Test matrix is comprehensive**: Good coverage of OAuth providers

### Comparison to Brave

| Aspect | Brave | Hodos Plan | Assessment |
|--------|-------|------------|------------|
| Exception list | `brave-unbreak.txt` in brave/adblock-lists repo | `hodos-unbreak.txt` embedded + updatable | ✅ Same pattern |
| Scriptlet exceptions | `#@#+js()` syntax | `#@#+js()` syntax | ✅ Correct |
| Per-site toggle | Shield Standard/Aggressive/Off | Full/Standard/Off (scriptlets toggle) | ✅ More granular — good |
| Auto-detection | None (reactive) | None (reactive) | ✅ Realistic |
| Update mechanism | Component Updater (CRX) | File override in AppData | ✅ Simpler, appropriate for MVP |

### Recommendations

#### 1. Verify `#@#+js()` Support in adblock-rust 0.10.3 (CRITICAL)

**Issue**: My research shows adblock-rust supports scriptlet injection, but explicit `#@#+js()` exception syntax support isn't confirmed for v0.10.3 specifically.

**Action**: Before starting Sprint 10, write a quick unit test:

```rust
#[test]
fn test_scriptlet_exception_syntax() {
    let mut filter_set = FilterSet::new(true);
    filter_set.add_filter_list("x.com##+js(set, test, true)", Default::default());
    filter_set.add_filter_list("x.com#@#+js()", Default::default());
    
    let engine = Engine::from_filter_set(filter_set, true);
    let resources = engine.url_cosmetic_resources("https://x.com/");
    
    assert!(resources.injected_script.is_empty(), 
        "#@#+js() should disable all scriptlets for x.com");
}
```

If this fails, you'll need to implement Layer 2 (C++ bypass) as the primary mechanism instead.

#### 2. Add Wildcard Domain Support

**Current**: `x.com#@#+js()` handles x.com exactly.

**Consider adding**: Subdomain patterns like `*.google.com#@#+js()` to cover:
- accounts.google.com
- myaccount.google.com
- apis.google.com

Check if adblock-rust supports `*.domain.com` in cosmetic filters.

#### 3. Add Exception List Versioning

**Recommendation**: Add a version number to `hodos-unbreak.txt`:

```
! Title: Hodos Browser Compatibility Exceptions
! Version: 1.0.0
! Last modified: 2026-02-25
```

This helps with debugging ("what version of the list was the user running?").

#### 4. Consider Future CDN Hosting

The plan mentions "consider CDN for hot-updates" as a post-sprint task. I'd elevate this to Sprint 10d or 10e:

**Why**: When a new site breaks (e.g., a popular bank updates their auth flow), you want to push a fix without shipping a browser update. Even a simple GitHub raw URL works:

```rust
const UNBREAK_URL: &str = 
    "https://raw.githubusercontent.com/ArcBit/Hodos-Browser/main/adblock-engine/src/hodos-unbreak.txt";
```

Check for updates every 24 hours, compare Last-Modified header, download if newer.

---

## Sprint 11: Menu + Settings — Review

### What's Excellent

1. **Menu pattern research is thorough**: Correctly identifies universal patterns across Chrome/Brave/Firefox/Edge
2. **Full-page settings is the right call**: Overlay settings don't scale
3. **Inline zoom controls**: Nice attention to detail — this is exactly how Chrome/Brave do it
4. **Settings wiring section**: Addresses the debt from Sprint 9a (settings persist but don't affect behavior)

### Concerns

#### 1. Scope Creep Risk (HIGH)

This sprint is very large:
- 11a: Menu button + overlay (~6 hours)
- 11b: Full-page settings with 9 section components (~8 hours)
- 11c: Wire settings to behavior (~4 hours)

**Total**: 18+ hours estimated, but likely 24-30 hours actual.

**Recommendation**: Split into two sprints:

**Sprint 11a**: Menu Button + Simplified Settings Page
- Menu overlay with all actions
- Settings page with 4 critical sections: General, Privacy, Downloads, About
- Defer: Import (already works in overlay), Profiles (already works in overlay), Wallet (already works in overlay)

**Sprint 11b**: Complete Settings Page + Wiring
- Add remaining settings sections
- Wire all settings to behavior
- Retire settings overlay

#### 2. Menu Overlay vs Separate Component

**Current plan**: Render menu as absolute-positioned React component inside header browser.

**Potential issue**: Header browser's V8 context is separate from main tabs. If menu actions need to access tab state, you may hit IPC complexity.

**Alternative consideration**: If complexity arises, the menu could be a small overlay HWND (like Privacy Shield) with its own render handler. But start with Option A as planned — it's simpler.

#### 3. Missing Keyboard Shortcut for Menu

**Add to 11a**: `Alt+F` or `F10` should open the menu (Chrome uses `Alt+F`, Firefox uses `Alt+F` or `F10`).

Add to `OnPreKeyEvent`:
```cpp
if (event.windows_key_code == VK_F10 || 
    (event.windows_key_code == 'F' && (event.modifiers & EVENTFLAG_ALT_DOWN))) {
    // Send IPC to toggle menu
    return true;
}
```

#### 4. Bookmarks Page Dependency

**Issue**: Menu includes "Bookmarks" action pointing to `/bookmarks` route, but I don't see a BookmarksPage component in the current codebase.

**Options**:
- A) Create a minimal bookmarks page in Sprint 11
- B) Point bookmarks action to existing bookmark manager (if any)
- C) Defer bookmarks menu item until bookmark bar/page is built

Recommend option C to reduce scope.

### Comparison to Brave

| Aspect | Brave | Hodos Plan | Assessment |
|--------|-------|------------|------------|
| Menu location | Rightmost (before profile) | Rightmost (before profile) | ✅ Correct |
| Menu content | Grouped with dividers | Grouped with dividers | ✅ Correct |
| Inline zoom | Yes (-, %, +, fullscreen) | Yes | ✅ Correct |
| Settings page | Full tab with sidebar | Full tab with sidebar | ✅ Correct |
| Settings URL | `brave://settings` | `http://127.0.0.1:5137/settings-page` | ⚠️ See below |

**Note on Settings URL**: Using `http://127.0.0.1:5137/settings-page` works but looks weird to users. Consider:
- Custom protocol: `hodos://settings` (requires `CefRegisterSchemeHandlerFactory`)
- Or just accept it for MVP — users rarely look at internal page URLs

---

## Sprint 12: Cookie + Fingerprint — Review

### What's Excellent

1. **eTLD+1 approach is correct**: Public Suffix List is the industry standard
2. **Session seed design matches Brave**: Per-session random token → HMAC per domain
3. **Farbling scope is smart**: Only farble small canvases to avoid breaking games/editors
4. **Federated login exception list**: Comprehensive coverage of Google/Microsoft/Apple/Facebook
5. **AudioContext fudge factor**: 0.00000014-0.00000214% is indeed inaudible

### Comparison to Brave's Fingerprinting

| Aspect | Brave | Hodos Plan | Assessment |
|--------|-------|------------|------------|
| Seed generation | Session-based | Session-based | ✅ Correct |
| Seed scope | Per-domain (eTLD+1) | Per-domain (eTLD+1) | ✅ Correct |
| Canvas farbling | Noise injection | Noise injection (LSB flip) | ✅ Correct |
| WebGL farbling | Vendor/renderer spoofing + noise | Vendor/renderer spoofing + noise | ✅ Correct |
| Navigator spoofing | hardwareConcurrency, deviceMemory, plugins | hardwareConcurrency, deviceMemory, plugins | ✅ Correct |
| Audio farbling | Multiplicative fudge | Multiplicative fudge | ✅ Correct |
| Third-party iframe handling | Uses top-level seed | Uses top-level seed | ✅ Correct |

**The plan is very well-aligned with Brave's proven approach.**

### Recommendations

#### 1. PSL Loading Strategy

**Current plan**: Parse PSL at startup (~200KB file, ~9000 entries).

**Recommendation**: Use CEF's built-in eTLD+1 function if available. Check for:
```cpp
#include "include/cef_parser.h"
// or
#include "include/cef_url_request.h"
```

CEF/Chromium internally uses the PSL for cookie handling. If exposed via API, use it to avoid maintaining your own parser.

If not available, consider:
- **Option A**: Embed PSL as compressed data, decompress at startup
- **Option B**: Ship pre-processed binary trie format for faster loading
- **Option C**: Use a third-party C++ PSL library (e.g., `libpsl`)

For MVP, Option A (embed raw file) is fine. Optimize later if startup time is affected.

#### 2. Add Storage Access API UI

**Current plan**: Relies on CEF's native SAA handling.

**Enhancement**: Add a permission prompt UI for SAA requests. When `document.requestStorageAccess()` is called:
1. Show a permission prompt: "Allow [embed domain] to access cookies on this site?"
2. Remember choice per first-party + third-party domain pair
3. Store in `domain_permissions` table

This gives users visibility and control. Brave shows a similar prompt.

**Scope**: Could be Sprint 12 Phase E add-on or defer to post-MVP.

#### 3. Fingerprinting Exception List

**Current plan**: No domain exceptions for fingerprinting (farbles everything except localhost).

**Consideration**: Some WebGL-heavy sites (Google Maps, Figma, game sites) might break. Have a plan for exceptions:

```javascript
// In fingerprint_protection.js
var EXCEPTION_DOMAINS = [
    'figma.com',
    'google.com/maps',
    // Add as needed
];

// Check if current domain should be excepted
if (EXCEPTION_DOMAINS.some(d => location.hostname.endsWith(d))) {
    return; // Skip fingerprint protection
}
```

Start with an empty list and add domains as breakage is discovered.

#### 4. WebGL Extension Fingerprinting

**Current plan**: Spoofs `UNMASKED_VENDOR_WEBGL` and `UNMASKED_RENDERER_WEBGL`.

**Additional vector**: `gl.getSupportedExtensions()` returns a list that varies by GPU. Consider:
- Returning a common subset of extensions
- Or sorting the list alphabetically (reduces entropy from order)

This is low priority but worth noting for future hardening.

#### 5. Screen Resolution Spoofing

**Current plan**: Randomize from common values `[1366, 1440, 1536, 1920, 2560]`.

**Concern**: If `screen.width` reports 1920 but CSS media queries see actual 2560, sites may behave oddly.

**Brave's approach**: They don't spoof screen dimensions in Standard mode (only in deprecated Strict mode) because it causes too much breakage.

**Recommendation**: Move screen spoofing to "Strict" mode only, or remove it for MVP. The entropy reduction is minimal (3-4 bits) compared to the breakage risk.

#### 6. Test with Cover Your Tracks First

**Before implementing**: Run the current browser (without any fingerprint protection) through Cover Your Tracks and document the baseline entropy score. After Sprint 12, re-test to measure improvement.

This gives you a concrete "before/after" metric for the changelog.

---

## Cross-Sprint Dependencies

```
Sprint 8 (Ad Blocking) ─────────┐
                                │
                                ▼
                    Sprint 10 (Scriptlet Compat)
                                │
                                │ (recommended first)
                                ▼
                    Sprint 12 (Cookie + Fingerprint)
                                │
                                │ (can parallel)
                                ▼
                    Sprint 11 (Menu + Settings)
```

**Recommended order**:
1. **Sprint 10 first**: Fixes x.com auth, high user value, enables proper testing
2. **Sprint 11a (menu only)** or **Sprint 12**: Either can go second
3. **Sprint 11b (full settings)**: Last, as it depends on features from other sprints

---

## Questions for Matt

1. **Sprint 10**: Do you want CDN auto-update for the exception list in the initial sprint, or defer to post-MVP?

2. **Sprint 11**: Agree with splitting into 11a (menu + minimal settings) and 11b (full settings + wiring)?

3. **Sprint 12**: Should screen resolution spoofing be:
   - A) Included in Standard mode (current plan)
   - B) Only in "Strict" mode
   - C) Removed entirely for MVP

4. **Sprint 12**: Do you want Storage Access API UI (permission prompt) in this sprint or defer?

5. **Priority**: If you had to pick one sprint to start Monday, which would it be?

---

## Summary of Key Recommendations

### Must-Do Before Starting

| Sprint | Action |
|--------|--------|
| 10 | Write unit test to verify `#@#+js()` works in adblock-rust 0.10.3 |
| 11 | Decide on scope split (11a/11b) |
| 12 | Run Cover Your Tracks baseline test |

### High-Value Improvements

| Sprint | Improvement | Effort |
|--------|-------------|--------|
| 10 | Add version number to exception list | 5 min |
| 10 | Add CDN auto-update for exception list | 2-3 hrs |
| 11 | Add `Alt+F` / `F10` keyboard shortcut | 30 min |
| 12 | Remove/defer screen resolution spoofing | -2 hrs (reduces scope) |
| 12 | Add fingerprint exception domain list infrastructure | 1 hr |

### Nice-to-Have (Defer OK)

- Custom `hodos://settings` URL scheme
- Storage Access API permission prompt UI
- WebGL extension list normalization

---

*Review complete. The plans are solid — proceed with confidence. The main risk is Sprint 11 scope, so consider the split. Let me know if you have questions!*
