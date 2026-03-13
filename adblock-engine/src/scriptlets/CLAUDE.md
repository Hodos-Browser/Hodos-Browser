# Scriptlets — Bundled Extra uBlock Origin Scriptlet Templates

> JavaScript IIFE templates injected into page contexts to intercept and modify network responses and DOM text for ad blocking.

## Overview

These are self-contained JavaScript scriptlets that supplement the base uBlock Origin scriptlet set (pinned to v1.48.4). They exist because uBlock Origin switched to ES module format after v1.48.x, which the `assemble_scriptlet_resources()` parser in `engine.rs` cannot handle. Instead, these are embedded at compile time via `include_str!()` and registered individually into the adblock engine via `engine.add_resource()`.

Each file is an IIFE with `{{1}}`, `{{2}}`, etc. template placeholders that the adblock-rust engine substitutes with arguments from filter rules at injection time.

## Files

| File | Scriptlet Name | Aliases | Trusted | Purpose |
|------|---------------|---------|---------|---------|
| `json_prune_fetch_response.js` | `json-prune-fetch-response.js` | `jpfr.js`, `jpfr` | No | Intercepts `fetch()` responses, parses JSON, deletes specified property paths |
| `json_prune_xhr_response.js` | `json-prune-xhr-response.js` | `jpxr.js`, `jpxr` | No | Intercepts `XMLHttpRequest` responses, parses JSON, deletes specified property paths |
| `remove_node_text.js` | `remove-node-text.js` | `rmnt.js`, `rmnt` | No | Watches DOM via MutationObserver, clears text of matching elements |
| `trusted_replace_fetch_response.js` | `trusted-replace-fetch-response.js` | `trusted-rpfr.js`, `trusted-rpfr` | Yes | Intercepts `fetch()` responses, performs string/regex replacement on body text |
| `trusted_replace_node_text.js` | `trusted-replace-node-text.js` | `trusted-rpnt.js`, `trusted-rpnt` | Yes | Watches DOM via MutationObserver, replaces text content of matching elements |
| `trusted_replace_xhr_response.js` | `trusted-replace-xhr-response.js` | `trusted-rpxr.js`, `trusted-rpxr` | Yes | Intercepts `XMLHttpRequest` responses, performs string/regex replacement on body text |

**Trusted vs untrusted:** Trusted scriptlets (`PermissionMask(1)`) can only be invoked by trusted filter lists (e.g., uBlock bundled lists). Untrusted scriptlets can be used by any filter list.

## Template Parameters

All scriptlets use positional `{{N}}` parameters. Unfilled parameters remain as their literal `{{N}}` string — each scriptlet checks for this and no-ops if required params are missing.

### Network Interception Scriptlets (fetch/XHR)

**JSON prune (`jpfr`, `jpxr`):**
- `{{1}}` — Space-separated property paths to delete (required). Supports dot notation (`ad.config`), array traversal (`[].adSlots`), and negated arrays (`[-].items`)
- `{{2}}` — Required paths (parsed but not actively enforced in this implementation)
- `{{3}}`/`{{4}}`, `{{5}}`/`{{6}}` — Key-value pairs; when key is `propsToMatch` and value contains `url:`, it sets the URL filter

**Trusted replace (`trusted-rpfr`, `trusted-rpxr`):**
- `{{1}}` — Pattern to match (string literal or `/regex/flags`) (required)
- `{{2}}` — Replacement string (defaults to empty string if omitted)
- `{{3}}` — URL needle — only intercept responses whose URL contains this string or matches this regex (all URLs if omitted)

### DOM Scriptlets (node text)

**Remove node text (`rmnt`):**
- `{{1}}` — HTML tag name to match, e.g. `script` (required)
- `{{2}}` — Pattern to match in `textContent` (string literal or `/regex/flags`) (required)

**Trusted replace node text (`trusted-rpnt`):**
- `{{1}}` — HTML tag name to match (required)
- `{{2}}` — Pattern to match (string literal or `/regex/flags`) (required)
- `{{3}}` — Replacement string (defaults to empty string if omitted)
- `{{4}}`/`{{5}}` — When `{{4}}` is `sedCount`, `{{5}}` is the max number of replacements

## Interception Techniques

### Fetch Proxy (`json_prune_fetch_response.js`, `trusted_replace_fetch_response.js`)
Wraps `window.fetch` with a `Proxy`. On matching URLs:
1. Calls original `fetch()` via `Reflect.apply()`
2. Clones the response, reads body as text
3. Modifies text (prune JSON paths or string replace)
4. Returns a new `Response` with modified body but original status/headers

### XHR Monkey-patch (`json_prune_xhr_response.js`, `trusted_replace_xhr_response.js`)
Patches `XMLHttpRequest.prototype.open` and `.send`:
1. `open()` — Stores URL in a `WeakMap` keyed by XHR instance
2. `send()` — If URL matches, adds `readystatechange` listener that modifies response at `readyState === 4`
3. Overrides `responseText` and `response` property descriptors to return modified content from a `WeakMap`

### DOM Observer (`remove_node_text.js`, `trusted_replace_node_text.js`)
Uses `MutationObserver` on `document` with `{ childList: true, subtree: true }`:
- **`remove_node_text`** — checks only direct added nodes matching the target tag name; clears `textContent` to empty string
- **`trusted_replace_node_text`** — checks direct added nodes AND their descendants via `querySelectorAll(tagLC)`; performs string/regex replacement on `textContent`; also runs an initial scan of existing elements on load (`document.querySelectorAll`); supports `sedCount` limit (tracks total replacements across all nodes)

Non-regex string replacement in both network and DOM scriptlets uses `text.split(pattern).join(replacement)` for global replace without needing the `g` flag.

## Shared Utilities

All scriptlets share two inline helper functions (duplicated in each file, not extracted):

- `isRegex(s)` — Detects `/pattern/flags` regex syntax by checking for leading `/` and valid trailing flags
- `toRegex(s)` — Parses a regex string into a `RegExp` object, returns `null` on invalid regex

Network scriptlets additionally share:
- `matchesUrl(url)` — Tests a URL against the configured needle using regex or `includes()`

JSON prune scriptlets additionally share:
- `pruneProperty(obj, path)` — Recursive property deletion supporting dot-separated paths, `[]` array traversal, and `[-]` negated array traversal

## Registration Pipeline

1. `engine.rs` defines the `EXTRA_SCRIPTLETS` array with compile-time `include_str!()` of each `.js` file
2. `load_extra_scriptlets()` base64-encodes each scriptlet's content and creates a `Resource` with `ResourceType::Template`
3. Called after `use_resources()` (which loads the base 1.48.4 scriptlets) so these supplement/override the base set
4. At runtime, when a filter rule references a scriptlet (e.g., `youtube.com##+js(json-prune-fetch-response, ...)`), the engine substitutes `{{N}}` placeholders with the rule's arguments
5. The resulting JavaScript is returned via the `/cosmetic-resources` endpoint (`handlers.rs`) as `injectedScript`
6. CEF injects it into the page's V8 context via `OnContextCreated` in `simple_render_process_handler.cpp`

## Disabling Scriptlets

- **Per-domain exception:** `#@#+js()` in filter lists (e.g., `hodos-unbreak.txt`) blanket-disables all scriptlet injection for a domain
- **User toggle:** The `/cosmetic-resources` endpoint accepts `skip_scriptlets: true` to return empty `injectedScript`

## Related

- `../engine.rs` — `EXTRA_SCRIPTLETS` array, `load_extra_scriptlets()`, `ExtraScriptlet` struct, scriptlet resource registration
- `../handlers.rs` — `/cosmetic-resources` endpoint that returns injected scriptlet JS to the browser
- `../../cef-native/src/handlers/simple_render_process_handler.cpp` — V8 injection of scriptlets via `OnContextCreated`
- `../../CLAUDE.md` — Root project context (Scriptlet Injection glossary entry, `AdblockCache` / cosmetic filtering docs)
