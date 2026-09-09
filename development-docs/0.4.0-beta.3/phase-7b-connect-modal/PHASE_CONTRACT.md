# Phase 7b — the connect modal tells one truth · PHASE CONTRACT

**Workstream:** Phase 7 bundle, sub-phase b · **Status:** 🚧 IN PROGRESS
**Tickets:** `consent_surface_fetches_third_party_favicon` · `connect_modal_two_views_drift` ·
`manifest_description_can_misdescribe_protocol`
**Opened:** 2026-09-02 · **Owner:** Matthew Archbold · **Platforms:** Windows (macOS = relay)
**Standard:** `../HARNESS.md`. **Base:** `c7ce5c6` (7a).

> ⚠️ **Written late, and saying so.** Rows 1 and 2 landed (`7dc00e4`, `b3487a8`, `0abc6d7`) before
> this file existed — a process miss against HARNESS §1, which requires the contract first. Their
> evidence was recorded as it was gathered in `MEASUREMENTS.md` and is transcribed below rather than
> reconstructed. Row 3 has its contract before its code, as it should be.

---

## 1. Goal

A user deciding whether to trust a site reads **one** screen that says the same thing everywhere,
shows what the site actually asked for, and discloses nothing to anyone while they read it.

## 2. Done means

- [x] Opening any consent modal makes **zero** requests to a third-party favicon service.
- [x] The omnibox, new tab and bookmarks likewise — from a local store, with a legible fallback.
- [ ] The connect decision has **one** view. There is no second screen whose wording can drift.
- [ ] A protocol's declared identifier is on screen beside the site's description of it, at every
      security level — so the site's sentence is never the only thing describing its own grant.
- [ ] `domain_approval` (the third view) carries the same identity and quiet-mode copy, and the same
      tooltips, as the connect bundle.
- [ ] Nothing a user could reach before is unreachable after.

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-PERIM` | the four privacy-perimeter gates | The identity checkbox is the **only** control over identity-key disclosure. Merging two views is exactly where a checkbox loses its binding and silently reads as ON. |
| `R-PROV` (P0.8) | site-sourced limits are marked | `renderLimitFields()` is shared by both views today. Deleting one view must not fork it. |
| `R-CLOSE` | overlay close guards | Same shared notification overlay. |
| — | **consent legibility** (7a) | The merged view is longer than either half. It must still fit — 7a's cap makes it scroll rather than clip, but a screen that needs scrolling to reach Connect is worse UX than one that does not. |

## 4. Evidence table

| ID | 🟢 GREEN | 🔴 RED — observed, and how | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P7b-A1` | No consent modal requests a third-party favicon | `git stash` the React fix, reopen → **2 Google hosts** (`s2/favicons` → `t2.gstatic.com/faviconV2?…&url=…`) | notification overlay browser, CDP `Network.requestWillBeSent`; instrument asserts its own trigger | T2 | 🟢 **GREEN** (M/N1–N2) |
| `P7b-A2` | Omnibox / new tab / bookmarks likewise, from the local store | `git stash` `NewTabPage.tsx` → **32** third-party requests naming real top sites, to Google **and** DuckDuckGo | each surface by URL, reload, load event asserted | T2 | 🟢 **GREEN** — new tab 0/127, bookmarks 0/132. ⚠️ **omnibox NOT measured** (no CDP target; owed) |
| `P7b-A2b` | A missing icon renders a legible fallback, never a broken image | 👤 Owner: *"All of the favicons on the new tab are broken"* — `<img src="">` | rendered DOM: `imgs 1, rendering 1, broken 0, letterTiles 7` | T3 👤 | 🟢 **GREEN** (N9) |
| `P7b-A3` | **One** connect view. `manifestShowCustomize` no longer exists | Grep the symbol: pre-change it gates two return paths; post-change it is absent. A second view cannot drift if there is no second view | the JSX, and the rendered screen | T1 + T3 👤 | ⬜ |
| `P7b-A4` | Per-item ticks are on the **first** screen, all ticked by default | Untick one protocol, Connect, read the DB: that protocol has **no** row in `domain_protocol_permissions` | the **written rows**, not the checkbox state. A tick that renders but does not persist is the defect | T2 | ⬜ |
| `P7b-A5` | The protocol identifier is displayed beside the site's description at security level **0, 1 and 2** | Fixture `[1,"3241645161d8"]` + `description:"Show your profile picture."` → pre-change the string `3241645161d8` appears **nowhere**. ⛔ A level-2 fixture would pass today via the counterparty footnote — that is the vacuous test to avoid | a **level-1** protocol | T1 + T3 👤 | ⬜ |
| `P7b-A6` | Identity + quiet-mode label **and tooltip** identical on the merged view and `domain_approval` | Change one string, and separately delete one `InfoIcon` → each must be caught. A label-only check passes today, which is how the tooltip half went unnoticed | rendered strings **and** tooltip presence | T1 | ⬜ |
| `P7b-A7` | Nothing reachable before is unreachable after | Diff rendered text of the pre-merge Summary ∪ Customize against the merged view | rendered text, both states of quiet mode | T2 | ⬜ |
| `P7b-A8` | The merged card still fits the viewport at the DPI-matrix cells | Feed the greedy fixture → card bottom within `innerHeight`; 7a's cap is the backstop, not the plan | `phase-7a-modal-viewport/measure.py`, same rig | T2 | ⬜ |

## 4b. 🍎 macOS — 🚨 `FaviconStore` was NEVER INITIALISED here. Defect found + fixed 2026-09-08

Asked for by `MAC_RELAY_P7_ROUND.md` M2, which predicted this exact failure. It was real.

`cef_browser_shell.cpp` (Windows entry) calls `FaviconStore::GetInstance().Initialize(profile_cache)`
at `:5910` and `.Shutdown()` at `:6174`, beside `SitePermissionStore` / `PaidContentCache`.
📏 **`cef_browser_shell_mac.mm` did neither and did not include the header.** Shipped in `b3487a8`
(2026-09-04); dead on macOS for four days.

⛔ **Silent by construction — both consumers are guarded, not fallible:**

| site | behaviour when `IsInitialized()` is false |
|---|---|
| `simple_handler.cpp :: OnFaviconURLChange :1230` | gates `DownloadImage` on `store.IsInitialized()` ⇒ **never downloads, never stores**. No error |
| `simple_handler.cpp :: favicon_get :8349` | `GetDataUri()` returns `""` ⇒ host **omitted** from the reply, React draws its letter tile — the documented correct fallback. No error |

⇒ macOS showed letter tiles everywhere and never created `favicons.db`. It reads as a rendering bug.

**📏 The artifact, because the log was the wrong instrument.** My first control was worthless and was
discarded: `build/bin/debug.log` has 0 `FaviconStore` lines — but also 0 `SitePermissionStore
initialized` lines, so it is simply not that sink. A zero from a blind instrument is not an absence.

| file | birth |
|---|---|
| profile dir `HodosBrowserDev/Default` | 2026-07-07 13:09:28 |
| `site_permissions.db` · `bookmarks.db` · `cookie_blocks.db` | 2026-07-07 13:09:41 ← positive control: this path *does* create such files |
| **`favicons.db`** | **2026-09-08 16:32:30** ← first launch after the fix |

**Fix + runtime verification (not merely "it initialises"):**

```
FaviconStore initialized at .../HodosBrowserDev/Default/favicons.db
sqlite> select host, icon_url, length(png), width from favicons;
127.0.0.1|http://127.0.0.1:5137/Hodos_Gold_Icon.svg|4552|64
```

⇒ 4,552 real PNG bytes at width 64. That also answers M2's second ask: **`CefBrowserHost::DownloadImage
(url, is_favicon=true, …)` works on macOS** — the CEF API never used before this phase on either platform.

⚠️ **Scope of the damage, stated so it is not overstated.** `P7b-A1`'s *privacy* subject held on macOS
regardless: the React surfaces stopped emitting `google.com/s2/favicons` whatever the store did, and
the store path was skipped entirely, so **no third-party request was ever made**. The de-Googling was
intact; only the local replacement was dead.

⚠️ Only `127.0.0.1` is stored so far — `example.com` declares `<link rel="icon" href="data:,">`, an
empty data URI, so its absence is correct rather than a second bug.

⬜ **Consequence for the owed rows:** `MAC_RELAY_P7_ROUND.md` M4 #1/#2 (zero third-party requests on
the consent modal / new tab) are **still not run on macOS**, and this fix *changes what they measure* —
before today macOS could not have produced a store hit **or** a Google request, so a green there would
have been vacuous. Re-run them now that the store is live.

## 5. Blast radius

- `BRC100AuthOverlayRoot.tsx` — the connect bundle (both views), `domain_approval`, and the shared
  `cardStyle`. **Every consent modal in the browser lives in this file.**
- `manifestShowCustomize` state, its reset in `applyParams`, and `customizeRowLabel` /
  `customizeCheckbox` styles — orphaned by the merge and removed with it.
- ⚠️ **`handleManifestConnect(true)` — "Allow without limits"** (raises caps to $1000/tx,
  $10000/session) is reachable **only from Customize** today. See §6.
- **Not touched:** `manifestConsent.ts` (T1f's subject), the engine, `ManifestFetcher` / `manifest.rs`.

## 6. Out of scope — and one thing that needs an owner answer

- The quiet-mode engine narrowing (**7c**), the dual store and branding (**7d**/**7e**).
- The plain-language rewrite of protocol *purpose* copy — a separate exercise, and one where the
  owner sees drafts rather than my wording.
- Pointing the tab strip / drag ghost / tab list at the favicon store (found in the Row 3 sweep).
- The paymail-avatar and certificate-avatar remote loads (Row 3 sweep) — money path, Phase 8.

🙋 **"Allow without limits" is promoted by this merge, and that is a decision, not a detail.** It sits
behind the Customize click today, so most users never see it. Delete Customize and it lands on the
primary screen for everyone. Three options: keep it as-is (promoted), fold it behind the limits
"Adjust" disclosure, or drop it. ⛔ **I have kept it and flagged it** — removing a money control
silently is not mine to do, and neither is promoting one. Owner call at the pixel review.

## 7. Rollback

Row 3 is one commit touching one file's JSX. `git revert` restores both views verbatim; no state, no
schema, no C++. Rows 1–2 revert independently.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `preflight -Full` + `-NegativeControl`
- [ ] `REGRESSION_SET.md` at the 7 boundary — `R-PERIM` live here
- [ ] 👤 Owner has **read the merged connect screen** — the row no gate can answer
- [ ] Omnibox measurement (`P7b-A2`) closed or explicitly carried
- [ ] Commit messages cite row IDs

| Item | Result | Date | By |
|---|---|---|---|
| preflight | 🟢 PASS (rows 1–2) | 2026-09-04 | assistant |
| preflight -NegativeControl | ⬜ owed at row 3 | | |
| regression set | ⬜ owed at phase close | | |
| owner pixel review | ⬜ **owed** | | |
