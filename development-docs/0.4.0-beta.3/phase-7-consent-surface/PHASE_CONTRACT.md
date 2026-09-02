# Phase 7 — the consent surface (ticket bundle) · PHASE CONTRACT

**Workstream:** ticket consolidation (`SPRINT_PLAN.md` §4.1) · **Tickets:** six, listed in §0 ·
**Status:** 🚧 **KICKOFF COMPLETE — AWAITING OWNER CONFIRMATION. No code written.**
**Opened:** 2026-09-02 · **Owner:** Matthew Archbold · **Platforms:** Windows (macOS = relay)
**Standard:** `../HARNESS.md`. **Base commit:** `887c2cd` (== `origin/0.4.0`, verified against the
remote by `git ls-remote`, not by a stale ref — no Mac pushes since Phase 6 closed).

---

## 0. Plan-vs-tree delta — measured before anything was written

> ⛔ Every claim below is a **code reading** unless it says *measured*. Nothing in this section is a
> live-run result; the live runs are §4. Labelling one as the other is the thing HARNESS §6 Q4 bans.

### 0.1 The four rows the plan called work — all four confirmed still live

| Row | Plan's claim | Tree, 2026-09-02 | Verdict |
|---|---|---|---|
| **R1 favicon** | consent modals fetch `google.com/s2/favicons` | **11** call sites in `BRC100AuthOverlayRoot.tsx` (lines 1518, 1581, 1757, 1872, 2026, 2109, 2268, 2529, 2911, 3103, 3279) | ✅ **REAL — and wider than filed**, see 0.3 |
| **R2 quiet mode** | `bundled_scope_grant` short-circuits before `scoped_grant_exists` | `matrix_c.rs :: decide_scoped_grant` — the two `if`s are in exactly the ticketed order, unchanged | ✅ **REAL, verbatim** |
| **R3 branding** | 7 of 28 branded; unmapped ⇒ stock Chrome | `kSitePermCaps` has **7** entries; `OnShowPermissionPrompt` `return false` on `types.empty()` | ✅ **REAL** — plus two *new* fall-through paths the ticket did not name, see 0.4 |
| **R4 dual store** | mirror covers only the two network types | `MirrorNetworkPermissionToChromium` opens with `if (type != Loopback && type != LocalNetwork) return;` — and its own comment says location/notifications/clipboard "have the SAME defect" | ✅ **REAL, and self-documented in the code** |

### 0.2 ❔ The two triage rows — verdicts

#### ❔ R5 `connect_modal_two_views_drift` → **PARTLY CLOSED. The drift MOVED, exactly as the ticket's status line suspected.**

The intra-`manifest_connect_bundle` drift the ticket was filed about is **mostly** closed — the
visible label text now matches. **The tooltips do not.**

| | Summary | Customize | Same? |
|---|---|---|---|
| Quiet-mode **label** | 2795 — *"…: **let** this site use…"* | 3039 — *"…: **Let** this site use…"* | ⚠️ one letter of case. Trivial, but line 2794's own comment says *"keep them identical"* |
| Identity **label** | 2752 | 3025 | ✅ byte-identical |
| Quiet-mode **tooltip** | 2797 — a full `InfoIcon tooltip=…` paragraph | 3039–3041 — **no `InfoIcon` at all** | 🔴 **absent on Customize** |
| Identity **tooltip** | 2753 — `<InfoIcon />`, inherits the default copy at line 186 | 3024–3026 — **no `InfoIcon` at all** | 🔴 **absent on Customize** |

🚨 **This is a live instance of the ticket's own thesis, not a cosmetic gap.** The identity tooltip is
the *only* place in the product that says *"This checkbox is the ONLY control over your identity key —
no permission a site lists can grant it, whatever the site calls it."* That sentence was **shipped
specifically to resolve the owner's confusion** recorded in this ticket's "third instance" section.
A user who clicks **Customize** — the view the owner said they preferred, and the only one offering
per-item ticks — never sees it. The drift did not close; it moved from the label into the tooltip.

**And there is a third view the P0.8 fix never touched.** `notificationType === 'domain_approval'`
(branch opens line 3094) still carries the **pre-fix, systematically-less-alarming** copy for both
checkboxes:

| | `domain_approval` (line) | The corrected copy |
|---|---|---|
| Quiet mode | **3207** — *"Allow this site to perform wallet operations without asking each time"* | *"Quiet mode: let this site use **any** protocol or basket without asking — **including ones it did not list above**"* |
| Identity | **3183** — *"Allow this site to identify you"* | *"Identity: Allow this site to identify you **across the Metanet**"* |

Its quiet-mode tooltip (3208) is also the weaker text — it says the wallet "won't prompt you for
individual protocol, basket, or counterparty grants", never that undeclared ones are included.
The identity `<InfoIcon />` at 3184 takes no `tooltip` prop, so it *does* inherit the good default
copy from line 186. So on this view the tooltip is right and the visible label is wrong.

⚠️ **This view is not a rare fallback — Phase 0.8 made it MORE common.** `ManifestFetcher`'s
`valid == true` now requires at least one *recognised* permission, so a site with a permission-free
or unrecognised manifest falls back here. The comment at line 2643 in the connect-bundle branch says
so in as many words: *"Unreachable since beta.3 Phase 0.8 … the interceptor falls back to the plain
domain_approval modal."*

⇒ **Verdict: work, but not the work the ticket proposed.** The copy fix on the third view is small
and safe. The *structural* proposal (collapse to one view) is unchanged and is still a redesign of
the primary consent screen for money and identity. Owner decision `D4` in §0.5.

#### ❔ R6 `manifest_description_can_misdescribe_protocol` → **STILL REAL, but narrower than filed.**

Neither of the ticket's three fixes is implemented. The protocol row renders the site's sentence
alone:

- Summary, line 2573: `{p.purpose || \`Use protocol "${p.name}"\`}` — the ID appears **only when the
  site supplies no purpose**, i.e. never in the attack the ticket describes.
- Customize, line 2970: `{p.purpose || p.name}` — same shape, same gap.

**What changed since filing** (2026-08-23, after the ticket): the *"Who these are with"* footnote
(line 2674) renders `{p.name}` for every `securityLevel === 2` protocol. The ticket's own worked
example is `"protocolID": [2, "3241645161d8"]` — **security level 2** — so that specific attack now
*does* put `3241645161d8` on screen, in the footnote, below the deceptive sentence.

⇒ The residual is **level 0 and level 1 protocols**, for which no identifier is displayed anywhere.
That is a real hole, but it is a smaller and better-defined one than the ticket states, and the
ticket's cheapest fix (#1, show the ID beside the description) closes it for all levels uniformly.

⇒ **Verdict: real work. Scope it to fix #1 only** — the gloss table (#2) and the §9.1 inconsistency
scan (#3) are separate, larger, and neither is needed to close the hole. Owner decision `D5`.

### 0.3 🆕 The favicon leak is wider than the ticket — three sites outside the consent surface

Not in the ticket, found by inventorying the tree. **All three send a domain to Google.**

| File | Line | Fires when | Leaks |
|---|---|---|---|
| `OmniboxOverlayRoot.tsx` | 166 (`sz=16`) | **as the user types** | every autocomplete-matched domain, keystroke by keystroke |
| `NewTabPage.tsx` | 45 | new-tab open | the user's most-visited sites |
| `BookmarksOverlayRoot.tsx` | 22 | bookmarks panel open | the user's bookmark list |

⚠️ **The omnibox one is arguably worse than the row this phase was scheduled for.** The consent modal
leaks one domain at the moment of one decision; the omnibox leaks a stream of them during ordinary
typing, in a browser sold on privacy. It is the same fix (`Tab::favicon_url` / a local source) and
the same test instrument.

⛔ It is **outside §4's scope fence**, which names the consent surface. I have not widened scope on my
own. Owner decision `D1`.

### 0.4 🆕 Two branding fall-through paths the ticket does not name

`OnShowPermissionPrompt` returns `false` — i.e. hands the user a stock Chrome bubble — for **more
than just unmapped types**:

```cpp
if (secure && types.size() == 1 && AllAsk(states)) { … }
return false;  // mixed / multi-type / insecure / busy → Chromium prompt
```

So a **multi-type request** (the common camera+mic pair) and an **insecure origin** get Chrome's UI
today for permissions we have already branded. The ticket's §5 review criterion 1 asks a reviewer to
find exactly this; it is here, and it means "brand the remaining 21" does **not** by itself achieve
the ticket's §1 goal ("no Hodos user should ever see a stock Chrome permission bubble").

Also: `FireHodosPermissionPrompt` is gated on `PendingPermissionManager::hasPending()` for the
network types — one at a time. A second prompt while one is parked falls through to Chrome.

### 0.5 ⛔ Decisions owed BEFORE the first commit

| # | Decision | Why it is yours, not mine |
|---|---|---|
| **D1** | Favicon: consent surface only (11 sites), or **also** omnibox / new tab / bookmarks (3 more)? | Scope-fence widening. My recommendation: **include them** — same fix, same test, and the omnibox leak is the larger one. But it is a fence, and §4 of the session prompt drew it. |
| **D2** | 🚨 Quiet-mode engine narrowing (R2): **do it, or defer it?** | CLAUDE.md invariants #2/#3 — a behaviour change on the auto-approve path. The ticket itself says it "needs its own contract, its own RED/GREEN, and an explicit owner yes". ⛔ **I will not touch `matrix_c.rs` without that yes.** See the sub-question below. |
| **D3** | Branding (R3): the **mechanism + 6 high-risk copy** (ticket §3a-c), or mechanism-only, or also the two Group C surfaces? | ⚠️ The inventory's own ordering says the two highest-value items are `OnJSDialog` (every `alert()` on every site draws Chrome's dialog) and `GetAuthCredentials` (**a stock Chrome password box inside a Hodos window**) — and the ticket §6 puts both **out of scope**. The ticket's priority and the inventory's priority disagree. That disagreement is the decision. |
| **D4** | Two views (R5): **copy fix on `domain_approval` only**, or the full collapse-to-one-view redesign? | Recommendation: **copy fix now, redesign not in beta.3.** The redesign needs fresh live testing of all five prompt paths plus the DPI matrix, and it is the primary consent screen for money. |
| **D5** | Misdescription (R6): fix #1 (**show the protocol ID**) only? | Recommendation: **yes, #1 only.** #2 and #3 are their own work and neither is needed to close the hole. |

**D2 sub-question, which decides how big D2 is.** The ticket names a migration problem I confirmed:
existing rows carry `bundled_scope_grant = 1` with no record of what was declared at connect time.
`domain_manifest_snapshots` (V24) covers sites approved *after* Phase 0.8 only. For older rows there
is nothing to narrow *to*, so the change must degrade to **"prompt on first use"**, never "deny".
If that is acceptable, D2 is bounded. If it is not, D2 is not a beta.3-sized row.

---

## 1. Goal

When a Hodos user is asked to trust a site, the screen they read tells the truth about what they are
granting, is legible, is the same wording wherever it appears, and — while asking — discloses nothing
to a third party.

## 2. Done means

*(Rows follow the owner's answers to `D1`–`D5`. Written against my recommendations; amended in the
same commit if the answers differ, per HARNESS §1.)*

- [ ] Opening any consent modal produces **zero outbound requests to `google.com`** — observed on the
      wire, not inferred from a rendered icon.
- [ ] The identity and quiet-mode checkboxes carry **the same label and the same tooltip** on all
      three views that show them (`manifest_connect_bundle` summary, its Customize subview, and
      `domain_approval`) — in particular, the *"ONLY control over your identity key"* sentence is
      reachable from **Customize**, which is where the user goes to tick things individually.
- [ ] A protocol's declared identifier is on screen next to the site's description of it, at **every**
      security level — so a site's sentence can never be the only thing describing its own grant.
- [ ] Setting Location / Notifications / Clipboard to **Block** in Site controls changes the site's
      **actual behaviour**, not only the panel's display.
- [ ] *(if D3 = brand)* No stock Chrome permission bubble reaches the user for any single-type secure
      request, **including a type a future Chromium bump adds**.
- [ ] *(if D2 = yes)* Quiet mode silences only what the manifest declared or the user granted;
      anything else prompts, and the prompt reaches the user rather than failing the call.

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-PERIM` | the four privacy-perimeter gates | **R2 edits `matrix_c.rs` directly** — the file the invariant is defined against. A narrowing that mis-orders the cascade could silence a gate instead of the flag. ⛔ Sensitive cert fields must still prompt unconditionally. |
| `R-INTEXT` | internal never prompts, external always gates | R2 changes what "silent" means on the scoped-grant path. If narrowing leaks into the internal path, the wallet UI starts prompting itself. Both halves, correct SUBJECT (Rust's view of `X-Requesting-Domain`). |
| `R-CLOSE` | overlay close guards | R1 and R3 both touch the shared notification overlay — the surface Phase 0.9 left an invisible click-eating window on. |
| — *(named, not in the set)* | **"Manage Site Permissions"** — CLAUDE.md load-bearing safeguard | R4 changes *which store governs*. Phase 5's lesson applies exactly: **a narrowing is a privilege change.** The failure mode is a gate that silently stops gating. |
| `R-GOLD` / `R-COUNT` | payment indicator, session counters | Owed from every prior boundary. Not at risk from this phase's code, but if any real payment happens this session, take them. |

## 4. Evidence table

⛔ No empty RED or SUBJECT cells. Rows are drafted now and **run after the code**; `Result` stays ⬜
until observed. The two baseline rows at the bottom are the exception — they are already run.

| ID | 🟢 GREEN — must be true | 🔴 RED — must be *seen* to fail, and how | 🎯 SUBJECT — proves the right thing was measured | Tier | Result |
|---|---|---|---|---|---|
| `P7-A1` | Opening each consent modal type emits **no request to any `google.com` host** | Revert the favicon change on one branch, reopen that modal → the `google.com/s2/favicons` request reappears in the capture | The **network**, not the icon. Capture on the wire (CDP `Network.requestWillBeSent` on the *overlay* browser + an OS-level check). ⚠️ Hodos's overlays are separate CEF browsers that CDP all reports as `type:"page"` — name which target was attached | T2 | ⬜ |
| `P7-A2` | The page's own favicon renders in the consent modal for a site that has one | Point the resolver at a host with no favicon → falls back to the **domain-initial avatar**, never to a wrong site's icon and never to Google | A site whose favicon is at a **non-default path** (`<link rel=icon>`), so a `/favicon.ico` guess would fail and `Tab::favicon_url` succeeds — this distinguishes the two implementations | T3 👤 | ⬜ |
| `P7-A3` | Identity + quiet-mode **label and tooltip** identical across all three views | Change one view's string by one word, and separately delete one view's `InfoIcon`, → the test goes red naming that view. ⛔ Two probes: a label-only check would pass today, which is how the tooltip half went unnoticed | The **rendered** strings *and the presence of the tooltip* on all three branches. Checking source constants would miss it — the Customize drift is a missing JSX element, not a different string | T1 | ⬜ |
| `P7-A4` | A protocol's identifier is displayed beside the site's description, at security level **0, 1 and 2** | Fixture: `[1, "3241645161d8"]` + `description: "Show your profile picture."` → without the fix, the string `3241645161d8` appears **nowhere** on screen | A **level-1** protocol. ⛔ A level-2 fixture would pass today via the "Who these are with" footnote — that is exactly the vacuous test this phase must not write | T1 + T3 👤 | ⬜ |
| `P7-A5` | Site controls → Notifications = **Block** ⇒ `Notification.requestPermission()` is denied | Before the mirror: the same steps leave the site **allowed** and no prompt appears. That is the red, and it is **shipped behaviour** — capture it before writing the fix | The **site's behaviour** (the JS promise's resolved value), not the panel's display. The panel showing "Block" is the lie under test | T2 | ⬜ |
| `P7-A6` | Same for **Location** and **Clipboard**, and **Reset** restores Ask for all three | Per type, not once for all three — `GEOLOCATION` vs the newer `GEOLOCATION_WITH_OPTIONS` may not be the setting Chromium actually consults | Chromium's stored content setting **and** the site's behaviour. One without the other has been wrong here before | T2 | ⬜ |
| `P7-A7` | Camera/mic are **unchanged** — still governed by our store, no content setting written | Add a mirror arm for mic, observe the media path change behaviour → revert it | `site_permissions.db` has a mic row and Chromium has **none** — the asymmetry in the ticket's evidence table is the control | T2 | ⬜ |
| `P7-A8` *(D3)* | No stock Chrome bubble for a single-type secure request of **any** of the 28 types | Delete one mapping arm → that type falls to Chrome's bubble, and exactly the right unit test goes red | A **real page** raising the permission. Per ticket §4.5: a type that cannot be triggered is recorded **unexercised**, never passed | T1 + T3 👤 | ⬜ |
| `P7-A9` *(D3)* | The six high-risk types never show the generic label | Point one at the generic label → its test goes red | The six named in the ticket §3c. ⛔ Generic copy must not be **vaguer** than Chrome's for that type | T1 | ⬜ |
| `P7-A10` *(D3)* | Stored `SitePermissionType` ids unchanged for the existing 7 | Renumber one → the frozen-id test goes red | `site_permission_mapping_test.cpp`. These integers have been reassigned once already in this project | T1 | ⬜ |
| `P7-A11` *(D2)* | Quiet mode silences a **declared** protocol; an **undeclared** one prompts, and the prompt reaches the user | Two-sided, each the other's control: a declared protocol that starts prompting is as much a failure as an undeclared one that stays silent | The Rust `PermissionDecision` **kind + reason**, per `R-PERIM`'s SUBJECT rule. A modal rendering is not proof the engine decided to prompt | T1 + T2 | ⬜ |
| `P7-A12` *(D2)* | A pre-Phase-0.8 domain with no manifest snapshot **prompts on first use** — never denies, never stays blanket-silent | Seed a `bundled_scope_grant = 1` row with no snapshot → observe which of the three it does | A row in the **dev** DB with no `domain_manifest_snapshots` entry. ⛔ A fresh profile is not a fresh test — one wallet DB serves all profiles | T2 | ⬜ |
| `P7-A13` | 👤 **Human reads every changed screen.** Labels legible, not clipped, not shattered across lines, contrast passes at both provenance states | Phase 0.8 shipped **six** defects here with every gate green. The red is the historical record; the control is that a human looked | Owner's eyes on the **rendered** modal at 100% and at the DPI matrix cells. ⛔ Not a screenshot I took and did not read | T3 👤 | ⬜ |
| `P7-T1f` | `manifest_consent_t1f.mjs` green | ticket §7 negative control | real `manifestConsent.ts` via node | T1 | 🟢 **26 checks pass, baseline run 2026-09-02, pre-code** |
| `P7-T1g` | `limit_field_contrast_t1g.mjs` green | ditto | measured contrast ratios | T1 | 🟢 **5 checks pass, baseline run 2026-09-02, pre-code** |

**Two-sided pairings:** `A5`↔`A7` (the three types gain a mirror; camera/mic must not),
`A11`'s own two halves, `A2`'s render↔fallback.

## 5. Blast radius

- **`BRC100AuthOverlayRoot.tsx` (3462 lines)** — 11 favicon sites across 8 modal branches; the
  `domain_approval` branch (3094–3269); the connect-bundle summary + Customize protocol lists. Every
  consent modal in the browser is in this one file, so a JSX slip breaks paths this phase is not about.
- **The shared notification overlay** — keep-alive, multiplexes wallet modals, payments, cert
  disclosure and permission prompts. ⚠️ Two of Phase 0.8's six defects were *"long-lived overlay,
  state written as if freshly mounted."* Anything added here must ask what else is initialised once.
- **`simple_handler.cpp`** — `MirrorNetworkPermissionToChromium`, `kSitePermCaps`,
  `OnShowPermissionPrompt`, `FireHodosPermissionPrompt`, and (for R1) the notification-overlay entry
  where a favicon param would be appended. `SetContentSetting` carries CEF's own warning that
  incorrect use destabilises Chromium.
- **`matrix_c.rs` (D2 only)** — the decision engine. `R-PERIM`'s subject.
- **Not touched, deliberately:** `ManifestFetcher.cpp` / `manifest.rs`. R6 fix #1 is **display only**;
  the engine must keep treating `protocolName` as opaque. The two-layer parser's "change both or
  neither" rule does not fire because neither is changed.

## 6. Out of scope

- The **collapse-to-one-view redesign** (R5's structural half) unless `D4` says otherwise.
- The **gloss table** and the **BRC-116 §9.1 inconsistency scan** (R6 fixes #2, #3).
- `OnJSDialog` / `GetAuthCredentials` (Group C) and Group B's no-hook bubbles, unless `D3` says
  otherwise. ⚠️ Recorded here **because I was tempted**: the inventory ranks them above this phase's
  own rows.
- The routing predicate (beta.4 W4/W6/W7/W8) · the money path (Phase 8) · anything needing a real
  install (→ `INSTALL_TEST_BATCH.md`) · macOS beyond a relay entry.
- `TICKET_prompt_denials_should_not_persist.md` — same three types, adjacent, **not this bundle**.

## 7. Rollback

Each row is an independent commit citing its own `P7-A*` ids; `git revert` any one of them alone.
The riskiest, R2 (`matrix_c.rs`), is a **single reordering of two `if` statements plus its tests** —
reverting restores blanket quiet mode exactly. No migration is written, so no DB state to unwind.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1` run — result + date recorded below
- [ ] `scripts/preflight.ps1 -NegativeControl` run — every T0 gate seen to fail
- [ ] `../REGRESSION_SET.md` run in full at this boundary — `R-PERIM` and `R-INTEXT` are **live here**
- [ ] Adversarial review complete, four questions answered in writing
- [ ] 👤 Owner has **looked at** every changed consent screen (`P7-A13`) — not a gate, a person
- [ ] Any baseline lowered in `../HARNESS.md` §4, residuals listed with reasons
- [ ] Commit messages cite the row IDs they satisfy

| Item | Result | Date | By |
|---|---|---|---|
| preflight | | | |
| preflight -NegativeControl | | | |
| regression set | | | |
| adversarial review | | | |
| owner human-eyes pass | | | |
