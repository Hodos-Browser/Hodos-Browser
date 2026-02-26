# Browser Profile & Account Management Research

**Created**: 2026-02-25
**Purpose**: Research how major browsers handle user account login, profiles, and multi-account UX to inform Hodos Browser Sprint 9 design.

---

## Executive Summary

There are **two distinct concepts** that browsers handle:

| Concept | What It Is | Examples |
|---------|------------|----------|
| **Browser Profile** | Local data container (bookmarks, history, cookies, settings) | Chrome Profile, Firefox Profile, Safari Profile |
| **Browser Account Sync** | Cloud sync tied to an identity provider | Google Account (Chrome), Firefox Account, Apple ID (Safari), Microsoft Account (Edge) |

**Key Insight**: You can have a profile without syncing, and you can switch profiles without logging out of websites within that profile.

---

## Browser-by-Browser Analysis

### Chrome

**Profile Concept:**
- Profiles are local data silos stored in `User Data\Profile 1\`, `User Data\Profile 2\`, etc.
- Each profile has its own: bookmarks, history, passwords, cookies, extensions, settings
- Profiles are visually distinguished by: name, avatar/photo, color theme
- Profile picker appears on startup (optional setting: "Show on startup")

**Account Sync:**
- Signing into Chrome with a Google Account enables cross-device sync
- Profile ≠ Google Account (you can have a profile without signing in)
- When signed in: bookmarks, passwords, history, settings sync to Google cloud

**Multi-Account UX:**
- Profile icon in top-right corner (avatar bubble)
- Click → dropdown showing all profiles + "Add" + "Manage profiles"
- "Guest mode" for temporary browsing (no persistence)
- Each Chrome window shows which profile it belongs to (color bar at top)

**Website Logins (separate from browser account):**
- x.com login, YouTube login, etc. are stored in cookies per-profile
- Signing out of Chrome ≠ signing out of websites
- Within a profile, all tabs share the same cookies/sessions

**Instance Behavior:**
- Multiple windows can be open with SAME profile
- Multiple windows can be open with DIFFERENT profiles simultaneously
- Each profile's windows are visually distinguished

### Firefox

**Profile Concept:**
- Profiles stored in separate folders under `~/.mozilla/firefox/`
- Very similar to Chrome: each profile has isolated data
- Profile Manager accessed via `about:profiles` or `-P` flag
- Default profile loads automatically unless you configure otherwise

**Account Sync (Firefox Account):**
- Optional Mozilla account for cross-device sync
- Syncs: bookmarks, history, passwords, tabs, add-ons, settings

**Multi-Account Containers (Firefox-specific feature):**
- Unique to Firefox: containers WITHIN a profile
- Each container has isolated cookies/sessions
- Example: "Work" container vs "Personal" container vs "Shopping"
- Same profile, but different website sessions per container
- Tabs are color-coded by container

**UX Elements:**
- Profile picker at startup (if multiple profiles exist)
- Container tabs have colored underline
- Right-click "Open in Container" option

### Safari

**Profile Concept (Safari 17+):**
- Profiles introduced in Safari 17 (late 2023)
- Each profile: separate history, cookies, website data, Tab Groups, favorites
- Distinguished by: name, symbol (icon), color
- Default profile called "Personal"

**Account Sync:**
- Tied to Apple ID
- Profiles sync between devices automatically if signed into same Apple ID

**UX Elements:**
- Toolbar button shows current profile (name + symbol + color)
- Click → switch profiles or open new window in different profile
- Focus integration: can auto-switch profiles when activating a Focus mode

**Unique Features:**
- Extensions available to all profiles but managed (on/off) per-profile
- "Open links with Profile" setting: certain website links always open in specific profile

### Edge

**Profile Concept:**
- Very similar to Chrome (both Chromium-based)
- Profiles stored in `User Data\Profile 1\`, etc.
- Each profile is a separate data silo

**Account Sync:**
- Tied to Microsoft Account (personal) or Microsoft 365 (work/school)
- Enterprise users can have managed work profiles

**UX Elements:**
- Profile avatar in top-right
- Profile picker modal
- Color-coded profile bars in title bar

---

## Key Design Patterns Across All Browsers

### 1. Profile Picker at Launch
| Browser | Shows by Default? | Can Disable? |
|---------|-------------------|--------------|
| Chrome | Optional ("Show on startup" checkbox) | Yes |
| Firefox | Only if multiple profiles | Yes |
| Safari | No (single window, switch via toolbar) | N/A |
| Edge | Optional | Yes |

### 2. Profile Indicator in UI
All browsers show current profile via:
- Top-right avatar/icon
- Color theme (Chrome, Safari, Edge)
- Name on hover or in dropdown

### 3. Cross-Tab Session Sharing
**Within a profile:** All tabs share cookies/sessions (logged into YouTube in one tab = logged in everywhere)

**Across profiles:** Complete isolation. Logging into x.com in Profile A doesn't affect Profile B.

### 4. Multi-Window Behavior
- Same profile can have multiple windows (sessions shared)
- Different profiles require different windows (cannot have Profile A and B tabs in same window)

---

## Account Types to Consider for Hodos

| Account Type | Provider | Use Case |
|--------------|----------|----------|
| **Google** | google.com | YouTube, Gmail, Drive, Search personalization |
| **Microsoft** | microsoft.com | Outlook, Office 365, OneDrive |
| **Apple** | apple.com | iCloud (less relevant for Windows/Linux users) |
| **BSV/Hodos Wallet** | Local | Our native wallet identity (already handled) |
| **None/Guest** | N/A | Privacy-focused users who don't want to sync |

**Recommendation:** Focus on Google first (highest usage), then Microsoft. Apple ID sync is low priority for Windows-first browser.

---

## Recommended UX for Hodos Browser

### First Launch / Setup Wizard

```
┌─────────────────────────────────────────────────────────────┐
│              Welcome to Hodos Browser                        │
│                                                              │
│  Would you like to import data from another browser?         │
│                                                              │
│  [Chrome - 3 profiles detected]    [Import Selected]         │
│    ☑ Work (john@company.com)                                │
│    ☑ Personal (john.doe@gmail.com)                          │
│    ☐ Gaming                                                  │
│                                                              │
│  [Brave - 1 profile detected]      [Import Selected]         │
│    ☑ Default                                                 │
│                                                              │
│  [Skip Import]                                               │
│                                                              │
│  ─────────────────────────────────────────────────────────   │
│                                                              │
│  Create your Hodos profile:                                  │
│  Name: [_______________]                                     │
│  Color: 🟡 🔵 🟢 🔴 🟣                                        │
│                                                              │
│  [Continue]                                                  │
└─────────────────────────────────────────────────────────────┘
```

### Profile Indicator (Header Bar)

```
┌──────────────────────────────────────────────────────────────┐
│  ←  →  🔄  │ https://youtube.com             │ 🔒 │ [👤 Work ▼] │
└──────────────────────────────────────────────────────────────┘
                                                      │
                                               Click to show:
┌────────────────────────┐
│  ✓ Work                │  ← current profile (checkmark)
│    Personal            │
│    Gaming              │
│  ────────────────────  │
│  + Add Profile         │
│  ⚙ Manage Profiles     │
└────────────────────────┘
```

### Multi-Instance Behavior

| Scenario | Behavior |
|----------|----------|
| User clicks profile avatar → different profile | Opens NEW WINDOW in that profile |
| User opens second Hodos instance | Shows profile picker, user chooses |
| Tabs within same window | Always same profile (share sessions) |
| Different windows | Can be different profiles |

### Profile vs Website Login (Important Distinction)

**Profile switching** (our responsibility):
- Changes which local data silo is active
- Different bookmarks, history, cookies appear

**Website login** (website's responsibility):
- User goes to x.com and logs in
- Session stored in cookies within current profile
- If user switches profiles, different x.com session (or logged out)

**We do NOT need to build:**
- Google account sync (yet) — that's optional cloud sync, not MVP
- OAuth flows for browser sync — users just log into websites normally

---

## Implementation Notes for Sprint 9

### Profile Storage Structure

```
%APPDATA%/HodosBrowser/
├── Default/                 # Original profile (renamed to user's first profile name)
│   ├── Bookmarks
│   ├── History
│   ├── Cookies
│   └── ...
├── Profile 2/               # Second profile
│   ├── Bookmarks
│   ├── History
│   └── ...
├── Profile 3/
│   └── ...
├── profiles.json            # Profile metadata (names, colors, etc.)
├── settings.json            # Global browser settings
└── wallet/                  # Wallet data (shared? or per-profile? — decision needed)
```

### Wallet Data: Shared or Per-Profile?

**Option A: Shared wallet across all profiles**
- Pros: Single backup, single identity, simpler
- Cons: Can't have different "identities" for work vs personal

**Option B: Separate wallet per profile**
- Pros: True isolation, different BSV identities
- Cons: Multiple backups, confusing for users, more complexity

**Recommendation:** Start with Option A (shared wallet). BSV wallet is like a "system" feature, not a browsing feature. Users can have different website sessions but same wallet. Revisit if users request multi-wallet.

### What to Import Per-Profile

When user imports from Chrome, each Chrome profile becomes a Hodos profile:

| Data | Import? | Notes |
|------|---------|-------|
| Profile name | ✓ | Direct copy |
| Bookmarks | ✓ | Per-profile |
| History | ✓ | Per-profile |
| Cookies | ⚠ | Hard (DPAPI), defer to post-MVP |
| Passwords | ✗ | Security risk, don't import |
| Extensions | ✗ | Not compatible |

---

## UI Component Requirements

### 1. Profile Picker Modal (Startup)

- Shown if: multiple profiles exist AND setting enabled
- Lists all profiles with names/colors
- "Skip" loads last-used profile
- "Add Profile" option
- "Guest Mode" option (optional, nice-to-have)

### 2. Profile Avatar Button (Header)

- Always visible in header bar
- Shows: profile name (or initial) + color indicator
- Click: opens profile dropdown
- Dropdown options:
  - Current profile (checkmark)
  - Other profiles (click → new window)
  - "Add Profile"
  - "Manage Profiles" → Settings

### 3. Profile Management (Settings)

- List all profiles
- Edit: name, color
- Delete (with confirmation, can't delete last profile)
- Import data into this profile

### 4. New Window Profile Selection

When opening "New Window" (Ctrl+N):
- Default: same profile as current window
- Menu option: "New Window in Profile..." → shows profile picker

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Cookie import fails | Users must re-login to websites | Clear messaging: "You may need to sign back into some websites" |
| Profile picker annoys users | Friction on every launch | Off by default; only show if user has multiple profiles |
| Wallet per-profile confusion | Users lose funds or backup wrong wallet | Start with shared wallet; single backup covers everything |
| Profile corruption | Data loss | Copy-on-import (never modify source profile) |

---

## Future Considerations (Post-MVP)

1. **Browser Sync Service**: Hodos account for cloud sync (like Firefox Account)
2. **Container Tabs**: Firefox-style containers within a profile
3. **Guest Mode**: Temporary profile that's deleted on close
4. **Profile Lock**: PIN/password to access certain profiles
5. **Auto-Switch Profiles**: Open certain URLs in specific profiles automatically (Safari-style)

---

*Research compiled 2026-02-25 from official browser documentation and support articles.*
