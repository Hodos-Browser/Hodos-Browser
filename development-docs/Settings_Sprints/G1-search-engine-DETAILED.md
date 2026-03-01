# G1: Default Search Engine — Detailed Implementation Plan

**Status**: Not Started
**Complexity**: Low-Medium
**Estimated Time**: 2-4 hours
**Dependencies**: None

---

## Executive Summary

Wire the existing `searchEngine` setting to the address bar and omnibox suggestions. The UI and persistence already work — this sprint connects them to actual functionality.

---

## Current State Analysis

### What Exists
- **UI**: Dropdown in `GeneralSettings.tsx` with Google, Bing, DuckDuckGo, Brave Search
- **Persistence**: `SettingsManager::SetSearchEngine()` saves to `settings.json`
- **Backend**: `BrowserSettings.searchEngine` field (default: "google")

### What's Missing
- `MainBrowserView.tsx` hardcodes `toGoogleSearchUrl()` — ignores the setting
- `GoogleSuggestService.cpp` only fetches from Google Suggest API
- No search URL templates for non-Google engines

---

## Research Findings

### Search URL Templates (Verified)

| Engine | Search URL | Notes |
|--------|-----------|-------|
| Google | `https://www.google.com/search?q={query}` | Standard |
| Bing | `https://www.bing.com/search?q={query}` | Standard |
| DuckDuckGo | `https://duckduckgo.com/?q={query}` | Standard |
| Brave | `https://search.brave.com/search?q={query}` | Standard |

### Suggest API Endpoints (Researched)

| Engine | Suggest API | Response Format | Availability |
|--------|-------------|-----------------|--------------|
| Google | `https://suggestqueries.google.com/complete/search?client=chrome&q={query}` | JSON array | ✅ Free, no auth |
| DuckDuckGo | `https://duckduckgo.com/ac/?q={query}` | JSON array `[{"phrase":"suggestion"}]` | ✅ Free, no auth |
| Bing | Deprecated / requires paid API key | - | ❌ Not viable |
| Brave | Not publicly documented | - | ❌ Not viable |

**Decision**: Implement suggestions for Google and DuckDuckGo only. Bing and Brave will show no suggestions (graceful degradation).

---

## Phase 1: Address Bar Search (1-2 hours)

### Step 1: Create Search Engine Utility

**File**: `frontend/src/utils/searchEngines.ts`

```typescript
export type SearchEngine = 'google' | 'bing' | 'duckduckgo' | 'brave';

interface SearchEngineConfig {
  name: string;
  searchUrl: string;
  suggestUrl: string | null; // null = no suggestions
  suggestParser: ((data: any) => string[]) | null;
}

const SEARCH_ENGINES: Record<SearchEngine, SearchEngineConfig> = {
  google: {
    name: 'Google',
    searchUrl: 'https://www.google.com/search?q=',
    suggestUrl: 'https://suggestqueries.google.com/complete/search?client=chrome&q=',
    suggestParser: (data) => data[1] || [], // Returns ["query", ["sug1", "sug2", ...]]
  },
  bing: {
    name: 'Bing',
    searchUrl: 'https://www.bing.com/search?q=',
    suggestUrl: null, // Requires paid API
    suggestParser: null,
  },
  duckduckgo: {
    name: 'DuckDuckGo',
    searchUrl: 'https://duckduckgo.com/?q=',
    suggestUrl: 'https://duckduckgo.com/ac/?q=',
    suggestParser: (data) => data.map((item: { phrase: string }) => item.phrase),
  },
  brave: {
    name: 'Brave Search',
    searchUrl: 'https://search.brave.com/search?q=',
    suggestUrl: null, // Not publicly available
    suggestParser: null,
  },
};

export function getSearchUrl(engine: SearchEngine, query: string): string {
  const encoded = encodeURIComponent(query);
  return SEARCH_ENGINES[engine].searchUrl + encoded;
}

export function getSuggestUrl(engine: SearchEngine, query: string): string | null {
  const config = SEARCH_ENGINES[engine];
  if (!config.suggestUrl) return null;
  return config.suggestUrl + encodeURIComponent(query);
}

export function parseSuggestions(engine: SearchEngine, data: any): string[] {
  const config = SEARCH_ENGINES[engine];
  if (!config.suggestParser) return [];
  try {
    return config.suggestParser(data);
  } catch {
    return [];
  }
}

export function getEngineConfig(engine: SearchEngine): SearchEngineConfig {
  return SEARCH_ENGINES[engine];
}
```

### Step 2: Update MainBrowserView.tsx

**File**: `frontend/src/components/MainBrowserView.tsx`

Replace hardcoded `toGoogleSearchUrl()` calls:

```typescript
import { getSearchUrl } from '../utils/searchEngines';
import { useSettings } from '../hooks/useSettings';

// In component:
const { settings } = useSettings();
const searchEngine = settings.browser.searchEngine as SearchEngine;

// Replace:
// const url = toGoogleSearchUrl(query);
// With:
const url = getSearchUrl(searchEngine, query);
```

### Step 3: Delete Old Google Helper

Remove `toGoogleSearchUrl()` from `urlDetection.ts` or mark as deprecated.

---

## Phase 2: Omnibox Suggestions (1-2 hours)

### Step 1: Update Suggestion Fetching

The omnibox currently fetches from Google. Update to use the search engine setting:

**File**: Update the omnibox overlay component (likely `OmniboxOverlay.tsx` or similar)

```typescript
import { getSuggestUrl, parseSuggestions, SearchEngine } from '../utils/searchEngines';

async function fetchSuggestions(query: string, engine: SearchEngine): Promise<string[]> {
  const suggestUrl = getSuggestUrl(engine, query);
  if (!suggestUrl) return []; // Engine doesn't support suggestions

  try {
    const response = await fetch(suggestUrl);
    if (!response.ok) return [];
    const data = await response.json();
    return parseSuggestions(engine, data);
  } catch {
    return [];
  }
}
```

### Step 2: C++ Backend Alternative (Optional)

If CORS issues prevent direct frontend fetching, the backend `GoogleSuggestService.cpp` can be extended:

1. Add IPC handler `search_suggestions` that accepts `{query, engine}`
2. Create `SearchSuggestService.cpp` with engine-specific HTTP requests
3. Return suggestions via IPC response

**Recommendation**: Try frontend-first. CORS shouldn't be an issue for these public endpoints.

---

## Gaps Identified

| Gap | Impact | Resolution |
|-----|--------|------------|
| Bing suggestions unavailable | Minor — search still works | Display "No suggestions available" or just empty list |
| Brave suggestions unavailable | Minor — search still works | Same as Bing |
| CORS for DuckDuckGo suggest | Possible — needs testing | Use backend proxy if needed |

---

## Test Checklist

- [ ] Set search engine to Google → type query → navigates to Google search
- [ ] Set search engine to Bing → type query → navigates to Bing search
- [ ] Set search engine to DuckDuckGo → type query → navigates to DuckDuckGo
- [ ] Set search engine to Brave → type query → navigates to Brave Search
- [ ] Google: suggestions appear while typing
- [ ] DuckDuckGo: suggestions appear while typing
- [ ] Bing: no suggestions (graceful empty state)
- [ ] Brave: no suggestions (graceful empty state)
- [ ] Setting persists across browser restart
- [ ] URL detection still works (typing URLs navigates directly)

---

## Files to Modify

| File | Changes |
|------|---------|
| `frontend/src/utils/searchEngines.ts` | **NEW** — Search engine utilities |
| `frontend/src/components/MainBrowserView.tsx` | Use dynamic search engine |
| `frontend/src/components/OmniboxOverlay.tsx` (or similar) | Dynamic suggestions |
| `frontend/src/utils/urlDetection.ts` | Remove hardcoded Google helper (optional cleanup) |

---

**Last Updated**: 2026-02-28
