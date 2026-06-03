# Pitch Event — Build AI on AWS Golden Pitch Competition

**Status:** CONFIRMED (details copied from event page by Matt, 2026-05-29)

## Event details

| Field | Value |
|---|---|
| **Event name** | Build AI on AWS Golden Pitch Competition |
| **Host** | Colorado SBDC TechSource, facilitated by Boulder SBDC |
| **Partners** | AWS, Futran Solutions, Beck Venture Center |
| **Date** | **Thursday, June 25, 2026, 5:00 PM – 7:00 PM** |
| **City** | Golden, CO |
| **Format** | In-Person |
| **Cost** | Free to attend |
| **Capacity** | 70 spaces (general attendance) |
| **Registration cutoff** | Thursday, June 25 at 4:45 PM (for attendance) |
| **Pitch slots** | Up to 10 finalists selected to pitch live |
| **Prize** | **$25,000 development grant** in professional engineering / development services + access to additional AWS funding |
| **Eligibility** | US-based small or medium-sized business with a clear digital project idea, operational or ready to launch |
| **Topic scope** | "AI-enabled or cloud-based project" — process automation, AI solution development, cloud modernization, analytics, web/mobile applications |
| **Application deadline** | **Thursday, June 12, 2026** |
| **Application form** | https://docs.google.com/forms/d/e/1FAIpQLSc8j5yr86xb9tii7VxM4NBqq7lNSgCmQB9mB4C-B5M7aifW0w/viewform?usp=publish-editor |
| **Host contact** | Audrey Miller, Senior Program Manager SBDC TechSource, Boulder SBDC — audrey.miller@bouldersbdc.com — (303) 442-1475 |

## Critical dates

| Date | Days from 2026-05-29 | Milestone |
|---|---|---|
| **2026-06-12** (Thu) | **+14 days** | Application due |
| ~2026-06-15 (estimated) | +17 days | Finalist notification (estimated, not confirmed) |
| **2026-06-25** (Thu) | **+27 days** | Pitch night |

**Working assumption:** finalist selection happens within 2-3 days of the June 12 deadline, leaving roughly 10-13 days from selection to pitch night for the PoC sprint.

## Implications for our timeline

| Track | Deadline | Notes |
|---|---|---|
| Partner alignment (John ✓, Jake to do) | ~2026-06-05 (Fri, ~7 days) | Both names on the application gives partnership credibility |
| Application submission | 2026-06-12 (Thu, 14 days) | Hard deadline |
| Canary A1 (wallet compat) | ~2026-06-08 (Mon, 10 days) | Informs what we promise in the application |
| Architecture v1 (for John/Jake meetings) | ~2026-06-05 (Fri, 7 days) | Needed before partner sign-on |
| PoC scope decision (α / β / γ) | ~2026-06-12 (Thu, 14 days) | Decide in concert with application content |
| PoC build sprint | 2026-06-15 – 2026-06-24 | If selected as finalist |
| Pitch deck v1 | ~2026-06-18 (Wed, 21 days) | For rehearsal cycles |
| Pitch night | 2026-06-25 (Thu, 27 days) | Live demo + slides |

## Eligibility check — does Hodos qualify?

- ✅ US-based small business (Marston Enterprises, Colorado)
- ✅ Clear digital project idea (Web3 browser with bundled AI agent)
- ✅ Operational and ready to launch (Hodos already builds + runs)
- ✅ "AI-enabled" project (bundled Dolphin Milk + Edwin = AI agent runtime + security)
- ✅ "Cloud-based" tangentially (x402 micropayments to cloud LLM endpoints; agent runs locally but pays cloud providers)
- ⚠️ Pitch positioning: judges will be AWS + small-business-cloud-focused, not BSV-native. The narrative needs translation — lead with the AI/security/no-subscription story; let BSV be the implementation detail that *enables* the story, not the story itself.

## What goes in the application

Top of mind for the Google Form (need to actually open it to confirm fields):
- Project name + 1-sentence summary
- Problem the project solves
- Solution overview (lay-friendly)
- Tech stack overview (AWS-friendly framing)
- Stage / readiness
- Team (Matt + John + Jake — needs all three signed off before submission)
- Revenue model / business viability
- AWS / cloud relevance ($25K is AWS-flavored grant)

## Risk register for this event

1. **BSV is not AWS's worldview.** Pitch must translate without erasing what makes us different. Lead with subscription fatigue + agent security; let "BSV micropayments" be the *mechanism* that makes pay-per-prompt possible.
2. **AWS prize is engineering services, not cash.** $25K of dev services from Futran is real value but doesn't unlock OAuth-Verification CASA costs or Mac notarization. Frame accordingly in expectations.
3. **Three-party pitch from a solo founder** could read as overcommitting. Need John & Jake's explicit commitment in writing before naming them in the application.
4. **27-day window is tight for a live demo of a 3-party integration.** Option α (Dolphin Milk on Hodos wallet, no Edwin yet) is the realistic live demo. Option β (with Edwin SecureVault) is stretch. Option γ (slides + recorded demo) is safety net.

## Source

- Event page copy verified by Matt 2026-05-29 from https://socgov38.my.site.com/ColoradoSBDC/s/event-function?language=en_US&c__recordId=a0HV10000087gYzMAI
- Application form: https://docs.google.com/forms/d/e/1FAIpQLSc8j5yr86xb9tii7VxM4NBqq7lNSgCmQB9mB4C-B5M7aifW0w/viewform?usp=publish-editor

## Related

- `PRODUCT_OUTLINE_v1.md` — overall product framing
- `EDWIN_VS_DOLPHIN_MILK_SECURITY.md` — security model comparison (writing now)
- `CANARY_A1_WALLET_COMPAT.md` — wallet compatibility check (dispatching now)
