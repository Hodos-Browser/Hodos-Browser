# TICKET — a security-level-0 protocol prompts, right after the user approved the site

**Opened** 2026-09-21 by the **owner**, at the keyboard. **Blocks the beta.3 build** (owner's call).
**Status:** ✅ **FIXED 2026-09-21** (`10b2916`) — engine arm `SilentProtocolLevelZero`, 5 engine tests + a wiring test, negative control run. 📏 **Live, owner at the keyboard:** after the fix, 2 `createSignature` calls under `[0, "xanaverse"]` went through with **zero** scoped prompts and 4 likes succeeded (gold pill on the originating tab) — the same call prompted at 12:24:04 on the old wallet. Confirmed again at 13:27 on the rebuilt shell. ⚠️ Owner lesson: this fix *hid* the queue freeze on Xanadu, so it should have landed AFTER that fix was live-verified — see the hang ticket.

> ### 📏 Confirmed — `zanaadu.com`, owner at the keyboard, 2026-09-21 12:24
> Every **"like"** does a `createAction` (a 1-sat token into basket `xanaverse-upvotes`) and then a
> **`createSignature` with `protocolID: [0, "xanaverse"]`, keyID `"1"`, counterparty `self`**. That call
> produced `protocol_permission_prompt, reason=scoped_grant_missing`. The manifest (`babbage` namespace)
> declares **10 protocols** and the approval stored exactly those 10 — **`xanaverse` is not among them.**
> ⇒ exactly the predicted shape: a **level-0** protocol the site uses but never declared. The reference
> wallet would return `true` without asking. ⛔ Permission-engine logic ⇒ owner approval before any change.

## What the owner sees

On **Xanadu**, dev build (current code): the site sends its manifest → the connect modal appears →
the owner approves everything → a **second** modal appears **immediately**, asking about a
**"level 0"** permission. 👤 *"What does level zero ask for? Why is it not in their manifest? Or if it
doesn't ask for anything, should we just be bypassing that?"*

## What "level 0" is

BRC-43 gives every protocol a **security level**: `0`, `1` or `2`. Higher is **narrower** (2 re-asks
per counterparty, 1 covers all counterparties at once). **Level 0 is the "open" level** — a protocol the
app itself has marked as not sensitive.

## ⭐ What the reference wallet does — checked, not recalled

`bsv-blockchain/wallet-toolbox :: src/WalletPermissionsManager.ts :: ensureProtocolPermission`,
fetched 2026-09-21:

> `// If security level=0, we consider it "open" usage`
> `if (level === 0) return true`

⇒ **The reference wallet never prompts for a level-0 protocol.** It returns before any permission
check. (Root `CLAUDE.md` rule 5: `wallet-toolbox` is the authoritative answer for how a conforming
wallet behaves.)

## What Hodos does instead

`hodos_permission_engine/src/matrix_c.rs :: decide_scoped_grant` treats **every** `ProtocolUse` the
same, whatever its level: a matching V18 row ⇒ silent, otherwise ⇒ `ProtocolPermissionPrompt`. And
`domain_permission_repo.rs :: is_protocol_granted` matches on the **exact** `(security_level,
protocol_name)` pair. So a level-0 protocol the manifest did not declare — or declared under a
different name or key — has no row, and prompts.

⇒ **The likely account of the owner's symptom:** Xanadu uses a level-0 protocol at runtime that its
manifest does not list, and we prompt where the reference would not. ⚠️ **Inferred, not observed** —
the first step is to read the actual request from the wallet log (protocol name, level, keyID) and
confirm it is level 0 and undeclared.

## The decision, and the one trade-off worth knowing

**Proposal:** match the reference — a level-0 protocol is silent for an **approved** site.

⚠️ **The trade-off the reference accepted:** the *site* chooses the level. A site could label anything
level 0 to avoid a prompt. The reference's position is that level 0 means the derived key is not
meant to protect anything — so there is nothing to consent to. ⛔ That argument holds only for
**key derivation**; it should be checked that nothing money-moving or identity-revealing is gated
*solely* on the protocol grant (payments have their own caps; identity reveal has its own perimeter
gate — `CLAUDE.md` §"privacy perimeter gates").

## Scope of the fix, if approved

Small and in one place: an early `Silent` arm in `decide_scoped_grant` for `ProtocolUse` at level 0,
**on an approved domain only** (an unknown or blocked domain still hits the trust branch first).
Needs the security level on `PermissionContext` if it is not already there.

## Evidence rows

- `L0-1` 🔴 before: Xanadu (or a fixture calling a level-0 protocol absent from its manifest) raises a
  second prompt after connect.
- `L0-2` ✅ after: the same call is silent, **and** a level-**1** undeclared protocol **still prompts**
  (negative control — a fix that silences every protocol has broken consent).
- `L0-3` ✅ an **unknown** domain calling a level-0 protocol is still gated by trust.
- `P13-R1`-style: the engine's existing `p7c_quiet_mode_never_changes_any_scoped_outcome` guard still passes.
