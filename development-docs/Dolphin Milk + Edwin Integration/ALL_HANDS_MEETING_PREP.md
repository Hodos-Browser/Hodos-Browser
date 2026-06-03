# All-Hands Convergence Meeting — Preparation

**Purpose:** Get all six engineers / co-founders in one video call to surface the convergence explicitly, get explicit alignment on the AWS pitch, and explicit consent for naming on the application.
**Target date:** Week of June 8-12, 2026 (before application due June 12)
**Host:** Matt Archbold
**Duration:** 60-90 minutes
**Mode:** Video call (Zoom or equivalent). Recording optional with consent.

---

## Why this meeting matters

Today: four engineers + one marketing/finance lead are independently building pieces of the same vision. They know each other (most met at the Austin hackathon April 2025), but they've never sat in a room together to look at the convergence as a whole.

After this meeting: explicit shared understanding of how the four components fit together, who's OK being named on the AWS application, and a starting framework for monetization between Hodos / Edwin / the signing network.

**This meeting is the single highest-leverage action between now and the June 12 application deadline.** Skipping it means we write the application without alignment, which risks Jake or Mitch saying *"don't put my name on that"* after the fact.

---

## Who to invite

| Name | Role | Why critical to the meeting |
|---|---|---|
| Matt Archbold | Hodos Browser, host | Coordinator, presenter |
| John Calhoun | Dolphin Milk + threshold-signing network co-builder | Owns the agent runtime + co-owns the signing infrastructure |
| Jake Jones | Edwin + former BSVA Head of Infrastructure (Teranode) | Owns the security envelope; credibility for the BSV-AWS story |
| Mitch Burcham | Binary Distributed Technologies, co-builds signing network | Owns the signing infrastructure side, Mines connection |
| Ishaan Lahoti | BSV dev, BINARY team, Hodos beta contributor | Mines connection, hackathon winner, already in Hodos code |
| Matt Calhoun | Calgooon Inc. (John's brother), marketing + finance | Marketing/finance lens, partnership voice |

**Six people. All on Zoom. Even four-out-of-six is a successful meeting.** Don't reschedule because one can't attend; capture the absent person's view via async message after.

---

## Agenda (90 min)

| Time | Item | Notes |
|---|---|---|
| 0:00 – 0:05 | Welcome + ground rules | Frame: not a sales pitch, a coordination conversation. Recording optional. |
| 0:05 – 0:35 | Round-robin intros (5 min each × 6) | Each person: what they're building, where it is today, what they need most |
| 0:35 – 0:50 | The convergence map | Matt presents the 4-layer view + how each piece fits + the hackathon-origin story |
| 0:50 – 1:05 | AWS pitch alignment | $25K, June 12 application, June 25 pitch, three-built-plus-fourth framing |
| 1:05 – 1:20 | Coordination / monetization / timeline | Three-pool fee model proposal, who operates what, how decisions flow |
| 1:20 – 1:25 | Explicit ask — naming on application | Each person says yes/no/conditional to being named |
| 1:25 – 1:30 | Wrap + next steps | Follow-up actions, who connects to whom, when we reconvene |

---

## The convergence map (Matt's 15-minute presentation)

This is the visual / narrative Matt walks the room through. Keep it short — most of the room are engineers; they'll get it fast.

### Slide 1 — Origin (1 min)
> *"We met at Austin in April 2025. Three of us placed top-three: Ishaan first, me second, John third. Mitch was competing with another team. None of us were coordinating, but each of us went home and built a piece of the same vision."*

### Slide 2 — Four pieces, one rail (3 min)
ASCII diagram showing the four layers + the BRC-100 wallet at port 31301 as the shared rail:

```
                  USER
                    │
              ┌─────▼──────┐
              │   HODOS    │  ← consumer surface, browser, install
              │   BROWSER  │      Matt — built, in beta
              └─────┬──────┘
                    │
        ┌───────────┴──────────┐
        │                      │
  ┌─────▼──────┐        ┌─────▼──────┐
  │  DOLPHIN   │        │   EDWIN    │  ← cryptographic gate
  │   MILK     │        │  ENVELOPE  │      Jake — built, MVP
  │   agent    │◀──────▶│            │
  │            │        │            │
  │ John —     │        │            │
  │ built,     │        │            │
  │ Apache 2.0 │        │            │
  └─────┬──────┘        └─────┬──────┘
        │                      │
        └──────┬───────────────┘
               │
        ┌──────▼──────────────────────┐
        │   BRC-100 WALLET @ :31301   │  ← the shared rail
        │   Hodos's wallet            │      Matt — built
        └──────────────┬──────────────┘
                       │
        ┌──────────────▼──────────────┐
        │  THRESHOLD SIGNING NETWORK  │  ← infrastructure layer
        │  John + Mitch (BINARY)      │      being built now
        │  CGGMP'24, Rust/WASM,       │      ↑
        │  BRC-100 compatible         │      THIS is what we
        └─────────────────────────────┘      pitch help on
```

### Slide 3 — Three are built, one is being built (3 min)
- **Built and working:** Hodos Browser, Dolphin Milk agent, Edwin envelope
- **Never been integrated together** — but they all target BRC-100 + BRC-31 + BRC-42, so integration is engineering, not invention
- **Being built right now by partner team:** threshold-signing network (John + Mitch announced May 4)
- **What we need:** integration engineering + a Hodos-operated signing node on AWS

### Slide 4 — Why now (2 min)
- Comet has 6 prompt-injection CVEs in 8 months. Industry consensus: *"unlikely to ever be fully solved"* (OpenAI, Dec 2025)
- OpenClaw 3.2M users have documented one-click RCE problems
- Subscription fatigue: $75/month of stacked AI subscriptions for casual users
- BSV is the only chain where per-prompt micropayments are economically viable. AWS itself published the million-TPS case study March 2026.
- **The market is ready, the stack is ready, we just need the integration push**

### Slide 5 — The ask (3 min)
- **Build AI on AWS Golden Pitch Competition, June 25.** $25K Futran engineering services. Application due June 12.
- **What I'd put on the application:** four (or as many as agree) names. The convergence story honestly framed. *"Three built, one being built, integration work is the ask."*
- **What I need from each of you:** yes/no on being named, level of involvement at pitch, any framing requests

### Slide 6 — Beyond the grant (3 min)
- **TSS as wallet backup architectural pivot.** Hodos's current backup is buggy and economically fragile. TSS replaces backup architecturally (no key = no backup needed; recovery = re-share).
- **Monetization split for envelope-gated transactions.** Hodos service fee + Edwin cut + signing-network operator pool. Discuss below.
- **The all-hands cadence.** If this meeting is useful, we do it again monthly through Q3.

---

## Monetization framework (the 15-min discussion)

Current state:
- **Dolphin Milk:** monetized via x402 endpoints (LLM provider pays Dolphin Milk per call). **Built in.** No Hodos-side decisions needed.
- **Hodos:** 1000 sats/tx service fee on outgoing transactions (per `WALLET_SERVICE_FEE_IMPLEMENTATION.md`). All to Hodos today.
- **Edwin:** no revenue today. BSL-1.1 licensed in-house at Marston.
- **Signing network:** per-signature payments to participating nodes via TSS protocol economics. Each node earns directly per signature. **Built into the protocol.**

Proposed model — three-pool split of the Hodos service fee:

| Transaction type | Hodos fee | Hodos keeps | Edwin gets | Signing network gets |
|---|---|---|---|---|
| User-initiated, no envelope, single-key signing | 1000 sats | 1000 sats | — | — |
| Agent-initiated, envelope-gated, single-key signing | 1000 sats | 600 sats | 400 sats | — |
| User OR agent, envelope-gated, TSS-signed | 1000 sats | 500 sats | 250 sats | 250 sats |
| User OR agent, no envelope, TSS-signed | 1000 sats | 750 sats | — | 250 sats |

Notes:
- The user-facing fee stays predictable at 1000 sats regardless of which mechanisms are involved
- Edwin earns only when its envelope is actually used (incentive: build envelope into more flows)
- Signing-network pool is a separate revenue stream from the per-signature TSS protocol economics — this is Hodos's "thank you" to the network operators on top of what they earn protocol-side
- Open: should the signing-network pool be paid to a specific entity (BINARY treasury? John's treasury?) or distributed proportionally to the nodes that actually participated in the signature? The protocol-economics path is the cleaner default.

**This is a starting framework, not a final answer.** The meeting discussion should refine it. Key questions:
- Does Jake think 40% (400/1000) of envelope-gated transactions is the right cut for Edwin?
- Do John + Mitch want the signing-network pool at all, given that nodes earn from the protocol?
- Is the 1000-sat user-facing fee the right number long-term, or does adoption demand lower?

---

## Talking points for tricky moments

### If Jake asks: *"why am I in this meeting if you and John have already been talking?"*
> *"Because the convergence only works if all four engineers are aligned, not just two. I've been talking to John independently because he and I were already on each other's radar. You're not a late add; you're a load-bearing piece. Edwin is the cryptographic gate that distinguishes this product from Comet's failure mode."*

### If Mitch asks: *"why is Hodos integrating with our signing network — are you replacing your own wallet?"*
> *"No. The Hodos wallet stays; the signing key under the wallet becomes a TSS share. The wallet's user-facing surface is unchanged. The integration is the wallet asking the network to participate when it needs a signature. Our pitch is helping deploy a Hodos-operated node and integrating the network with Edwin's envelope policy."*

### If John asks: *"are you trying to entangle Dolphin Milk in Hodos's licensing or distribution?"*
> *"No. Dolphin Milk stays Apache 2.0 upstream. Hodos bundles your binary. Your monetization stays your x402 endpoints. We're not asking for any commitments from you that limit Dolphin Milk's independence."*

### If anyone asks: *"what if we don't win the grant?"*
> *"The integration is still the right architectural direction. The grant accelerates it; the absence of the grant doesn't kill it. Hodos's wallet backup pivot to TSS is going to happen either way because the existing backup isn't economically viable at scale."*

### If anyone says: *"I'm not sure I want my name on the application"*
> *"That's totally fine. Two options: (a) you're named with whatever framing you want, or (b) we cite your project but not your name. No pressure. I'd rather have honest credits than uncomfortable ones."*

---

## What Matt should NOT say in the meeting

- Don't promise revenue projections
- Don't commit anyone else's roadmap timing without their explicit ask
- Don't frame Hodos as the "lead" company — frame it as the integration layer
- Don't oversell the grant ($25K is engineering services, not life-changing money — be honest)
- Don't surprise people. Send the convergence map + agenda 48 hours in advance.

---

## After the meeting

Within 24 hours of the meeting:
1. **Summary email** to all attendees + any absentees. Capture decisions, named/not-named choices, action items.
2. **Update `CONVERGENCE_NARRATIVE.md`** with any changes the discussion produced.
3. **Update `NETWORK_CONNECTIONS.md`** with any new verified facts.
4. **Start `APPLICATION_DRAFT.md`** with the now-aligned team + naming list.
5. **Capture the monetization framework** that landed (or note that it's parked for follow-up).

Within 7 days:
- AWS application submitted (June 12).
- Follow-up email to anyone who needs more context.
- Schedule the next all-hands cadence (recommend monthly).

---

## Logistical TODOs for Matt before the meeting

- [ ] Pick a date/time that works for as many as possible (poll via shared message)
- [ ] Send Zoom link + agenda + the convergence map (this doc) 48 hours in advance
- [ ] Have a one-pager visual of the convergence map ready to share-screen
- [ ] Decide whether to record (with consent) — useful for the meeting summary
- [ ] If feasible, fly to Austin OR get the right subset together in person — but don't sacrifice the meeting happening over the meeting being in-person

---

## Related

- `CONVERGENCE_NARRATIVE.md` (sibling) — the story we're telling
- `ARCHITECTURE_TECHNICAL.md` (sibling) — the technical layer the meeting validates
- `THRESHOLD_ECDSA_EXPLORATION.md` (sibling) — what the 4th layer is
- `../../marketing/intelligence/features/Dolphin Milk + Edwin Integration/NETWORK_CONNECTIONS.md` — verified team facts
- `../../marketing/intelligence/features/Dolphin Milk + Edwin Integration/JAKE_OUTREACH_DM.md` — preliminary Jake outreach (Monday send)
- `../../marketing/intelligence/features/Dolphin Milk + Edwin Integration/AUDREY_EMAIL_DRAFT.md` — Monday Audrey email
