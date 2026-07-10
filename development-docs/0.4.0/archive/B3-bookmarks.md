# B3 — Bookmarks (make functional)

**Status:** ✅ DONE (2026-06-19) — landed as chunk (a) of the Header/Omnibox UX pass. See `HEADER_UX_PHASE.md` §5a for the as-built design. The "research pending / not source-verified" notes below were stale: `BookmarkManager` + the full `bookmark_*` IPC + the typed `window.hodosBrowser.bookmarks` bridge + Ctrl+D + browser-profile import were already built; this chunk added the UI (`BookmarksOverlayRoot` + `useBookmarks` + header button), un-stubbed the menu action, and made Ctrl+D a toggle. No bookmarks bar; HTML import/export deferred. macOS port deferred (see `MACOS_PORT_0_4_0.md`).
**Type:** feature (med) · **0.4.0:** ✅ shipped (Win); mac deferred

## Summary
Bookmark UI buttons exist but are non-functional. Make bookmarks actually work, with UX informed by
how other browsers do it.

## ⚠️ Not yet source-verified
The "buttons exist but non-functional" claim came from the planning conversation, not source
inspection. **First step of this item's kickoff: confirm current state in source** — locate the
bookmark buttons, any existing bookmark storage, and what (if anything) is already wired.
- Browser data (history, bookmarks) is stored in the C++ layer per CLAUDE.md
  (`%APPDATA%/HodosBrowser/Default/`). Check for an existing bookmarks store/manager before building one.

## Open questions / research needed
- What bookmark scaffolding already exists (C++ store? React UI? IPC)? — **reuse-first audit.**
- UX research: how Chrome/Firefox/Brave/etc. handle bookmark bar, folders, import/export, sync.
- Does this follow the overlay pattern (per CLAUDE.md UI rules) for any bookmark manager panel?

## Dependencies
None hard. Independent of A4 — good early-start candidate.

## To fill after kickoff
Acceptance criteria · Reuse map (file:line) · Risk table · Implementation order · Test plan (Win/macOS) · What this does NOT do.
