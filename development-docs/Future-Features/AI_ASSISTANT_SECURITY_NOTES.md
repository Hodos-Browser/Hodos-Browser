# Native AI assistant — security notes captured early

**Status:** NOT scoped, NOT started. Captured 2026-08-04 while doing the DevTools/CDP hardening,
because that work answers questions this feature will ask, and the answers are much cheaper to
honour up front than to retrofit.

**Why this file exists:** the assistant is the **go-to-market wedge** in the 2026-H2 strategy and is
scoped as a **Demo-Day prototype (Nov 19)** — near-term enough that the constraints below should
shape the prototype, not be discovered after it.

**Read with:** `../0.4.0/DEVTOOLS_SECURITY_DESIGN.md` (the four D-decisions and research spike Q3).

---

## 1. The uncomfortable framing

An in-browser assistant that reads pages and takes actions is, mechanically, **the exact threat we
just spent a session closing** — a process that reads content across origins, executes actions, and
can reach the wallet. The difference is that this one is invited by the user.

That is not an argument against building it. It is an argument for building it as a **first-party,
in-process surface with its own permission identity**, rather than as a privileged insider.

## 2. The single most important design decision

> **Treat the assistant as untrusted content that the user is collaborating with — NOT as the user.**

It should get its **own permission identity** (the way a domain does) and its wallet actions should
route through the **same Rust permission engine** as web content — not bypass it because "the user
asked for this."

Get this wrong once and every other mitigation is decoration. Get it right and Hodos has a genuine
structural advantage: an agent that *cannot* exceed the caps the user already set, enforced in Rust,
not in the agent's prompt.

## 3. Do not build it on the remote debugging port

The path of least resistance will be CDP over `--remote-debugging-port`, because that is how every
AI browser agent works today (Puppeteer/Playwright). **Don't.** It would reopen exactly the hole
`DEVTOOLS_SECURITY_DESIGN.md` D2 closes, and — worse — would require it *on in production*, which is
the one thing that decision rules out.

Options, best first:
1. **First-party in-process surface.** Same shape as DevTools itself: `ShowDevTools()` is in-process
   and needs no socket. The assistant should reach the browser the same way.
2. **`--remote-debugging-pipe`** (research spike Q3) — CDP over an inherited fd. Only the launching
   process can reach it; unreachable rather than authenticated.
3. **A socket with prompt-on-connect or a one-time token** — last resort. Reuses the overlay prompt
   system, but reintroduces a listener.

## 4. Prompt injection is the defining threat

If the assistant reads page content **and** can act, then page content is untrusted input that can
issue instructions. A hostile dApp can embed invisible text — "transfer 10000 sats to …", "approve
this permission", "open the wallet and read the recovery phrase". This is the dominant unsolved
problem in agentic browsing; assume it is not solved by prompting.

For a browser with a wallet that **auto-approves payments under a cap**, the blast radius is real
money. Mitigations, roughly in order of how much they actually buy:

- **Route every wallet action through the permission engine under the assistant's own identity**
  (§2). Caps then bound the damage regardless of what the page said.
- **Never let assistant-initiated spends be silent**, even under the per-tx cap. The cap encodes
  "sites the user chose to trust", which is a different trust context from "an agent acting on the
  user's behalf". A prompt here is the correct friction, and it is consistent with the gold-pill
  principle that every spend is visible.
- **Separate READ from ACT.** Reading a page is low risk and can be liberal. Clicking, submitting,
  navigating and spending are where the gates belong.
- **Keep the assistant out of privileged origins** — the same wallet/overlay origins DevTools is
  being scoped away from (D4). An assistant that cannot drive the wallet UI cannot be talked into
  driving it.

## 5. The "search the web" part collides with the moat

The strategy names the moat as a **surveillance-free, wallet-native economic layer**. An assistant
that ships page content to a cloud LLM exports the user's browsing to a third party — which cuts
directly against that positioning, and against the farbling/adblock/cookie work that defines the
product today.

This is a **product-strategy decision, not an implementation detail**, and it is better made before
the prototype than after a demo: local model vs cloud, what leaves the machine, and whether that is
disclosed per-request or configured once. There is a strong story available here — "the assistant
that doesn't phone home" is a differentiator no incumbent can copy without abandoning their own
business model — but only if the architecture supports the claim.

## 6. What this means for work already queued

- **`DEVTOOLS_SECURITY_DESIGN.md` D2/D4** are prerequisites, not obstacles. Closing the port and
  scoping DevTools off privileged origins is what makes a *safe* assistant surface possible later.
- **Research spike Q3** (pipe mode) should be done before the assistant needs an automation path.
- The **Rust permission engine** (`hodos_permission_engine`) is the asset here. Adding an
  "assistant" caller identity to the existing Matrix C cascade is far less work than inventing a
  parallel gate — and reuse is the standing rule (`feedback_reuse_existing_code_first`).
