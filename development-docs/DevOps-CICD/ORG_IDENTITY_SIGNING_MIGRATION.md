# Org Identity & Code-Signing Migration (Marston Enterprises)

**Created:** 2026-06-22 · **Owner:** DevOps/CI-CD · **Status:** Windows done; macOS in progress.
**Why:** ship under the **company** name ("Marston Enterprises"), not the founder's personal name. Per root CLAUDE.md Invariant #12.

> **Headline: there is NO reputation loss on either platform.**
> - **Windows** is *already* signed as Marston Enterprises (Azure cert `CN=Marston Enterprises`) — nothing changes.
> - **macOS** is a *conversion* (Team ID + certs + apps survive) and Apple's Gatekeeper trust is **binary** (signed + notarized = trusted) with **no per-developer reputation score** — so the new org cert is trusted identically from its first build. The earlier fear of "rebuilding reputation from scratch" does not apply.

---

## Current state (verified 2026-06-22)

| Platform | Signing identity today | Action |
|----------|------------------------|--------|
| Windows (Azure Trusted Signing) | **`CN=Marston Enterprises`** — org validation already COMPLETE | Housekeeping only (check expiry) |
| macOS (Apple Developer ID) | `Developer ID Application: Matthew Archbold` (**individual** account) | **Convert** account to Organization, issue org cert |

CI signing-identity string is hardcoded in `release.yml` at **3 sites** (~lines 493, 567, 661) as `Developer ID Application: Matthew Archbold` — these get updated once the org cert exists.

---

## Windows — done; one housekeeping item

`CN=Marston Enterprises` is definitive proof the Azure **organization identity validation** passed (Microsoft stamps the CN only after verifying the legal entity against public records; customers cannot set a custom CN). Verified against the Artifact Signing FAQ.

**Only risk:** the identity validation has an **expiry date**, and if it lapses, **all Windows signing stops**. Microsoft emails a reminder 60 days out; renewal is a click in the portal (if errors, a full re-validation is required).

**Action:** Azure portal → Artifact Signing account → **Identity Validations** → note the **expiry date** → calendar reminder 60 days before to click **Renew**. Confirm billing account name = exactly "Marston Enterprises".

---

## macOS — convert Individual → Organization

### The long pole: D-U-N-S number (start TODAY)
A **D-U-N-S** is a free 9-digit business ID from Dun & Bradstreet that Apple **requires** for org accounts. It's the only externally-gated wait (~5–7 business days via Apple's tool).

1. **Lookup first** (many small companies already have one): `https://developer.apple.com/enroll/duns-lookup/` — sign in with the Apple ID you'll use, enter the **exact** legal name + Colorado registered address. If found → copy the number. If not → submit the free request right there.
2. Info D&B needs: exact legal entity name, physical street address (no PO box), phone (a human verifies), entity type (LLC/Inc), employee count, year founded.
3. **Gotcha:** if D&B has the entity mis-listed as a *sole proprietorship*, Apple rejects it — fix via D&B support with the CO registration docs (adds ~1 week).

### Apple Organization enrollment / conversion
- It's a **conversion, not a fresh account.** Apple preserves: **Team ID, existing Developer ID certificate, existing apps, App Store Connect access.** Only the seller/display name changes.
- Start it: `https://developer.apple.com/contact/submit/` → sign in → Membership & Account → Program Enrollment → request Individual→Organization conversion. Provide: legal entity name (e.g. "Marston Enterprises LLC"), D-U-N-S, address, role (must have authority to bind the company), phone.
- Apple reviews + **calls the phone number** (~2 weeks), then emails to finalize, then $99/yr.
- **Total realistic time: ~3–4 weeks** from D-U-N-S submission (Apple review can overlap the D-U-N-S wait).

### Requirements / gotchas
- Must be a **formal legal entity** (LLC/Corp) — a DBA/sole-prop does **not** qualify for org.
- **Company website must be LIVE** at the company domain (parked/"coming soon" page fails verification).
- Enrollment **email domain must match the website domain** (no Gmail).
- The verification **phone call is real** — use a line that reaches a human, not a VOIP that won't ring.
- Cert portal goes dark ~24–48h during migration — don't schedule a release into that window.

### After conversion — the cert + CI change
- The **old** personal cert stays valid until expiry (Developer ID certs last 5 years); old signed builds keep passing Gatekeeper. No re-signing needed.
- For builds that should show the company name under Gatekeeper, **create a new Developer ID Application certificate** under the org (its CN becomes `Developer ID Application: Marston Enterprises LLC (<sameTeamID>)`).
- **Pipeline change (later, not now):** update the 3 `release.yml` codesign identity strings to the new cert name. `notarytool` uses the App Store Connect API key scoped to the same Team ID — unchanged; **rotate the API key post-conversion** as a 2-minute belt-and-suspenders check (Apple doesn't explicitly document key behavior across conversion).

---

## Prerequisites — RESOLVED (2026-06-24)
1. ✅ **Registered legal entity:** Marston Enterprises **LLC**, Colorado, Good Standing (formed Aug 2025).
2. ✅ **D-U-N-S number obtained** (Apple confirmed eligibility to enroll).
3. ✅ **Company website + matching email:** `marstonenterprises.com` + `matthew.archbold@marstonenterprises.com` (company domain, not Gmail — satisfies Apple).
4. ⏳ **Still to supply at enrollment:** a business phone Apple can call to verify.

> Actual identity numbers (EIN, CO SoS ID, D-U-N-S) are kept in the owner's **private** company folder
> (`Marston Enterprises/ME/company/COMPANY_IDENTITY.md`), **not** in this repo. Pull them from there at
> enrollment time. **Next step:** Apple → enroll, choosing **Convert to Organization** (do not start a
> fresh account), then issue a new org Developer ID cert and update the 3 `release.yml` signing strings.

## Timeline summary
| Item | Wait | Gating |
|------|------|--------|
| D-U-N-S (Apple tool) | ~5–7 business days | externally gated — **start first** |
| Apple org review + phone call | ~2 weeks | can overlap D-U-N-S |
| New Developer ID cert + CI string update | minutes | after conversion |
| Windows | done | just track Azure expiry |
