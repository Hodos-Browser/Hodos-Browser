# TICKET — Chrome / Chromium-browser import (bookmarks, history, passwords)

**Status:** 🅿️ **DEFERRED to beta.5.** Cut from beta.3 as Phase 6 / WS4 by owner decision 2026-09-02.
**Origin:** beta.3 sprint item #4. Full measurement + cut rationale:
`development-docs/0.4.0-beta.3/phase-6-chrome-import/PHASE_CONTRACT.md`.
**Owner:** deferred deliberately — *"I don't want to deal with it right now."* Do not pull forward
without an explicit ask.

---

## Why this was cut, in one paragraph

The original ask — "import Chrome profiles, passwords, cookies, history so a user's setup *just works*
as on their other computer" — is only partly achievable, and the blocked part is blocked by **Chrome's
own design, not ours.** Measured on a real machine 2026-09-02 (Chrome 152): cookies and passwords are
**App-Bound-Encrypted** (`app_bound_encrypted_key` present in `Local State`, `APPB` prefix; key bound
to the Chrome binary), and the cookie DB can't even be **copied** while Chrome runs
(`ERROR_SHARING_VIOLATION`). The only lawful secret route is Chrome's **user-initiated CSV export**.
Meanwhile the safe part (bookmarks/history) is **already written** — just disconnected from the live
UI. None of it was worth blocking beta.3's defect-bearing phases.

## Constraints any future implementation MUST respect

1. ⛔ **Never defeat Chrome's encryption.** No extracting/decrypting the ABE or DPAPI keys, no reading
   `Network/Cookies` or `Login Data` behind Chrome's back. If a route needs a key Chrome protects,
   it is out.
2. ⛔ **No live-session / cookie import into the wallet browser.** Settled NO (owner, 2026-09-02).
   Security-surface expansion, and blocked anyway.
3. 🚨 **Imports write to the user's own `HistoryManager` / `BookmarkManager` SQLite with no undo.** A
   bad import is data corruption in their real browser data. Any acceptance test needs a **restore
   story**, and its RED must be *observed*, not reasoned (beta.3 HARNESS §8 rule).
4. **Extend `ProfileImporter`, do not duplicate it.** (`cef-native/include/core/ProfileImporter.h` +
   `src/core/ProfileImporter.cpp`.)

## The work, in three independently-shippable slices

### Slice A — Reconnect the existing bookmarks/history import (small, safe, fixes a real gap)
The importer, the React hook (`frontend/src/hooks/useImport.ts`), and an Import tab all exist — but
the tab lives only on the **orphaned** settings overlay `SettingsOverlayRoot` (`/settings`). The live
**Settings** menu opens the full-page `SettingsPage` (`/settings-page`), which has **no Import
section**, so users can't reach it. Add an Import section to `SettingsPage` (sibling of General /
Privacy / Downloads / Wallet / About) driving the existing IPC. Verify the underlying import still
works (it was last exercised via the dead overlay).

### Slice B — "Moving to a new computer" file import (bookmarks HTML)
Chrome exports **bookmarks as an HTML file** (History has *no* file export — Google Takeout JSON
only). Accept a user-selected bookmarks HTML file and import it. Covers the cross-machine case for
non-secret data. Follow the CEF **visible file-input** pattern (CLAUDE.md — hidden `.click()` inputs
are unreliable in CEF).

### Slice C — Password CSV import (bigger, security-sensitive) — separate, deliberate
- ✅ **A password manager already exists** — Chromium's own is live in Hodos (the save-password bubble
  already appears while browsing). Import targets that store; **we do not build a manager.**
- Route: user does Chrome → Password Manager → Export (OS re-auth) → plaintext `url,username,password`
  CSV → hands it to Hodos → we parse into Chromium's password store. Cannot be automated.
- ⚠️ **Blockers to resolve first, or the feature is incoherent/unsafe:**
  - **Brand the save/update-password bubble.** It renders as stock Chrome today. Tracked in
    `development-docs/0.4.0-beta.3/PROMPT_BRANDING_INVENTORY.md` (Group B, `kManagePasswords`, "the one
    the owner specifically asked about"; **no CEF hook exists** — needs a fork patch or feature
    suppression). Importing credentials into a manager whose UI looks un-Hodos is a trust problem.
  - **Naming / labelling.** Users must understand they are importing/exporting **plaintext
    credentials** — name the UI and its copy so the data and its sensitivity are unambiguous.
  - **Secure handling of the CSV** inside a wallet browser: minimise time on disk, delete after
    import, never log values. A wallet browser holding bank logins is a bigger target than a normal
    one — state the posture explicitly.

## macOS
Chrome on macOS uses **Keychain**, not DPAPI/ABE. Assume no symmetry with the Windows analysis; Mac
researches its own half. Relay, never claim.

## Cross-references
- `development-docs/0.4.0-beta.3/phase-6-chrome-import/PHASE_CONTRACT.md` — measurements + cut record
- `development-docs/0.4.0-beta.3/SPRINT_PLAN.md` §6 decision 3 — the cut decision
- `development-docs/0.4.0-beta.3/PROMPT_BRANDING_INVENTORY.md` — save-password bubble branding
