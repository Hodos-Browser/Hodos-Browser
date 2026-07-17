# Lessons Learned — The Edwin Windows Install (and what it means for bundling into Hodos)

> 📁 Part of the **Dolphin Milk + Edwin Integration** doc set (technical half) — see `README.md`.
> **Captured:** 2026-06-26, from a long hands-on session running EdwinPAI on Matt's Windows 11 + WSL2 box.
> **Why this doc exists:** the pain of getting Edwin working on Windows is the single most important input to the "make it easy for casual users" goal. These are field findings, grounded in the actual install (`~/edwinpai`, EdwinPAI `1.0.0-beta.8`, pi-agent-core `0.66.1`, model `openai/gpt-5.5`, OpenAI embeddings).

---

## 1. The headline lesson

**The enemy is WSL + the slow file bridge, NOT Node itself.** Edwin felt broken on Windows because it ran inside a Linux VM (WSL) while the files lived on the Windows drive — every multi-minute hang was that slow bridge (9P). Node runs fine natively on Windows and Mac. So the casual-user path is: **bundle Edwin and run it NATIVELY (no WSL), started and managed by Hodos on a localhost port** — the same way Hodos already runs the wallet and adblock engine — with cheap, safe defaults baked in. The user never sees Node, a daemon, or a port.

**Constraint (Matt, 2026-06-26): we do NOT rewrite Edwin** — it's Jake's project; we pull his updates and integrate. So the work is *packaging + native builds*, not a rewrite. (The earlier "extract a lean Rust vault" idea is set aside per that constraint.)

---

## 2. Why Edwin is so hard on Windows (the plain version)

Edwin is written in **Node.js** and depends on pieces that are **compiled for a specific operating system** (image processing, local AI/embeddings, terminal control, crypto). Those are built for **Linux**. Windows isn't Linux, so Edwin runs inside **WSL2** — a small Linux system running inside Windows.

The catch: your actual files live on the **Windows C: drive**, and the Linux side has to reach across a **bridge (called 9P)** to read them. **That bridge is slow** — reading lots of files across it takes *minutes* instead of seconds. That's the source of every multi-minute hang we saw (gateway boot from `/mnt/c` ≈ 5 min; re-indexing your repos stalls and gets killed).

So "Edwin on Windows" is really *"Edwin running in a little Linux VM, talking to Windows over a slow bridge."* Inherently fiddly. A Mac or native Linux machine doesn't have the bridge problem at all — which is why Edwin feels fine there and miserable here.

**Implication:** a browser shipped to normal Windows users cannot ask them to run WSL or know what `/mnt/c` is. Edwin has to run **natively** on Windows (and macOS) with **no WSL/Linux layer** — bundled and launched by Hodos, with its native modules pre-built per OS. This is a packaging/build problem, not a rewrite.

---

## 3. Cost & configuration gotchas (the $20 lesson)

We spent **$20 of OpenAI credit in ~2 weeks on roughly a dozen questions**, then hit `429 (out of quota)`. Root causes — all configuration, not Edwin malfunctioning:

- **Premium model everywhere** (`gpt-5.5`) for routine questions.
- **"Deep Workflow" was switched on** (`deepWorkflowEnabled: true`) — an *advanced* memory-plugin feature that turns some questions into a full multi-step research run ("full RLM pipeline"). It ran (there are `rlm/*-deep-result.md` files). Each run is expensive.
- **Think level** (off/low/med/high) is opaque — it controls how hard the model reasons (= token cost) and the UI explains none of it.
- **No cost guardrails** anywhere — no budget warning, no "this setting costs money" heads-up.

**Lesson for Hodos:** casual users need **cheap, safe defaults** (small model for routine, heavy stuff opt-in), **plain-language settings**, and **visible cost + a budget cap with alerts**. The default experience must be near-free per question.

---

## 4. Setup-flow gaps that make a *working* Edwin look broken

A recurring pattern: the desktop's management tabs read config/registry files that onboarding never creates, so a correctly-working Edwin looks empty or broken on day one.

- **Knowledge → Sources tab** reads `~/.shad/sources.yaml`, which onboarding never wrote → shows "0 sources / 0 collections / file not found," even though recall is actually configured and working via a *different* surface.
- **Skills panel** showed "none," but Edwin actually has **74 skills loaded (31 ready)** — the panel reads a management endpoint, not the loaded set.
- **Workflows tab** is empty + the workflows plugin is disabled + it expects you to hand-write YAML into a folder. No casual user can or will do that.
- **Recall** ("point recall at your files") is the *correct* thing to do and the user did it right — but the term is jargon and the behavior isn't explained.

**Lesson for Hodos:** every status surface must show **real info the moment the app opens**, onboarding must **populate what the UI reads**, and configuration must be **conversational or guided — never "edit this YAML file."**

---

## 5. What "easy for casual users" concretely requires (learned the hard way)

1. **No Linux, no WSL, no compilation** — native binary per OS, like the Hodos wallet.
2. **Cheap, safe defaults** — routine questions cost a fraction of a cent; expensive features off by default and clearly labeled.
3. **No jargon, no manual file/YAML editing** — guided or conversational setup ("tell me which folders to use," not "point recall at a collection path").
4. **Honest status surfaces** — tabs show real, accurate info on first launch; "empty" should mean empty, not "misconfigured."
5. **Cost transparency + a budget cap with alerts** — never let a user discover a $0 balance via a cryptic `429`.
6. **It just works on install** — bundled, signed, started by the Hodos shell; the user never sees a port, a gateway, or a daemon.

---

## 6. What's genuinely good in Edwin (keep these)

- The **BRC-103 signed-request / BSV-auth gateway** and the **signed-envelope vault** — the real differentiator and the security backbone of the whole integration (`EDWIN_VS_DOLPHIN_MILK_SECURITY.md`).
- The **idea** of qmd/shad recall (index files once, retrieve only the relevant snippets) is sound and is exactly right for a browser-bundled assistant — it just needs cheap defaults and a non-jargon setup.

---

## 7. Pointers

- Full research bundle (loop/harness study, know-thyself draft, Jake feedback, OpenClaw comparison): `C:\Users\archb\Marston Enterprises\Edwin\research-2026-06-17\`
- Existing Jake feedback in this folder: `EDWIN_SETUP_FEEDBACK_FOR_JAKE.md` (reconcile/merge with the research-bundle version before sending).
- Architecture direction this validates: `ARCHITECTURE_TECHNICAL.md` §1-2, §8.
