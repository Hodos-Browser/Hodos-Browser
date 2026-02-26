# Sprint 9c: Clear Browsing Data — Research & Plan

**Created**: 2026-02-25
**Status**: Research Complete

---

## Current State Assessment

### What Exists

| Feature | Status | Location | Refresh on Delete? |
|---------|--------|----------|-------------------|
| **History: Clear All** | ✅ Works | HistoryPanel.tsx | ✅ Yes (clears local state) |
| **History: Delete Single** | ✅ Works | HistoryPanel.tsx | ✅ Yes (filters local state) |
| **History: Search** | ✅ Works (DB search) | useHistory.ts | N/A |
| **Cookies: Delete All** | ✅ Works | CachePanel.tsx | ❓ Needs verification |
| **Cache: Clear** | ✅ Works | CachePanel.tsx | ✅ Yes (refreshes size) |
| **Bookmarks: Delete** | ❓ Unknown | BookmarkManager | Check |

### UI Refresh Issues Found

1. **History delete/clear** — Hook correctly updates local state, but user reported needing to refresh. Possible causes:
   - C++ backend returning `false` (operation failed)
   - Stale closure in React hooks
   - HistoryManager not actually deleting in SQLite

2. **Need to verify**: Does the C++ `DeleteAllHistory()` actually return `true`?

---

## Chrome's Clear Browsing Data Categories

### Basic (Quick Clear)
| Category | What Gets Cleared | User Impact |
|----------|-------------------|-------------|
| **Browsing History** | Visited URLs, search history, autocomplete | Can't revisit easily |
| **Cookies and Site Data** | Cookies, localStorage, sessionStorage, IndexedDB | **Signs you out of ALL sites** |
| **Cached Images/Files** | Temporary files for faster loading | Pages load slower initially |

### Advanced Options
| Category | What Gets Cleared | User Impact |
|----------|-------------------|-------------|
| **Download History** | List of downloads (not files) | Can't see what was downloaded |
| **Passwords** | Saved logins | Must re-enter passwords |
| **Autofill Data** | Addresses, payment methods | Must re-enter forms |
| **Site Settings** | Permissions (camera, mic, location) | Must re-grant permissions |
| **Hosted App Data** | Extension/PWA data | Extension state lost |

### Time Range Options (Chrome)
- Last hour
- Last 24 hours
- Last 7 days
- Last 4 weeks
- All time

---

## Implications of Clearing Data

### Clearing Cookies — IMPORTANT
- **Signs you out of Google, Facebook, Twitter, etc.**
- **Breaks "Remember me" on all sites**
- User MUST understand this before confirming
- Recommendation: Add warning in confirmation dialog

### Clearing Cache
- Safe to clear anytime
- Only slows initial page loads
- No authentication impact

### Clearing History
- Safe, no auth impact
- Affects autocomplete suggestions
- Can't "go back" to find pages

---

## Recommended Hodos Implementation

### MVP Clear Data Dialog

**Location**: Settings → Privacy tab (or dedicated "Clear Data" section)

**UI Layout**:
```
┌─────────────────────────────────────────────┐
│  Clear Browsing Data                        │
├─────────────────────────────────────────────┤
│  Time range: [All time ▼]                   │
│                                             │
│  ☑ Browsing history (4,200 entries)        │
│  ☑ Cookies and site data (145 cookies)     │
│    ⚠️ This will sign you out of most sites │
│  ☑ Cached images and files (12.3 MB)       │
│                                             │
│  [ Cancel ]              [ Clear Data ]     │
└─────────────────────────────────────────────┘
```

### Phase 1 (MVP)
- [ ] Clear History (all or by time range)
- [ ] Clear Cookies (all)
- [ ] Clear Cache (all)
- [ ] Confirmation dialog with warnings
- [ ] Show counts/sizes before clearing

### Phase 2 (Post-MVP)
- [ ] Clear by domain (e.g., only google.com cookies)
- [ ] Clear passwords
- [ ] Clear download history
- [ ] "Clear on exit" automation (from settings)

---

## History Panel UX Improvements

### Current Issues
1. **100 entry limit** — Only showing 100, but 4200 exist
2. **No pagination** — Can't load more
3. **Search scope unclear** — User doesn't know if searching page or DB

### Recommendations

**Search Behavior:**
- Search should query the DB (current behavior ✅)
- Add placeholder text: "Search all history..."
- Show "Searching X entries" feedback

**Pagination:**
- Add "Load more" button at bottom
- Or infinite scroll with virtualization
- Show total count: "Showing 100 of 4,200"

**Delete Feedback:**
- Show toast/snackbar on successful delete
- Animate item removal (fade out)

---

## Implementation Checklist

### Pre-work Verification
- [ ] Test history.delete() in browser console — does it return true?
- [ ] Test history.clearAll() — does it return true?
- [ ] Check SQLite after clear — are rows actually deleted?
- [ ] Verify cookie deletion works

### Backend (C++)
- [ ] Add `HistoryManager::ClearHistoryRange(startTime, endTime)` if not exists
- [ ] Add `CookieManager::ClearAllCookies()` if not exists
- [ ] Add cache clearing via CEF API

### Frontend (React)
- [ ] Create ClearDataDialog component
- [ ] Add time range dropdown
- [ ] Show counts before clearing
- [ ] Add warning for cookie clearing
- [ ] Improve history pagination
- [ ] Add delete confirmation toasts

### IPC Messages Needed
- `clear_history_range` — Clear history between timestamps
- `clear_all_cookies` — Clear all cookies
- `clear_cache` — Clear browser cache
- `get_history_count` — Get total history count
- `get_cookie_count` — Get total cookie count
- `get_cache_size` — Get cache size in bytes

---

## Testing Checklist

- [ ] Clear history → verify UI updates without refresh
- [ ] Clear history → verify SQLite is actually empty
- [ ] Clear cookies → verify signed out of google.com
- [ ] Clear cookies → verify localStorage cleared
- [ ] Clear cache → verify cache directory emptied
- [ ] Import history (4200) → clear all → verify all deleted
- [ ] Partial clear (last hour) → verify only recent deleted
