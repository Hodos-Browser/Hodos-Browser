# Phase 7c — narrowing quiet mode (`bundled_scope_grant`) · PHASE CONTRACT

**Workstream:** Phase 7 consent surface, row **R2** · **Ticket:** `TICKET_quiet_mode_wider_than_manifest.md`
**Status:** 🚧 **CODE WRITTEN, NOT COMMITTED.** Owner answered `D1`–`D4` on 2026-09-07 (§9).
T0/T1 green; the live rows (`A2`–`A5`, `A11`) are **not yet run** — they need the dev stack back up.
**Opened:** 2026-09-04 · **Owner:** Matthew Archbold · **Platforms:** Windows (macOS = relay)
**Standard:** `../HARNESS.md`. **Base commit:** `c0b0672` (== `origin/0.4.0`, tree clean).
**Predecessors:** 7a (`c7ce5c6`), 7b (`7dc00e4`…`ced5673`) — both landed and pushed.

---

## 0. Plan-vs-tree delta — measured before anything was written

> ⛔ Every claim is a **code reading** or a **database/log query**, labelled as such. Nothing here is
> a live-run result; live runs are §5.

### 0.1 🚨 The parent contract's fix description is a **no-op**. Reordering the two `if`s changes nothing.

`phase-7-consent-surface/PHASE_CONTRACT.md` §7 describes R2 as *"a single reordering of two `if`
statements plus its tests"*. **Code reading — `matrix_c.rs :: decide_scoped_grant` (lines 163–195):**

```rust
if ctx.bundled_scope_grant   { return silent(SilentBundledScopeGrant); }   // A
if ctx.scoped_grant_exists   { return silent(SilentScopedGrantExists);  }   // B
```

Swap A and B and enumerate all four inputs:

| `scoped_grant_exists` | `bundled_scope_grant` | A-then-B (today) | B-then-A (swapped) |
|---|---|---|---|
| f | f | prompt | prompt |
| f | **t** | silent | silent |
| **t** | f | silent | silent |
| **t** | **t** | silent | silent |

**Identical in every cell.** Both arms return `Silent`; order only changes which `EngineReason` is
attached. ⇒ The narrowing cannot be a reorder. It must **remove or condition** the `bundled_scope_grant`
arm. That changes the size, the rollback story, and the evidence this phase owes.

### 0.2 🚨 The narrowing target the parent contract names is **forbidden by an owner-approved invariant**.

The parent's `D2` sub-question says post-Phase-0.8 rows can narrow to `domain_manifest_snapshots`
(V24). **Code reading — `rust-wallet/src/database/domain_manifest_snapshot_repo.rs`, module header,
rule 1** (owner-approved 2026-08-22, gate `P0.8-A11`, invariant `R-SNAPSHOT`):

> *"**Informational only, never a decision input.** … Nothing in this table is ever read by
> `hodos_permission_engine::decide` — a `PermissionContext` has no field it could occupy, and it must
> stay that way."*

**Confirmed by grep:** `domain_manifest_snapshots` appears in exactly four files; the only
`permission_service` hit is a *comment* (`context_builder.rs:668`) noting it is not mutated. No read.

⇒ The only record of "what this site was granted" the engine may lawfully consult is the **V18 child
tables** — `domain_protocol_permissions` / `domain_basket_permissions` — which is precisely what
`scoped_grant_exists` already computes (`request_gate.rs`, lines 393–435).

⭐ **Therefore the narrowing collapses to one statement:** *`bundled_scope_grant` stops having any
silencing effect of its own.* There is no third thing for it to mean without adding a new engine
input, which `R-SNAPSHOT` closes off and which no ticket has asked for.

### 0.3 🚨 The "legacy rows" problem is **100 % of the affected population**, not a migration tail.

**Measured — dev wallet DB (`%APPDATA%/HodosBrowserDev/wallet/wallet.db`, read-only snapshot taken
2026-09-04 19:13; dev wallet was live, so the WAL was copied with it):**

| id | domain | quiet | proto | basket | snapshot |
|---:|---|---:|---:|---:|---:|
| 58 | todo.metanet.app | **1** | 0 | 0 | 0 |
| 64 | ramp.bsvblockchain.tech | **1** | 0 | 0 | 0 |
| 75 | socialcert.net | **1** | 0 | 0 | 0 |
| 78 | teragun.com | **1** | 0 | 0 | 0 |
| 80 | now.bsvblockchain.tech | **1** | 0 | 0 | 0 |
| 81 | zanaadu.com | **1** | 0 | 0 | 0 |
| 88 | brc-cloud.bcryderman.workers.dev | **1** | 0 | 0 | 0 |
| 103 | chaintap.utxoengineer.com | **1** | 0 | 0 | 0 |
| 108 | beta.zanaadu.com | 0 | **10** | **6** | **1** |
| 18, 29, 36, 37, 38 | pixelwar / metanetapps / buttercupdapp / hodos-test.local:8000 / github.com | 0 | 0 | 0 | 0 |
| 39 | app.treechat.com | 0 | 1 | 0 | 0 |

**8 of 8** rows with `bundled_scope_grant = 1` have **zero** V18 grants **and** zero approved
snapshot. Under the narrowing every one of them goes from blanket-silent to prompt-on-first-use.

**Cause, measured by code reading:** the per-scope `POST /domain/permissions/{protocol,basket}` writes
live **only** inside the manifest branch of `BRC100AuthOverlayRoot.tsx` (lines 1249–1301). The
`domain_approval` path (`handleAllow` / `handleAllowAdvanced`, lines 840–890) sends
`add_domain_permission` with `bundledScopeGrant` and **nothing else** — no V18 rows, ever.

⛔ **So this is not historical.** Every *future* no-manifest connect produces the same shape, and
Phase 0.8's `ManifestFetcher` tightening (`valid == true` now requires a recognised permission) made
the `domain_approval` fallback **more** common, not less. "Legacy rows" is the steady state.

### 0.4 ⚠️ The ready-made test case does not exercise the path it was offered for.

`beta.zanaadu.com` (id 108) is **`bundled_scope_grant = 0`**. It is an excellent control for the
*narrow* path — and it already demonstrates the intended end state. **Measured, wallet logs
`%APPDATA%/HodosBrowserDev/logs/wallet_r*.log`, 47 scoped decisions total:**

```
45 × engine Silent (scoped) … /createSignature kind=CounterpartyUse
 1 × engine Silent (scoped) … /createHmac       kind=ProtocolUse
 1 × engine Prompt (scoped) minted approval id=2a473dbb… /createSignature kind=ProtocolUse
 1 × 🔐 X-User-Approved consumed (scoped) … id=2a473dbb…
```

i.e. one ungranted protocol prompted, the user answered, the approval was consumed, the call
completed. **The narrow path works end-to-end today.** What is missing is a `bundled_scope_grant = 1`
subject — one must be created or seeded (`A4` below).

### 0.5 ⭐ Blast radius is far smaller than "every ProtocolUse and BasketAccess".

The ticket's wording ("**every** `ProtocolUse` and `BasketAccess` … is silent") is true of the engine
arm but overstates what reaches it. **Code reading — `request_gate.rs :: ScopedCall::call_kind`
(267–274):** any protocol call carrying a counterparty maps to `CounterpartyUse`, which
`decide_scoped_grant` silences **before** the bundled arm (Fix #3, line 165). **Measured above: 45 of
47 real decisions were `CounterpartyUse`.**

⇒ Narrowing bites only **counterparty-less `ProtocolUse`** and **non-protected `BasketAccess`**.
Protected baskets (`default`, `backup-*`, `admin *`) already force-prompt via the
`bundled_override = Some(false)` path (`request_gate.rs`, 447–451) and are unaffected either way.

### 0.6 🆕 The change makes the **shipped consent copy false** — and lands back in the file 7b rewrote.

Not named in the ticket. `BRC100AuthOverlayRoot.tsx` asserts the wide behaviour in four places, all
of which become wrong the moment the engine arm goes:

| Line | What it says / does |
|---|---|
| 2632 | label — *"let this site use **any** protocol or basket without asking — including ones it did not list above"* |
| 2633 | tooltip — *"can use ANY protocol or basket without prompting - including ones it did not declare in its manifest"* |
| 2645–2657 | gold callout — *"Quiet mode is on, so this site can use any protocol or basket … not just the ones listed above"* |
| 2687 / 2725 | `disabled={manifestAllowBundledScope}` — the per-item ticks are **inert by design** while quiet mode is on |

⭐ The disabling was a deliberate P0.8 choice: *"disabling makes the contradiction unrepresentable."*
**Once quiet mode is no longer wider than the list, the contradiction it was avoiding cannot arise** —
so the ticks should become live and the callout should go. That is a consent-surface edit, in the same
file and the same view 7b just consolidated. It is not optional: shipping the engine change alone
leaves a checkbox on screen claiming a power the engine no longer has, which is the exact defect class
Phase 7 exists to close.

### 0.7 ✅ Self-heal confirmed end-to-end — with one caveat that could defeat it.

- **UI:** `handleScopedAlwaysAllow` (line 1020) fires `grant_scoped_permission` (writes the V18 row +
  invalidates `SubPermissionCache`) **before** approving, so the next same-scope call passes silently.
- **C++:** `protocol_permission_prompt` and `basket_permission_prompt` are wired in
  `HttpRequestInterceptor.cpp` (1427–1441 fire, 1499–1500 dispatch, 2959–2966 payload) and handled in
  `simple_handler.cpp` (5951). The prompt reaches the user; it does not fail the call.
- ⚠️ **Caveat.** Manifest-derived grants are written `key_id = '*'` (measured — all 10 rows on id 108).
  `handleScopedAlwaysAllow` writes the **exact** keyID from the call (`base.protocolKeyId =
  scopedProtocolKeyId`). `is_protocol_granted` matches `key_id = ?4 OR key_id = '*'`
  (`domain_permission_repo.rs`, 391). **A site that varies keyID per call would re-prompt forever and
  never self-heal.** Decision `D3`.

### 0.8 Instrument notes (what could not be measured, stated rather than hidden)

- `permission_audit_log` has **0 rows** — it cannot tell us how often quiet mode is load-bearing in
  the field. The 47-decision log sample above is the only frequency evidence, and it is one session
  on one site. ⚠️ Do not extrapolate it to prompt-volume claims.
- The engine's own T1 gate is `preflight.ps1 :: T1a` (`cargo test --manifest-path rust-wallet/Cargo.toml`),
  which covers `hodos_permission_engine` as a workspace member. **Adding tests inside T1a is not an
  instrument edit** (working rule 6 bans loosening patterns/baselines, not adding coverage). No gate
  pattern or baseline is touched by this phase.

---

## 1. Goal

Quiet mode silences only what the site declared and the user approved. Anything else prompts, the
prompt reaches the user, and answering it once makes the site quiet for that scope. The screen says
exactly that and nothing more.

## 2. Done means

- [ ] A declared, user-approved protocol or basket on a quiet-mode site is **Silent**.
- [ ] An **undeclared** one on the same site **Prompts**, the modal reaches the user, and
      *Always allow* makes the next identical call Silent.
- [ ] A `bundled_scope_grant = 1` row with **no** V18 grants **prompts on first use** — never denies,
      never stays blanket-silent.
- [ ] Protected baskets still prompt; `CounterpartyUse` is unchanged; sensitive cert fields still
      prompt unconditionally.
- [ ] The connect modal's quiet-mode label, tooltip and callout describe the new behaviour, and the
      per-item ticks are live.
- [ ] No schema change. No migration. `bundled_scope_grant` and `default_bundled_scope_grant` remain,
      with their meaning restated in the docs that define them.

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-PERIM` | the four privacy-perimeter gates | This phase edits `matrix_c.rs`, the file the invariant is defined against. ⛔ SUBJECT is the Rust `PermissionDecision` **kind + reason**, not whether a modal renders. Sensitive cert fields must still prompt unconditionally. |
| `R-INTEXT` | internal never prompts, external always gates | `dispatch_scoped_grant` returns `Proceed` on a missing `X-Requesting-Domain` (line 331) **before** any engine call — unchanged by this phase, and must be shown to stay unchanged, or the wallet UI starts prompting itself. |
| `R-SNAPSHOT` / `P0.8-A11` | `domain_manifest_snapshots` is never a decision input | §0.2. The obvious-looking implementation violates it. The evidence must show `PermissionContext` gained no field. |
| — *(named, not in the set)* | **"Manage Site Permissions"** — CLAUDE.md load-bearing safeguard | Revoking a V18 row must still take effect. With `scoped_grant_exists` now the sole silencer, revoke becomes **more** load-bearing, not less. |
| `R-GOLD` / `R-COUNT` | payment indicator, session counters | Payment branch is a separate cascade arm; the existing test `fix4_bundle_grant_does_not_affect_payment_kind` is its guard and must keep passing under whatever replaces it. |

## 4. The legacy-row plan (`D2`)

**Rule: degrade to prompt-on-first-use. Never deny. Never stay blanket-silent.**

This falls out of the design rather than needing migration code:

1. `bundled_scope_grant` stops silencing ⇒ a row with no V18 grants reaches
   `EngineReason::ScopedGrantMissing` ⇒ `Prompt`. That is prompt-on-first-use, by construction.
2. `Always allow` writes the V18 row ⇒ the site self-heals per scope. No user is stranded.
3. **No migration is written.** The column is not dropped, not rewritten, not backfilled — so there
   is no DB state to unwind and `git revert` restores the old behaviour exactly (§8).
4. ⛔ **Backfilling V18 rows from `domain_manifest_snapshots` is explicitly rejected** — it would make
   the snapshot a decision input by the back door, which is what `R-SNAPSHOT` forbids, and it would
   silently grant permissions from a manifest that may have changed since approval (`R-SNAPSHOT`
   rule 2).
5. **What the owner will actually see on the next build:** the eight sites in §0.3 —
   `todo.metanet.app`, `socialcert.net`, `teragun.com`, `zanaadu.com`, `ramp.` and
   `now.bsvblockchain.tech`, `brc-cloud.bcryderman.workers.dev`, `chaintap.utxoengineer.com` — will
   each raise scoped prompts they have never raised before, once per scope, until answered. This is
   the intended behaviour and it is also the most visible consequence of the phase. Naming it here so
   it is not discovered as a bug.

## 5. Evidence table

⛔ Rows are drafted now and **run after the code**; `Result` stays ⬜ until observed. No empty RED or
SUBJECT cells.

| ID | 🟢 GREEN — must be true | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P7c-A1` | Quiet-mode site, **declared+granted** protocol ⇒ `Silent` / `SilentScopedGrantExists` | Revoke that V18 row → the same call Prompts. Its own pair with `A2` | Rust `PermissionDecision` kind **+ reason**. `SilentBundledScopeGrant` appearing at all is a failure — the reason string is the discriminator | T1 + T2 | ⬜ |
| `P7c-A2` | Same site, **undeclared** protocol ⇒ `Prompt`, and the modal is **on screen** | Restore the old engine arm on a scratch branch → it goes Silent again and no modal appears | Both halves: the Rust decision **and** `🔒 protocol_permission_prompt notification queued` in the C++ log with a matching `requestId`. Neither alone is proof | T1 + T2 | ⬜ |
| `P7c-A3` | Answering `A2` with **Always allow** makes the *next identical call* Silent | Answer **Allow once** instead → the next call Prompts again. ⛔ Two different buttons, two different outcomes — a test that only exercises one proves nothing about the other | The V18 row written (`domain_protocol_permissions`) **and** the second call's engine reason | T2 | ⬜ |
| `P7c-A4` | A `bundled_scope_grant = 1` row with **zero** V18 grants **prompts** — never denies, never silences | Seed the row and observe which of the three it does. ⛔ There is no such row today with a snapshot; `beta.zanaadu.com` is quiet=**0** and does **not** exercise this (§0.4) | A **real** quiet=1 domain in the dev DB. ⛔ A fresh profile is not a fresh test — one wallet DB serves all profiles | T2 | ⬜ |
| `P7c-A5` | `CounterpartyUse` is **unchanged** — still Silent for approved domains | Remove the Fix #3 short-circuit → the 45-per-session counterparty calls start prompting | `kind=CounterpartyUse` lines in the wallet log, count compared against a pre-change baseline on the same site | T1 + T2 | ⬜ |
| `P7c-A6` | Protected baskets still prompt regardless of the flag | Set `bundled_override` to `None` for a protected basket → it silences | `is_protected_basket` path in `dispatch_scoped_grant`, with `default` and a `backup-*` name, not one of them | T1 | ⬜ |
| `P7c-A7` | `R-PERIM` — the four gates behave exactly as before; **sensitive cert fields prompt unconditionally** | Flip each gate's precondition, per gate, and observe the opposite outcome; then force the cert gate silent and watch its test go red | Rust `PermissionDecision` kind + reason for all four. ⛔ Not modal appearance | T1 (+T2 e2e owed) | ⬜ |
| `P7c-A8` | `R-INTEXT` — an internal wallet-UI call still never prompts; an external one still gates | Stub the origin derivation to always emit `X-Requesting-Domain` → internal starts prompting; suppress it → external goes silent. **Both observed** | **Absence** of the header in the Rust log for internal, **presence with the exact page host** for external | T2 | ⬜ |
| `P7c-A9` | `PermissionContext` gains **no** new field, and nothing in `permission_service` reads `domain_manifest_snapshots` | Add a snapshot read on a scratch branch → the `R-SNAPSHOT` assertion goes red naming it | `context.rs` field list diff + a grep assertion over `permission_service/`. ⛔ The grep must assert its own trigger fired, not print a clean zero | T1 | ⬜ |
| `P7c-A10` | Existing engine tests still pass, and the four `fix4_*` tests are **rewritten, not deleted** | Delete one instead of rewriting → coverage silently drops with a green gate. Each rewritten test must be seen to fail against the old engine and pass against the new | `matrix_c.rs` test module + `tests/decision_matrix.rs`. Count before/after stated explicitly | T1 | ⬜ |
| `P7c-A11` | 👤 **Human reads the changed connect screen.** Label, tooltip and (absent) callout describe the real behaviour; the per-item ticks are live and legible | P0.8 shipped **six** consent-surface defects with every gate green; 7a shipped a 2-px button strip. The red is the historical record; the control is that a person looked | Owner's eyes on the **rendered** modal at 100 % and at DPI matrix cells #4/#6/#9 | T3 👤 | ⬜ |
| `P7c-A12` | `npm run build` clean | ⛔ **Never `npx tsc --noEmit`** — it passed last session on code `npm run build` rejected | The build the product ships | T0 | ⬜ |

**Two-sided pairings:** `A1`↔`A2` (granted vs undeclared), `A2`↔`A5` (what must change vs what must
not), `A3`'s two buttons, `A8`'s two halves.

### 5.1 Live run — 2026-09-07, dev stack (wallet 31401, vite 5137, browser CDP 9322)

All five rows below are **one call shape** (`POST /createHmac`, `protocolID [2,…]`, `keyID "1"`) sent
with `X-Requesting-Domain`, differing only in domain and protocol name. That sameness is the point:
the only variable is whether the scope was granted.

| Row | Subject | Observed | |
|---|---|---|---|
| `A1` | `beta.zanaadu.com` · `server hmac` and `wallet settings` — **granted** V18 rows | **200, handler ran** | 🟢 |
| `A2` | `beta.zanaadu.com` · `p7c undeclared probe` — same site, **not** granted | **202** `protocol_permission_prompt` / `scoped_grant_missing` | 🟢 |
| `A4` | `todo.metanet.app`, `socialcert.net`, `teragun.com` — `bundled_scope_grant=1`, **zero** grants | **202 prompt** — never 403, never silent | 🟢 |
| `A3` | `beta.zanaadu.com` · `p7c selfheal probe`: probe → write the Always-allow row → probe again | **202 → grant → 200** | 🟢 |
| `A5` | Same call with a real 66-hex counterparty, on a quiet=0 **and** a quiet=1 site | **200 silent both** — `CounterpartyUse` unchanged | 🟢 |

⭐ `A1`↔`A2` is the load-bearing pair: same domain, same session, same call — one granted, one not,
opposite outcomes. Without it, `A4`'s three prompts would be equally consistent with *"7c broke the
engine and now everything prompts."*

**Also observed:** zero occurrences of `bundled_scope` in the wallet log across the whole run — the
removed `EngineReason` is genuinely unreachable, not merely unused.

**DB left at baseline.** The one grant written for `A3` was revoked; `beta.zanaadu.com` is back to its
original 10 protocol rows. Approvals minted by the probes are single-use and expire in 600 s.

⛔ **Two instrument failures during this run, both caught before they became results:**

1. The first probe omitted `createHmac`'s required `counterparty` field, so serde returned **400
   after** the gate ran. A 400 is not a silence — reading it as one would have manufactured a green
   for `A1`. Re-run with a complete body.
2. `-SkipHttpErrorCheck` is PowerShell 7 only; on 5.1 every row printed `OTHER ()`. **Vacuous, and it
   looked it** — which is the only reason it was caught. A probe that had defaulted to a clean zero
   would have passed silently.
   ⚠️ Also: `Invoke-WebRequest` treats **202 as success**, so a naive success/catch split labels a
   prompt as a silence. Branch on `StatusCode`, never on which branch you landed in.

### 5.2 Still owed

| Row | What | State |
|---|---|---|
| `A11` | 👤 owner reads the changed connect screen | 🟢 **CLOSED 2026-09-07 by owner observation** — *"I have already looked at the connect screen. It looks good."* ⚠️ Recorded as the owner's eyes on the **connect** screen. The DPI matrix cells (#4/#6/#9) were **not** part of that look and are still owed at the boundary. |
| `D3` | keyID `*` on Always-allow | 🟢 **CLOSED 2026-09-07 by owner live test** — the button itself wrote `keyID=*`. §5.3. |
| `A2` (UI half) | the **scoped permission modal** actually renders for an undeclared scope | 🟢 **CLOSED 2026-09-07 by owner live test** — see §5.3. |
| `A6`–`A9` | protected baskets, `R-PERIM`, `R-INTEXT`, `R-SNAPSHOT` end-to-end | ⬜ T1 green; T2 owed at the boundary |
| preflight | `scripts/preflight.ps1` (+ `-NegativeControl`) | ⬜ **Not run.** ⚠️ Deliberately deferred rather than run against a live dev stack — the running wallet holds `hodos-wallet.exe`, which already produced one `Access is denied` link failure this session. Run it with the dev stack **stopped**, or it reports a red that is about file locks, not code. |

### 5.3 👤 Owner live test — 2026-09-07, `teragun.com`, both paths

Driven by the owner, not by me. Two sessions an hour apart happened to exercise the **legacy** path
and the **connect** path, which is the pair the desk probes in §5.1 could not reach.

⛔ **Correction first, because it shaped the first reading of this.** The owner said *"I don't think
it has a manifest"* and I repeated it back as fact without checking. **teragun.com does have one** —
`https://teragun.com/manifest.json`, `namespace=babbage` (the deprecated one), 4 protocols, 0
baskets, 1 certificate. The log said so plainly and I had not looked. ⭐ Nothing in the result
changes, but the first analysis was built on an unmeasured premise, which is the thing this project
keeps paying for.

**Session 1, 17:49 — the legacy path (`A4`), and the one that proves the phase.**

teragun was already `approved` from before the manifest work, so it carried quiet mode **on** with
**zero** stored grants — the exact 8-of-8 shape in §0.3. No connect modal was involved.

```
17:49:07.563  engine Prompt (scoped) minted approval … /createSignature kind=ProtocolUse
17:49:14.986  POST /domain/permissions/protocol  proto="teragun auth"  keyID=*
17:49:15.080  X-User-Approved consumed  → the call completed
```

⭐ Line 1 was **silent before this phase**. Line 2 is `D3` working through the button — the grant
written is `*`, not the call's literal keyID. Line 3 is the call resuming rather than failing, which
is the half of `A2` the ticket explicitly asked to confirm. Three open rows closed by one click.

**Session 2, 18:48 — the connect path, after the owner revoked and reconnected.**

```
18:47:44  DELETE /domain/permissions?domain=teragun.com          (owner revoked)
18:47:48  manifest parsed … (namespace=babbage, 4 protocols, 0 baskets, 1 certs)
18:47:48  engine Prompt (domain-trust) type=ManifestConnectBundle reason=new_domain_with_manifest
18:48:04  POST /domain/permissions  + approved manifest snapshot (1944 bytes)
18:48:04  4 × POST /domain/permissions/protocol   … all keyID=*
18:48:04  1 × POST /domain/permissions/certificate  fields=["userName","profilePhoto"]
```

Resulting state, read back through the wallet API:

```
trust      : approved
quiet mode : OFF          ← no checkbox exists to turn it on
protocol grants: 4
   level 2  identity key retrieval     keyId=*
   level 1  identity resolution        keyId=*
   level 0  teragun auth               keyId=*
   level 0  teragun lottery payout     keyId=*
```

⭐ **This is the phase's whole thesis, on screen and in the database:** four specific grants matching
the four things the site asked for, instead of one invisible blanket. Anything else prompts.

⚠️ **Two observations worth carrying forward, neither a defect in this phase:**

- **Both new grants are security level 0.** At level 0 the protocol identifier appears nowhere on
  screen except the ID that Phase 7b added beside the site's description — so 7b's fix is load-bearing
  on this very common shape, not a corner case. ⛔ The `securityLevel === 2` footnote alone would
  have shown the user nothing here.
- **The manifest uses the deprecated `babbage` namespace**, not `metanet`. Parsed correctly by the
  compatibility arm. Relevant to the Auto-Approve Engine video, whose outline says not to show
  `babbage` on screen — a real shipping site uses it.

**Also surfaced, and filed rather than fixed here:** two prompts appeared in session 1 (the scoped
grant plus a Chromium **loopback** prompt). Diagnosed in full —
`TICKET_wallet_quiet_detector_blind_to_long_polls.md`. ⛔ Not caused by this phase, and the
loopback grant is **per requesting site, not per target** (our store is keyed
`(domain, permission_type, state)` with no target column), so it cannot be folded into a wallet
approval without granting undisclosed access to every local service.

## 6. Blast radius

- **`matrix_c.rs :: decide_scoped_grant`** — the decision engine, `R-PERIM`'s subject. Two of the
  four `fix4_*` tests assert the behaviour being removed and must be rewritten in the same commit.
- **`decision.rs :: EngineReason::SilentBundledScopeGrant`** — becomes unreachable. Removing it is a
  public-enum change across the crate; leaving it is dead code. `D4`.
- **`context.rs :: PermissionContext.bundled_scope_grant`** — likewise. Keeping the field costs
  nothing and keeps `build_scoped_grant_context`'s override plumbing intact for the protected-basket
  guardrail; removing it touches five call sites.
- **`BRC100AuthOverlayRoot.tsx`** — the four copy/behaviour sites in §0.6, in the single connect view
  7b just built. ⚠️ A JSX slip here breaks every consent modal in the browser; they all live in this
  one 3,4xx-line file.
- **Docs that state the old meaning:** `rust-wallet/src/database/CLAUDE.md` (V22 row, line 328),
  `migrations.rs` V22 header comment (1204), `TICKET_quiet_mode_wider_than_manifest.md` (close it).
- **Not touched, deliberately:** `domain_manifest_snapshots` and its repo (`R-SNAPSHOT`); the V22 /
  V25 schema (invariant #2 — no migration); `ManifestFetcher.cpp` / `manifest.rs`; the payment branch.

## 7. Out of scope

- Any new engine input (a "declared set" field on `PermissionContext`) — `R-SNAPSHOT` closes it and
  nothing asks for it.
- Backfilling V18 rows from stored snapshots (§4.4).
- The keyID-widening question beyond `D3`'s yes/no — the BRC-73 `keyID` gap itself is standards work,
  not beta.3.
- `TICKET_manifest_description_can_misdescribe_protocol` — closed by 7b (`3afe5d0`); noted here only
  because the ticket flags the interaction (narrowing **raises** the value of that fix, which is
  already shipped).
- macOS beyond a relay entry in `MAC_RELAY_P7_ROUND.md`. ⚠️ This phase is Rust + React only — no
  platform-conditional C++ — so the Mac risk is a rebuild, not a port.

## 8. Rollback

One commit for the engine change (`matrix_c.rs` + its tests), one for the consent copy, one for docs.
`git revert` the engine commit alone and blanket quiet mode returns exactly — **no schema change, no
migration, no DB state to unwind**. ⚠️ Reverting the engine commit *without* the copy commit leaves
the modal understating what quiet mode does, which is the safer of the two mismatches but should not
be left standing.

---

## 9. ⛔ Decisions owed BEFORE the first commit

| # | Decision | My recommendation | Why it is yours |
|---|---|---|---|
| **D1** | 🚨 Given §0.1 and §0.2 — the narrowing can only mean *"`bundled_scope_grant` stops silencing; `scoped_grant_exists` governs alone"*. **Do it?** | **Yes.** It matches your 2026-08-23 steer (*"just prompt later but expand our engine as needed"*), it is the smallest change that closes the ticket, and doing it before third-party adoption is when extra prompts are cheapest. | CLAUDE.md invariants #2/#3 — a behaviour change on the auto-approve path. ⛔ I will not touch `matrix_c.rs` without an explicit yes. |
| **D2** | Legacy rows: the §4 plan (prompt-on-first-use, self-heal, **no migration**), knowing it hits **8 of 8** quiet-mode sites and every future no-manifest connect (§0.3) — not a tail. | **Accept as written.** The alternative (backfill) violates `R-SNAPSHOT`; denying is off the table. | You said prompting is acceptable; you did not price it at 100 % of the population. That number is new information and the call should be re-made against it. |
| **D3** | Should *Always allow* on a scoped prompt write `key_id = '*'` (protocol-wide, matching manifest grants) instead of the exact keyID? §0.7. | **Yes — write `'*'`.** Otherwise a site that varies keyID re-prompts forever and never self-heals, which turns "one prompt per scope" into a prompt storm. It also makes the two grant sources consistent. ⚠️ It is a **widening** of what one click grants, so it is your call, not mine. | It changes what a single user click authorises. Phase 5's rule applies in reverse: a widening is also a privilege change. |
| **D4** | Dead-code disposal: remove `EngineReason::SilentBundledScopeGrant` and `PermissionContext.bundled_scope_grant`, or leave them? | **Leave the context field** (the protected-basket override plumbing uses it and it costs nothing); **remove the `EngineReason` variant** so no future edit can reach a reason that no longer describes anything true. | Working rule 3 — I should not delete what my change did not orphan, and this one is a judgement call about which half my change actually orphans. |

### 9.1 Answers, 2026-09-07

| # | Answer | Note |
|---|---|---|
| **D1** | ✅ **Yes.** *"quiet mode should only approve exactly what is in the manifest and has been shown to the user"* | Implemented as §0.2 describes: the `bundled_scope_grant` arm is gone; `scoped_grant_exists` governs alone. |
| **D2** | ✅ **Accepted** with the 8-of-8 number on the table | No migration written. |
| **D3** | ⬜ **Not reached.** Deferred — no code touched the prompt-path keyID | The keyID-storm risk in §0.7 is therefore **still open**. Raise it before the live run, not after. |
| **D4** | ✅ **Split, as recommended** | `EngineReason::SilentBundledScopeGrant` removed; `PermissionContext.bundled_scope_grant` retained as the subject of `p7c_quiet_mode_never_changes_any_scoped_outcome`. |

**Plus one decision the owner made that this contract did not anticipate:** the *"Quiet mode"*
checkbox is **deleted from the UI**, not relabelled. Owner: *"I don't even know why we have quiet
mode."* Once narrowed it is the tick-list restated, so a second control could only agree or
contradict. A **select-all** master checkbox was offered and explicitly **declined**.

⚠️ That decision cascaded further than either of us named up front, and the extra removals are
**forced, not scope creep** — each surviving control would have moved a value nothing reads:

1. connect screen, manifest view — checkbox + gold callout gone; per-item ticks re-enabled
2. connect screen, **no-manifest** view — checkbox gone (it described a list that does not exist)
3. **Manage Site Permissions** — the `DomainPermissionForm` quiet-mode toggle gone
4. **wallet settings** — *"Start new sites in quiet mode"* (the V25 default) gone
5. the React state, the V25 read, and the `bundledScopeGrant` field on `DomainPermissionSettings`
6. `bundled_scope_grant_override` and its `build_scoped_grant_context` parameter — the override was
   a **no-op that still read as protective**, the worst kind of dead code in a permission gate

⛔ **Schema untouched.** V22 and V25 columns remain; the connect path now writes
`bundledScopeGrant: false` so no stored row claims a blanket grant nothing honours.

### 9.2 🐛 Found while working, NOT fixed here

- **`scripts/stop-dev.ps1` cannot be run the documented way.** `.\scripts\stop-dev.ps1` dies with
  *"Cannot bind argument to parameter 'Path' because it is an empty string"* — `$PSScriptRoot` is
  empty at param-binding time. Works with an explicit `-RepoRoot`. ⚠️ This is the safety tool from
  the 2026-09-01 production-wallet incident; a safety tool that fails to start is how people go back
  to hand-writing kills. Its own logic is fine — `-WhatIf` correctly spared all 77 installed-build
  processes and stopped only the 20 dev ones.
- **Orphaned doc comment in `context_builder.rs`.** The `build_scoped_grant_context` doc block sits
  above `build_payment_context` with no function between, so rustdoc attaches it to the wrong one.
  Verified pre-existing against `HEAD`. Only the sentences 7c invalidated were corrected.
- **A literal `—` escape** in `BRC100AuthOverlayRoot.tsx` where an em-dash was meant
  (pre-existing, in a comment).

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1` run — result + date recorded below
- [ ] `scripts/preflight.ps1 -NegativeControl` run — every T0 gate seen to fail
- [ ] `../REGRESSION_SET.md` run in full — `R-PERIM` and `R-INTEXT` are **live here**
- [ ] Adversarial review complete, four questions answered in writing
- [ ] 👤 Owner has looked at the changed connect screen (`P7c-A11`)
- [ ] `TICKET_quiet_mode_wider_than_manifest.md` closed with the measured findings
- [ ] Commit messages cite the row IDs they satisfy

| Item | Result | Date | By |
|---|---|---|---|
| preflight | | | |
| preflight -NegativeControl | | | |
| regression set | | | |
| adversarial review | | | |
| owner human-eyes pass | | | |
