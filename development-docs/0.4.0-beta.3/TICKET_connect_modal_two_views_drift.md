# TICKET — the connect modal has two views of one consent, and they drift

**Status:** ❔ **NEEDS OWNER REVIEW** — P0.8 rewrote much of this surface — is the drift closed, or just moved? Labelled 2026-08-31 by the triage pass; see `TICKET_TRIAGE_2026-08-31.md`.

**Filed** 2026-08-23, beta.3 Phase 0.8 live testing. **Owner-raised.**
**Severity** MEDIUM — no known exploit; the failure mode is a user consenting to
something the screen described inaccurately.

## The observation

The owner, reading the real modal for the first time:

> "There is inconsistency in the messages for the check box when in the modal
> home and when I click customize. […] I like the look and feel of the modal
> when the user clicks customize, do we even need the other one. That has
> everything."

`manifest_connect_bundle` renders **two** views of the same decision:

| | Summary (opens first) | Customize (one click away) |
|---|---|---|
| Quiet-mode label | *"Allow this site to perform wallet operations without asking each time"* | *"**Quiet mode:** Let this site use **any** protocol or basket without asking — **including ones it did not list above**"* |
| Identity label | *"Allow this site to identify you"* | *"**Identity:** Allow this site to identify you **across the Metanet**"* |
| Per-item ticks | ✗ not offered | ✓ per protocol / basket / certificate / counterparty |
| Limit fields | ✓ (shared `renderLimitFields()`) | ✓ (same renderer) |

⛔ **The summary understated BOTH grants**, and in the same direction each time:
it dropped the clause that made the risk legible.

- quiet mode — omitted *"including ones it did not list above"*, the single most
  important fact about the widest control on the screen;
- identity — omitted *"across the Metanet"*, i.e. that this key is **the same on
  every BRC-100 site** and is therefore exactly what lets sites correlate the
  user between them.

Both were on the view most users actually read. **Fixed 2026-08-23**: the summary
now carries Customize's wording verbatim for both. Keep them identical.

⚠️ Note the pattern in the drift: the shorter label was not merely shorter, it
was **systematically less alarming**. Summarising a consent string tends to drop
the clause that creates the alarm, because that clause is the qualifier. That is
an argument against ever maintaining a "short version" of a consent label.

## A third instance, same root, different table

Owner, 2026-08-23, on the BRC-73 fixture's `[1, "identity key retrieval"]` entry
described as *"Use your public identity key as your passwordless account"*:

> "this should be tied to 'Identity' not Quiet mode, correct? […] Or should we
> wrap Identity key into Quiet mode?? I am leaning toward not doing that."

The lean is right and the confusion is ours. That row is a **protocol grant**
(`domain_protocol_permissions`, gated as `ProtocolUse`). The Identity checkbox is
a **different mechanism** (`domain_permissions.identity_key_disclosure_allowed`,
gated as `CallKind::IdentityKeyReveal` through the privacy perimeter). Unticking
the protocol does **not** restrict identity disclosure. Both rows were written
for the same connect — verified in the DB.

So the screen shows a permission *described* as identity-related, sitting in the
protocol section, next to a separate Identity control that is the one that
actually governs identity. Same family as
`TICKET_manifest_description_can_misdescribe_protocol.md`.

### ⛔ Do NOT fix this by grouping on the protocol name

The obvious remedy — detect "identity-ish" protocols and file them under
Identity — keys off `protocol_name`, which is **untrusted, site-authored text**.
That would hand a site the ability to choose which section of the user's consent
screen its permission is displayed in. Strictly worse than today. Any grouping
must key off something the wallet controls, not off what the site called it.

### Owner decision 2026-08-23: leave them decoupled, fix the copy

The owner proposed two couplings — co-toggle, or grey the protocol row out with
*"controlled by the Hodos Identity toggle"* — and both were declined **for the
same reason**: either one makes a single tick produce **two grants, with the
site choosing which**, because recognising "the identity protocol" requires
trusting `protocol_name`.

⚠️ Greying-out is the **more** dangerous of the two, not the safer-looking one:
it removes the user's ability to untick while asserting that another control
handles it — an assertion that is false about the mechanism.

⭐ What makes the coupling tempting is that it looks like a convention:
bitgenius.net and our BRC-73 fixture independently use the string
`identity key retrieval`. **A convention everyone happens to follow is not a
rule anyone enforces** — it binds only the honest sites, which are not the
threat model.

**Shipped instead:** one sentence added to the Identity tooltip —
*"This checkbox is the ONLY control over your identity key — no permission a
site lists can grant it, whatever the site calls it."* Fixes the confusion the
owner actually experienced, couples nothing.

**The real fix is upstream, not in our modal:** a RESERVED protocol identifier
that BRC-73 defines and a site cannot repurpose, which the wallet could then
recognise safely. Same shape as the gap recorded in
[[project_p08_manifest_brc73_scoped_2026_08_22]] — our engine has four controls,
BRC-73 has one — and it belongs in the same upstream conversation.

## Why this is worth a structural fix, not just the wording patch

Nearly every defect Phase 0.8 produced is the same shape: **two representations
of one truth, drifting.**

- two manifest parsers (`ManifestFetcher.h` C++ + `manifest.rs`) — "change both
  or neither"
- counterparty **displayed** but not **written** (fixed 2026-08-23)
- protocolID enforced vs description displayed, nothing tying them
  (`TICKET_manifest_description_can_misdescribe_protocol.md`)
- two labels for the quiet-mode checkbox
- two labels for the identity checkbox
- a protocol whose *description* claims an identity role its *mechanism* does not have

⭐ The owner found the label drift **three separate times** across one session,
each time preferring the Customize wording. Three independent instances is not a
run of oversights; it is the structural case for having one view.

A single view removes this whole class from the consent screen. The owner also
noted the capability they wanted — *"accept/grant access to certain protocols
and baskets and reject others or accept/reject all"* — **already exists** in
Customize. It is not missing; it is hidden behind a second click, behind the
screen with the worse wording.

## Proposal

Collapse to ONE view: Customize's structure and wording as the only connect
screen, with the summary's branding header and an "everything is ticked"
starting state so the fast path stays one click.

## ⛔ Why it was NOT done on 2026-08-23

It is a redesign of the **primary consent screen for money and identity**, late
in a phase, and would need fresh live testing of every prompt path
(`domain_approval`, `manifest_connect_bundle`, scoped, payment, certificate)
plus the DPI matrix. Owner chose: fix the wording now, ticket the merge.

## Related

- `phase-0.8-manifest-shape/UI_FOLLOWUPS.md`
- `TICKET_quiet_mode_wider_than_manifest.md` — narrowing quiet mode changes what
  the label should SAY, so schedule that first or together
- `TICKET_manifest_description_can_misdescribe_protocol.md` — same defect class
