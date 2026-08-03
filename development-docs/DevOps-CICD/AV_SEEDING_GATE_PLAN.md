# AV Seeding Gate — design plan

**Status:** **Phase 1 BUILT** (2026-08-03) — merged into `.github/workflows/promote.yml`,
not yet exercised on a real promote. Phase 2 not started. **Created:** 2026-08-03.

> **No secrets or accounts required.** An earlier revision of this plan called
> VirusTotal's API to verify submission automatically. That was **withdrawn** —
> VirusTotal's free tier states plainly that it "**must not be used in business
> workflows, commercial products or services**," which covers our release
> pipeline, and VT Enterprise cannot be justified for one lookup per release.
> Both halves of the gate are now attestation; see §3.

> **Where this lives when it ships.** This is a design doc for unbuilt work. Once
> Phase 1 lands, the living behavior belongs in `BUILD_AND_RELEASE.md` §2.5 (which
> already owns AV seeding) and this file gets archived. Do not let the two describe
> the same behavior at once — one home per fact.

---

## 1. The problem, plainly

Windows shows a scary "Windows protected your PC" screen on downloads it hasn't
seen enough of. Getting rid of it means **seeding reputation**: after each public
release we upload the installer to VirusTotal and submit it to Microsoft Defender
as a software developer. Microsoft then starts trusting our publisher name.

`BUILD_AND_RELEASE.md` documents this thoroughly as **Step 8** of the release
checklist. The process is good. **Execution is not** — it depends on someone
remembering, after the release is already out the door and the pressure is off:

| Release | Seeded? | What happened |
|---|---|---|
| beta.7–10, 15 | ✅ | Done and recorded |
| beta.11–14 | ❌ | Skipped during the macOS auto-update scramble |
| **beta.16–27** | ❌ | **No record at all** — twelve releases, not even a cert check |
| beta.28 | ✅ | Done well: VirusTotal 0/72 clean, Defender ID recorded |
| **beta.29** | ❌ | **This is the build the public is downloading right now** |

The beta.28/29 pair is the clearest illustration of the failure. beta.28 was
seeded carefully — and then never promoted. beta.29 was promoted and never
seeded. **The work went into the build nobody installs, and the build everyone
installs got nothing.**

Nothing in the pipeline connects "this build is going public" to "this build
needs seeding." That's the gap this closes.

### Why it matters more for 0.4.0

Reputation is keyed on **file hash + signing certificate**. The Chromium/CEF
136→150 rebuild changes every binary in the package, so **every hash resets**.
0.4.0 starts from zero on file reputation and leans entirely on whatever
publisher reputation the certificate has accumulated — which is exactly the
thing these twelve skipped releases failed to build.

---

## 2. What the gate is

`promote.yml` is the workflow that makes a tested draft release public. It is
already the deliberate ship gate — a build cannot reach a customer without it.
That makes it the one place a reminder cannot be skipped.

The gate adds a check partway through: **before the release goes public, prove
the seeding was done.** If it wasn't, promotion stops with an error telling you
exactly what to do.

### What you'll actually experience

Today, promoting looks like: open the workflow, paste the tag, paste the
checksums, run it.

After this change it looks like:

1. Download the draft installer (same file you tested).
2. Upload it to VirusTotal, submit it to Microsoft Defender — the existing
   Step 8 process, unchanged.
3. Copy the submission ID Microsoft gives you.
4. Run the promote workflow, pasting that ID into a new box.

If you skip step 2, the workflow stops before anything goes public and tells you
so. Nothing is half-published — the check runs **before** the irreversible flip.

If you genuinely need to ship without seeding (an emergency security fix), there
is an escape hatch: type a reason into a waiver box. It promotes, but the waiver
and your reason are recorded loudly in the run log. It is deliberately not
silent.

---

## 3. Why neither vendor is queried automatically

This is the constraint that shapes the whole design.

| Vendor | Auto-verifiable? | Why not |
|---|---|---|
| **VirusTotal** | **No — licensing** | The public API would answer "have you seen this hash?" perfectly. But the free tier states it "**must not be used in business workflows, commercial products or services**." Hodos is a commercial product and a CI pipeline is a business workflow, so we are not entitled to it. VT Enterprise runs into thousands per year — indefensible for one lookup per release. |
| **Microsoft Defender** | **No — no API exists** | Submission is a web form. The status API is part of Microsoft 365 Defender and needs a paid corporate tenant we do not have (D2). |
| Norton | n/a | Only submitted when Norton actually flags us. Out of scope. |

So **both halves are attestation.** The gate cannot prove the submissions
happened — it can only prove the operator has evidence in hand.

### What we check instead: hash-linked evidence

Attestation does not have to mean a checkbox. Every VirusTotal report URL
embeds the file's hash:

```
https://www.virustotal.com/gui/file/6408a0f8…f74d32/detection
                                    └── sha256 of the scanned file
```

The workflow already knows the SHA-256 of the installer it is about to publish,
so it **compares the two**. A mismatch blocks. That catches the realistic
mistake — pasting last release's URL — which a checkbox never would.

The Defender half has no equivalent handle, so it is a format-checked UUID and
nothing more. A fabricated one passes. That is fine and intentional: the problem
being solved is *forgetting*, not *lying*, and making someone visit the portal
to obtain a real-looking ID captures most of the value.

### What was lost with the API

The earlier API design also returned the detection count, letting the gate block
a build that antivirus would flag. **That automated check is gone.** It becomes
a human judgment — the operator is already on the report page to copy the URL,
and the detection count is the largest thing on it. An optional
`virustotal_detections` input records what they saw (`0/72`) into the ledger, so
the noise-floor trend D1 wanted is still captured across releases, just not
enforced.

**The manual upload itself is unaffected.** Submitting our own binary through
the VirusTotal website is what the public site is for, and it is what Step 8 has
always done. Only the *automated API* was the problem. (Strictly, the
"business workflows" language sits on the API-key page; whether it reaches the
manual web path is a separate question in VirusTotal's general ToS that we have
not read. Worth a look sometime — but note that the part which actually seeds
SmartScreen is the **Microsoft Defender** submission, through Microsoft's own
developer channel, which carries no such restriction. If VirusTotal ever became
unusable entirely, the load-bearing half of the gate survives.)

---

## 4. Phase 1 — the gate (build this first)

### Where it goes

Inside `promote.yml`, in the existing `promote` job, as steps inserted between
**"Cryptographically re-verify the published bytes"** and **"Promote release to
live (latest)"**.

That position matters. By then the workflow has already downloaded the exact
bytes that will go public, so we can hash them. And it is still before the flip,
so a failure blocks publication rather than being discovered afterwards.

It is deliberately **not** a separate workflow. A separate workflow is exactly as
forgettable as the manual step it replaces.

### Four new inputs on the workflow

| Input | Required | What it's for |
|---|---|---|
| `virustotal_report_url` | Yes, unless waived | The report URL from your manual upload. Its embedded hash is checked against the installer |
| `virustotal_detections` | No | The count off that report, e.g. `0/72`. Recorded in the ledger to track our noise floor |
| `defender_submission_id` | Yes, unless waived | The UUID Microsoft gives you after submitting |
| `av_seeding_waiver` | No | A reason to promote without seeding. Emergency use only |

### Step A — VirusTotal report, hash-checked

1. Hash the installer the workflow already downloaded.
2. Take the pasted report URL and reject it unless it is a
   `virustotal.com/gui/file/…` URL.
3. Extract the 64-hex SHA-256 embedded in it.
4. React:

| Case | Action |
|---|---|
| URL missing | **Stop.** Names the file and the upload URL. |
| Not a VirusTotal URL | **Stop.** |
| No SHA-256 in the URL (md5/sha1-keyed report) | **Stop**, with instructions to grab the sha256 form from the report's Details tab. |
| Hash ≠ installer hash | **Stop**, printing both hashes. Almost always a stale URL from the previous release. |
| Hash matches | Pass. Record the URL and the operator's detection count. |

No network call, no key, no outage mode — the check is entirely local, so unlike
the API design there is nothing here that can fail for reasons outside the
operator's control.

### Step B — Defender attestation

Check the submission ID looks like a UUID. If it's empty and no waiver was
given, stop with a message pointing at the WDSI portal and the reminder to
choose "Software developer" as the submission type.

### Step C — waiver path

If a waiver reason was given, skip A and B, and write a loud warning plus the
reason into the run log and the run summary. Never silent.

### Step D — recovery mode

`promote.yml` already detects when a release is *already public* and runs in
recovery mode to re-sync the website. **Skip the whole gate in that mode** — the
flip already happened, so blocking achieves nothing but a failed re-run.

### Step E — the summary block

Write a formatted block into the workflow's run summary page containing:
certificate thumbprint, installer SHA-256, VirusTotal report URL, detection
count, Defender submission ID.

This is the `§2.5.2` ledger row, pre-formatted for copy-paste — which sets up
Phase 2.

### Size

~100 lines of workflow YAML across four steps. **No secrets, no accounts, no
network calls** — dropping the API made the gate both legal and simpler.

### Verification done at build time (2026-08-03)

The four steps were extracted from the YAML and exercised directly. Every path
is pure shell, so unlike the API design **all of it was testable locally** and
all of it passed:

| Case | Expected | Result |
|---|---|---|
| Waiver empty | continue to checks | ✅ |
| Waiver set | skip checks, loud warning + summary block | ✅ |
| VirusTotal URL missing | **block** | ✅ |
| Non-VirusTotal URL | **block** | ✅ |
| Stale URL (hash of another release) | **block**, both hashes printed | ✅ |
| md5-keyed report URL | **block** with guidance | ✅ |
| Correct hash + detections | pass, both recorded | ✅ |
| Correct hash, uppercase, no `/detection` suffix, stray whitespace | pass | ✅ |
| Defender ID missing | **block** | ✅ |
| Defender ID malformed | **block** | ✅ |
| Defender ID valid UUID | pass | ✅ |
| Ledger row, with and without detection count | correct §2.5.2 shape | ✅ |

### How to test it safely

Run the workflow against a tag that is **already public**. Recovery mode skips
the flip, so the new steps can be shaken out with no risk to a live release.

---

## 5. Phase 2 — auto-write the ledger (later, separately)

Phase 1 makes you *do* the seeding. It does not make you *record* it — and the
twelve-release gap at beta.16–27 is a recording failure as much as a doing one.

Phase 2 has the workflow append the `§2.5.2` submission-record row to
`BUILD_AND_RELEASE.md` and commit it automatically, turning the ledger into a
byproduct of promoting rather than a separate act of discipline.

**Deliberately deferred, for two reasons.** It adds a new write-side-effect to
the most safety-critical workflow in the repo. And there's a wrinkle: the
workflow checks out the *tag's* source, but the ledger row is generated after
the tag is cut — so it would need to commit to the default branch instead, which
is a different and more invasive operation. Worth doing. Worth doing on its own.

---

## 6. Decisions for the owner

| # | Decision | Outcome |
|---|---|---|
| **D1** | **How many antivirus detections should block a release?** | **MOOTED 2026-08-03.** Originally decided as "any detection blocks, waiver absorbs false alarms" — then the VirusTotal API was withdrawn on licensing grounds, and with it the automated count. The judgment is now the operator's, made on the report page they are already looking at. The optional `virustotal_detections` input preserves the *data* (recorded per release in §2.5.2) without the *enforcement*. **Revisit only if a flagged build ever slips out** — that would be the evidence that a human check isn't enough. |
| **D2** | **Do we have a Microsoft 365 Defender tenant?** If so, the Defender half could be verified for real rather than attested. | **DECIDED 2026-08-03 — no.** (It is a paid corporate security subscription; we'd know.) Defender stays operator attestation. |
| **D4** | **May we call VirusTotal's free API from CI?** | **DECIDED 2026-08-03 — no.** The API-key page states the free tier "must not be used in business workflows, commercial products or services." Caught by the owner before the key was ever added. The API call was removed and replaced with hash-checked URL attestation. Revisit only if VT Enterprise is ever bought for unrelated reasons. |
| **D3** | **Should the gate also cover macOS?** | **No.** Gatekeeper trust is binary — signed and notarized is trusted, with no per-developer reputation score (see `ORG_IDENTITY_SIGNING_MIGRATION.md`). There is nothing to seed. This is a Windows-only problem. |

---

## 7. What this does not fix

- **beta.29.** Already public. The gate only runs before the flip. Seeding
  beta.29 remains a manual task, tracked on the 0.4.0 list.
- **beta.28's Defender submission**, still logged as *Pending*. No API to poll;
  someone has to check the portal and update the record.
- **Reputation itself.** §2.5.3 recommends getting 5–10 testers to install
  within 24 hours of publishing. That's the part that actually moves the needle,
  and it cannot be automated.
- **The 0.4.0 hash reset.** The CEF rebuild invalidates all accumulated file
  reputation regardless. The gate ensures we seed the new build promptly; it
  cannot preserve what the new hashes lose.

---

## Related

- `BUILD_AND_RELEASE.md` §2.5 (signing + AV reputation), §2.5.1 (cert chain
  verification), §2.5.2 (per-release submission ledger), §2.5.3
  (reputation-building strategy) — the process this gate enforces
- `.github/workflows/promote.yml` — the workflow being modified
- `ORG_IDENTITY_SIGNING_MIGRATION.md` — why macOS needs no equivalent
