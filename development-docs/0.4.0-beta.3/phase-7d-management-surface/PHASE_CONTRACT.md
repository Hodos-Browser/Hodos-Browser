# Phase 7d — the management half of the consent surface · PHASE CONTRACT

**Workstream:** ticket consolidation · **Tickets:** `TICKET_edit_limits_modal_usability.md` (5 items) +
`TICKET_site_permission_dual_store.md` (= Phase 7 §0.1 row **R4**)
**Status:** 🚧 **KICKOFF COMPLETE — AWAITING OWNER CONFIRMATION. No code written.**
**Opened:** 2026-09-08 · **Owner:** Matthew Archbold · **Platforms:** Windows (macOS = relay)
**Standard:** `../HARNESS.md`.
**Base commit:** `a052033` — verified `HEAD == git ls-remote origin refs/heads/0.4.0`, not from a
stale ref. ⚠️ This **corrects the session prompt**, which states base `9989935` and *"nothing since
`c0b0672` has been pushed — six commits sit local-only."* Both are false as of this session; see
`D-11`.

---

## 0. Plan-vs-tree delta — measured before anything was written

> ⛔ Every claim below is a **code reading** unless it says *measured live*. Nothing here is a
> live-run result; live runs are §4. Labelling one as the other is what HARNESS §6 Q4 bans.

Phase 7c found four false claims in one document. **This kickoff found eight**, plus two
confirmations. The largest is `D-1`: the ticket's central measurement was taken on a file that does
not render the thing it measured.

### 0.1 🚨 `D-1` — The file named for item 5 does not render the list

| | Ticket / session prompt | Tree, 2026-09-08 |
|---|---|---|
| File | `ApprovedSitesTab.tsx`, **503 lines** | **450 lines** |
| Contents | "the approved list", rendered "in insertion order" | ⛔ **zero `.map(` calls in the whole file.** It is the **defaults** panel (per-tx / per-session / rate / max-tx defaults, identity-disclosure default, prefill default, Reset-all) |
| The list | — | `<DomainPermissionsTab />`, **line 415**, a child component in `frontend/src/components/DomainPermissionsTab.tsx` (**527 lines**) |

⇒ The statement *"`ApprovedSitesTab.tsx` contains no search, filter or sort — measured, the words do
not appear in the file"* is **true and irrelevant**. The words are absent because the **list** is
absent. Everything item 5 asserts about list behaviour was measured on the wrong file.

### 0.2 🚨 `D-2` — "No search, filter or sort" is false: sort **and pagination** already ship

In `DomainPermissionsTab.tsx`:

| Feature | Where | Shape |
|---|---|---|
| **Sort** | `sorted` useMemo, 127–138; `handleSort`, 152–159; clickable headers, 291–297 | by `domain` (localeCompare) or any numeric column, asc/desc toggle |
| **Pagination** | `ROWS_PER_PAGE = 12` (83); `paged` useMemo, 144–147; footer 436 | **12 rows per page**, not a scroll |
| **Search / filter** | — | ✅ genuinely absent — the one true half of the claim |

⇒ The ticket's *"the list is rendered in insertion order and the user scrolls"* is wrong twice. It is
**sorted**, and it **pages**. That changes what item 5 must build: a filter has to compose with an
existing sort and an existing pager, not replace a scroll.

### 0.3 ⭐ `D-3` — The item-5 index-vs-identity finding — **identity, not index. But `page` is the real trap.**

The ticket asked: *"If any row action is keyed by list index rather than by domain + grant identity,
filtering silently makes it revoke the wrong row. Check before writing the filter, not after."*

**Checked. Every row action is keyed by identity. Not one is keyed by index.**

| Action | Site | Key |
|---|---|---|
| Edit limits | `DomainPermissionsTab` 366 → 166 | `setEditingDomain(perm)` → `editingDomain.domain` |
| Revoke site | 375 → 195 | `setRevokeTarget(perm)` → `revokeTarget.domain` |
| Revoke cert type | 412 → 216 | `handleRevokeCertType(perm.domain, ct.certType)` — domain passed explicitly |
| Revoke sub-permission | `DomainPermissionForm` 157–166 | `DELETE /domain/permissions/${row.kind}?id=${row.id}` — **DB primary key** |
| React keys | 320, 401, `DomainPermissionForm` 511 | `perm.id`, `field`, `` `${row.kind}-${row.id}` `` |

⇒ **Filtering cannot make Revoke act on the wrong row.** The hazard the ticket predicted is absent,
and after 7c that is the difference between a cosmetic change and a permission change. The check was
worth running; the answer is clean.

🚨 **But there IS an index-shaped hazard, and it is `page`.**

```
paged = sorted.slice(page * 12, page * 12 + 12)                       // 144-147
useEffect(() => { setPage(0); }, [sortKey, sortDir, permissions.length]);  // 149
```

`permissions` is the **unfiltered** source array. A filter narrows the *derived* list without
changing `permissions.length`, so the reset effect **does not fire**. Type a query while on page 2
and the table renders **empty**, with a footer reading `13–12 of 3`. The user's conclusion is "I have
no such site", on the panel that is now the entire per-site permission state.

⇒ Not an implementation detail. It gets its own evidence row (`P7d-A2`), and its RED is *"add the
filter without touching the reset effect."*

### 0.4 🚨 `D-4` — `DomainPermissionForm` has **five** render sites, four of them consent modals

The ticket: *"One component, two entry points — fix once, lands twice."* Measured:

| # | Site | What it is | Renders the granted list? |
|---|---|---|---|
| 1 | `DomainPermissionsTab.tsx` 473 | **Edit Limits** modal — the management path | ✅ |
| 2 | `BRC100AuthOverlayRoot.tsx` 142 | `edit_permissions` overlay — the **right-click** path | ✅ |
| 3 | `BRC100AuthOverlayRoot.tsx` 1655 | payment modal → *Modify limits* (`showModifyLimits`) | ✅ |
| 4 | `BRC100AuthOverlayRoot.tsx` 1923 | second payment modal → *Modify limits* | ✅ |
| 5 | `BRC100AuthOverlayRoot.tsx` 3047 | `domain_approval` → *Advanced settings* (`showAdvanced`) | ❌ `hideDisclosureSection={true}` (3055) |

⇒ **Items 2 and 3 land inside the connect and payment consent modals** — the screens 7a/7b/7c just
finished — unless they are scoped by prop. That makes them structural, not the cheap tidy-ups the
ticket implies. Owner decision `D-A`.

⚠️ Sites 3, 4 and 5 already wrap the form in **their own collapsible**. Item 3 (collapse the limits
inside the form) would put a collapsible inside a collapsible there: two clicks to reach a limit
field on the payment screen.

### 0.5 ⭐ `D-5` — Item 2's tooltip hazard is **absent** (the check the ticket asked for)

The ticket: *"⛔ Check for tooltips first … if any sub-permission row grows a tooltip, capping this
list clips it."*

Measured in `DomainPermissionForm.tsx` 511–541: each row is a plain flex `div` with a `label` string,
an optional `sublabel` string, and a `HodosButton`. Grep over the whole file: **no `InfoIcon`, no
`Tooltip`, no `position: absolute`.**

⇒ `overflow-y: auto` on that container cannot clip a tooltip. Item 2 is safe as specified — the
cheapest of the four remaining items.

### 0.6 ⭐ `D-6` — Item 3's `R-PROV` constraint does **not** bite in the management modal, and **does** on the connect path

The ticket: *"the rule may not bite — but it must be checked, not assumed."*

Checked. `DomainPermissionForm.tsx` contains **no provenance mechanism at all**: `sourceOf`,
`siteSuggests`, `shouldExpandLimits`, `manifest`, `provenance` — none appear. `shouldExpandLimits`
(`manifestConsent.ts` 243–249) is called at exactly **one** site, `BRC100AuthOverlayRoot.tsx:666`,
against the connect path's **own** limit UI, not the form's.

The Edit-Limits modal is fed `editingDomain.perTxLimitCents` etc. — the **stored row**, the user's
own values.

⇒ ✅ Collapsing is safe **in the management modal**.
⛔ Per `D-4` the same component renders on the connect and payment paths, where a site-sourced value
*can* be on screen. A collapse that is not prop-scoped re-opens `R-PROV` on the screen 7b closed.
This is the concrete reason `D-A` is a decision and not my call.

### 0.7 🚨 `D-7` — Item 1's close paths: two mechanisms on **one** path, and the ticket names the wrong one for the common case

The ticket's implementation note: *"this overlay's close paths are C++, not React … ⛔ Do not
implement this as a React `onClick` handler and assume it holds."*

| Path | Click-outside mechanism today | Guard |
|---|---|---|
| **Wallet overlay → Approved Sites → Edit Limits** — click inside the overlay, outside the dialog | **MUI `<Dialog>` backdrop** (`DomainPermissionsTab` 461–467, `onClose={() => setEditingDomain(null)}`). Escape too | ⛔ **React.** Exactly the mechanism the ticket says not to rely on — and it is the common case |
| Same path — click **outside the wallet overlay window** | `WalletOverlayWndProc` `WM_ACTIVATE(WA_INACTIVE)` → `HideWalletOverlay()` (`cef_browser_shell.cpp` 2210–2226) | ✅ C++, `g_wallet_overlay_prevent_close` — the ticket's mechanism, correct for this half |
| **Right-click → Manage Wallet Permissions** | `MENU_ID_MANAGE_PERMISSIONS` → `CreateNotificationOverlay(…, "edit_permissions", domain, "")` (`simple_handler.cpp` 10273–10300). `NotificationOverlayWndProc` (`cef_browser_shell.cpp` 2414) has **no `WM_ACTIVATE` arm**, and the `WH_MOUSE_LL` roster in `simple_app.cpp` has **no notification hook** | 🆕 **It does not close on click-outside at all today.** Only `WM_CLOSE` → `DestroyWindow` |

⇒ Item 1 is **two mechanisms on the wallet-overlay path**, not one mechanism on two paths. A
C++-only fix leaves the backdrop click — the common case — unfixed. On the right-click path there is
nothing to fix, and the risk is *adding* a guard that breaks its working `WM_CLOSE`.

### 0.8 ⛔ `D-8` — Item 5's stated motive is false: the right-click path never sees the list

The ticket: *"the entry point that matters most — right-click → 'Manage Site Permissions', the
quick-revoke path CLAUDE.md names as load-bearing — lands them in this same list."*

Measured (`simple_handler.cpp` 10273–10300): the handler takes `browser->GetMainFrame()->GetURL()`,
strips scheme/path/port to a bare domain, and opens the **single-domain** editor for that domain. It
never renders `DomainPermissionsTab`. There is no list on that path, so search cannot help it.

⇒ Item 5 still stands on its own — the wallet-overlay list is real, it pages at 12, and 15 sites
already exceed one page. But it is a **wallet-overlay-only** improvement, a smaller claim than the
argument that set its priority. Recording this because that argument is why it was ordered first.

### 0.9 ✅ `D-9` — R4 (dual store) confirmed **verbatim**, including where its comment lives

`simple_handler.cpp :: MirrorNetworkPermissionToChromium` (754) opens with, exactly:

```cpp
if (type != SitePermissionType::Loopback && type != SitePermissionType::LocalNetwork) return;
```

and the block **above** it (740–753) says in as many words: *"location/notifications/clipboard have
the SAME defect but predate this phase and are covered by their own ticket."* Call sites:
`site_permissions_set` → one call (8358); `site_permissions_reset` → two explicit calls
(8375–8376). Extending means +3 arms in the mirror **and +3 calls in reset** — reset is not a loop.

### 0.10 🚨 `D-10` — "`CEF_CONTENT_SETTING_TYPE_*` exists for geolocation, notifications and clipboard" is true but **incomplete, and the incompleteness is the risk**

Measured in `cef-binaries/include/internal/cef_types_content_settings.h`:

| Our type | CEF type(s) | Reading the header |
|---|---|---|
| **Notifications** | `…_NOTIFICATIONS` (60) | ✅ single, unambiguous |
| **Clipboard** | `…_CLIPBOARD_READ_WRITE` (208) **and** `…_CLIPBOARD_SANITIZED_WRITE` (212) | 🚨 the second is documented *"special-cased in the permissions layer to **always allow**, and as such doesn't have associated prefs data"* ⇒ **it cannot be blocked.** Only READ_WRITE is writable |
| **Location** | `…_GEOLOCATION` (59) **and** `…_GEOLOCATION_WITH_OPTIONS` (582) | 🚨 the second's own comment: the permission *"is migrating to use permissions with options, **which won't be stored as ContentSettings**"* |

⚠️ And both new types are **live in our build**: `cef_api_hash.h` sets
`CEF_API_VERSION = CEF_API_VERSION_EXPERIMENTAL` (999999), so every `CEF_API_ADDED(14000)` guard
compiles in. Engine: `CEF_VERSION 150.0.43-7871.3576+g9ccef04+chromium-150.0.7871.187`.

⇒ `P7-A6`'s RED warning (*"`GEOLOCATION` vs the newer `GEOLOCATION_WITH_OPTIONS` may not be the
setting Chromium actually consults"*) is not hypothetical. It must be **measured per type, live**,
before the mirror arm is written — and if Location turns out unenforceable through
`SetContentSetting`, that is a **different fix**, not this one. Owner decision `D-D`.

⚠️ Clipboard carries a residual **of the same shape as the defect being fixed**: after the mirror,
the panel says Block, sanitized write still works, because Chromium always allows it. Decision `D-C`.

### 0.11 ⚠️ `D-11` — The session prompt's base-commit and push claims are stale

It states base `9989935` and *"nothing since `c0b0672` has been pushed — six commits sit local-only,
so a Mac relay or a `git ls-remote` check will not see them."*

Measured: `HEAD = a052033`; `git ls-remote origin refs/heads/0.4.0 = a052033`. **Everything is
pushed**, and the base is two commits later than stated. (The owner's framing in the session opener
was the correct one.) Recorded because a Mac relay written from the prompt would have been wrong.

### 0.12 ⚠️ `D-12` — Doc drift, reported not fixed (working rule 3)

`cef-native/include/core/CLAUDE.md:173` documents the enum as `{ Camera=1, Microphone=2, Location=3,
Notifications=4, Clipboard=5 }`. The real `cef-native/include/core/SitePermissionType.h` has **seven**
— `LocalNetwork = 6`, `Loopback = 7`, added in P0.9. `kSitePermCaps` has all 7.

### 0.13 ⚠️ `D-13` — The approved-sites list renders in a **TAB**, not the wallet overlay (found while running `A1`)

My own §4 SUBJECT column originally said "the wallet overlay browser". Measured 2026-09-08:
`WalletPanel.tsx :: handleManageSites` does `cefMessage.send('tab_create',
'http://127.0.0.1:5137/wallet?tab=4')`. The wallet **overlay** is `/wallet-panel?iro=50`; the
approved-sites list is a normal browser **tab** at `/wallet?tab=4`, hosted by `WalletOverlayRoot`
(a component whose name no longer describes where it runs).

⇒ Two consequences, both recorded rather than quietly absorbed:
- The evidence rows now name the tab, and the probe resolves it by URL substring — attaching to
  `/wallet-panel` would have measured a screen with no list on it at all.
- ⛔ The CEF native-`<input>` rule does not strictly bite on a tab. The box is native anyway, and
  the **code comment now states the true reason** (the sibling component is rendered in the
  notification overlay; the host is still named `WalletOverlayRoot`) instead of the false one I
  first wrote. A comment asserting a constraint that does not apply is a claim, and it was wrong.

### 0.14 🚨🚨 `D-14` — The first negative control was a **false GREEN**, caused by Vite Fast Refresh

⛔ **This is the finding to carry forward, not the filter.**

The `A2` negative control (both guards deleted) printed **GREEN — all checks passed**. Taken at face
value it would have meant the guards were unnecessary and the bug imaginary. It was an instrument
failure:

| Run | State | `sync` | `+2 frames` | `+400 ms` |
|---|---|---|---|---|
| Guards deleted, **no reload** (HMR-applied) | `page` already reset to 0 by an earlier remount of the *guarded* build | 0 rows | 4 rows | 4 rows ⇒ **false GREEN** |
| Guards deleted, **after hard reload** | clean | 0 rows | 0 rows | **0 rows ⇒ RED** |

Vite Fast Refresh **preserves React state across an edit**. The state it preserved was `page = 0`,
produced by the very guard I had just deleted. So the probe measured leftover state from the
previous build and reported the defect absent.

⚠️ Two lessons, both already paid for elsewhere in this sprint:
1. **A hard reload is now mandatory** and is baked into `reload_and_settle()` in the probe — not left
   to whoever runs it next.
2. ⛔ **My first GREEN run was measured under the same contaminated conditions** and was therefore
   worth nothing either. It was re-run after reload. Both directions of `A2` are now reload-clean.

🚨 And a third, which is why this section is 🚨🚨: **I nearly concluded the opposite.** Faced with a
green negative control I first suspected the served bundle was stale, and "confirmed" it by grepping
the served module for the string `NEGCTRL` — which MISSed **because Vite strips comments**, not
because the code was stale. That reasoning was wrong in a way that would have sent me to fix the
wrong thing. What settled it was reading the transformed module line-by-line and, decisively,
measuring frame-by-frame after a reload. ⭐ Grep for a **behavioural** token, never a comment.

📏 Corrected failure shape: the contract predicted a footer reading `13–12 of 3`. **Measured**, the
pager *unmounts* (`totalPages` collapses to 1), so the user gets an empty table, **no footer at all**,
and a count line still claiming *"4 of 15 approved sites match "app""*. No visible clue — worse than
predicted.

### 0.15 Instrument notes — what was **not** measured, stated rather than hidden

- ✅ *"15 approved domains in the dev profile"* — **now measured**, 2026-09-08, `GET
  /domain/permissions/all` on the dev wallet (31401): exactly **15**. The ticket's figure holds, and
  15 > `ROWS_PER_PAGE` (12) is what makes `A2` reachable at all.
- ⬜ *"beta.zanaadu.com: 10 protocol + 6 basket + 1 cert grants"* — still **not re-measured**; it
  sizes items 2/3, which are not started. ⛔ Not treated as confirmed.
- ⬜ `A13`'s 👤 half — real keystrokes into the box. The probe proves the element is a native
  `INPUT` and that a value round-trips through React's onChange, but every result this session is
  **CDP-driven**: no physical keyboard or mouse has touched this control. ⚠️ Same residual the
  Phase 4 tab menu carried (memory: owner O1/O2). Owner pass owed.
- ⬜ Whether the wallet overlay currently sends `wallet_prevent_close` while the Edit-Limits dialog
  is open — read of `WalletPanelPage.tsx` not performed; it is item 1's implementation detail, and
  the mechanism (`D-7`) is established regardless.

---

## 1. Goal

The screen a user visits **after** trusting a site tells the truth about what that site has, lets
them find the site, lets them change it without losing the change by accident — and a control on it
that says *Block* actually blocks.

## 2. Done means

*(Rows follow the owner's answers to `D-A`–`D-E`. Amended in the same commit if the answers differ,
per HARNESS §1.)*

- [ ] Typing in the approved-sites filter narrows the list **and** the pager agrees with what is on
      screen — never an empty table with a non-empty count.
- [ ] Filtering never changes which site Edit or Revoke acts on.
- [ ] Setting **Notifications** to Block in Site controls ⇒ `Notification.requestPermission()`
      resolves `"denied"` and no prompt appears. Measured on the **site**, not the panel.
- [ ] Same for **Location** and **Clipboard** — or, where the platform cannot deliver it, the
      limitation is written down and the panel does not claim otherwise (`D-C`, `D-D`).
- [ ] **Reset** restores Ask in both stores for all three.
- [ ] Camera/mic **unchanged** — still our store only, still no Chromium content setting written.
- [ ] *(item 1)* The Edit-Limits modal does not discard unsaved edits on a stray click, on **either**
      of the wallet-overlay close mechanisms.
- [ ] *(items 2/3, if `D-A` permits)* The granted list caps and scrolls; the limits section
      collapses — **on the management modal only**, with the connect and payment modals byte-identical
      to their 7c state.

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| — *(named, not in the set)* | **"Manage Site Permissions"** — CLAUDE.md load-bearing safeguard | Both tickets land on it. `D-7`: the right-click path currently has *no* click-outside close; adding a guard there could break its `WM_CLOSE` and strand the overlay. |
| `R-CLOSE` | overlay close guards | Item 1 edits close behaviour directly, on the shared notification overlay **and** the wallet overlay. |
| `R-PROV` | a user must not approve values hidden behind a collapse (P0.8 §6a) | ⛔ **Live via `D-4`+`D-6`.** Safe in the management modal, unsafe on the connect path — and it is **one component**. |
| `R-PERIM` | the four privacy-perimeter gates | Not edited. But R4 changes *which store governs* a permission decision, and Phase 5's rule applies: **a narrowing is a privilege change.** Take the row at the boundary. |
| `R-GOLD` / `R-COUNT` | payment indicator, session counters | Sites 3 and 4 of `D-4` are the **payment** modals. If items 2/3 reach them, the gold pill's screen changed. |

## 4. Evidence table

⛔ No empty RED or SUBJECT cells. Rows drafted now, **run after the code**; `Result` stays ⬜ until
observed.

| ID | 🟢 GREEN — must be true | 🔴 RED — must be *seen* to fail, and how | 🎯 SUBJECT — proves the right thing was measured | Tier | Result |
|---|---|---|---|---|---|
| `P7d-A1` | Typing a substring narrows the approved-sites list to matching domains | Delete the filter predicate (pass-through) → the same query returns all rows and the test names the query | The **rendered rows**, read from the DOM. ⚠️ 📏 The subject is a **tab** at `/wallet?tab=4`, not an overlay — see `D-13`. Probe resolves it by URL substring via `phase-1/cdp.py :: pick`, which errors on ambiguity | T2 | 🟢 **`app` → 4/15 rows, all matching, 2026-09-08** |
| `P7d-A2` | 🚨 With >12 sites, filtering **from page 2** shows the matches and the count agrees with the table | Both guards deleted → **0 rows** at 400 ms while the count line still reads *"4 of 15 approved sites match"*. ⛔ **Only reproduces after a hard reload** — see `D-14` | The **count line and the row count together**. A row-count-only assertion passes on the broken build | T1 + T2 | 🟢 **GREEN with guards; RED without, 2026-09-08** |
| `P7d-A3` | Edit / Revoke act on the domain the user clicked, **while a filter is active** | Re-key one action to `paged[i]` by index → filter, click row 1, observe the wrong domain in the confirm dialog | The **domain string** of the first rendered row vs the filtered set. Per `D-3` this passes today — a **regression guard**, labelled as one, not as a discovered bug | T1 | 🟢 **first filtered row in set, 2026-09-08** |
| `P7d-A4` | Site controls → **Notifications = Block** ⇒ `Notification.requestPermission()` resolves `"denied"`, no prompt | ⛔ **Before** the mirror: same steps leave the site allowed and no prompt appears. Shipped behaviour, captured as the control before any code | The **JS promise's resolved value** on the page. The panel showing "Block" is the lie under test | T2 | ⬜ |
| `P7d-A5` | Same for **Location** — `navigator.geolocation.getCurrentPosition` errors `PERMISSION_DENIED` | Per type, not once for all three. ⛔ First determine **which** of `GEOLOCATION` / `GEOLOCATION_WITH_OPTIONS` our build consults (`D-10`) — writing the unconsulted one produces a green content setting and an unchanged site | Chromium's **stored content setting** *and* the site's behaviour. One without the other has been wrong on this exact surface before | T2 | ⬜ |
| `P7d-A6` | Same for **Clipboard** — `navigator.clipboard.readText()` rejects | ⛔ Assert the **residual too**: sanitized write stays allowed (CEF header: "special-cased … always allow"). A test that only checks read passes while the panel over-claims | `CLIPBOARD_READ_WRITE` behaviour **and** a sanitized-write probe. The residual is a result, not an excuse | T2 | ⬜ |
| `P7d-A7` | **Reset** returns all three to Ask in **both** stores, and the site prompts again | Reset with only the SQLite half wired → our rows read Ask while Chromium's exception survives and the site stays allowed | Both stores read back, **plus** the site's next request producing a prompt | T2 | ⬜ |
| `P7d-A8` | **Camera/mic unchanged** — our store governs, Chromium has **no** entry | Add a mirror arm for mic, observe the media path change, revert it | `site_permissions.db` has a mic row and Chromium has none — the asymmetry in the ticket's evidence table is the control. `A7` of Phase 7 | T2 | ⬜ |
| `P7d-A9` *(item 1)* | An unsaved edit survives a click outside the **dialog** and a click outside the **overlay window** | ⛔ **Two probes, one per mechanism** (`D-7`). Guard only the C++ half → the backdrop click still discards, and the backdrop click is the common case. A single probe would pass on a half-fix | Which **mechanism** fired: MUI `onClose` reason vs `WM_ACTIVATE` in the browser log | T2 + T3 👤 | ⬜ |
| `P7d-A10` *(item 1)* | The right-click `edit_permissions` overlay still closes on its own Close/OK | Item 1's guard is a real risk to a path that has **no** click-outside close today (`D-7`) — break `WM_CLOSE` and the overlay strands | The **notification overlay HWND** actually going away, not the React view changing | T2 | ⬜ |
| `P7d-A11` *(items 2/3)* | Granted list caps + scrolls; limits collapse — **management modal only** | Render sites 3/4/5 of `D-4` before and after; any pixel difference on the payment or connect modal is the red | The **connect and payment modals**, not the one being changed. `R-PROV` lives on those | T1 + T3 👤 | ⬜ |
| `P7d-A12` | 👤 **Human reads every changed screen** — filter box legible and focusable, list not clipped, contrast holds | P0.8 shipped six defects here with every gate green. The record is the red; the control is that a person looked | Owner's eyes on the **rendered** panel at 100% and DPI cells #4/#6/#9. ⛔ Not a screenshot I took and did not read | T3 👤 | ⬜ |
| `P7d-A13` | The filter box is a **native `<input>`** and characters reach it | Swap it for a MUI `TextField` → `boxTag` stops being `INPUT`, or the typed value never lands | **`tagName` and the value read back after typing**, not the element merely rendering | T2 👤 | 🟢 **`INPUT`, value round-trips, 2026-09-08.** ⚠️ 👤 half (real keystrokes) still owed |
| `P7d-A14` | A query matching nothing shows an explicit message, not a bare empty table; **Clear** restores the list | Remove the `filtered.length === 0` arm → headers over an empty body with no explanation | The **rendered message text** and the row count after Clear | T2 | 🟢 **`zzzznope` → no table, message shown, Clear restores, 2026-09-08** |

**Two-sided pairings:** `A4`/`A5`/`A6` ↔ `A8` (three types gain a mirror; camera/mic must not) ·
`A9`'s two mechanisms · `A11`'s changed-screen ↔ unchanged-screen · `A2`'s count ↔ footer.

## 5. Blast radius

- **`DomainPermissionsTab.tsx`** — the list, its sort, its pager. Items 5 and the `A2` reset bug.
- **`DomainPermissionForm.tsx`** — ⚠️ **five render sites** (`D-4`), four of them consent/payment
  modals. Any change here without a prop gate reaches the screens 7a/7b/7c settled.
- **`DomainPermissionsTab.tsx` MUI `<Dialog>` + `WalletOverlayWndProc`** — item 1, two mechanisms.
- **`NotificationOverlayWndProc` / `MENU_ID_MANAGE_PERMISSIONS`** — the right-click path. Currently
  has no click-outside close; the risk is a regression, not a fix.
- **`simple_handler.cpp :: MirrorNetworkPermissionToChromium`, `site_permissions_set`,
  `site_permissions_reset`** — R4. ⚠️ CEF's own header warns incorrect `SetContentSetting` use
  destabilises Chromium; scope strictly to the three types decided in `D-C`/`D-D`.
- **Not touched, deliberately:** `matrix_c.rs` and the permission engine — this phase changes no
  wallet-permission decision. `SitePermissionType.h` ids — frozen, stored in SQLite.

## 6. Out of scope

- **Item 4 (persist denials)** — a schema change (CLAUDE.md invariant #2) needing an explicit owner
  yes, and it must land with the `TICKET_prompt_denials_should_not_persist.md` reconciliation stated
  in the same commit. Decision `D-B`.
- **always-deny** — deliberately deferred by the ticket to a protocol gap; do not build it.
- The **omnibox / new-tab / bookmarks favicon leaks** (Phase 7 §0.3) — still open, still not this.
- `TICKET_wallet_quiet_detector_blind_to_long_polls.md` and the 7c adversarial review — owed from
  the 7c boundary, listed in §8, not folded in here.
- macOS beyond a relay entry.

## 7. Rollback

Each item is an independent commit citing its own `P7d-A*` ids; `git revert` any one alone. The
riskiest is R4 — three added arms in one `if` plus three calls in `site_permissions_reset`.
Reverting restores today's shipped (broken) behaviour exactly. ⚠️ It does **not** unwind content
settings already written to the profile; a revert must be followed by a Reset on any site touched, or
the Chromium exception outlives the code that wrote it.

## 8. ⛔ Decisions owed BEFORE the first commit

| # | Decision | Why it is yours, not mine |
|---|---|---|
| **D-A** | 🚨 Items 2 + 3: **prop-gate to the management modal**, or let them land on the connect and payment modals too? | `D-4` — one component, five sites. My recommendation: **prop-gate, default off.** 7a/7b/7c just settled those screens, `R-PROV` lives on them (`D-6`), and two of them are the payment path. But a prop gate is a deliberate divergence between screens, and that is a product call. |
| **D-B** | Item 4 (persist denials): **defer to beta.4**, or do it here? | Schema change, invariant #2. Recommendation: **defer.** It is the only item in the ticket that cannot be reverted with `git revert`. |
| **D-C** | Clipboard: mirror `CLIPBOARD_READ_WRITE` and **accept** that sanitized write stays allowed — silently, or with a note on the panel? | `D-10`. The residual is the same shape as the defect ("panel says Block, something still works"). Recommendation: **mirror READ_WRITE, and say so on the panel** — one line, because shipping a quieter version of the bug we are fixing is how it comes back. |
| **D-D** | Location: if measurement shows our build consults `GEOLOCATION_WITH_OPTIONS` — which the header says *"won't be stored as ContentSettings"* — do we **ship Notifications + Clipboard and file Location**, or hold all three? | `D-10`. Recommendation: **ship what works, file the rest with the measurement attached.** ⛔ What I will not do is write `GEOLOCATION`, watch the content setting appear, and call the row green — that is the vacuous-probe failure this sprint keeps paying for. |
| **D-E** | Order: your stated order is **item 5 first, then 1–3**. R4 (the dual store) is the row with teeth. Confirm **5 → R4 → 1 → 2/3**, or keep R4 last? | Recommendation: **5 → R4 → 1 → 2/3.** Item 5 first for your stated reason (finding a site stops being a scroll, which every later test needs); R4 second because it is the shipped safety defect and items 1–3 are usability. |

### 8.1 Answers — 2026-09-08, owner

All five recommendations accepted as written.

| # | Decision |
|---|---|
| **D-A** | ✅ **Prop-gate items 2 + 3 to the management modal, default off.** The connect and payment modals must be byte-identical to their 7c state — `A11` is the check. |
| **D-B** | ✅ **Item 4 (persist denials) deferred to beta.4.** Not in this phase. Schema change, invariant #2. |
| **D-C** | ✅ **Mirror `CLIPBOARD_READ_WRITE` and say so on the panel.** The sanitized-write residual is disclosed, not silent. |
| **D-D** | ✅ **Ship what is enforceable; file the rest with the measurement attached.** ⛔ Writing `GEOLOCATION`, seeing the content setting appear, and calling `A5` green is explicitly not a pass. |
| **D-E** | ✅ **Order: 5 → R4 → 1 → 2/3.** |

### 8.2 Progress

| Item | State |
|---|---|
| **5 — search box** | ✅ **Done.** `A1`, `A2`, `A3`, `A13` (probe half), `A14` green; `A2` RED observed. 👤 owner pass owed. |
| **R4 — dual store** | ⬜ Next. |
| **1 — close guard** | ⬜ |
| **2 / 3 — scroll box + collapse** | ⬜ |

---

## 9. Owed from the 7c boundary — carried, not absorbed

- `R-INTEXT` end-to-end, both halves, stubbed in both directions — **unrun.**
- The 7c adversarial review — **not done.**
- `TICKET_wallet_quiet_detector_blind_to_long_polls.md` — ⛔ its negative control needs a **first**
  connect to a manifest site with a slow auth step. A test on an already-approved site *cannot fail*,
  which is how the bug survived.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1 -Full` run — result + date recorded below. ⛔ Bare `preflight.ps1` skips
      `T1d` and prints *"0 failed, 1 skipped. This is NOT a pass."* A skip is never a pass
- [ ] `scripts/preflight.ps1 -NegativeControl` run — every T0 gate seen to fail
- [ ] `../REGRESSION_SET.md` run in full at this boundary
- [ ] Adversarial review complete, four questions answered in writing
- [ ] 👤 Owner has **looked at** every changed screen (`P7d-A12`)
- [ ] Any baseline lowered in `../HARNESS.md` §4, residuals listed with reasons
- [ ] Commit messages cite the row IDs they satisfy

| Item | Result | Date | By |
|---|---|---|---|
| preflight -Full | | | |
| preflight -NegativeControl | | | |
| regression set | | | |
| adversarial review | | | |
| owner human-eyes pass | | | |
