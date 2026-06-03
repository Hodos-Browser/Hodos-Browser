# Jake 1:1 Meeting Prep — 2026-06-02 (Tue)

**Time:** Tomorrow mid-morning (proposed 10:00 AM MT)
**Duration:** 30 minutes
**Format:** Video call (Google Meet or Zoom)
**Outcome we want:** Jake aligned in principle + four explicit decisions captured + Edwin's actual current state understood + (ideally) explicit OK to be named on the AWS application

---

## Jake's preview tells us where to start

In his email reply, Jake gave us a free architectural insight:

> *"that's a key problem Edwin solves. The alternative is to containerize and completely isolate the agent which defeats most of the purpose of using a Personal AI in the first place"*

**What this tells us:**
1. He sees the SAME problem we do (agent + signing power = unsolvable)
2. He's already thought about the alternative (containerization) and rejected it
3. His framing — *"defeats most of the purpose of using a Personal AI"* — is itself deck-quotable

**Use this as the opening:** *"Your line about containerization defeating the purpose really landed — that's the whole thing. Comet, OpenClaw, all of them are containerizing in some form and it doesn't actually solve the problem."* Quote him back to himself; it signals you read carefully and validates his thesis.

---

## Pre-meeting reading (last 30 min before the call)

In order of importance:

1. **`EDWIN_VS_DOLPHIN_MILK_SECURITY.md`** — the security comparison. Be fluent in the policy-vs-cryptographic distinction. You'll likely refer to it.
2. **`ARCHITECTURE_TECHNICAL.md` §4-5 + §9** — the PermissionEngine-as-envelope-issuer synthesis + the 7 open design questions for Jake.
3. **`CANARY_A1_WALLET_COMPAT.md` summary** — the wallet compat verdict. Be ready to say *"wallet compatibility is validated, 3 trivial patches"* in one sentence.
4. **Jake's email reply** — re-read 30 seconds before the call to set tone.

Don't bring up the threshold-signing network or the all-hands meeting in this 1:1. Those are downstream conversations.

---

## Topics to cover from us (priority order)

### Topic 1 — Where does SecureVault live in Hodos? (10 min)
The single most consequential engineering decision. Three options from `ARCHITECTURE_TECHNICAL.md` §9:
- (a) New endpoints on Hodos's wallet at :31301 (wallet becomes its own vault)
- (b) Separate process between Dolphin Milk and the wallet
- (c) Inside Hodos's C++ shell as part of `PermissionEngine`

**What we want:** Jake's gut + reasoning. Don't try to decide today; capture his lean.

**Our lean (don't reveal first; let him state his):** Option (a) with PermissionEngine extension producing the envelope spec. Reasoning: minimizes process count, reuses existing decision logic, envelope becomes the cryptographic artifact attached to PermissionEngine decisions.

### Topic 2 — BSL-1.1 clearance shape (5 min)
Edwin is BSL-1.1 (→ Apache 2.0 in 2030). The "competitive product" clause technically applies if Hodos+Edwin ships as a personal-AI product. What shape of clearance works for Jake? Options:
- Written one-pager exception for Hodos use
- License amendment / dual-license
- Wait for the 2030 Apache conversion (not feasible)
- Some other structure

**What we want:** Jake's preferred shape, not a final commitment. Just understand what he needs from us.

### Topic 3 — The UX problem (5 min)
Avoiding 50 prompts per chat session. Our synthesis: PermissionEngine returns Silent/Prompt/Deny; Silent decisions auto-issue envelopes; user policies bound the auto-issuance scope. Same envelope schema, narrower issuance source.

**What we want:** Jake's reaction. Does the synthesis fit his envelope model? Are there subtleties we're missing? (E.g., envelope TTL refresh, nonce replay risk, what happens if user policy changes mid-session.)

### Topic 4 — AWS pitch + naming (5 min)
Brief description of Build AI on AWS, June 25, $25K Futran services. June 12 application deadline.

**What we want — three explicit yes/no questions:**
1. Are you OK being named on the application?
2. Is "Head of Network Infrastructure at the BSV Association — led Teranode development" the right framing of your background, or do you have a preferred phrasing?
3. Are you OK being mentioned in the pitch night itself (June 25 attendance optional)?

---

## What to listen for in Jake's Edwin update

He said he wants to give us an Edwin update. Real questions to capture:

| Question | Why it matters |
|---|---|
| **What's the stable surface to target?** Current shipped (2026.2.3) or post-refactor `main`? | Determines what we promise in the application |
| **What's the refactor about?** Local commit log shows lancedb→qmd memory backend, web-UI/canvas disabled, channel extensions pruned. What's the destination? | Tells us if Edwin's surface is converging or still in motion |
| **What's the next major milestone for him?** Beta release? Public launch? | Aligns timelines with our pitch and integration plan |
| **Is the vault primitive extractable as a standalone library?** | Affects how we link from Hodos — depend on a crate vs. reimplement against his schema |
| **Is the envelope schema stable enough to propose as a BRC?** | Long-term standardization play |
| **What does HE want from a Hodos partnership?** Distribution? Users? Validation? Co-marketing? | Determines the monetization shape and our value to him |

**Don't pepper him with all six.** Pick 2-3 based on what he volunteers. Capture the rest async after.

---

## Monetization — light touch

You already mentioned in the email: *"I could add a micro-fee to On Chain Innovations whenever Edwin is used."* That's enough framing for this meeting. Don't get into the three-pool model from `ALL_HANDS_MEETING_PREP.md` — that's for the bigger group with John + Mitch in the room.

**If he asks about specifics:** *"I have a starting framework but I'd rather lock the architecture first and then we figure out the economics. Probably something like a portion of the per-transaction service fee whenever the envelope is invoked. Open to whatever shape works for you."*

That's it. Don't commit to numbers in a 1:1.

---

## Four decisions we want to walk out with

| # | Decision | Acceptable answers |
|---|---|---|
| 1 | Jake is open in principle to Edwin integration in Hodos | Yes / Yes-with-conditions / Need-more-time |
| 2 | Where SecureVault lives — his lean (not final) | (a) / (b) / (c) / "I'd want to think about it but my gut says X" |
| 3 | BSL-1.1 clearance — what shape works | Specific shape proposed, OR "let's discuss this offline" |
| 4 | OK to name on AWS application | Yes / Yes-with-framing-tweak / No |

Anything else captured is gravy.

---

## What NOT to bring up in this 1:1

Save for the all-hands meeting (or never):

- **The threshold-signing network** (John + BINARY's work). Even though it's relevant, Jake should hear about it as part of the group conversation, not a side-channel briefing.
- **TSS as wallet-backup pivot.** Same reason.
- **The specific monetization split percentages** (60/40, three-pool model). Light-touch framing only.
- **Comparisons between Edwin and OpenClaw's product surface.** Edwin and OpenClaw are competitors in the personal-AI space; pointing it out could feel like we're saying "your competitor has 3.2M users, hint hint." Just stay focused on the security thesis.
- **Mitch + Ishaan's Mines connections.** Beck-pitch material, not Jake-meeting material.
- **The June 4 Boulder recon trip.** Irrelevant to him.

---

## Tone notes

- Engineer-to-engineer, not founder-pitching-founder
- Quote him back to himself early (the containerization line)
- Confident on the wallet compat result — *"validated, 3 trivial patches"* — but humble on Edwin architecture (it's his domain)
- Match his time discipline — he proposed Tuesday or Wednesday, doesn't waste time. Start on time, end on time.
- Don't oversell. He's a credentialed BSV-AWS engineer; he'll smell hype immediately.

---

## After the meeting — within 2 hours

1. **Email summary** to Jake — capture decisions, action items, what we'd do next. Keep it short, factual.
2. **Update this doc** (or write a `JAKE_MEETING_OUTCOME.md`) with what landed.
3. **Update `CONVERGENCE_NARRATIVE.md` + `ARCHITECTURE_TECHNICAL.md`** if his Edwin update changes any architectural assumption.
4. **Ping John** about the all-hands meeting — now informed by what Jake said.

---

## Logistics checklist for tomorrow morning

- [ ] Send the invite tonight (Monday) — 10:00 AM MT Tuesday, 30 min, Google Meet or Zoom link in the invite
- [ ] Subject: *"Hodos + Edwin integration chat — Matt + Jake"*
- [ ] Body: 1-2 sentences confirming time + meeting link. Don't re-pitch.
- [ ] Test the camera/audio 5 minutes before
- [ ] Have all the docs open in browser tabs, ready
- [ ] Have a notebook (paper or doc) open for capturing Jake's Edwin update
- [ ] Don't share screen unless he asks — this is a conversation, not a demo
- [ ] End on time. Offer to follow up async on anything that didn't fit.

---

## Related

- `EDWIN_VS_DOLPHIN_MILK_SECURITY.md` — pre-read
- `ARCHITECTURE_TECHNICAL.md` — pre-read (§4-5 + §9)
- `CANARY_A1_WALLET_COMPAT.md` — pre-read (summary)
- `CONVERGENCE_NARRATIVE.md` — context only, don't pitch from
- `ALL_HANDS_MEETING_PREP.md` — what comes AFTER this 1:1
