# The "is the wallet busy?" detector counts a long poll as silence

**Status:** 🔵 OPEN — filed 2026-09-08 from the Phase 7c live test.
**Sprint:** 📌 Not assigned. Candidate for **Phase 7d** (same consent surface) or beta.4.
**Surface:** `simple_handler.cpp :: ShowDeferredPermissionTask` + `hodos::WalletActivityTracker`.

---

## What it is

beta.3 P0.9 defers the branded **loopback / local-network** permission prompt so a wallet **connect
modal**, if one is coming, can claim it — one decision answering both instead of two prompts stacked
on each other. The deferral holds while the host is *"still talking to the wallet"*:

```cpp
const bool walletBusy =
    hodos::WalletActivityTracker::GetInstance().IsTalkingToWallet(pr.host, kWalletQuietMs) ||
    PendingRequestManager::GetInstance().hasPendingForDomain(pr.host);
```

`kWalletQuietMs = 1200`, polled every 200 ms, with a 15 s backstop.

## The defect

`IsTalkingToWallet` measures **request events**. A request that is *open but not completing* emits
nothing, so an in-flight long poll reads as silence and the quiet window expires underneath it.

`/waitForAuthentication` is exactly that shape, and it is what a dApp calls **at connect time** —
the one moment the deferral exists to protect.

## Measured, 2026-09-07 (`teragun.com`)

```
17:49:04.401  page console  "[Wallet] Waiting for authentication..."     ← long poll opens
17:49:04.412  OnShowPermissionPrompt … mapped=[loopback]
17:49:05.656  "No connect modal claimed teragun.com … (waited 1200ms)"   ← declared quiet
17:49:06.762  page console  "[Wallet] Authentication received"           ← poll was open the whole time
```

The host was mid-handshake with the wallet for the entire window in which it was judged quiet.

## Why it did not bite here, and why that is not reassuring

teragun was **already approved**, so no connect modal was ever coming and the prompt would have
shown anyway once the 15 s backstop expired. ⛔ **The mechanism failed silently in the one case
where its failure happened not to matter.** On a *first* connect — the case it was written for — the
same blindness fires the standalone prompt while the connect modal is still being assembled, which
is precisely the "prompt that is about to be replaced" the P0.9 comment says was already measured
and fixed once:

> *MEASURED 2026-08-24 13:24:16: the old fixed window expired 71 ms before the modal arrived and did
> exactly that.*

The fixed window was replaced by this quiet detector to close that. The quiet detector has its own
blind spot in the same place.

## Fix sketch — not decided

Track **in-flight** requests, not just completed ones: increment on dispatch, decrement on
completion, and report busy while the count is non-zero (with the existing 15 s backstop as the
guard against a wedged wallet). `hasPendingForDomain` already does something like this for modals;
the wallet-activity side does not.

⚠️ Do not simply raise `kWalletQuietMs`. The P0.9 comment is explicit that a longer fixed guess is
what failed before — *"NOT a guess at how long the connect modal takes — that guess failed."*
A bigger number moves the race, it does not remove it.

## Test note

⛔ The negative control has to be a **first** connect to a site with a manifest, with the wallet's
authentication step slow enough to still be open when the window expires. A test on an
already-approved site cannot fail — which is how this survived.

## Related

- `phase-7c-quiet-mode/PHASE_CONTRACT.md` §5.3 — the live session this came out of
- `simple_handler.cpp :: ShowDeferredPermissionTask` — the deferral, and the P0.9 comments recording
  the earlier fixed-window failure
- `HttpRequestInterceptor.cpp :: CreateNotificationOverlayTask` — `IsConnectModalType` gating, i.e.
  **only** connect modals may claim a parked network permission. ⭐ That restriction is deliberate and
  correct; do not widen it to "any wallet modal" as a shortcut for this bug. A payment or
  protocol-grant modal claiming a loopback permission would grant local-app access from a screen
  that never mentions it.

## ⭐ Adjacent, not the same bug — expected behaviour worth writing down

A site approved **before** P0.9 (2026-08-24) has no stored loopback decision, and no connect modal
will ever appear for it again, so it raises exactly **one** standalone loopback prompt on next use
and never again. 15 approved domains exist in the dev profile, so this is a bounded, self-limiting
catch-up — **not** a bug, and not to be re-diagnosed as one. Confirmed on teragun: the answer
persisted as `('teragun.com', 7 /* Loopback */, 1 /* Allow */)`.
