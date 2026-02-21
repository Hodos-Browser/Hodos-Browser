# Working Notes — Research & Decisions Backlog

**Purpose**: Track things that come up during sprints but shouldn't derail current work. Each item needs research, a decision, and eventual implementation.

---

## 1. Proprietary Codec Support (AAC, H.264, MP3)

**Discovered**: Sprint 2 testing — `zedvibe.org` fails with "AAC audio codec not supported"

**Problem**: Default CEF binaries from `cef-builds.spotifycdn.com` ship without proprietary codecs (AAC, H.264, MP3) due to patent licensing. Only open codecs included (VP8, VP9, AV1, Opus, Vorbis). This is a compile-time flag (`proprietary_codecs=true`, `ffmpeg_branding="Chrome"`), not a runtime toggle.

**Impact**: Most audio/video streaming sites broken. YouTube works (VP9/AV1 fallback) but most other sites don't.

**Options to research**:
- Does Spotify CDN offer a CEF 136 build variant with proprietary codecs enabled?
- Can we swap in a proprietary-enabled binary without rebuilding the CEF wrapper?
- Should we build CEF from source with the flag? (Hours of compile time, ~50GB disk)
- Patent/licensing implications for distributing a browser with proprietary codecs

**Decision needed**: Which option, and when to implement (before or after MVP launch)

---

## 2. CEF Binary Strategy — Prebuilt vs Source Build

**Context**: Currently using prebuilt CEF binaries from `cef-builds.spotifycdn.com`. Need to decide on long-term strategy.

**Questions to research**:
- **Version selection**: Always use newest CEF/Chromium? Pin to LTS? What's the upgrade cadence? How do we track security patches?
- **Prebuilt vs source**: Prebuilt is fast but limited (no proprietary codecs, no custom patches). Source build gives full control but is heavy (~50GB, hours to compile). What do other CEF-based browsers (Brave, Vivaldi, etc.) do?
- **Wrapper compatibility**: When upgrading CEF versions, what breaks? Does the `libcef_dll_wrapper` need rebuilding? Do API signatures change between versions?
- **Testing**: How to validate a new CEF version doesn't break existing functionality? Need a regression test checklist (media playback, WebRTC, SSL, permissions, cookie blocking, HTTP interception, V8 injection).
- **Full Chromium build**: Is it worth building Chromium directly (like Brave does) instead of using CEF? Pros: full control, all codecs, custom patches. Cons: massive build infrastructure, ongoing merge burden.

---

## 3. Production Installation & Distribution

**Questions to research**:
- **Installer format**: MSI? NSIS? WiX? MSIX (Windows Store)? What do other browsers use?
- **Code signing**: Need a code signing certificate for Windows (SmartScreen warnings without one). Cost, providers, process.
- **Installation directory**: `Program Files` vs `AppData\Local`? Chrome installs per-user in AppData. Affects auto-update permissions.
- **Uninstaller**: What needs cleanup? Registry entries, file associations, default browser registration.
- **macOS**: DMG with drag-to-Applications? Notarization with Apple? Separate research item.

---

## 4. Auto-Update Mechanism

**Questions to research**:
- **Update delivery**: How to push updates to users who have downloaded the browser?
- **Update protocol**: Squirrel (Electron-style)? Google's Omaha/Sparkle? Custom solution? What does Brave use?
- **Differential updates**: Full download vs delta patches? CEF binaries are ~100MB+.
- **Update cadence**: Tied to CEF/Chromium releases? Independent app updates?
- **Rollback**: If an update breaks something, can users roll back? How?
- **Deployment infrastructure**: Dedicated GitHub repo for releases? S3/CDN for binaries? GitHub Releases has file size limits (2GB).
- **Channels**: Stable, beta, canary? Worth the complexity for MVP?

---

## 5. Open Source Dependency Updates (CEF, Ad-blocker, etc.)

**Context**: We depend on CEF binaries, and plan to use Brave's open-source ad-blocker (`adblock-rust`). These dependencies will need periodic updates.

**Questions to research**:
- **CEF upgrades**: How to upgrade without breaking the wrapper, HTTP interception, V8 injection, or other customizations? What's the typical breakage surface between CEF versions?
- **Ad-blocker filter lists**: How are EasyList/EasyPrivacy/etc. updated? At build time? At runtime (download on schedule)? Where are they stored? How does Brave handle this?
- **Data migration**: When upgrading CEF, does the profile data format (`%APPDATA%/HodosBrowser/Default/`) change? Could an upgrade corrupt cookies, history, permissions, or bookmarks?
- **Wallet DB**: Our SQLite wallet DB has its own migration system. CEF upgrades shouldn't affect it, but need to verify isolation.
- **Breaking changes**: How to detect if a CEF API we use was removed or changed signature? Can we pin to a CEF API version?
- **Dependency pinning**: Lock exact versions in build scripts? Use ranges? How to balance security patches vs stability?

---

## 6. User-Agent String

**Discovered**: Sprint 2 — discussing Google login compatibility

**Context**: Google and other sites check the user-agent string to decide whether to allow login. CEF includes a Chrome-like UA but may append custom identifiers.

**Questions to research**:
- What is our current UA string? (Check in DevTools or debug logs)
- Does it include "HodosBrowser" or other non-standard identifiers that trigger blocks?
- Should we match Chrome's UA exactly? Legal/ethical considerations?
- Chrome is deprecating the UA string in favor of Client Hints — do we need to support `Sec-CH-UA` headers?

---

*Add new items below as they come up during sprints.*
