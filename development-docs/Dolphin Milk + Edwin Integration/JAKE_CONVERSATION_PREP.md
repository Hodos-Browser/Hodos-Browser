# Talking to Jake — Plain-English Prep

> 📁 Part of the **Dolphin Milk + Edwin Integration** doc set — see `README.md`.
> **Created:** 2026-06-29. **What this is:** a plain-language cheat-sheet for the conversation
> we need to have with Jake. No jargon. Written so Matt can lead the conversation and understand
> the answers. The deep technical versions live in `design/` and `research/` — this is the human version.

---

## The situation in one paragraph

We have the right ideas — a secure vault, the signed "permission slip" model, the file-memory/search,
and our own browser interface. That part is genuinely good. The problem is that **Edwin doesn't run
well on our own Windows machine yet**, and some of *why* is ours to fix and some of it is Jake's.
This doc sorts out which is which, and turns it into a short list of things to ask Jake — so the
conversation is productive instead of vague.

The honest headline: **most of the build is ours to do and we can start it anytime. Two pieces of Edwin
truly need Jake — signing and memory — and they're really one conversation.** This isn't as blocked as
it feels.

---

## What's actually going wrong (in plain terms)

### 1. It only runs smoothly on Mac/Linux, not Windows
**What's happening:** Edwin is built for Mac/Linux. To run it on Windows, it has to sit inside a
mini-Linux system (called WSL), and reaching your Windows files from there crosses a slow "bridge."
That bridge is the cause of the multi-minute freezes. We measured it: the same search took **0.5
seconds** the right way and **1 minute 43 seconds** across the bridge.

**Whose job:** Mostly **ours** — we plan to bundle Edwin so it runs *directly* on Windows with no
mini-Linux and no bridge, the same way our wallet already runs. That's packaging work, not a rewrite.
**But** the two "locked" pieces of Edwin (signing and memory — see #3 and #4) need Windows versions
from Jake, or a workaround.

### 2. It cost $20 for about a dozen questions, with so-so answers
**What's happening:** This is **not Edwin being broken**, and it isn't really about which model is
"best." A "deep research" mode was left switched on that takes *every* question — even a simple one —
and fires off a long chain of expensive calls, as if it had to research everything from scratch. It
never stops to ask *"is this actually a hard question?"* first. And it billed each call straight out of
your prepaid OpenAI credit, with no running meter and no cap. So a dozen questions quietly became
hundreds of premium calls. Worse, the answers were still poor — because piling expensive reasoning on
top of weak memory just produces expensive nonsense. (Which is exactly why memory matters — see #3.)

**Whose job:** Both. In *our* product we'll set cheap, safe defaults, **answer simple questions simply**
(only escalate to the expensive deep mode when a question really needs it), and show a visible spending
cap — so a normal question costs a fraction of a cent and nothing can drain your balance. That part is
ours and we control it. It would also help if **Edwin shipped with those safer defaults out of the
box** — worth raising with Jake, and something we could contribute back.

### 3. The two "locked" pieces of Edwin — the heart of the Jake conversation
Most of Edwin is open and we can run it ourselves. But **two pieces are "locked" — closed, and built
only for Mac.** Both need a Windows answer from Jake, and they belong in the *same* conversation.

**Piece A — Signing (the security / permission-slip piece).**
A lot of Edwin's safety depends on *signing* — creating a tamper-proof "permission slip" that proves
you authorized something (this is what stops a hacked agent from moving your money). The part that does
this is locked, and Jake built it only for Mac.
- *The good news:* Edwin has a **side-door** that lets an outside program do the signing instead — and
  our wallet can do exactly that. Jake **already uses this side-door** for his own desktop app, so it's
  proven, not a hack.
- *What we need from Jake:* "Will you officially support that side-door and keep it stable, so our
  wallet can be the signer?" If yes, we don't need a Windows version of his locked signing piece.

**Piece B — Memory (this is the core of our whole product).**
Our bet is that **memory is the hardest, most important thing in AI — the bottleneck.** The best AI has
to have the best memory, so we **don't compromise here.** Edwin's memory has two layers:
- the basic "find related snippets in your files" layer, which is **open** — we can run it on Windows
  ourselves; and
- a **quality layer** that understands your question better and **re-ranks the results so the AI gets
  the *right* information, not just the nearest** — and *that* layer is the locked, Mac-only piece
  (called shad-core).

The quality layer is exactly what makes memory *best* instead of *okay*, so we can't wave it away.
- *What we need from Jake:* "What's your plan to make the memory quality layer work on Windows — will
  you build it, should we run a lighter version, and what does it add that we'd give up without it?
  Memory is core for us, so this one matters a lot."

**One strategic note (worth sitting with, not deciding today):** our single biggest differentiator —
memory — partly lives inside Jake's closed code. That's a reason to *deepen* the partnership so it's
reliable on every platform, **and** to make sure we could stand on our own memory layer if we ever had
to. Name it as a choice, not an accident.

---

## The questions for Jake — short list

**Topic 1 — the two locked pieces (have these together; they're one conversation):**

1a. **Signing side-door.** "Edwin can let an outside program do its signing — the same way your desktop
   app already does. Will you officially support that and keep it stable, so our wallet can be the
   signer? Yes or no?" *(If yes, no Windows version of your signing piece is needed.)*

1b. **Memory quality layer on Windows.** "The part of memory that re-ranks results so the AI gets the
   *right* information isn't built for Windows. What's the plan — will you build it, should we run a
   lighter version, and what does it add that we'd lose without it? **Memory is core for us, so this
   one matters a lot.**"

**The smaller ones (good to ask, not blocking):**

2. **Cheaper defaults.** "Edwin came set to the most expensive model and a deep-research mode by
   default — a dozen questions cost $20. Could the defaults be cheaper and safer out of the box, and
   answer simple questions simply? We're doing that on our side too, and happy to help."

3. **Which version to build on.** "You're mid-rebuild. Which version of Edwin should we build against
   so we're not chasing a moving target?"

4. **The interface inside our browser.** "Can Edwin's screen work nicely inside a panel in our browser,
   and can the rough first-run edges get smoothed (empty tabs, confusing setup)?"

5. **The money split.** "When the agent pays for things through the user's wallet, what's a fair split
   between us?"

---

## How to frame it with Jake

We are **not** asking Jake to do our work. The plan is: we write the code we need and send it to him as
a contribution (a "pull request"); he reviews and merges it; we pull it back. So most of this is *"will
you accept this approach and these changes,"* not *"please go build us something."* The only true
"please build" items are a couple of Windows versions of his locked pieces — and the signing side-door
(#1) is specifically the thing that lets us avoid even that for now.

---

## What we can do *without* Jake (so we're not just waiting)

All of this is ours and can start anytime: bundling Edwin to run natively on Windows, the Edwin button
and in-browser page, separate profiles, the basic file-memory with cheap cloud-based search, the
guided setup with cheap defaults and a spending cap, the three small wallet fixes, and the
permission-slip plumbing. The full build plan is in `implementation/IMPLEMENTATION_PLAN_v1.md`.

Only the *signing handshake* (Phase 3) truly waits on Jake's answer to question #1.
