# G1: Default Search Engine

**Status**: Not Started
**Complexity**: Low-Medium
**Estimated Phases**: 2

---

## Current State

- UI exists in `GeneralSettings.tsx` — dropdown with Google, Bing, DuckDuckGo, Brave Search
- Setting persists to `settings.json` via `SettingsManager`
- **Not wired**: `MainBrowserView.tsx` hardcodes `toGoogleSearchUrl()` — setting is ignored
- Omnibox suggestions also hardcoded to Google Suggest API

---

## What Needs to Happen

### Phase 1: Address Bar Search

**Goal**: Typing a non-URL query uses the selected search engine.

**Changes needed**:
- [ ] Create search URL templates for each engine (Google, Bing, DuckDuckGo, Brave)
- [ ] Read `browser.searchEngine` setting in `MainBrowserView.tsx`
- [ ] Replace hardcoded `toGoogleSearchUrl()` with engine-aware function
- [ ] Update `urlDetection.ts` or create `searchEngines.ts` utility

**Design decisions**:
- Search URL templates are straightforward (all use `?q=` pattern)
- No backend changes needed — purely frontend

### Phase 2: Omnibox Suggestions

**Goal**: Autocomplete suggestions come from the selected engine's suggest API.

**Changes needed**:
- [ ] Research suggest API endpoints for Bing, DuckDuckGo, Brave
- [ ] Update omnibox overlay to use engine-specific suggest URL
- [ ] Handle engines without suggest APIs (fallback to no suggestions)
- [ ] Consider: should omnibox show engine icon/name?

**Design decisions**:
- DuckDuckGo has a suggest API but it's more limited than Google's
- Brave Search has an autocomplete API
- Bing has Autosuggest API (may require API key)
- Decision: do we want suggestions at all for non-Google engines, or just search?

---

## Search URL Templates (Research)

| Engine | Search URL | Suggest API |
|--------|-----------|-------------|
| Google | `https://www.google.com/search?q={query}` | `https://suggestqueries.google.com/complete/search?client=chrome&q={query}` |
| Bing | `https://www.bing.com/search?q={query}` | Needs API key? Research needed |
| DuckDuckGo | `https://duckduckgo.com/?q={query}` | `https://duckduckgo.com/ac/?q={query}&type=list` |
| Brave | `https://search.brave.com/search?q={query}` | `https://search.brave.com/api/suggest?q={query}` (research needed) |

---

## Test Checklist

- [ ] Change search engine to each option → verify address bar search uses correct engine
- [ ] Verify omnibox suggestions work (or gracefully absent) for each engine
- [ ] Verify setting persists across browser restart
- [ ] Verify homepage setting still works (no regression)

---

**Last Updated**: 2026-02-28
