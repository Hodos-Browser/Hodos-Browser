# Working Notes — Research & Decisions Backlog

**Purpose**: Track things that come up during sprints but shouldn't derail current work. Each item needs research, a decision, and eventual implementation.

**Last reorganized**: 2026-03-01

---

# PART A: CRITICAL — PRODUCTION BUILD DECISIONS

These items are interrelated and must be resolved together before the first production release.

---

## A1. Proprietary Codec Support (H.264, AAC, MP3)

**Status**: 🔴 BLOCKING — Single biggest UX issue for the browser

**Discovered**: Sprint 2 testing — `zedvibe.org` fails with "AAC audio codec not supported"

### The Problem

Default CEF binaries from `cef-builds.spotifycdn.com` ship **without proprietary codecs** (H.264, AAC, MP3) due to patent licensing. Only open codecs included (VP8, VP9, AV1, Opus, Vorbis). This is a **compile-time flag** (`proprietary_codecs=true`, `ffmpeg_branding="Chrome"`), not a runtime toggle.

### Real-World Impact (confirmed 2026-02-27)

| Site | Static Images | Video | Root Cause |
|------|--------------|-------|------------|
| **x.com** | ✅ JPEG/PNG work | ❌ No playback | x.com converts ALL animated GIFs to H.264 MP4. HLS via MSE requires H.264. |
| **Reddit** | ✅ Thumbnails work | ❌ Spinner, never plays | Reddit video uses H.264 MP4. No fallback. |
| **YouTube** | ✅ All work | ✅ Plays fine | YouTube has VP9/AV1 fallback. |
| **Twitch** | ✅ | ❌ Many streams | Some streams H.264 only |
| **Instagram** | ✅ | ❌ | H.264 only |
| **TikTok** | ✅ | ❌ | H.264 only |
| **News sites** | ✅ | ❌ | Embedded video typically H.264 |

**x.com "intermittent images" explained**: Static JPEG/PNG load fine. But x.com converts animated GIFs to MP4 rendered as `<video>` elements — these appear as broken "images."

### Diagnostic Command

```javascript
// Run in DevTools console on any page
const v = document.createElement('video');
console.log('H.264:', v.canPlayType('video/mp4; codecs="avc1.42E01E"'));
console.log('AAC:', v.canPlayType('audio/mp4; codecs="mp4a.40.2"'));
console.log('VP9:', v.canPlayType('video/webm; codecs="vp9"'));
console.log('AV1:', v.canPlayType('video/webm; codecs="av01.0.01M.08"'));
// H.264 and AAC will be empty string (unsupported), VP9/AV1 will be "probably"
```

### Options (Requires Research — See Part E)

| Option | Effort | Result |
|--------|--------|--------|
| Find prebuilt CEF with codecs | Low | Uncertain if exists |
| Swap ffmpeg binary only | Medium | May not work (compile-time) |
| Build CEF from source | High (~50GB, hours) | Full control |
| Build full Chromium | Very High | Maximum control |
| License codecs commercially | $$$ | Avoid patent issues |

**Decision needed**: Which option, tied to Production Build decision (A2)

---

## A2. CEF Binary Strategy — Prebuilt vs Source Build

**Status**: 🟡 DECISION NEEDED — Directly impacts A1 (codecs)

**Context**: Currently using prebuilt CEF 136 binaries from `cef-builds.spotifycdn.com`.

### Key Questions

1. **Codecs**: Prebuilt binaries lack proprietary codecs. Source build required for H.264/AAC.

2. **Version selection**: 
   - Always newest CEF/Chromium?
   - Pin to LTS?
   - What's the security patch cadence?

3. **Upgrade path**: 
   - When upgrading CEF, what breaks?
   - Does `libcef_dll_wrapper` need rebuilding?
   - Do API signatures change?

4. **Full Chromium alternative**: Is building Chromium directly (like Brave) better than CEF?

### Options Comparison (Requires Research — See Part E)

| Approach | Codecs | Build Time | Disk Space | Customization | Maintenance |
|----------|--------|------------|------------|---------------|-------------|
| **Prebuilt CEF** | ❌ None | Minutes | ~500MB | Limited | Low |
| **CEF from source** | ✅ Yes | 2-4 hours | ~50GB | Medium | Medium |
| **Full Chromium** | ✅ Yes | 4-8 hours | ~100GB | Maximum | High |

**Decision needed**: Production build approach — see Part E deep dive

---

## A3. Production Installation & Distribution

**Status**: 🟡 PLANNING NEEDED — Installer plan required before G5 (default browser) setting makes sense.

**Note (2026-03-01)**: Settings Sprint G5 ("Set as Default Browser") is deferred until an installer exists. The settings button just opens `ms-settings:defaultapps`, but Hodos won't appear in the browser list without protocol handler registration, file associations, and registry entries — all installer tasks. Need to create a dedicated installer sprint/plan.

### Installer Format Options

| Format | Platform | Pros | Cons |
|--------|----------|------|------|
| **NSIS** | Windows | Free, flexible, widely used | Dated tooling |
| **WiX** | Windows | MSI standard, enterprise-friendly | XML complexity |
| **MSIX** | Windows | Modern, Windows Store ready | App signing requirements |
| **Squirrel** | Windows | Auto-update built-in | Electron-focused |
| **DMG** | macOS | Standard user expectation | Notarization required |

### Code Signing

- **Windows**: Need EV code signing certificate for SmartScreen trust
  - Cost: ~$300-500/year
  - Providers: DigiCert, Sectigo, GlobalSign
  - Without it: "Windows protected your PC" warning

- **macOS**: Need Apple Developer ID ($99/year)
  - Notarization required for Gatekeeper
  - Without it: "Cannot be opened" error

### Installation Directory

| Location | Pros | Cons |
|----------|------|------|
| `Program Files` | Standard, admin install | Requires elevation, harder auto-update |
| `AppData\Local` | Per-user, no elevation | Chrome does this |

### Uninstaller Requirements

- Remove files from install directory
- Remove `%APPDATA%/HodosBrowser` (optional — user choice)
- Remove registry entries (file associations, default browser)
- Remove Start Menu / Desktop shortcuts

---

## A4. Auto-Update Mechanism

**Status**: 🟡 PLANNING NEEDED

### Options

| System | Used By | Pros | Cons |
|--------|---------|------|------|
| **Squirrel.Windows** | Electron apps | Delta updates, simple | Windows-only |
| **Omaha** | Chrome, Edge | Proven at scale | Complex setup |
| **Sparkle** | macOS apps | Standard for Mac | Mac-only |
| **Custom** | — | Full control | Development cost |
| **GitHub Releases** | Small projects | Free, simple | 2GB file limit |

### Considerations

- **Delta vs Full**: CEF binaries are ~100MB+. Delta patches save bandwidth.
- **Channels**: Stable, Beta, Canary? Worth complexity for MVP?
- **Rollback**: Can users roll back a bad update?
- **Forced updates**: Security patches should auto-apply

### Infrastructure Needed

- Static file hosting (S3, CloudFront, GitHub Pages)
- Update manifest endpoint
- Version checking logic in browser

---

# PART B: PROFILE & MULTI-INSTANCE

---

## B1. Critical Bug: Cookie/Cache Isolation Broken

**Status**: 🔴 BUG — Must fix before production

**Discovered**: Sprint 9d testing (2026-02-25)

### The Problem

In `cef_browser_shell.cpp`, `CefSettings.cache_path` is hardcoded to `"HodosBrowser\\Default"` at line 2115 **BEFORE** `CefInitialize()` at line 2276. The `--profile=` argument is parsed **AFTER** `CefInitialize()` at line 2290.

**Impact**: CEF's cookie store, localStorage, IndexedDB, and HTTP cache ALL go to the `Default` directory regardless of profile. Logging into x.com on Profile A means you're also logged in on Profile B.

### The Fix

```cpp
// Parse --profile= FIRST (before CefInitialize)
std::string profileId = ProfileManager::ParseProfileArgument(GetCommandLineW());
ProfileManager::GetInstance().Initialize(user_data_path);
ProfileManager::GetInstance().SetCurrentProfileId(profileId);
std::string profile_cache = ProfileManager::GetInstance().GetCurrentProfileDataPath();

CefString(&settings.root_cache_path).FromString(user_data_path);
CefString(&settings.cache_path).FromString(profile_cache);  // Profile-specific!

CefInitialize(main_args, settings, app, nullptr);
```

### Test Checklist (after fix)

1. Log into x.com on Profile A → verify logged in
2. Open Profile B → navigate to x.com → verify NOT logged in
3. Log into x.com with different account on Profile B
4. Close/reopen Profile B → verify session persists
5. Verify Profile A's session unaffected
6. Verify history is separate (visit youtube on A, check Ctrl+H on B)
7. Stress test: browse actively on both profiles simultaneously

---

## B2. Multi-Window Same-Profile Support

**Status**: 🟢 POST-MVP — Current lock behavior is safe

**Current behavior**: Profile lock prevents second instance of same profile. "Profile Locked" error appears.

**Target UX (Chrome model)**: Second window opens in existing process, shares cookies/tabs.

### Implementation Phases

**Phase 1: Multi-window within same process** (Medium effort)
- `WindowManager` alongside `TabManager`
- Each window has own header, tab bar, tabs
- All windows share CefInitialize, cookies, history
- Ctrl+N opens new window

**Phase 2: Single-instance detection** (Medium effort)  
- Named pipe at `<profile_dir>/hodos_instance_pipe`
- New launch sends `{"action": "new_window"}` to existing instance
- Existing instance creates window, new process exits cleanly

**Phase 3: Tab drag-out** (High effort, post-MVP)
- Detect tab drag beyond window bounds
- Create new HWND, re-parent browser
- Complex — CEF doesn't natively support re-parenting

---

## B3. Shared Services Across Profile Instances

**Context**: Multiple profile instances share:

| Service | Port | Concern |
|---------|------|---------|
| Wallet backend | 3301 | First instance starts it; subsequent must detect |
| Adblock engine | 3302 | Same — check if port already bound |
| Frontend dev server | 5137 | Dev only, read-only, no conflict |

**Action**: Add port-in-use detection before starting services.

---

# PART C: AD BLOCKING

---

## C1. Architecture Summary

**Implemented**: Sprint 8 (2026-02-23)

- **Separate process** at `adblock-engine/` on port 3302
- **C++ starts it** via `CreateProcessA` + Job Object
- **Non-critical**: If it fails, browsing continues unblocked

### Crate Version Pinning (Critical)

```toml
adblock = "=0.10.3"       # Last compatible with stable Rust 1.85.1
rmp = "=0.8.14"           # Required for rmp-serde compat
actix-web = "=4.11.0"     # 4.13+ requires Rust 1.88
default-features = false   # Enables Send+Sync for RwLock<Engine>
```

### C++ Integration

- `AdblockCache.h`: URL→bool cache + sync WinHTTP POST to `/check`
- `AdblockBlockHandler`: `CefResourceRequestHandler` returning `RV_CANCEL`
- Hook in `GetResourceRequestHandler()` BEFORE wallet interception

---

## C2. Entity-Aware Blocking (disconnect.me)

**Implemented**: 2026-02-27

**Problem**: EasyList blocks same-org CDN domains (e.g., `pbs.twimg.com` on `x.com`).

**Solution**: `EntityMap` with 1859 organizations, 9500+ domains from disconnect.me `entities.json`. If URL domain and source domain share entity → allow (first-party CDN).

**License**: CC BY-NC-SA 4.0 — include attribution in About page.

---

## C3. Exception List Auto-Update (Post-MVP)

**Current**: `hodos-unbreak.txt` embedded via `include_str!()`, local override at `%APPDATA%/HodosBrowser/adblock/hodos-unbreak.txt`

**Enhancement**: Add to filter list auto-update cycle (6-hour background task):
1. Host at stable URL (GitHub Pages or CDN)
2. Add to `FILTER_LISTS` array
3. Include `! Expires: 7 days` header

---

## C4. adblock-rust Quirks

- **`$elemhide` NOT supported** in v0.10.3 — only `$generichide` works
- **`cosmetic_resources()` fix**: Returns hostname-specific CSS even when `generichide=true`. Our wrapper explicitly returns empty when `generichide=true`.
- **`serialize()`** — NOT `serialize_raw()` (that's newer API)

---

# PART D: OTHER TECHNICAL ITEMS

---

## D1. CEF Wrapper (`libcef_dll_wrapper`) Notes

**Discovered**: Sprint 4 — `CMakeCache.txt` contained stale path, wrapper hadn't been rebuilt in 5 months.

### Key Lessons

- **Never let CMakeCache go stale**: If project moves/clones to new location, delete CMakeCache and reconfigure
- **macOS wrapper rebuild required**: Static `.a` is platform-specific. Source is cross-platform C++.
- **macOS CMakeLists.txt changes needed**:
  - Wrap `MSVC_RUNTIME_LIBRARY` in `if(MSVC)`
  - Wrap `WIN32_LEAN_AND_MEAN`/`NOMINMAX` in `if(WIN32)`
  - Add macOS deployment target

### CEF Find() API Non-Functional

Even after wrapper rebuild, `CefBrowserHost::Find()` doesn't trigger callbacks. Sprint 4 used JavaScript fallback. May be CEF 136 regression — test with cefclient sample.

---

## D2. CEF Built-In Menu Command IDs — Auto-Disable Quirk

**Discovered**: Sprint 5

**Problem**: When building custom context menu with `model->Clear()` then re-adding CEF's built-in command IDs (`MENU_ID_BACK`, `MENU_ID_COPY`, etc.), CEF auto-disables them.

**Root Cause**: CEF's internal command state manager gets out of sync after `Clear()`.

**Fix**: Use custom command IDs in `MENU_ID_USER_FIRST` (26500+) range and handle ALL commands manually. Navigation: `browser->GoBack()`. Editing: `frame->ExecuteJavaScript("document.execCommand('copy')")`.

---

## D3. User-Agent String

**Context**: Some sites check UA to decide login compatibility.

**Questions**:
- What is our current UA string?
- Does it include "HodosBrowser" that triggers blocks?
- Should we match Chrome's UA exactly?
- Do we need Client Hints (`Sec-CH-UA`) support?

---

## D4. Open Source Dependency Updates

**Concerns**:
- **CEF upgrades**: What breaks? Wrapper rebuild? API changes?
- **Filter list updates**: Runtime download vs build-time bundling
- **Data migration**: Does CEF profile format change between versions?
- **Wallet DB**: Our SQLite has its own migrations, isolated from CEF

---

## D5. Settings Functionality

**Status**: Tracked in `development-docs/Settings_Sprints/00-SPRINT-INDEX.md` (2026-03-01)

Detailed sprint plans created for each non-functional setting. Priority order agreed:
1. PS1 (shield toggles — broken UI)
2. D1 (download settings)
3. G1 (search engine — DDG default + suggest swap)
4. G4 (new tab page)
5. G2 (session restore with lazy tab loading)
6. PS3 (clear on exit)

Deferred: G3 (bookmark bar — remove placeholder), G5 (default browser — needs installer).

---

# PART E: DEEP DIVE — CODEC ISSUES & PRODUCTION BUILD

**Research completed**: 2026-03-01

---

## E1. Codec Issues Deep Dive

### Why Codecs Are Missing

CEF (Chromium Embedded Framework) inherits Chromium's open-source licensing philosophy. Google Chrome includes proprietary codecs (H.264, AAC, MP3) because Google pays the patent licensing fees. Chromium and CEF exclude them by default to avoid patent liability for downstream users.

**The key flags** (compile-time, not runtime):
```
proprietary_codecs=true
ffmpeg_branding="Chrome"
```

Without these flags, CEF includes only royalty-free codecs: VP8, VP9, AV1, Opus, Vorbis, Theora.

### The Patent Licensing Situation

**MPEG-LA / Via Licensing** administers H.264 (AVC) patents:

| Usage | Royalty Status |
|-------|----------------|
| Free internet video delivery | **Free** (extended "in perpetuity" since 2010) |
| First 100,000 units distributed | **Free** (threshold) |
| Above 100,000 units | Royalties apply (~$0.10-0.20 per unit, caps apply) |
| End-user decoder use | Covered by OS/device licenses |

**Key insight**: For a browser under 100,000 installations, there's effectively no licensing cost. Above that, royalties kick in but have annual caps.

### Option Analysis

#### Option 1: Find Prebuilt CEF with Codecs
**Verdict**: ❌ Does not exist

The Spotify CEF builds (cef-builds.spotifycdn.com) are built WITHOUT proprietary codecs. No public source offers prebuilt CEF with H.264/AAC enabled. This is explicitly stated in CEF GitHub issues.

#### Option 2: Swap FFmpeg Binary Only
**Verdict**: ❌ Does not work

The codec support is determined at **Chromium compile time**, not by the FFmpeg binary alone. CEF's media pipeline has conditional code paths that are compiled in/out based on the flags. You cannot drop in a different FFmpeg DLL and get codec support.

#### Option 3: Build CEF from Source with Codecs
**Verdict**: ✅ Recommended approach

**Requirements**:
- Windows: Visual Studio 2022, Windows SDK with Debugging Tools
- Disk: ~50-60GB for source + build artifacts
- RAM: 16GB minimum, 32GB recommended
- Time: First build 3-6 hours, subsequent builds 30-60 minutes
- Tools: depot_tools, Python 3.9+, Git

**Build flags** (in update.bat):
```batch
set GN_DEFINES=is_component_build=false proprietary_codecs=true ffmpeg_branding=Chrome is_official_build=true
```

**Effort**: Medium-high. Well-documented process, but requires build infrastructure.

#### Option 4: Build Full Chromium (like Brave)
**Verdict**: ⚠️ Overkill for now, consider later

Brave forks Chromium directly, applying patches for privacy/features. This gives maximum control but:
- Build time: 4-8+ hours
- Disk: ~100GB
- Ongoing merge burden with upstream Chromium
- Only worthwhile if we need deep Chromium modifications

**Recommendation**: Start with CEF source build. Move to full Chromium only if CEF limitations become blocking.

#### Option 5: Cisco OpenH264
**Verdict**: ⚠️ Partial solution (WebRTC only)

Cisco's OpenH264 is BSD-licensed, with Cisco paying MPEG-LA royalties. It's used in Firefox and WebRTC. However:
- Primarily for **encoding** (WebRTC video calls)
- Chromium uses FFmpeg for **decoding** (video playback)
- Would help WebRTC H.264, but not general media playback

Not a solution for our primary use case (video playback on x.com, Reddit, etc.).

### Recommended Solution: CEF Source Build

**Phase 1: Build Infrastructure Setup**
1. Set up Windows build machine (or CI runner) with 64GB+ disk
2. Install Visual Studio 2022, Windows SDK, depot_tools
3. Clone CEF automate-git.py scripts
4. Document the build process in our repo

**Phase 2: Initial Codec-Enabled Build**
1. Configure with proprietary codec flags
2. Build CEF (first time: 3-6 hours)
3. Test with x.com, Reddit, Twitch
4. Verify `canPlayType()` returns "probably" for H.264/AAC

**Phase 3: Integration with Hodos**
1. Replace prebuilt CEF binaries with our custom build
2. Update libcef_dll_wrapper to match
3. Test all existing functionality (HTTP interception, V8 injection, etc.)
4. Validate Widevine DRM if needed (Netflix, Spotify)

**Licensing**: Under 100K users, we're in the free tier. Add MPEG-LA attribution to About page. If we grow beyond 100K, budget for licensing (~$20K-50K/year based on volume).

---

## E2. Production Build Options Deep Dive

### The Three Paths

| Approach | Codecs | Build Time | Maintenance | Customization |
|----------|--------|------------|-------------|---------------|
| **A) Prebuilt CEF** | ❌ None | Minutes | Low | Limited |
| **B) CEF from Source** | ✅ Yes | 3-6 hours | Medium | Medium |
| **C) Full Chromium** | ✅ Yes | 6-12 hours | High | Maximum |

### Detailed Analysis

#### Path A: Continue with Prebuilt CEF (Current State)
**What it means**: Keep using binaries from cef-builds.spotifycdn.com

**Pros**:
- Zero build infrastructure needed
- Fast iteration (just download new version)
- Well-tested binaries

**Cons**:
- No proprietary codecs (BLOCKING)
- No custom patches possible
- Dependent on Spotify's build schedule

**Verdict**: ❌ Not viable for production (codec issue)

#### Path B: CEF from Source (Recommended)
**What it means**: Build CEF ourselves using automate-git.py

**Pros**:
- Proprietary codecs enabled
- Can pin to specific Chromium version for stability
- Can apply minor patches if needed
- Reasonable build time (3-6 hours first, 30-60 min incremental)
- CEF API remains stable between versions

**Cons**:
- Need build infrastructure (~50GB disk, CI time)
- Must track CEF releases for security patches
- libcef_dll_wrapper must be rebuilt when upgrading

**Build Process Summary**:
```bash
# 1. Setup (one-time)
mkdir cef_build && cd cef_build
git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git
# Download automate-git.py from CEF wiki

# 2. Configure (update.bat on Windows)
set GN_DEFINES=proprietary_codecs=true ffmpeg_branding=Chrome is_official_build=true
python automate-git.py --download-dir=. --branch=6533 --no-debug-build

# 3. Build
# Creates Release/ directory with libcef.dll, chrome_elf.dll, etc.
```

**Version Strategy**:
- Pin to CEF branch that matches our current version (136 = branch 6533)
- Upgrade quarterly or when security patches released
- Test each upgrade against regression suite

**Verdict**: ✅ Best balance of effort vs. capability

#### Path C: Full Chromium Fork (Brave-style)
**What it means**: Fork chromium/chromium, apply patches, build entire browser

**Pros**:
- Maximum control over everything
- Can modify any Chromium behavior
- How Brave, Edge, Opera do it

**Cons**:
- Massive build (100GB+, 6-12 hours)
- Continuous merge burden with upstream
- Need dedicated team for Chromium maintenance
- CEF abstraction provides most of what we need

**When to consider**:
- If CEF's CefBrowserHost API becomes limiting
- If we need deep modifications (custom network stack, etc.)
- If we have a team dedicated to browser engine work

**Verdict**: ⚠️ Overkill now, reconsider in 12-18 months if needed

---

## E3. Do Codec Issues and Build Decision Overlap?

**YES — They are the same decision.**

The codec problem can ONLY be solved by building from source (CEF or Chromium). Therefore:

1. **We must build CEF from source** to get codecs
2. Once we're building from source, we get:
   - Proprietary codecs (H.264, AAC, MP3)
   - Ability to pin versions for stability
   - Option to apply patches
   - Control over build flags

**The unified recommendation**:

| Phase | Action | Timeline |
|-------|--------|----------|
| **Now** | Set up CEF source build infrastructure | 1-2 days |
| **Now** | Build CEF 136 with proprietary codecs | 3-6 hours |
| **Now** | Replace prebuilt binaries, test | 1 day |
| **MVP** | Ship with custom CEF build | - |
| **Post-MVP** | Establish quarterly CEF upgrade cadence | Ongoing |
| **Future** | Evaluate full Chromium if CEF limits us | 12-18 months |

---

## E4. Immediate Action Plan

### Step 1: Build Machine Setup
```
- Windows 10/11 with 100GB free disk
- Visual Studio 2022 (Desktop C++ workload)
- Windows SDK with "Debugging Tools for Windows"
- Python 3.9+
- Git
```

### Step 2: First CEF Build with Codecs
```bash
# Create build directory
mkdir C:\cef_build
cd C:\cef_build

# Get depot_tools
git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git
set PATH=C:\cef_build\depot_tools;%PATH%

# Get automate-git.py
# Download from: https://bitbucket.org/chromiumembedded/cef/raw/master/tools/automate/automate-git.py

# Create update.bat
set GN_DEFINES=is_component_build=false proprietary_codecs=true ffmpeg_branding=Chrome is_official_build=true
python automate-git.py --download-dir=C:\cef_build --branch=6533 --no-debug-build --x64-build
```

### Step 3: Integration
1. Copy built binaries to `cef-binaries/` in Hodos repo
2. Rebuild libcef_dll_wrapper against new headers
3. Test: Run codec diagnostic in DevTools
4. Test: x.com videos, Reddit videos, Twitch streams

### Step 4: CI/CD (Later)
- Set up GitHub Actions or dedicated build server
- Automate CEF builds on new releases
- Store binaries in GitHub Releases or S3

---

## E5. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Build fails | Medium | High | Follow CEF wiki exactly, ask on ceforum |
| Wrapper API mismatch | Low | Medium | Pin exact CEF version, rebuild wrapper |
| Licensing issue >100K users | Low (for now) | Medium | Budget for licensing if growth happens |
| Security patch delay | Medium | High | Subscribe to CEF releases, quarterly updates |
| Widevine DRM not working | Medium | Medium | Test early, may need Google relationship |

---

*This deep dive provides the technical foundation for the production build decision. Recommend proceeding with CEF source build (Path B) as the immediate next step.*

---

# PART F: DEBUGGING REFERENCE

---

## F1. x.com Media Debugging — Lessons Learned (2026-02-27)

### What was fixed

1. **Cosmetic CSS Phase 1 suppression**: `cosmetic_resources()` returned selectors even when `generichide=true`
2. **`$elemhide` → `$generichide`**: adblock-rust 0.10.3 only supports `$generichide`
3. **Entity-aware blocking**: disconnect.me entity list prevents same-org CDN blocking

### What was NOT the problem

All ruled out:
- Network blocking (debug log showed 200 OK)
- Cookie blocking (only blocks cookies, not requests)
- CORS (regular `<img>` tags don't require CORS)
- Fingerprint protection (x.com in `IsAuthDomain()`)
- Scriptlet injection (x.com has `#@#+js()` exception)

### The actual root cause

**Missing proprietary codecs** — x.com converts GIFs to H.264 MP4. Without codec, `<video>` elements appear as missing "images."

---

*Add new items at the end of the appropriate section.*
