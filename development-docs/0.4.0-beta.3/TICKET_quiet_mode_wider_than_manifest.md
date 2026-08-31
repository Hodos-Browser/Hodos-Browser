# Quiet mode grants protocol + basket access beyond what the manifest declared

**Status:** 🔴 **OPEN — verified 2026-08-31.** V25 made the default *configurable*; `migrations.rs :: migrate_v24_to_v25` states narrowing quiet mode to what a manifest declared is "the open follow-up". Labelled 2026-08-31 (had no status line).

**Filed 2026-08-23**, out of beta.3 Phase 0.8's live testing. Surfaced by the owner asking what
happens when a manifest declares something the engine does not model. **Phase 0.8 shipped the UI
half (option 1a); the engine half is deferred here.**

## The defect

`domain_permissions.bundled_scope_grant` — the connect modal's *"Allow this site to perform wallet
operations without asking each time"*, **ticked by default** — short-circuits the scoped-grant
cascade **before** the per-permission rows are consulted:

```rust
// rust-wallet/crates/hodos_permission_engine/src/matrix_c.rs :: decide_scoped_grant
if ctx.bundled_scope_grant {
    return PermissionDecision::silent(EngineReason::SilentBundledScopeGrant);
}
if ctx.scoped_grant_exists {            // ← never reached while quiet mode is on
    return PermissionDecision::silent(EngineReason::SilentScopedGrantExists);
}
```

So while quiet mode is on, **every** `ProtocolUse` and `BasketAccess` from that domain is silent —
whether or not the manifest declared it, and whether or not the user ticked it in Customize. The
V18 rows the checkboxes write are never read.

Protected baskets (`default`, `backup-*`, `admin *`) **are** still excluded — `dispatch_scoped_grant`
forces `bundled_scope_grant = false` for those before calling the engine. So it is not unbounded.

**Pre-existing**: introduced by Phase 2.6-D Fix #4, not by Phase 0.8. But Phase 0.8 is the phase
about this modal telling the truth, and a list of unticked permissions sitting under a ticked
"don't ask me" box implies a granularity the engine does not have.

## What Phase 0.8 already did (option 1a — UI only, no engine change)

- While quiet mode is on, the per-permission checkboxes in Customize are **disabled**, with a
  callout: *"Quiet mode is on, so this site can use any protocol or basket without asking — not
  just the ones listed below. Untick Quiet mode to choose individually."*
- The checkbox label now says what it does: *"Let this site use **any** protocol or basket without
  asking — including ones it did not list above"*.
- The summary tooltip says the same.

⭐ Disabling rather than cross-toggling was deliberate. If both controls are editable at once they
can contradict each other, and you need a rule like *"unticking an item also unticks quiet mode"* —
hidden state changes the user did not ask for. Disabling makes the contradiction **unrepresentable**:
one rule, nothing to keep in sync. (Owner steer: *"consistent, safe, and simple… if something gets
too complicated fall back to simple but safe and consistent."*)

## What is still owed — the engine half

**Narrow the flag so quiet mode covers only what the manifest declared** (or what the user has
otherwise granted), i.e. consult `scoped_grant_exists` first and let `bundled_scope_grant` act as a
"don't prompt for the things already granted" silencer rather than a blanket one.

⛔ This is an **engine behaviour change on the auto-approve path** — CLAUDE.md invariants #2/#3.
It needs its own contract, its own RED/GREEN, and an explicit owner yes. Points to check:

- What happens to a site that legitimately uses an **undeclared** protocol after connect? Today:
  silent. After: a scoped-grant prompt. That is more prompts, and the "Always allow" answer writes
  the V18 row, so it self-heals per protocol. Confirm the prompt actually reaches the user rather
  than failing the call.
- `CounterpartyUse` is silenced **earlier** (Fix #3) and is unaffected either way.
- The **keyID widening** rides along: BRC-73 has no `keyID`, so a manifest-derived protocol grant is
  written with `key_id = "*"` (any key under that protocol). Faithful to the spec, but it means even
  the narrowed flag is wider than a per-call "Always allow" grant would be. Decide whether that is
  intended and document it either way.
- Migration question: existing rows have `bundled_scope_grant = 1` with **no** record of what was
  declared at connect time. ⭐ The V24 `domain_manifest_snapshots` table gives a partial answer for
  sites approved after Phase 0.8 — for older ones there is nothing to narrow *to*, so the change
  must degrade to "prompt on first use" rather than "deny".

## Owner steer, 2026-08-23

> *"I agree we should just prompt later but expand our engine as needed as we learn. No one is using
> it yet so this is a good start."*

So: prompting is the acceptable fallback, and the engine grows as real apps arrive. That argues for
doing the narrowing **before** meaningful third-party adoption, while extra prompts cost nothing.

## Related

- `0.4.0-beta.3/phase-0.8-manifest-shape/PHASE_CONTRACT.md` — the phase that surfaced this
- `TICKET_manifest_description_can_misdescribe_protocol.md` — the sibling. ⚠️ Note the interaction:
  while quiet mode is blanket, a misdescribed protocol buys an attacker nothing extra, because the
  site could use that protocol anyway. Narrowing quiet mode **raises** the value of fixing the
  description problem — do not treat them as independent
- `matrix_c.rs :: decide_scoped_grant`, `request_gate.rs :: dispatch_scoped_grant`
