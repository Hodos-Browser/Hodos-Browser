# TICKET — `debug_output.log` is unfiltered in production: full URLs with query strings, unbounded, 1.2 GB observed

**Found:** 2026-08-11, incidentally, while verifying that the new renderer-crash handler's
origin redaction actually redacted.
**Status:** OPEN, not started. **Pre-existing** — not introduced by the crash handler.
**Needs owner decision** before any change: this is shipped production behaviour.

## The three facts

1. **`Logger` has no level filtering at all.** `cef-native/src/core/Logger.cpp` contains
   **zero** comparisons on `level` — the parameter is used only to render the level *name*
   into the line. So every `LOG_DEBUG_*` call writes to disk in **every build**, production
   included. There is no `HODOS_DEV` gate and no minimum-level setting.

2. **Full URLs, including query strings, are logged at DEBUG.** Measured by parking a tab on
   `https://example.com/private/path?token=SUPERSECRET123&q=my+search+terms` and grepping:
   **13 occurrences**, from pre-existing lines —

   ```
   [DEBUG] 🌐 Resource request: https://example.com/private/path?token=…&q=my+search+terms
   [DEBUG] 🔗 Tab 1 URL updated to: https://example.com/private/path?token=…
   [DEBUG] 📑 Tab list sent to window 0: {"tabs":[{…"url":"https://…"}]}
   ```

   Query strings routinely carry search terms, session tokens, password-reset tokens and
   document ids.

3. **It is unbounded.** The production log on the machine where this was found is **1.2 GB**
   (`%APPDATA%/HodosBrowser/logs/debug_output.log`); the dev one is 97 MB. The **wallet** logs
   rotate (`wallet_r00011.log`, …); this one does not. *(Only the file size was inspected —
   the contents of a real browsing profile's log were deliberately not read.)*

## Why it matters more for us than for most browsers

Hodos ships **no telemetry**, and that is a stated privacy position. But a plaintext,
unbounded, unrotated browsing-history file — with tokens in it — sitting in `%APPDATA%` is
the same data the telemetry stance is meant to avoid, just stored locally instead of sent.
Anything that reads the profile directory (backup software, sync clients, malware, a shared
machine, a support-log request) gets it. "We don't phone home" remains true and is not the
whole claim a user would infer.

Secondary, and more mundane: 1.2 GB of unbounded growth is a disk-usage bug on its own.

## Options — not evaluated in depth, a starting point only

1. **Gate by level in production.** Cheapest and most obvious: compile out or runtime-skip
   `DEBUG` unless `HODOS_DEV` is set. ⚠️ Note the consequence before choosing it — it also
   removes the diagnostic trail we rely on for user bug reports, which is precisely why the
   crash handler was added at `ERROR`/`WARNING` rather than `DEBUG`.
2. **Redact at the log site.** Log `scheme://host` instead of the full URL for the
   resource-request / tab-URL lines, the way `RedactedOriginForCrashLog` already does in
   `simple_handler.cpp`. Keeps the trail, drops the secrets. More call sites to touch.
3. **Rotate and cap.** Independent of 1 and 2, and worth doing regardless — the wallet
   already rotates, so there is a pattern to copy.

A plausible combination is **2 + 3** (keep diagnosis, drop secrets, bound the size) with **1**
as a further tightening if wanted. Owner's call.

## Related

- `cef-native/src/handlers/simple_handler.cpp :: RedactedOriginForCrashLog` — the redaction
  helper added 2026-08-11, and a working example of option 2.
- The renderer-crash handler added the same day logs `scheme://host` **only**, verified: with
  a token and search terms live in the tab's URL, the crash line contained **zero**
  occurrences of either.
