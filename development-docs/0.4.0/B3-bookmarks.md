# B3 — Bookmarks (make functional)

**Status:** ⏳ Research pending — **can start now** (not Brave-gated)
**Type:** feature (med) · **0.4.0:** likely

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
