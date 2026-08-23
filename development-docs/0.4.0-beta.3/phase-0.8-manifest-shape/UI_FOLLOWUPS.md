# Phase 0.8 — UI follow-ups from live testing

**Owner-reported 2026-08-23**, after clicking through the real bitgenius connect prompt and the
wallet's *Default Limits for New Sites* screen. None of these are built yet.

---

## 1. ⚠️ QUESTION TO SETTLE FIRST — "can we move it to the bottom as well"

The owner's note is ambiguous and **must be resolved before building**, because the two readings
mean different work:

- **(a) the counterparty hex** — move `(with 0279887cdd…)` out of each inline permission line and
  into a footnote block at the bottom of the modal.
- **(b) the pre-fill toggle** — move it below something else in *Default Limits for New Sites*.

Reading (a) fits the surrounding sentences (they are about the hex and what it means). Reading (b)
fits item 3 below, which is also about that toggle's placement. **Ask, do not guess.**

---

## 2. Explain the counterparty — info affordance

### What it is (so the copy is correct)

A Level-2 protocol grant is scoped by **who the operation is with** (BRC-42/43; BRC-116 §4.1). The
field holds one of:

| Value | Meaning |
|---|---|
| `"self"` | operations paired with the user's own key — e.g. data only they can read |
| `"anyone"` | a well-known public counterparty; **anybody** can derive the matching key |
| 66-hex | **one specific party** — a compressed public key. For bitgenius, their server |

⭐ A named counterparty is **narrower and safer** than `anyone`. The copy should say that, because a
wall of hex reads as scarier than "anyone" when it is in fact the opposite.

⛔ **There is nothing for the user to *do* with the value.** It is an identifier, not a decision.
The info text should explain what it *means* for their safety and stop there.

### Build

- An `InfoIcon`-style tooltip (the pattern already used by *"Allow identity-key disclosure by
  default"* and the quiet-mode checkbox) attached to the counterparty note.
- Draft copy: *"This permission is limited to one specific party — this site's server, identified by
  its public key. That is narrower than allowing the site to use this protocol with anyone. You do
  not need to do anything with this value."*
- For `self`: *"…only with your own keys — nothing is shared with anyone else."*
- For `anyone` / unspecified: ⚠️ this is the **widest** case and should read as such.

### ⭐ Later, not now

`rust-wallet/src/identity_resolver.rs` resolves identity keys → names/avatars via BSV Overlay
(BRC-52 certificates, 10-min cache). So *"with BitGenius's server"* is reachable instead of raw hex.
Out of scope here; note it so the hex is not treated as permanent.

---

## 3. *Default Limits for New Sites* — layout

Current state after the 2026-08-23 move: *"Allow identity-key disclosure by default for new sites"*
and *"Pre-fill new-site limits with the site's recommended settings"* are in the same section but
**stacked vertically**.

**Wanted:** both on the **same horizontal line**, with the pre-fill toggle to the **right** — either
right-justified, or centre-right beside the identity-key checkbox.

⚠️ Watch the DPI matrix here. Two checkbox+label pairs side by side is exactly the shape that clips
at 125%/1366 and 150%/1366 (`DevOps-CICD/DPI_RESOLUTION_TEST_MATRIX.md`, cells #4/#6/#9). Let the row
wrap rather than truncate — the identity-key label is long and the pre-fill label is longer.

**Also:** add padding between that section and the one holding **Save Defaults** / **Reset All Sites
to Defaults**. They currently sit too close.

---

## 4. 🚨 STILL OPEN, needs an owner yes — the counterparty is displayed but never written

Not cosmetic. Recorded here so it is not lost among the UI items.

The connect modal displays the Level-2 counterparty, but `handleManifestConnect`
(`BRC100AuthOverlayRoot.tsx`) posts to `/domain/permissions/protocol` **without** it, so every grant
row lands as `counterparty = NULL` = **any counterparty**. Measured live 2026-08-23:

```
(1, 'identity key retrieval', '*', None)
(2, '3241645161d8',           '*', None)   ← modal said "with 0279887cdd…"
(2, 'auth message signature', '*', None)
(2, 'server hmac',            '*', None)   ← modal said "with you only"
```

⛔ **Introduced by Phase 0.8** — the display was added without the write, so the screen now claims
something narrower than the grant. That is this phase's own defect class, inverted.

**Bounded:** with quiet mode ON (the default) it changes nothing — quiet mode silences ProtocolUse
regardless. It bites only with quiet mode off.

**Proposed fix (three lines), needs an explicit yes** because it changes what is written to a
permission row (CLAUDE.md invariants #2/#3):

- send `counterparty` when the declared value is a **concrete 66-hex compressed pubkey**;
- leave it **NULL** for `'self'`, `'anyone'` and empty.

⚠️ Why the split: `is_protocol_granted` matches `counterparty IS NULL` (any) OR an exact string. What
the request-time counterparty actually is for a `'self'` protocol is **UNVERIFIED** — if it arrives
as a derived pubkey rather than the literal `"self"`, storing `"self"` would fail to match and
wrongly **deny** a call the user approved. Denying is safer than over-granting but is still a
regression, so verify before widening the fix to those two values.

**Verify like this:** with quiet mode OFF, connect to a site declaring `[2, X]` with
`counterparty: "self"`, trigger that protocol, and read what `dispatch_scoped_grant` builds into
`ScopedCall::Protocol{ counterparty }` in the wallet log before deciding.

---

## Related

- `PHASE_CONTRACT.md` — §6a consent provenance, §8 evidence
- `TICKET_manifest_description_can_misdescribe_protocol.md` — the sibling display-vs-reality problem
- `TICKET_quiet_mode_wider_than_manifest.md` — why item 4 is currently masked
