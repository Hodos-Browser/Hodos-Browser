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

---

## 👤 Owner question, 2026-09-21 — "is there any reason that would be bad?" — the trade-off, recorded

**Yes, it is a real trade-off, and it is the reference wallet's.** The security level is an *input to the key*,
alongside the protocol name and keyID — **the requesting site is not.** So every site asking for
`[0, "xanaverse"]` / keyID `1` / counterparty `self` derives the **same** key Xanadu uses. Silent level 0 therefore
means:

1. **Cross-site use of another app's level-0 key.** A connected (approved) site can silently sign, HMAC, encrypt or
   decrypt under a level-0 protocol another app uses — e.g. produce signatures "as the user" for `xanaverse`.
2. **Linkability.** Level-0 keys are identical on every site. ⚠️ Not introduced by this fix — `getPublicKey` already
   has it; see `0.4.0-beta.4/tickets/TICKET_derived_public_keys_have_no_prompt_and_can_match_across_sites.md`,
   which also confirms `wallet-toolbox` skips the prompt at level 0 even for public-key revelation.

**Why it stands:** BRC-43 defines level 0 as open; apps are to use level 1/2 for anything sensitive, and the reference
wallet behaves exactly this way. Bounds that still hold: approved domains only (unknown → connect prompt, blocked →
deny); payments keep their caps; identity-key reveal keeps its own perimeter gate. Level 0 alone cannot move money
or reveal the identity key.

**Rejected alternative — "only skip if a higher level is already approved":** different levels are different keys,
so a level-1/2 grant says nothing about the level-0 key; and it would not have fixed Xanadu (`xanaverse` was never
granted at any level). Every middle option collapses into "ask once and remember" — the prompt the owner rejected.

**Better long-term direction (beta.4):** record *which site* used *which* derived key, so misuse is visible and
revocable, rather than prompting up front.

---

## ✅ 👤 OWNER DECISION, 2026-09-21 — A stays. Supersedes the "trade-off" framing above where they conflict.

**Correction to the section above.** It listed the key's inputs as protocol name + keyID + level and **omitted the
counterparty**, which is the input that makes a key site-scoped. When a site sends **its own public key** as the
counterparty, the derived key differs per site by the math (root `CLAUDE.md` §"Key vocabulary": *site-scoped key*).
"Every site gets the same key" holds only for `counterparty: self | anyone` — the shape Xanadu's upvote used — not
in general.

**The prompt is not a mitigation.** 👤 *"The pop-up is useless... there's no information in the modal for the user
to know anything whatsoever."* A level-0 prompt cannot tell a user whether a key is shared with another site, and the
site is already approved at a higher level. Re-introducing it would add friction and no protection. ⛔ Do not
"fix" level-0 by restoring the prompt, once-per-site or otherwise.

**The mitigation is the beta.4 plan** (owner): every site sends its public key as the counterparty; the wallet
records that key alongside how each key was derived; and it checks on every derivation that **no two sites are using
the same public key**. See `0.4.0-beta.4/tickets/TICKET_derived_public_keys_have_no_prompt_and_can_match_across_sites.md`.
Two sites deliberately coordinating to share a key is the residual that plan detects.
