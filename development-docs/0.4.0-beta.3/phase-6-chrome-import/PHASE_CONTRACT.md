# Phase 6 — Chrome import (WS4) — ⛔ CUT FROM beta.3

> **Outcome: CUT.** Owner decision 2026-09-02. No code was written. This phase's honest first
> deliverable was a scope decision, and the decision is: the achievable value is small, the rest is
> blocked by Chrome's own encryption, and the defect-bearing Phases 7–10 are worth more. The deferred
> work is captured as a ticket in `development-docs/0.4.0-beta.5/`.
>
> This file is the record of *why*, so the cut is a decision and not a drift (HARNESS §7).

---

## §0 — Plan vs. tree, measured 2026-09-02 (fifth kickoff; the prior four each found drift)

Measured on this machine, not carried from the plan. Two of my own early claims were **wrong and are
corrected below** — flagged 🔧.

### 0.1 What `ProfileImporter` does today — and 🔧 it is NOT reachable, contra my first claim

| Claim | Measured | Verdict |
|---|---|---|
| Chrome/Brave/Edge bookmarks + history | ✅ `ProfileImporter.cpp` — `DetectProfiles`/`ImportBookmarks`/`ImportHistory`/`ImportAll` | CONFIRMED |
| No cookie / password import | ✅ header has neither; `DetectProfiles` probes only `Bookmarks` + `History` | CONFIRMED |
| Firefox stub, never called | ✅ `GetFirefoxProfilePath()` → `""` | CONFIRMED |
| 🔧 *"reachable from the UI / shipping"* (my first handback) | ⛔ **WRONG. It is orphaned.** The Import tab + buttons exist **only** in the old settings *overlay* `SettingsOverlayRoot` (`/settings`). The live **Settings** menu item navigates to the full-page `SettingsPage` (`/settings-page`; `MainBrowserView.tsx:500`), whose sections are General / Privacy / Downloads / Wallet / About — **no Import.** The `/settings` overlay is triggered by IPC `overlay_show_settings`, sent only from a legacy fallback branch in `initWindowBridge.ts`, not from the Settings button. | **DELTA — built but disconnected** |

⇒ The importer is real code wired to a screen the current UI can't reach. "Extend, not duplicate"
still holds — but there is also a latent *reconnect* task, not a *make-it-usable-from-scratch* one.
The owner confirmed from the running build: **there is no import button anywhere in Settings.**

### 0.2 ABE blocks cookies/passwords on THIS machine — measured, not cited

**Installed Chrome:** `152.0.7977.65` (≫ the Chrome 127 ABE cutoff).

**`Local State` → `os_crypt`** (unlocked file, read directly):

| Key | Present | Prefix | Meaning |
|---|---|---|---|
| `encrypted_key` | ✅ 293 B | `DPAPI` | legacy v10 key, Windows-account bound |
| `app_bound_encrypted_key` | ✅ 644 B | `APPB\x01` | **App-Bound Encryption present** — wrapped by the SYSTEM-service DPAPI, validated against the Chrome binary path |

⇒ Cookie/password values are **v20 / ABE-wrapped**; unwrapping the key is the "extract keys Chrome
protects" that the scope fence rules OUT.

**Second, independent wall — bytes unreachable while Chrome runs:**

| DB | Copy method | Result (Chrome running) |
|---|---|---|
| `History` | `CopyFileA` (importer's own method) | ✅ copied 15,958,016 B — opened `FILE_SHARE_READ` |
| `Network/Cookies` | `CopyFileA` | ⛔ err=32 `ERROR_SHARING_VIOLATION` |
| `Network/Cookies` | PowerShell `FileShare.ReadWrite` | ⛔ in use by another process |
| `Network/Cookies` | `esentutl /y /d` | ⛔ no copy produced |

⇒ Cookies are held under an exclusive lock the importer cannot bypass, on top of being ABE-encrypted.

### 0.3 The only lawful secret route is Chrome's user-initiated CSV — and 🔧 Hodos DOES have a manager

Chrome's password **CSV export** (Password Manager → Settings → Export → OS re-auth via Windows
Hello/password → plaintext `.csv` of `url,username,password`) is the one path yielding decrypted
passwords without us defeating anything — the *user* authorizes it. It **cannot be automated** (the
re-auth is OS-gated precisely to stop background export).

🔧 **Correction to my second handback:** I said Hodos has "nowhere to put" imported passwords. The
owner corrected this from the running build: **Chromium's own password manager is live in Hodos** —
the save-password bubble already appears while browsing. So password import would target the existing
Chromium password store, **not** a manager we build. What it *does* still need:

- the generic **save/update-password bubble is unbranded** (stock Chrome) — already tracked in
  `PROMPT_BRANDING_INVENTORY.md` (Group B, `kManagePasswords`, "the one the owner specifically asked
  about"; no CEF hook exists);
- **clear naming/labelling** so users understand exactly what they're importing/exporting (plaintext
  credentials), and safe handling + deletion of the CSV inside a wallet browser.

Chrome exports **bookmarks as HTML**, **passwords as CSV**; **history has no file export** (Takeout
JSON only); **cookies/sessions have no export.** So a "moving to a new computer" file flow realistically
covers bookmarks (HTML) and passwords (CSV) only.

### 0.4 macOS — relayed, not claimed

Chrome on macOS uses **Keychain**, not DPAPI/ABE. No symmetry assumed. Mac researches its own half.

### 0.5 Security posture — owner-settled

Importing another browser's **live logged-in sessions/cookies into a wallet browser: NO** (owner,
2026-09-02). It is a security-surface expansion, and it is technically blocked here anyway.

---

## §1 — Decision (SPRINT_PLAN §6 items 1–3)

**3. Cut line → CUT.** WS4 leaves beta.3. Rationale: the safe, valuable part (bookmarks/history) is
already built (just disconnected); passwords/cookies are blocked by ABE + file lock; the password
slice, even lawfully, wants branding + naming + secure-file handling work; and Phases 7–10 carry
shipping defects queued behind this. **1 (scope)** and **2 (UX/session posture)** are therefore moot
for beta.3; session import is a settled NO regardless.

Deferred work → `development-docs/0.4.0-beta.5/TICKET_chrome_import_bookmarks_history_passwords.md`.

---

## §2 — Harness posture for a cut phase

- **No code written ⇒ no build, no diff.** There is nothing for `R-INTEXT` / `R-GOLD` / `R-PERIM` /
  `R-COUNT` / `R-CLOSE` to regress against; this phase touches neither wallet, trust boundary, nor
  browser-data writes. Per HARNESS §8 this is stated explicitly, not left blank: **not-at-risk, and
  not run because there is no change — not SKIPPED-as-PASS.**
- The **5 → 6 boundary is a no-op boundary**: HEAD is unchanged by Phase 6 (docs only). `preflight`
  was run after the doc edits to confirm no gate drift; result recorded in the sign-off.
- The **real risk this phase would have carried** — corrupting the user's own `HistoryManager` /
  `BookmarkManager` SQLite with no undo — is **not incurred**, because nothing runs. It is carried
  into the beta.5 ticket as the first constraint any future implementation must satisfy (a restore
  story, with an *observed* RED).

### Sign-off — 2026-09-02 (docs-only change; confirms no gate drift)

| Run | Result |
|---|---|
| `preflight.ps1` | **INCOMPLETE** (0 failed, 1 skipped: T1d frontend build, `-Full` not requested). All T0 gates PASS **at baseline** — G2=2, G11=60, G12=4, **unchanged** by this phase. T1a–c, T1e–g PASS. |
| `preflight.ps1 -NegativeControl` | **PASS** — every gate detected its injected violation (none blind). |

Interpretation: Phase 6 wrote **no code**, so no gate *could* move; the baseline-unchanged result
confirms the doc edits introduced no drift. INCOMPLETE (not PASS) is recorded honestly per HARNESS §8
— the skip is the standard un-requested frontend build, not a Phase-6 gap.
