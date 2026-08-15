# Auto-Approve Rationale — Why Hodos Differs from Brave

**Date:** 2026-05-05.

## The industry norm

**Brave Wallet does NOT auto-approve any signing operation.** Issue [#27592](https://github.com/brave/brave-browser/issues/27592) was explicitly closed as not-planned. Brave's stance: *"every signature gets a prompt."* This is the product of years of phishing post-mortems on Ethereum.

Most consumer wallets follow the same pattern: every signing or spending operation gets an explicit user confirmation, no exceptions.

## Hodos's 3-layer model

Hodos auto-approves via three combined gates:

1. **Per-domain whitelist** — user explicitly grants ongoing permission to a domain
2. **Per-tab session spend cap** — total spend in a session limited; further spending requires explicit approval
3. **Per-call rate limit** — frequency cap per domain to prevent runaway spending

Calls that pass all three gates execute without an interactive prompt.

## Why we differ

**BRC-29 PeerPay micropayments are sub-cent.** Watching a 30-minute video with metered streaming might trigger 50–100 micropayments at 1 satoshi each. Brave's "every signature gets a prompt" model would make this UX unusable. The whole point of HTTP-402 / BRC-121 / BRC-29 PeerPay is **machine-speed, sub-perceptible payments**. Auto-approve is the feature, not a corner-cut.

This is a fundamentally different design space from Ethereum dApps where every signature is a tx with non-trivial value. BSV's economic model assumes high-frequency small-value flows that can't sustain UX-blocking confirmations.

## Defensive practices Hodos already follows

Hodos's 3-layer model isn't unbounded auto-approve — it's structured to fail safely:

- ✓ **Default whitelist is empty.** User must explicitly grant per-domain trust.
- ✓ **Session spend caps are conservative by default.** Caps at the tab/session boundary, not lifetime.
- ✓ **Per-call rate limits prevent runaway loops** even within an approved domain.
- ✓ **Every auto-approve fires a visible notification** (the user sees what was spent and where).
- ✓ **Hard whitelist** — one tier prevents *any* prompt for known-good flows; user opts in.
- ✓ **Origin + favicon shown** on every approval prompt (Brave's cheapest anti-phishing win — we do this too).

## Where this needs to be communicated

1. **Demo video** — explain the 3-layer model on screen when first reaching a payable site. Show a domain being whitelisted explicitly. Show the auto-approve notification firing for a sub-cent transaction.
2. **User-facing settings docs** — "Why does Hodos auto-approve when other wallets don't?"
3. **Phase 4 demo HTML pages** — the `demo-brc121-402` page is the natural place to explain this UX choice in context.
4. **LLM-ready dev guides** (Phase 4) — note for developers integrating with Hodos: small payments will silently complete; large payments still prompt.

## Open work (Q15)

- Default spend cap value (concrete number) — TBD
- Default per-call rate limit (calls/min) — TBD
- Notification UX (toast? activity-log entry? both?) — currently notification fires; verify visibility
- Periodic "you spent X across these N sites this session" rollup — does this exist?

## Not adopting from Brave

Brave's hard "no auto-approve" stance. If we adopted it we'd kill the BRC-29 / BRC-121 use case Hodos is built around. The right comparison isn't "Brave vs Hodos" — it's "Brave's constraints (general-purpose Ethereum browser, all signatures expensive) vs Hodos's constraints (BSV micropayment-first browser, most signatures trivially cheap)."
