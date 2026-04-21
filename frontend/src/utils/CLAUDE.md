# frontend/src/utils
> Pure utility functions for URL detection and omnibox suggestion ranking used by browser chrome components.

## Overview

Two focused utility modules that power the browser's address bar behavior:

1. **URL detection** (`urlDetection.ts`) — determines whether user input is a URL or search query, normalizes URLs, and builds search engine URLs
2. **Suggestion ranking** (`suggestionRanker.ts`) — merges and deduplicates history-based and Google autocomplete suggestions for the omnibox dropdown

Both modules are pure functions with no side effects, framework dependencies, or CEF/IPC calls.

## Files

| File | Purpose |
|------|---------|
| `urlDetection.ts` | URL-vs-search detection, URL normalization, search URL construction |
| `suggestionRanker.ts` | Omnibox suggestion merging, deduplication, ranking, and inline autocomplete |

## Key Functions

### `isUrl(input: string): boolean`

Determines if user input should be navigated to directly or treated as a search query. Recognizes:
- Explicit protocols (`http://`, `https://`, `file://`)
- `localhost` with optional port/path
- IPv4 addresses with optional port/path
- Domain-like strings with valid TLDs (2-6 alpha chars after last dot)

```typescript
isUrl('google.com')           // true
isUrl('localhost:3000')       // true
isUrl('192.168.1.1/admin')   // true
isUrl('how to cook pasta')   // false
```

### `normalizeUrl(input: string): string`

Adds `https://` prefix to bare URLs that pass `isUrl()`. Returns input unchanged if it already has a protocol or isn't a URL.

```typescript
normalizeUrl('github.com')           // 'https://github.com'
normalizeUrl('https://example.com')  // 'https://example.com' (unchanged)
normalizeUrl('search terms')         // 'search terms' (unchanged, not a URL)
```

### `toGoogleSearchUrl(query: string): string`

Converts a search query string to a Google search URL with proper encoding.

### `toSearchUrl(query: string, engine: string): string`

Converts a search query to a URL for the specified engine. Supported engines: `"google"`, `"duckduckgo"`. Falls back to DuckDuckGo for unknown engines.

### `rankAndMergeSuggestions(historyResults, googleResults, query): Suggestion[]`

Merges history entries (with frecency scores) and Google autocomplete strings into a single ranked suggestion list.

- History suggestions are prioritized over Google suggestions (positive scores vs negative)
- Deduplication at domain level — history wins over Google when domains overlap
- Google suggestions are also deduplicated against history by checking if search terms match history domains
- Results capped at **6 items**

```typescript
const merged = rankAndMergeSuggestions(historyEntries, googleStrings, 'git');
// Returns up to 6 Suggestion[] sorted by score descending
```

### `getAutocompleteSuggestion(suggestions: Suggestion[], query: string): string | null`

Returns the best inline autocomplete string for the address bar, or `null` if no good match exists. Only the top-ranked suggestion is considered.

For history suggestions, tries matching against:
1. URL without protocol (e.g., `github.com/...`)
2. Hostname suffix for subdomains (e.g., typing `google` matches `mail.google.com`)
3. Page title

For Google suggestions, matches against the search term text.

## Usage Patterns

### Address Bar Navigation (`MainBrowserView.tsx`, `NewTabPage.tsx`)

```typescript
import { isUrl, normalizeUrl, toSearchUrl } from '../utils/urlDetection';

function handleNavigate(input: string) {
  if (isUrl(input)) {
    navigate(normalizeUrl(input));
  } else {
    navigate(toSearchUrl(input, searchEngine));
  }
}
```

### Omnibox Suggestions (`useOmniboxSuggestions.ts`)

```typescript
import { rankAndMergeSuggestions, getAutocompleteSuggestion } from '../utils/suggestionRanker';

const suggestions = rankAndMergeSuggestions(historyResults, googleResults, query);
const autocomplete = getAutocompleteSuggestion(suggestions, query);
```

## Related

- `../types/omnibox.ts` — `Suggestion` and `HistoryEntryWithFrecency` types consumed by `suggestionRanker.ts`
- `../hooks/useOmniboxSuggestions.ts` — hook that calls both ranker functions
- `../pages/MainBrowserView.tsx` — primary consumer of URL detection utilities
- `../pages/NewTabPage.tsx` — also uses URL detection for its search bar
