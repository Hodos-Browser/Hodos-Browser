# Narratives — Public Messaging on Decentralized Naming and Bootstrap

**Created:** 2026-05-09
**Status:** Working messaging seed — refine as R&D crystallizes
**Owner:** Matt Archbold

## Caveat

This doc lives in the product repo for convenience while the messaging is forming alongside the R&D. It is **outward-facing communication intent**, not internal engineering. It deliberately uses persuasive register rather than analytical. When we have a proper marketing repo or a permanent home for this kind of content, it should move there.

If you're a future Hodos engineer reading this in the source tree and wondering why it's here: it's here because the engineering decisions and the public story are tightly coupled, and the founder wanted them to evolve together. Treat it as PR scaffolding to be cleaned up later, not as anything that should ship inside the product itself.

---

## Three audiences, three angles

### 1. Builders (other BSV operators, indexer providers, paymail providers, app developers)

**Core message:** *Hodos is a partner, not a competitor. The infrastructure layer is currently under-supplied; the opportunity is yours to take.*

**Talking points:**
- Hodos is a client — a browser and wallet. We're not vertically integrating into indexer/overlay services unless we have to.
- Indexers and overlay providers are the *most under-supplied part of the BSV stack today.* That's an opportunity, not a problem to be solved by Hodos alone.
- Micropayments enable specialization. An indexer doesn't need venture funding to be sustainable. A few sats per query, multiplied by real usage, is a real business.
- Hodos's role is to *consume* infrastructure honestly, *route* our user payments to operators who deliver value, and *amplify* operators who build well. That's a much better deal for builders than competing with us would be.
- We will run our own indexer reluctantly if no one else does. Don't let us.

### 2. Front-runners and early adopters (BSV holders, ecosystem investors)

**Core message:** *Speculation that isn't backed by participation is self-defeating. Use, build, integrate, partner.*

**Talking points (philosophical, founder's voice):**
- "Bitcoin's value is a function of its network. Refusing to transact, build, or partner is refusing to participate in the very thing that creates the value."
- "Gresham's law is a function of Metcalfe's law. Hoarded money doesn't transact. Untransacted money doesn't grow networks. Stagnant networks don't grow value. The hodler who refuses to participate is undermining their own bet."
- "We're not anti-hodl. We're anti-hodl-as-strategy-instead-of-participation. There's a difference between holding a position and refusing to act on it."
- "Front-runners who participate become the foundation of the network. Front-runners who only speculate become its drag. Be the foundation."
- "Cutting off your nose to spite your face." (Saying applies, use carefully)

**Why this works on this audience:**
- BSV holders are already long-term-oriented; they're more receptive to "participate to win" than short-term traders are.
- The argument doesn't ask them to give up their position; it asks them to add participation on top of it.
- Frames hodling as a *partial* strategy that's incomplete without participation — which preserves their pride in their position while still pushing them toward participation.

**Tone notes:**
- Persuasive, not preachy. Don't moralize.
- Confident, not defensive. We're describing how networks work, not asking permission.
- Personal where appropriate — this is the founder's view, can be voiced from "I."

### 3. End users (people who would use Hodos)

**Core message:** *A wallet, a name, a browser — yours to own, easy to use, hard for anyone to take away.*

**Talking points:**
- "Your wallet is yours. Your name is yours. Your browser is yours. Your data is yours. We're just the rendering layer."
- "Pay a few sats to look up a name. Pay a few sats to publish a page. The infrastructure is paid by use, not by surveillance."
- "If Hodos disappears tomorrow, your wallet still works, your data is still on-chain, and another browser can pick up where we left off. We've built ourselves to be replaceable on purpose."
- "We don't run a registry. We don't own your name. We just help you find it."

**Why this works:**
- Most users don't care about decentralization theory; they care about not being screwed.
- Replaceability is an oddly powerful pitch — "we built it so we don't matter" inverts the usual lock-in incentive.
- Ownership language ("your X is yours") is concrete and emotional in a way that "decentralized" isn't.

---

## The Gresham/Metcalfe framing — refined for use

This is the founder's specific contribution. Quote-ready phrasings, in order of how internal-philosophical to how outward-aimed they are:

**Most philosophical (founder's voice, internal):**
> "Speculation is a forecast about adoption. Refusing to participate in adoption while speculating on its outcome is forecasting against your own behavior."

**Mid-formal (essays, long-form posts):**
> "Gresham's law is a function of Metcalfe's law. Hoarded money doesn't transact. Untransacted money doesn't grow the network. A stagnant network doesn't generate the value the speculation was pricing in. The hodler-who-doesn't-participate is the saboteur of their own thesis."

**Punchier (X.com, sound-bite):**
> "Hodling without participating is betting against your own bet."

**Saying-form:**
> "Hodlers who refuse to use, build, or partner are cutting off their noses to spite their face."

**As a refrain we can return to:**
> "Use it. Build on it. Partner on it. Then hodl."

The order of those four verbs matters. **Hodl is the last word, not the first.** That keeps the speculative class with us instead of feeling attacked.

---

## What NOT to say

Things that would alienate the audience or signal weakness:

- "Hodlers are the problem." — Confrontational. Loses the audience we want.
- "We need to fix the BSV community." — Patronizing. We're not the parent.
- "Bitcoin is dying because of [anyone]." — Defensive and false-casual.
- "Adoption is everyone's responsibility." — True but vague; doesn't actually motivate.
- Anything that sounds like a moral indictment of speculation as such. Speculation is fine; passive speculation as a complete strategy is what we're pushing against.

The framing should always be: **"add participation to your portfolio of behaviors."** Not "stop speculating."

---

## When to use this messaging

The full bootstrap argument (Gresham/Metcalfe) is most appropriate when:

- A long-form essay or blog post, not a quick reply
- A talk or interview where you can build the argument carefully
- A direct conversation with a known holder who'd respond to the philosophical frame

It is **not** appropriate for:

- Quick X.com replies (too dense; gets misread as anti-hodler)
- Press releases (too philosophical; doesn't fit the format)
- First contact with a builder or new user (too theoretical)

For everyday messaging, simpler frames work better:
- To builders: "We're a partner. Build the infrastructure."
- To users: "Your name, your wallet, your data."
- To holders: "Use it. Build on it. Then hodl."

---

## How this evolves

This document is **scaffolding for a message that hasn't fully crystallized yet.** Expect it to:

1. Get refined as we test specific phrasings with real audiences (X.com replies, blog posts, conference talks).
2. Get pushed into actual marketing collateral once Hodos's product story is more shippable.
3. Eventually move to a marketing repo (or wherever the marketing function lives) when this kind of writing has a permanent home.
4. Inform the eventual website copy, demo videos, and onboarding flows.

The core argument — that participation, not just holding, is the rational bet for early adopters — should remain stable even as the surface phrasings evolve. That argument is the founder's contribution to the BSV ecosystem's self-understanding, and it's worth defending with care.

---

## Related Hodos docs

- `BOOTSTRAP_PROBLEM.md` (this folder) — the theory behind the messaging
- `README.md` (this folder) — the federated-naming design these messages will eventually describe to the public
- (future) `Hodos/marketing/...` — the eventual home for this material
