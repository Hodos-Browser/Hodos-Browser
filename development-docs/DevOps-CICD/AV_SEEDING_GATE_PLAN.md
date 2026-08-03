# AV Seeding Gate — design plan

**Status:** PLAN — not built. **Created:** 2026-08-03.

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

## 3. What can be checked automatically, and what can't

This is the constraint that shapes the whole design.

| Vendor | Auto-verifiable? | Why |
|---|---|---|
| **VirusTotal** | **Yes** | Public API. Ask "have you seen this file?" by its hash. If VirusTotal has never seen it, it answers 404 — a definitive "not submitted." |
| **Microsoft Defender** | **No** | Submission is a web form with no public status API. Microsoft's status API is part of Microsoft 365 Defender and needs a corporate tenant we do not appear to have (confirm before building). |
| Norton | n/a | Only submitted when Norton actually flags us. Out of scope. |

So the gate **verifies VirusTotal and trusts you on Defender.**

The Defender half is an honesty check, not a security control — a made-up ID
would pass. That is fine and intentional. The problem being solved is
*forgetting*, not *lying*. Making someone visit the portal to get a real-looking
ID captures almost all the value.

### The free bonus

Because we're already asking VirusTotal about the file, we get its detection
count back. That means the gate can also **stop a release that antivirus
software would flag** — before customers hit it, instead of after they email us.
That is arguably worth more than the reminder.

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

### Two new inputs on the workflow

| Input | Required | What it's for |
|---|---|---|
| `defender_submission_id` | Yes, unless waived | The UUID Microsoft gives you after submitting |
| `av_seeding_waiver` | No | A reason to promote without seeding. Emergency use only |

There is deliberately **no VirusTotal input** — the workflow derives the file
hash itself and looks it up. Less for you to paste, less to get wrong.

### Step A — VirusTotal check

1. Hash the installer the workflow already downloaded.
2. Ask VirusTotal about that hash, using a new repository secret
   `VIRUSTOTAL_API_KEY`.
3. React to the answer:

| Response | Action |
|---|---|
| **404 — file unknown** | **Stop.** Not submitted. Error names the file and the upload URL. |
| **200, detections = 0** | Pass. Record the report URL. |
| **200, detections > 0** | **Stop.** An antivirus engine flags this build. See §6 D1. |
| API error or rate-limited | Retry 3×, then warn and continue. A VirusTotal outage must never block a release. |
| Secret not configured | Warn and skip. The gate degrades gracefully rather than bricking promotion the day it merges. |

The last two rows are the important safety valves. This gate is a reminder, and
a reminder that can take down the release pipeline is worse than no reminder.

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

Roughly 70 lines of workflow YAML, one new repository secret. VirusTotal's free
tier is ample (4 lookups/minute, 500/day; we use one per release).

### How to test it safely

Run it against a tag that is **already public**. Recovery mode skips the flip, so
there is no risk to a live release while shaking out the new steps.

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

| # | Decision | Recommendation |
|---|---|---|
| **D1** | **How many antivirus detections should block a release?** A single obscure engine flagging a Chromium installer is common and usually meaningless. Blocking on any detection risks false alarms; blocking on none loses the benefit. Options: any detection, a threshold like 3+, an allowlist of known-noisy engines, or warn-only. | **Start at "any detection blocks," and let the waiver absorb false alarms.** We don't yet know our real noise floor — beta.28 came back 0/72. A few releases of data will tell us whether a threshold is needed. Starting strict and loosening is safer than the reverse. |
| **D2** | **Confirm we have no Microsoft 365 Defender tenant.** If we do, the Defender half could be verified for real instead of attested. | Check before building. Low likelihood, high payoff if true. |
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
