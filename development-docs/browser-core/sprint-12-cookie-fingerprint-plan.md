# Sprint 12: Third-Party Cookie Blocking + Fingerprinting Protection — Implementation Plan

**Created**: 2026-02-25
**Status**: Planning
**Estimated Duration**: 3-5 days
**Dependencies**: Sprint 8 (Ad Blocking) complete, Sprint 10 (Scriptlet Compatibility) recommended

---

## Problem Statement

Hodos Browser currently has **tracker-based** cookie blocking (`CookieBlockManager` with `DefaultTrackerList.h` — 24 known tracker domains) but does NOT block third-party cookies broadly. The browser also has **zero** fingerprinting protection — users are fully trackable via Canvas, WebGL, AudioContext, and Navigator API fingerprinting.

**Industry context**: Google abandoned third-party cookie deprecation in Chrome (April 2025). Safari and Firefox already block third-party cookies by default. Brave blocks them and adds fingerprint farbling. We should match or exceed Brave's privacy stance to align with our security-first brand.

---

## Research Summary

### Third-Party Cookie Blocking

**How "third-party" is determined**: Compare the eTLD+1 (effective top-level domain + 1) of the cookie's domain against the top-level page's eTLD+1. If they differ, the cookie is third-party.

**Current Hodos implementation**: `CookieBlockManager::IsThirdParty()` uses simple substring matching on normalized domains. This works for common cases (`example.com` vs `tracker.com`) but fails for public suffix edge cases like `co.uk`, `blogspot.com`, `github.io`.

**SameSite interaction**: Cookies with `SameSite=None` (+ `Secure`) are the ones sent cross-site. `SameSite=Lax` and `Strict` are already restricted. Our blocking should focus on `SameSite=None` cookies in cross-site contexts.

**Storage Access API**: W3C-standardized mechanism for embedded third-party content to request cookie access with user gesture. Supported in CEF 136 (Chromium 136) natively — no custom C++ code needed.

**Exception domains**: Federated login (Google, Microsoft, Apple), embedded payments (Stripe, PayPal), and embedded content (YouTube embeds) need third-party cookies. We need an allowlist.

### Fingerprinting Protection

**Vectors ranked by entropy** (most to least identifying):

| Vector | Entropy | Protection Approach | Priority |
|--------|---------|-------------------|----------|
| Canvas | 8-12 bits | Noise injection (Brave's farbling) | HIGH |
| WebGL GPU info | 5-8 bits | Return generic vendor/renderer strings | HIGH |
| Navigator properties | 5-7 bits | Override with generic values | HIGH |
| AudioContext | 3-5 bits | Multiplicative fudge factor | MEDIUM |
| Screen resolution | 3-4 bits | Round to common values | MEDIUM |
| Hardware concurrency | 1-3 bits | Random value 2-8 | HIGH (easy) |
| Device memory | 1-2 bits | Fixed value (8) | HIGH (easy) |
| Plugins | 2-3 bits | Empty array | HIGH (easy) |

**Brave's farbling approach**:
1. Generate a random **session token** at browser startup (memory-only, never persisted)
2. For each page, compute a **per-domain seed**: `HMAC-SHA256(session_token, eTLD+1)`
3. All API overrides use this seed as PRNG input → consistent within a session, different across sessions
4. Third-party iframes use the **top-level page's** seed (not their own domain's)

**Firefox's approach**: Uniformity — make all RFP users look identical (hardcoded values). More resistant to statistical attacks but requires large anonymity set.

**Our approach**: Brave-style farbling (randomized per-session, per-domain). Better UX than Firefox's approach (doesn't break sites as aggressively) and sufficient for our user base.

---

## Implementation Plan

### 12a: Upgrade Third-Party Cookie Detection (Day 1, ~4 hours)

**Goal**: Replace simple domain substring matching with proper eTLD+1 comparison.

#### Step 1: Evaluate CEF's Built-In eTLD+1 Function

Check if CEF 136 exposes `CefGetRegistrableDomain()` or equivalent. If available, use it directly.

If not available, embed a minimal Public Suffix List:
- Download from `https://publicsuffix.org/list/public_suffix_list.dat` at build time
- Parse into a `std::unordered_set<std::string>` at startup
- Implement `GetETLDPlus1(domain)` function

#### Step 2: Create eTLD+1 Extraction Utility

**File**: `cef-native/include/core/DomainUtils.h` (NEW)

```cpp
#pragma once
#include <string>

class DomainUtils {
public:
    // Initialize with Public Suffix List data
    static void Initialize();

    // Extract registrable domain (eTLD+1)
    // "sub.example.co.uk" → "example.co.uk"
    // "blog.example.com" → "example.com"
    static std::string GetRegistrableDomain(const std::string& domain);

    // Check if two domains share the same eTLD+1
    static bool IsSameSite(const std::string& domain1, const std::string& domain2);

private:
    static bool IsPublicSuffix(const std::string& domain);
};
```

**Implementation**: Parse the PSL file into a trie or sorted set. The PSL has ~9000 entries and the file is ~200KB. Loading and parsing at startup adds <50ms.

If CEF's Chromium layer exposes `net::registry_controlled_domains::GetDomainAndRegistry()`, prefer that over our own parser. Check the CEF 136 include paths for `cef_url.h` or similar utilities.

#### Step 3: Upgrade CookieBlockManager::IsThirdParty()

**File**: `cef-native/src/core/CookieBlockManager.cpp`

Replace the current substring-based comparison:

```cpp
// Before (current):
bool CookieBlockManager::IsThirdParty(const std::string& cookieDomain,
                                        const std::string& pageDomain) {
    // Simple normalization and substring check
    ...
}

// After:
bool CookieBlockManager::IsThirdParty(const std::string& cookieDomain,
                                        const std::string& pageDomain) {
    std::string cookieETLD1 = DomainUtils::GetRegistrableDomain(cookieDomain);
    std::string pageETLD1 = DomainUtils::GetRegistrableDomain(pageDomain);
    return cookieETLD1 != pageETLD1;
}
```

#### Step 4: Add SameSite Awareness

In `CanSendCookie` / `CanSaveCookie`, check the cookie's `SameSite` attribute:

```cpp
bool CookieBlockManager::CanSendCookie(
    const CefCookie& cookie,
    const std::string& requestUrl,
    const std::string& topLevelUrl) {

    std::string cookieDomain = CefString(&cookie.domain).ToString();
    std::string pageDomain = ExtractDomain(topLevelUrl);

    // Same-site cookies are always allowed
    if (!IsThirdParty(cookieDomain, pageDomain)) {
        return true;
    }

    // SameSite=Strict or Lax: browser already restricts, let through
    if (cookie.same_site == CEF_COOKIE_SAME_SITE_STRICT_MODE ||
        cookie.same_site == CEF_COOKIE_SAME_SITE_LAX_MODE) {
        return true;
    }

    // Third-party cookie (SameSite=None) — check blocking policy
    if (thirdPartyCookieBlockingEnabled_) {
        // Check exception list
        if (IsAllowedThirdParty(cookieDomain, pageDomain)) {
            return true;
        }
        return false; // Block
    }

    return true;
}
```

#### Verification Checklist (12a)

- [ ] `DomainUtils::GetRegistrableDomain("sub.example.co.uk")` returns `"example.co.uk"`
- [ ] `DomainUtils::IsSameSite("a.google.com", "b.google.com")` returns `true`
- [ ] `DomainUtils::IsSameSite("google.com", "youtube.com")` returns `false`
- [ ] Third-party cookies from ad networks are blocked
- [ ] First-party cookies continue to work normally
- [ ] Build C++ successfully

---

### 12b: Third-Party Cookie Policy Engine (Day 1-2, ~4 hours)

**Goal**: Add configurable cookie blocking policy with exception handling.

#### Step 1: Three Blocking Modes

**File**: `cef-native/include/core/CookieBlockManager.h`

```cpp
enum class CookieBlockPolicy {
    AllowAll,          // No third-party blocking
    BlockTrackers,     // Block known trackers only (current DefaultTrackerList)
    BlockAllThirdParty // Block all third-party cookies (new default)
};
```

Wire this to the `SettingsManager` privacy setting `thirdPartyCookieBlocking`:
- `true` → `BlockAllThirdParty` (new default)
- `false` → `AllowAll`

**Future**: Add `BlockTrackers` as a middle option in the Privacy Shield / settings UI.

#### Step 2: Federated Login Exception List

**File**: `cef-native/include/core/CookieBlockManager.h` or `DefaultTrackerList.h`

Built-in allowlist for cross-site authentication domains:

```cpp
static const std::vector<std::pair<std::string, std::string>> COOKIE_EXCEPTIONS = {
    // {third-party domain, first-party domain pattern}
    // Google OAuth — allow Google cookies on any site using Google Sign-In
    {"accounts.google.com", "*"},
    {"google.com", "youtube.com"},      // Google/YouTube shared auth
    {"youtube.com", "google.com"},

    // Microsoft OAuth
    {"login.microsoftonline.com", "*"},
    {"login.live.com", "*"},

    // Apple Auth
    {"appleid.apple.com", "*"},

    // Facebook SDK (OAuth on third-party sites)
    {"facebook.com", "*"},

    // Payment providers
    {"stripe.com", "*"},
    {"paypal.com", "*"},
    {"js.stripe.com", "*"},
};
```

The `"*"` wildcard means "allow this third-party domain's cookies on any first-party site" (because OAuth flows redirect through many different sites).

#### Step 3: Per-Site Cookie Override

Reuse the existing `domain_permissions` infrastructure. Add a `cookie_policy` column or use the existing `allowed_third_party_` set in `CookieBlockManager`.

When a user encounters breakage:
1. Open Privacy Shield panel
2. Toggle "Allow third-party cookies on this site"
3. This adds the current page's domain to the per-site exception list

#### Step 4: Wire to Settings

Connect the global setting from `SettingsManager::GetPrivacySettings().thirdPartyCookieBlocking` to `CookieBlockManager`:

```cpp
// On startup and when setting changes:
void CookieBlockManager::SetPolicy(CookieBlockPolicy policy) {
    std::lock_guard<std::shared_mutex> lock(mutex_);
    policy_ = policy;
}
```

#### Step 5: Privacy Shield Panel Update

**File**: `frontend/src/components/PrivacyShieldPanel.tsx`

Update the cookie blocking toggle to show:
- Number of third-party cookies blocked on this page
- Toggle to allow third-party cookies for this specific site
- Link to full cookie settings

#### Verification Checklist (12b)

- [ ] With blocking enabled: visit ad-heavy site → third-party cookies blocked
- [ ] First-party cookies (login sessions, preferences) still work
- [ ] Google Sign-In works on third-party sites (OAuth flow)
- [ ] YouTube works when logged into Google
- [ ] PayPal/Stripe embedded checkouts work
- [ ] Per-site "Allow cookies" toggle works
- [ ] Settings toggle persists across restarts

---

### 12c: Fingerprinting Protection — Session Seed Infrastructure (Day 2, ~3 hours)

**Goal**: Create the per-session, per-domain seed system that all fingerprinting protections will use.

#### Step 1: Generate Session Token at Startup

**File**: `cef-native/cef_browser_shell.cpp`

Generate a random 32-byte session token at browser startup, stored only in memory:

```cpp
#include <random>
#include <array>

static std::array<uint8_t, 32> g_fingerprint_session_token;

void InitFingerprintProtection() {
    std::random_device rd;
    std::mt19937_64 gen(rd());
    std::uniform_int_distribution<uint8_t> dist(0, 255);
    for (auto& byte : g_fingerprint_session_token) {
        byte = dist(gen);
    }
}
```

Called once in `wWinMain()` / `main()`.

#### Step 2: Compute Per-Domain Seed

**File**: `cef-native/include/core/FingerprintProtection.h` (NEW)

```cpp
#pragma once
#include <string>
#include <cstdint>

class FingerprintProtection {
public:
    static FingerprintProtection& GetInstance();

    // Initialize session token (called once at startup)
    void Initialize();

    // Get per-domain seed for fingerprint farbling
    // Uses HMAC-SHA256(session_token, eTLD+1) → 32-bit seed
    uint32_t GetDomainSeed(const std::string& url);

    // Check if fingerprint protection is enabled
    bool IsEnabled() const;

private:
    FingerprintProtection() = default;
    std::array<uint8_t, 32> sessionToken_;
    bool enabled_ = true;
};
```

The `GetDomainSeed()` function:
1. Extracts the eTLD+1 from the URL
2. Computes HMAC-SHA256 using the session token as key
3. Takes the first 4 bytes as a uint32_t seed

OpenSSL (already linked via vcpkg) provides HMAC-SHA256.

#### Step 3: Pass Seed to Renderer via IPC

In `OnBeforeBrowse` or `OnLoadingStateChange` (same pattern as scriptlet pre-caching):

```cpp
// Compute seed for navigation target
uint32_t seed = FingerprintProtection::GetInstance().GetDomainSeed(url);

// Send to renderer process
CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("fingerprint_seed");
msg->GetArgumentList()->SetInt(0, static_cast<int>(seed));
msg->GetArgumentList()->SetString(1, url);
browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
```

In the renderer process, cache the seed for use during `OnContextCreated`:

```cpp
// In SimpleRenderProcessHandler
static std::mutex s_seedMutex;
static std::unordered_map<std::string, uint32_t> s_domainSeeds;

void OnProcessMessageReceived(...) {
    if (message_name == "fingerprint_seed") {
        uint32_t seed = static_cast<uint32_t>(args->GetInt(0));
        std::string url = args->GetString(1).ToString();
        std::lock_guard<std::mutex> lock(s_seedMutex);
        s_domainSeeds[url] = seed;
    }
}
```

#### Verification Checklist (12c)

- [ ] Session token generated at startup (non-zero, random)
- [ ] Same URL returns same seed within a session
- [ ] Different URLs return different seeds
- [ ] Different browser sessions return different seeds for the same URL
- [ ] Seed is passed to renderer via IPC
- [ ] Build C++ successfully

---

### 12d: Canvas + WebGL Fingerprinting Protection (Day 2-3, ~4 hours)

**Goal**: Inject JavaScript that farbles Canvas and WebGL API outputs using the per-domain seed.

#### Step 1: Create Fingerprint Protection Script

**File**: `cef-native/src/handlers/fingerprint_protection.js` (embedded as string constant)

```javascript
(function(seed) {
    'use strict';

    // Mulberry32 PRNG seeded with per-domain session seed
    function mulberry32(a) {
        return function() {
            a |= 0; a = a + 0x6D2B79F5 | 0;
            var t = Math.imul(a ^ a >>> 15, 1 | a);
            t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t;
            return ((t ^ t >>> 14) >>> 0) / 4294967296;
        }
    }
    var rng = mulberry32(seed);

    // === Canvas Farbling ===
    var _getImageData = CanvasRenderingContext2D.prototype.getImageData;
    CanvasRenderingContext2D.prototype.getImageData = function() {
        var data = _getImageData.apply(this, arguments);
        // Only farble small canvases (likely fingerprinting probes)
        if (data.width * data.height < 65536) {
            for (var i = 0; i < data.data.length; i += 4) {
                // Flip least significant bit of R channel based on PRNG
                if (rng() < 0.1) {
                    data.data[i] ^= 1;
                }
            }
        }
        return data;
    };

    var _toDataURL = HTMLCanvasElement.prototype.toDataURL;
    HTMLCanvasElement.prototype.toDataURL = function() {
        // Create a temporary canvas, copy content, farble, return
        var canvas = this;
        if (canvas.width * canvas.height < 65536) {
            var ctx = canvas.getContext('2d');
            if (ctx) {
                var imgData = ctx.getImageData(0, 0, canvas.width, canvas.height);
                // getImageData is already farbled above
                ctx.putImageData(imgData, 0, 0);
            }
        }
        return _toDataURL.apply(this, arguments);
    };

    var _toBlob = HTMLCanvasElement.prototype.toBlob;
    HTMLCanvasElement.prototype.toBlob = function(callback) {
        var canvas = this;
        if (canvas.width * canvas.height < 65536) {
            var ctx = canvas.getContext('2d');
            if (ctx) {
                var imgData = ctx.getImageData(0, 0, canvas.width, canvas.height);
                ctx.putImageData(imgData, 0, 0);
            }
        }
        return _toBlob.apply(this, arguments);
    };

    // === WebGL Fingerprinting ===
    function protectWebGL(proto) {
        var _getParameter = proto.getParameter;
        proto.getParameter = function(param) {
            var debugInfo = this.getExtension('WEBGL_debug_renderer_info');
            if (debugInfo) {
                if (param === debugInfo.UNMASKED_VENDOR_WEBGL) {
                    return 'Google Inc. (NVIDIA)';
                }
                if (param === debugInfo.UNMASKED_RENDERER_WEBGL) {
                    return 'ANGLE (NVIDIA, NVIDIA GeForce Graphics, OpenGL 4.5)';
                }
            }
            return _getParameter.call(this, param);
        };

        var _readPixels = proto.readPixels;
        proto.readPixels = function() {
            _readPixels.apply(this, arguments);
            // Farble the pixel data (last argument is the output array)
            var pixels = arguments[arguments.length - 1];
            if (pixels && pixels.length && pixels.length < 262144) {
                for (var i = 0; i < pixels.length; i += 4) {
                    if (rng() < 0.1) {
                        pixels[i] ^= 1;
                    }
                }
            }
        };
    }

    if (typeof WebGLRenderingContext !== 'undefined') {
        protectWebGL(WebGLRenderingContext.prototype);
    }
    if (typeof WebGL2RenderingContext !== 'undefined') {
        protectWebGL(WebGL2RenderingContext.prototype);
    }

    // === Navigator Properties ===
    var fakeHardwareConcurrency = 2 + Math.floor(rng() * 7); // 2-8
    Object.defineProperty(navigator, 'hardwareConcurrency', {
        get: function() { return fakeHardwareConcurrency; },
        enumerable: true, configurable: true
    });

    Object.defineProperty(navigator, 'deviceMemory', {
        get: function() { return 8; },
        enumerable: true, configurable: true
    });

    // Empty plugins array
    Object.defineProperty(navigator, 'plugins', {
        get: function() { return []; },
        enumerable: true, configurable: true
    });

    // === AudioContext Farbling ===
    if (typeof AudioBuffer !== 'undefined') {
        var _getChannelData = AudioBuffer.prototype.getChannelData;
        AudioBuffer.prototype.getChannelData = function(channel) {
            var data = _getChannelData.call(this, channel);
            // Apply tiny multiplicative fudge factor (inaudible)
            var fudge = 1.0 + (rng() - 0.5) * 0.0000004;
            for (var i = 0; i < data.length; i++) {
                data[i] *= fudge;
            }
            return data;
        };
    }

    if (typeof AnalyserNode !== 'undefined') {
        var _getFloatFrequencyData = AnalyserNode.prototype.getFloatFrequencyData;
        AnalyserNode.prototype.getFloatFrequencyData = function(array) {
            _getFloatFrequencyData.call(this, array);
            var fudge = 1.0 + (rng() - 0.5) * 0.0000004;
            for (var i = 0; i < array.length; i++) {
                array[i] *= fudge;
            }
        };
    }

    // === Screen Resolution ===
    // Round to common values
    var commonWidths = [1366, 1440, 1536, 1920, 2560];
    var commonHeights = [768, 900, 864, 1080, 1440];
    var widthIdx = Math.floor(rng() * commonWidths.length);
    Object.defineProperty(screen, 'width', {
        get: function() { return commonWidths[widthIdx]; },
        enumerable: true, configurable: true
    });
    Object.defineProperty(screen, 'height', {
        get: function() { return commonHeights[widthIdx]; },
        enumerable: true, configurable: true
    });

})(FINGERPRINT_SEED);
```

**Note**: `FINGERPRINT_SEED` is a placeholder replaced at injection time with the actual per-domain seed.

#### Step 2: Inject Protection Script in OnContextCreated

**File**: `cef-native/src/handlers/simple_render_process_handler.cpp`

After existing scriptlet injection (line ~573), inject the fingerprinting protection script:

```cpp
// Fingerprint protection injection
if (FingerprintProtection::GetInstance().IsEnabled() &&
    url.find("127.0.0.1") == std::string::npos) {

    uint32_t seed = 0;
    {
        std::lock_guard<std::mutex> lock(s_seedMutex);
        auto it = s_domainSeeds.find(url);
        if (it != s_domainSeeds.end()) {
            seed = it->second;
        } else {
            // Fallback: use URL hash as seed
            seed = std::hash<std::string>{}(url) & 0xFFFFFFFF;
        }
    }

    // Replace placeholder with actual seed
    std::string script = FINGERPRINT_PROTECTION_SCRIPT;
    size_t pos = script.find("FINGERPRINT_SEED");
    if (pos != std::string::npos) {
        script.replace(pos, 16, std::to_string(seed));
    }

    frame->ExecuteJavaScript(script, url, 0);
}
```

#### Step 3: Third-Party Iframe Handling

For third-party iframes, use the **top-level page's seed**, not the iframe's domain seed. This prevents trackers from getting consistent fingerprints across different embedding sites.

In `OnContextCreated`, determine if the frame is the main frame:
```cpp
bool isMainFrame = frame->IsMain();
if (!isMainFrame) {
    // Use the top-level URL's seed instead
    std::string topUrl = browser->GetMainFrame()->GetURL().ToString();
    // Get seed for topUrl's eTLD+1...
}
```

#### Verification Checklist (12d)

- [ ] Canvas `toDataURL()` returns different hashes across browser sessions
- [ ] Canvas `toDataURL()` returns consistent hash within a single session
- [ ] WebGL `getParameter(UNMASKED_VENDOR_WEBGL)` returns generic value
- [ ] `navigator.hardwareConcurrency` returns value between 2-8 (varies per domain)
- [ ] `navigator.deviceMemory` returns 8
- [ ] `navigator.plugins` returns empty array
- [ ] AudioContext fingerprint differs across sessions
- [ ] YouTube, Google Docs, and other canvas-using sites still function
- [ ] Games/interactive canvas apps still work (large canvases not farbled)

---

### 12e: Privacy Shield Integration + Settings Toggle (Day 3, ~3 hours)

**Goal**: Add fingerprinting protection controls to the Privacy Shield panel and Settings page.

#### Step 1: Add Fingerprinting Toggle to Privacy Shield

**File**: `frontend/src/components/PrivacyShieldPanel.tsx`

Add a fourth toggle row:

```
┌──────────────────────────────────────┐
│  Privacy Shield for example.com      │
│                                      │
│  ● Ad Blocking          [ON/OFF]     │
│  ● Scriptlet Injection  [ON/OFF]     │
│  ● Cookie Blocking      [ON/OFF]     │
│  ● Fingerprint Shield   [ON/OFF]     │  ← NEW
│    Randomizes device fingerprint     │
└──────────────────────────────────────┘
```

#### Step 2: Settings Page Toggle

**File**: `frontend/src/components/settings/PrivacySettings.tsx`

Add fingerprinting protection section with two levels:

```tsx
<SettingsCard title="Fingerprinting Protection">
    <SettingRow
        label="Fingerprint protection"
        description="Randomizes browser fingerprint to prevent cross-site tracking"
        control={
            <Select value={level} onChange={handleChange} size="small">
                <MenuItem value="off">Off</MenuItem>
                <MenuItem value="standard">Standard (Recommended)</MenuItem>
                <MenuItem value="strict">Strict</MenuItem>
            </Select>
        }
    />
    <Typography variant="body2" sx={{ color: '#888', mt: 1 }}>
        Standard: Randomizes Canvas, WebGL, Audio fingerprints. Compatible with most sites.
        <br />
        Strict: Also spoofs screen resolution, timezone, and language. May break some sites.
    </Typography>
</SettingsCard>
```

#### Step 3: Wire Settings to C++

Add `fingerprintProtectionLevel` to `SettingsManager`:
- `"off"` → No fingerprint injection
- `"standard"` → Canvas + WebGL + Navigator + Audio farbling (default)
- `"strict"` → All of standard + screen spoofing + timezone spoofing (future)

#### Verification Checklist (12e)

- [ ] Privacy Shield panel shows fingerprint toggle
- [ ] Settings page shows fingerprint protection level
- [ ] Toggling off → no fingerprint script injected
- [ ] Toggling on → script injected, APIs farbled
- [ ] Setting persists across restarts

---

### 12f: Testing Against Fingerprinting Test Sites (Day 3-4, ~3 hours)

**Goal**: Verify fingerprinting protection effectiveness against standard test tools.

#### Test Sites

| Site | Tests | Expected Result |
|------|-------|----------------|
| coveryourtracks.eff.org | Full fingerprint entropy analysis | Reduced entropy score, "randomized fingerprint" status |
| browserleaks.com/canvas | Canvas fingerprint | Different hash each session |
| browserleaks.com/webgl | WebGL renderer info | Generic renderer string shown |
| browserleaks.com/javascript | Navigator properties | hardwareConcurrency randomized, plugins empty |
| creepjs.com | Advanced fingerprinting (76+ vectors) | Reduced confidence score |
| amiunique.org | Browser uniqueness | "Not unique" or low uniqueness score |

#### Compatibility Testing

Test these sites with fingerprinting protection **enabled** to verify no breakage:

| Site | Feature to Test | Risk Level |
|------|----------------|------------|
| youtube.com | Video playback, comments | Low (canvas farble only on small canvases) |
| google.com/maps | Map rendering (WebGL) | Medium (WebGL renderer spoofing) |
| docs.google.com | Text editing, drawing | Medium (canvas API hooks) |
| figma.com | Design tool (heavy canvas) | Medium-High |
| discord.com | Emoji rendering, video call | Low |
| amazon.com | Product images, zoom | Low |
| github.com | Code rendering, charts | Low |

If breakage is found:
1. First try narrowing the farbling scope (e.g., skip canvas farbling for `google.com/maps`)
2. If needed, add domain to exception list (similar to Sprint 10's `hodos-unbreak.txt`)
3. Document in test-site-basket.md

---

## Files Changed Summary

| File | Changes |
|------|---------|
| **NEW** `cef-native/include/core/DomainUtils.h` | eTLD+1 extraction utility |
| **NEW** `cef-native/src/core/DomainUtils.cpp` | PSL parser + eTLD+1 implementation |
| **NEW** `cef-native/include/core/FingerprintProtection.h` | Session seed + protection manager |
| **NEW** `cef-native/src/core/FingerprintProtection.cpp` | HMAC seed computation + enabled flag |
| `cef-native/src/core/CookieBlockManager.cpp` | Upgrade `IsThirdParty()` to eTLD+1 |
| `cef-native/include/core/CookieBlockManager.h` | Add `CookieBlockPolicy` enum, exception list |
| `cef-native/cef_browser_shell.cpp` | Init FingerprintProtection + DomainUtils |
| `cef-native/src/handlers/simple_handler.cpp` | IPC for fingerprint seed, cookie policy changes |
| `cef-native/src/handlers/simple_render_process_handler.cpp` | Fingerprint script injection in `OnContextCreated`, seed cache |
| `cef-native/include/core/SettingsManager.h` | Add `fingerprintProtectionLevel` setting |
| `cef-native/src/core/SettingsManager.cpp` | Persist fingerprint setting |
| `cef-native/CMakeLists.txt` | Add new source files |
| `frontend/src/components/PrivacyShieldPanel.tsx` | Add fingerprint toggle |
| `frontend/src/components/settings/PrivacySettings.tsx` | Add fingerprint protection level selector |
| `frontend/src/hooks/usePrivacyShield.ts` | Add fingerprint state |

---

## Cross-Platform Notes

- **DomainUtils**: Pure C++, cross-platform. PSL data embedded or loaded from file.
- **FingerprintProtection**: Uses OpenSSL HMAC (already linked on both platforms). `std::random_device` is cross-platform.
- **CookieBlockManager**: Already has `#ifdef` platform conditionals.
- **Render process injection**: `simple_render_process_handler.cpp` is shared cross-platform.
- **Frontend**: Pure React, cross-platform.
- **Public Suffix List file**: Store in `%APPDATA%/HodosBrowser/` (Windows) / `~/Library/Application Support/HodosBrowser/` (macOS).

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| Canvas farbling breaks web apps | Games, image editors malfunction | Only farble small canvases (<256x256); skip visible canvases |
| WebGL renderer spoofing breaks feature detection | Sites assume wrong GPU capabilities | Only spoof `UNMASKED_*` info, leave capability parameters (MAX_TEXTURE_SIZE, etc.) real |
| Navigator property overrides detectable | Advanced fingerprinters detect protection | Use `Object.defineProperty` on prototypes; accept that sophisticated scripts can detect |
| eTLD+1 parsing edge cases | Cookie blocking too aggressive/permissive | Use PSL (industry standard), test with common edge cases (co.uk, blogspot.com) |
| AudioContext farbling audible | Music/video quality degraded | Fudge factor is 0.00000014-0.00000214% — well below audible threshold |
| OAuth breaks with strict cookie blocking | Users can't log in to sites | Built-in exception list for Google, Microsoft, Apple, Facebook, Stripe, PayPal |
| Script injection timing | Protection not active for early scripts | Use `OnContextCreated` (earliest possible point) — same proven pattern as scriptlet injection |

---

## Architecture Decisions Log

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Farbling vs blocking | Farbling (Brave-style) | More web-compatible; doesn't break canvas apps; sufficient for tracking prevention |
| Per-session seed | HMAC-SHA256(session_token, eTLD+1) | Consistent within session (doesn't break cross-tab state), different across sessions |
| eTLD+1 extraction | Public Suffix List | Industry standard, same as Chromium internally uses |
| Canvas farble scope | Small canvases only (<256x256) | Fingerprinting probes use small canvases; games/editors use large canvases |
| Cookie blocking default | Block all third-party | Matches Safari/Firefox/Brave; strong privacy stance |
| Navigator spoofing | Fixed generic values | Simple, effective, minimal breakage |

---

## Post-Sprint Tasks

1. Update `development-docs/browser-core/CLAUDE.md` with Sprint 12 completion
2. Update `00-SPRINT-INDEX.md` status
3. Update root `CLAUDE.md` Key Files table (add DomainUtils, FingerprintProtection)
4. Test against full Thorough basket (30-45 min) — focus on auth + canvas-heavy sites
5. Run Cover Your Tracks test and document entropy score before/after
6. Consider: Should we bundle a pre-built PSL for offline use?
7. Consider: Auto-update PSL periodically (it changes monthly)?

---

*This document was generated based on research into Brave's farbling system, Firefox's Resist Fingerprinting, Chrome's third-party cookie handling, CEF's CefCookieAccessFilter API, and the EFF's Cover Your Tracks methodology.*
