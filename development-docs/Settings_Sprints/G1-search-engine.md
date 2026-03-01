# G1: Default Search Engine

**Status**: Not Started
**Complexity**: Low
**Estimated Phases**: 1

---

## Decisions (2026-03-01)

- **Engines**: Keep only **DuckDuckGo** (default) and **Google** (fallback). Drop Bing and Brave Search.
- **Default**: DuckDuckGo — aligns with privacy-browser brand. Google available for users who want it.
- **Suggest API**: Swap alongside search URL. Both APIs are free, no keys needed.
- **DuckDuckGo privacy model**: Revenue from contextual keyword ads (Bing Ads syndication) + affiliate commissions + Privacy Pro subscription. Does NOT sell user data. Microsoft tracking exception fixed Aug 2022.

---

## Current State

- UI exists in `GeneralSettings.tsx` — dropdown with Google, Bing, DuckDuckGo, Brave Search
- Setting persists to `settings.json` via `SettingsManager`
- **Not wired**: `MainBrowserView.tsx` hardcodes `toGoogleSearchUrl()` — setting is ignored
- Omnibox suggestions hardcoded to Google Suggest API in `GoogleSuggestService.cpp`

---

## What Needs to Happen

### Phase 1: Address Bar Search + Suggest API Swap

**Goal**: Typing a non-URL query uses the selected search engine, and omnibox suggestions come from the matching engine's suggest API.

**Frontend changes**:
- [ ] Trim dropdown in `GeneralSettings.tsx` to only Google and DuckDuckGo
- [ ] Change default from Google to DuckDuckGo in `SettingsManager` (C++)
- [ ] Create `searchEngines.ts` utility with search URL + suggest URL templates
- [ ] Read `browser.searchEngine` setting in `MainBrowserView.tsx`
- [ ] Replace hardcoded `toGoogleSearchUrl()` with engine-aware function
- [ ] Update `urlDetection.ts` to use `searchEngines.ts`

**Suggest API swap**:
- [ ] Pass selected engine to omnibox overlay via IPC or settings read
- [ ] Update `GoogleSuggestService.cpp` (or rename to `SearchSuggestService`) to accept engine-specific URL
- [ ] Add response parser adapter in `useOmniboxSuggestions.ts` for DuckDuckGo format
  - Google format: `["query", ["suggestion1", "suggestion2", ...]]`
  - DuckDuckGo format: `[{"phrase": "suggestion1"}, {"phrase": "suggestion2"}, ...]`
- [ ] Both APIs are free, no authentication needed:
  - Google: `https://suggestqueries.google.com/complete/search?client=chrome&q={query}`
  - DuckDuckGo: `https://duckduckgo.com/ac/?q={query}&type=list`

**Design decisions** (resolved):
- Search URL templates are straightforward (both use `?q=` pattern)
- No API keys needed for either engine
- DDG suggest quality is more limited than Google's — acceptable trade-off for privacy

---

## Search URL Templates

| Engine | Search URL | Suggest API | Auth |
|--------|-----------|-------------|------|
| DuckDuckGo (default) | `https://duckduckgo.com/?q={query}` | `https://duckduckgo.com/ac/?q={query}&type=list` | None |
| Google | `https://www.google.com/search?q={query}` | `https://suggestqueries.google.com/complete/search?client=chrome&q={query}` | None |

---

## Test Checklist

- [ ] Default search engine is DuckDuckGo on fresh install
- [ ] Address bar search uses DuckDuckGo by default → verify DDG results page loads
- [ ] Switch to Google → verify address bar search uses Google
- [ ] Omnibox suggestions appear when DDG is selected (from DDG suggest API)
- [ ] Omnibox suggestions appear when Google is selected (from Google suggest API)
- [ ] Verify setting persists across browser restart
- [ ] Verify homepage setting still works (no regression)
- [ ] Verify inline autocomplete still works with both engines

---

**Last Updated**: 2026-03-01
