# 📖 A peer BRC-100 wallet deleted the auto-approve layer we still rely on, and its callers reach us by design

**Found:** 2026-09-21, reading the `b-open-io/1sat-sdk` release of 2026-09-20 against our own wallet.
**Status:** ⬜ UNASSIGNED · **Track:** unassigned — ⭐ suggest **track 2 (1Sat Ordinals)**, because that
track already takes a dependency on this SDK · **Filed by:** Claude, at the owner's request

> ⚠️ **Method note.** ⛔ **This is not a defect ticket.** Nothing here says our wallet is wrong. It
> records a peer implementation making an architectural decision in the opposite direction from ours,
> and asks for a written comparison before track 2 starts.
>
> **Everything below is reading** — their four pull requests and one commit, and our own
> `simple_handler.cpp`, `HttpRequestInterceptor.cpp`, `manifest.rs`, `request_gate.rs` and
> `domain_manifest_snapshot_repo.rs`. ⛔ **Nothing was executed.** Their endpoint has not been run,
> our interceptor has not been tested against it, and the port claim below is a code reading of a
> comment that cites a measurement someone else made on 2026-08-19.
>
> ⛔ **Not verified:** that our interceptor actually re-points a `1sat serve wallet-api` caller. The
> interception is written for the MetaNet bridge on the same port; whether their client's framing
> survives the re-point is unknown and is the first thing to measure.

---

## What happened, on their side

`@1sat/wallet-server` 0.0.56 / `@1sat/wallet` 0.0.112, released 2026-09-20.

Their app-facing endpoint — `1sat serve wallet-api`, `127.0.0.1:3321` — used to answer every dApp by
calling the **admin** wallet-toolbox `Wallet` with the request origin as originator. In their words:
*"That `Wallet` is the admin surface: it ignores the originator on key operations, holds no grants and
never prompts, so every app got full access."* A gate had been bolted onto the router in front of it —
a `SENSITIVE_METHODS` list, a manifest-based `isOriginTrusted` bypass, an `approvalPolicy` hook, and an
auto-approve path when no policy was set.

They removed that layer. Four decisions came out of it:

| # | Decision | Their stated reason |
|---|---|---|
| 1 | Enforcement moves into `LocalWalletPermissionsManager`; anything not granted is **denied immediately** | *"BRC-100 permissions belong to `WalletPermissionsManager`. That layer was in the wrong place."* |
| 2 | ⭐ **No prompting at all.** A first pass put a terminal y/N question in and it was thrown out | *"Nobody sits at this terminal… an agent driving an app never sees this process's output."* A blocking prompt turned a `git push` through their remote helper into a hang |
| 3 | **The denial teaches.** The 400 body carries the exact `1sat permissions grant …` command | The agent reads the fix out of the app's own error output and retries. Grants are re-read on every check |
| 4 | App identity is the **`Origin` header only** — `Originator` and `X-1Sat-Origin` removed, absent or opaque origin is a 400 | Browsers set `Origin` and page scripts cannot change it; the old parser read `X-1Sat-Origin` **first** |

⭐ **The protocol surface did not change.** Apps still make ordinary BRC-100 calls. What changed is that
a call which used to succeed now returns 400 with an instruction. Backward-incompatible in **behaviour**,
not in **wire format**.

## Why it matters to us

**1. Their callers reach our wallet by design.** `simple_handler.cpp` deliberately intercepts foreign
local wallet bridge ports and re-points them at ours:

> *"Foreign local wallet bridges we deliberately intercept and re-point at OUR wallet, so a dApp
> hardcoded to another wallet's port still reaches Hodos. That re-pointing is the entire reason this
> browser owns its own ports."*

`3321` is named in that comment as the MetaNet bridge port. It is **the same port `1sat serve
wallet-api` binds**. So an app written against their endpoint, opened in Hodos, is re-pointed to our
wallet on `31401` — and meets our consent model instead of theirs, with no idea it switched.

⚠️ That is the intended behaviour and it is a feature. The consequence is that **we inherit their
callers**, and their callers are now being taught a grant-then-retry loop that we do not implement.
An app that expects a 400 carrying a grant command gets something else from us.

**2. They deleted the thing we have.** Our auto-approve engine sits in the same architectural position
as the `approvalPolicy` hook and auto-approve path they removed. ⭐ **The question worth answering is
whether their argument is about agents or about architecture.** If it is about agents — nobody is at
that terminal — it does not touch us, because a person is at our browser. If it is about where
enforcement belongs regardless of who is watching, it applies to us and we should know that before
track 2 rather than after.

**3. One of their bug fixes is a map of a hazard in our own schema.** Commit `4931ad8`: their standing
allowances live in an injected local store, while the inherited grouped and connect-time spending check
read **on-chain tokens only**, so a granted allowance looked missing and *"the allowance was requested
again every session."* We hold authoritative permission state in `domain_permissions` plus the V18
child tables, with `domain_manifest_snapshots` beside it that is deliberately never a decision input.
Any beta.4 work on standing allowances or spending limits has exactly this shape: **the grant written
in one place and the check reading another.**

**4. Labels arrive whether we want them or not.** Their `buildInputAssetLabel` stamps
`p <scheme> input id <id>` on any action that spends a basket asset, **unconditionally — there is no
opt-out on the action** (their PR #80). If track 2 consumes their action-building at any layer, our
wallet receives actions carrying those labels and needs a decided answer: ignore, or route.

## How exposed are we — answer this first

| If | Then |
|---|---|
| Their client framing survives our port re-point | We are already serving their apps today, and the grant-loop mismatch is live, not future |
| It does not survive | Their apps fail in Hodos in some other way, which is worse and also unmeasured |
| Track 2 consumes only their **types and action building**, not their server | Items 1 and 2 drop to background reading; items 3 and 4 stay |

⛔ **None of these is known.** The first is a one-session measurement: run `1sat serve wallet-api`,
open a dApp that targets it in Hodos, read `debug_output-<pid>.log` for the re-point.

## What already protects us, and how it shapes the work

- **`hodos::IsWalletOrigin`** answers the interception question on the parsed authority, deliberately
  broader than strict equality — *"an un-intercepted request is not ungated, it is TRUSTED, so
  narrowing a matcher here is a privilege change."*
- **Origin is not a header for us.** We derive the calling origin inside the browser, which is
  structurally stronger than reading `Origin` off an HTTP request. Their PR #78 is a fix for a problem
  our architecture mostly does not have. ⚠️ *Mostly* — we do serve an HTTP transport, and whether any
  header can influence the origin we attribute has not been read.
- **We already parse and store the manifest** (`manifest.rs`, `domain_manifest_snapshots`), so their
  `isOriginTrusted` manifest bypass is a shape we can recognise rather than one we have to learn.

## Proposed work — a read and a written answer, not a code change

⛔ **No code changes are proposed by this ticket.** The deliverable is a document.

1. **Read their reasoning properly** — PRs #78, #79, #80, #81 and commit `4931ad8`, plus
   `LocalWalletPermissionsManager` and the `wallet-api` skill they published.
2. **Write the comparison**, one table: for each of their four decisions, what we do, which is better
   for our user, and why. ⭐ **Their user is an agent and ours is a person** — that difference has to
   be carried through every row rather than stated once.
3. **Measure the port question** (above).
4. **Recommend**, in three buckets: adopt · decline with a reason on file · defer with a re-check
   condition.

**Deliberately out of scope:** changing the auto-approve engine; implementing a grant-then-retry error
path; taking `@1sat/*` as a runtime dependency of the Rust wallet. ⭐ Their SDK is **TypeScript**
(4.67 MB of it) and our wallet is Rust — this is a reference, and the house rule applies: port the
patterns and the semantics, never the code.

## Test and negative control

A read ticket still owes a control, and here it is: ⛔ **the comparison is not finished until it names
at least one decision of theirs that is better than ours and one of ours that is better than theirs.**
A comparison that comes back all one way is advocacy, not a reading, and should be sent back.

## Watch conditions

- ⚠️ They are at **0.0.112** and this release's own description is *"the old behaviour is deprecated and
  removed."* Pin versions and re-read before each track-2 milestone.
- ⚠️ **The repo has no LICENSE file** *(measured: the GitHub API reports `license: null`)*. The published
  `@1sat/wallet` package declares `"license": "MIT"` in its `package.json`. Settle this before any
  dependency decision, not after.

## Links

- Release commit `52cfe69` — wallet 0.0.112, wallet-server 0.0.56, cli 0.0.121
- https://github.com/b-open-io/1sat-sdk/pull/78 — Origin header only
- https://github.com/b-open-io/1sat-sdk/pull/79 — the permissions manager rebuild
- https://github.com/b-open-io/1sat-sdk/pull/80 — pass-through modules, and the unconditional label
- https://github.com/b-open-io/1sat-sdk/pull/81 — the `wallet-api` skill, their own documentation of the model
- `cef-native/src/handlers/simple_handler.cpp` :: the `is_wallet_traffic` gate and `hodos::IsWalletOrigin`
- `rust-wallet/src/permission_service/request_gate.rs` :: `domain_trust_gate`
- `rust-wallet/src/database/domain_manifest_snapshot_repo.rs` :: the three snapshot rules
