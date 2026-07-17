# Casual-User Onboarding & Cost-Control UX — Making a Native-AI Browser "Just Work"

> 📁 Part of the **Dolphin Milk + Edwin Integration** doc set — see `README.md`. Forensic deep-dive companion to the main study docs.
> **Created:** 2026-06-28 by a research workflow (web-cited). **STUDY, not a decision** — options & trade-offs, no winner picked. Claim tags **[FACT]/[VISION]/[INFERRED]/[SPECULATION]/[UNVERIFIED]** preserved.

## Casual-User Onboarding & Cost-Control UX — Making a Native-AI Browser "Just Work"

**Purpose & bottom line.** This study examines how to make Edwin-in-Hodos genuinely easy for a non-technical user. The short answer is that none of the leading AI browsers have solved cost-control UX for casual users, because they all use flat subscriptions and never need to. Hodos, with BSV micropayments baked in, faces a problem the market has not yet solved: surfacing real per-query cost without triggering the "mental transaction cost" panic that killed every micropayment scheme of the 1990s-2010s. The path forward is a combination of (a) cheap safe defaults so the default behavior is nearly free, (b) a visible budget cap with a plain-language spend gauge that removes anxiety without demanding attention, and (c) zero-jargon, guided onboarding that populates every status surface before the user ever sees a tab. The Edwin-specific failures documented in the lessons-learned file are among the clearest concrete specifications for a first-run flow that exist anywhere.

---

### 1. How Leading AI Browsers Onboard — A Comparative Survey

#### 1.1 Brave Leo

[FACT] Brave Leo is the closest to Hodos's positioning: privacy-native, built into the browser, no fork of the rendering engine. Its onboarding pattern is the most instructive template available.

**First-run flow:** [FACT] Leo requires no account creation and no login. It is accessible the moment Brave is installed via a sidebar icon. The free tier works immediately with no payment information. [FACT] The default model is Llama 3.1 8B — a small, open-weight model that is cheap to run on Brave's servers and therefore rate-limited but free. Premium users ($15/month) unlock larger models (Claude Sonnet, DeepSeek R1). [FACT] "Automatic Mode," introduced in 2025, lets Leo select the right model for the task without user intervention, directly reducing the cognitive burden of model selection.

**How AI is introduced without jargon:** Leo opens as a sidebar chat. The first interaction is a text prompt field with suggested questions ("Summarize this page," "Help me write an email"). No model names appear until the user explicitly visits settings. The implicit message is: *ask a question, get an answer.*

**Cost introduction:** [FACT] There is no per-query cost. The subscription model (or free rate limit) abstracts spending entirely. [FACT] The privacy pitch is foregrounded instead: "Conversations are never stored on Brave's servers." Cost is a non-issue for users until the rate limit is hit, at which point they are prompted to upgrade.

**Cost-control UX:** None needed — flat-fee subscription means spend is known in advance. This is the core structural difference from BSV micropayments.

*Sources: [Brave Leo Roadmap 2025](https://brave.com/blog/leo-roadmap-2025-update/), [How do I use Brave Leo](https://support.brave.app/hc/en-us/articles/20958609786637-How-do-I-use-Brave-Leo), [Automatic Mode](https://brave.com/blog/automatic-mode-leo/), [Leo Pricing — eesel AI](https://www.eesel.ai/blog/brave-leo-pricing)*

---

#### 1.2 Perplexity Comet

[FACT] Comet launched July 9, 2025, initially at $200/month for Perplexity Max subscribers, then opened free to all in October 2025.

**First-run flow:** [FACT] Download is standard (DMG/installer), and the onboarding begins with an audio-guided introduction (the guide explicitly says "turn up your volume"). [FACT] Users link a Google account immediately during setup, which gives Comet access to email and calendar context. [FACT] Import of bookmarks and passwords from Chrome is offered. The system then demonstrates the "Spaces" feature for organizing browsing by topic, and the voice interface for natural language commands. The emphasis is on showing capability through concrete task completion — "Trip Planner Pro," "Find a computer mouse under $50" — rather than explaining what AI is.

**How AI is introduced without jargon:** Task-first, not feature-first. The onboarding shows you completing a real thing rather than listing features. The smart address bar accepts URLs and natural language interchangeably, so users discover AI capability organically by typing a question where they would type a URL.

**Model selection:** [FACT] Free users see no model selector. Max subscribers can choose Claude Opus 4.6, Claude Sonnet 4.6, GPT-5.4, Gemini 3.1 Pro, Kimi K2.5, or use "Model Council" (runs all three top models in parallel and synthesizes the result). [INFERRED] Showing model choice to free users before they are invested is deliberately avoided.

**Cost-control UX:** None visible. The browser includes a "Purchases" section for payment method integration but no expenditure monitoring or spending alerts. [FACT] Cost is fully abstracted by the subscription.

*Sources: [Comet Guide — AI Chief](https://aichief.com/ai-tutorials/a-look-inside-comet-perplexitys-ai-powered-browser/), [Introducing Comet — Perplexity](https://www.perplexity.ai/hub/blog/introducing-comet), [Comet 101 — Substack](https://sidsaladi.substack.com/p/perplexity-comet-ai-browser-101-complete), [SwitchTools Review](https://www.switchtools.io/blog/perplexity-comet-ai-agent-browser)*

---

#### 1.3 Dia (The Browser Company)

[FACT] Dia launched in public beta June 2025, built by The Browser Company (the Arc team), Chromium-based, macOS.

**First-run flow:** [FACT] Invite-gated initially (Arc members got immediate access). The onboarding is consistently praised as "beautiful" — visually polished, walkthrough-style, aesthetically consistent. [FACT] The AI is embedded in the URL bar as the primary interaction surface; there is no separate AI panel to discover. [FACT] Preferences are set conversationally: "To customize tone of voice, style of writing, and coding settings — all you have to do is talk to the chatbot." This is the most radical anti-jargon move observed: configuration via natural language, not settings screens.

**How AI is introduced:** The URL bar is the entry point for both navigation and AI. No separate mode or toggle to enable. The user's first interaction with AI is indistinguishable from their first web search.

**Notable onboarding lesson:** [FACT] The History feature (which uses browsing history as context) is opt-in, not default. Privacy controls are surfaced at relevant moments, not buried in settings.

**Criticism — the real problem:** [INFERRED based on published critique] The polished onboarding masks a differentiation gap. Critics note that Dia is "a repackaged Arc with ChatGPT-style extras" — the onboarding is excellent but the underlying value proposition is weak. The lesson for Hodos is that beautiful onboarding is necessary but not sufficient; the native BSV wallet integration must deliver something the AI browsers above cannot.

**Cost-control UX:** None. No per-query cost model exists.

*Sources: [TechCrunch — Dia Beta Launch](https://techcrunch.com/2025/06/11/the-browser-company-launches-its-ai-first-browser-dia-in-beta/), [Dia Review — Caneraras](https://www.caneraras.com/learn/dia-browser-review-ai-first-browser), [The Huge Problem — Medium](https://medium.com/@theo-james/the-hyped-dia-browser-is-out-but-with-a-huge-problem-42aabcc2e723)*

---

#### 1.4 Microsoft Edge Copilot

[FACT] Microsoft launched "Copilot Mode" in Edge on July 28, 2025, integrating it as a first-class browsing mode rather than a sidebar toggle. The feature has since been re-integrated directly into Edge rather than as a named "mode." [FACT] As of June 2026, Edge Copilot offers model choice including Claude and GPT-5.2 in Copilot Chat.

**First-run flow:** [FACT] Copilot Mode was opt-in during the experimental phase — users had to deliberately enable it. Once enabled, users see a new tab page combining search, chat, and navigation. The AI contextually understands what the user is researching without copy-paste. [FACT] Voice input is explicitly called out as a non-technical-user accessibility feature: "could be handy for people who aren't as tech-savvy when it comes to booking things online, or for those with limited mobility."

**How AI is introduced:** The onboarding leans on familiar Microsoft patterns. Copilot is positioned as a "helper" for practical tasks — booking appointments, shopping lists, drafting content. The implicit framing is "your assistant," not "an AI."

**Cost-control UX:** [FACT] Free during experimental phase. Microsoft 365 subscribers get higher usage; no per-query cost is surfaced. The enterprise cost-management tools (Azure spend caps) are entirely separate from consumer Edge UI.

*Sources: [TechCrunch — Copilot Mode Launch](https://techcrunch.com/2025/07/28/microsoft-edge-is-now-an-ai-browser-with-launch-of-copilot-mode/), [Microsoft Copilot in Edge](https://www.microsoft.com/en-us/microsoft-copilot/for-individuals/do-more-with-ai/ai-for-daily-life/ai-browser-innovation-with-copilot-in-edge), [Neowin — Killing Copilot Mode](https://www.neowin.net/news/microsoft-is-killing-copilot-mode-in-edge-but-ai-features-arent-going-away/)*

---

#### 1.5 OpenAI Atlas (ChatGPT Browser)

[FACT] Atlas launched October 21, 2025, macOS only initially, available to Free, Plus, Pro, and Go users globally.

**First-run flow:** [FACT] Standard DMG install → drag to Applications → auto-adds to Dock. [FACT] Sign in uses existing ChatGPT credentials, removing the need to create a new account. [FACT] Import bookmarks, passwords, and browsing history from Chrome during setup. [FACT] Incentive to set as default browser: "users gain 7 days of increased rate limits." [FACT] Atlas notifies users when onboarding steps remain incomplete — a progressive completion model rather than forcing all steps upfront.

**Privacy-by-default during onboarding:** [FACT] "Include Web Browsing and Shared Links" for AI training is off by default. [FACT] Page visibility controls let users restrict which pages ChatGPT can see, surfaced during setup. Incognito mode is offered early.

**Model selection:** [FACT] Users can switch between models and pin favorites. Model choice is accessible but not foregrounded in basic use.

**Cost-control UX:** [FACT] No spending controls are documented. The free/Plus/Pro/Go tier hierarchy is the only cost management mechanism. No per-query cost display. API cost controls (spending caps, alerts) exist on the developer portal but are invisible in the consumer browser.

*Sources: [Getting Started with Atlas — OpenAI Help](https://help.openai.com/en/articles/12628555-getting-started-with-atlas), [Atlas Features — Seraphic Security](https://seraphicsecurity.com/learn/ai-browser/openai-atlas-browser-features-pros-cons-security-and-privacy/), [Atlas Release Notes](https://help.openai.com/en/articles/12591856-chatgpt-atlas-release-notes), [Atlas Review — Efficient App](https://efficient.app/apps/atlas)*

---

#### 1.6 ChatGPT (Web/App — for reference)

[FACT] Consumer ChatGPT has never exposed per-query cost to end users. Tiers are: Free ($0, now with ads in US via "Go" rebranding), Plus ($20/mo), Pro ($100/mo, formerly $200/mo). [FACT] API users have dashboard-level monthly spending caps and threshold alerts — but these are developer tools, not consumer UX. [FACT] Conversational checkout launched September 2025, allowing in-chat purchases — the first mainstream example of money flow visible inside an AI chat interface, though not cost display.

---

#### Synthesis: What the Leaders Do and Do Not Do

| Dimension | Brave Leo | Comet | Dia | Edge Copilot | Atlas |
|---|---|---|---|---|---|
| Login required to start | No | Yes (Perplexity acct) | Yes (invite) | Soft (Microsoft acct) | Yes (OpenAI acct) |
| Default model visible to user | No (automatic) | No (abstracted) | No | No | Pin favorites |
| Config requires YAML/files | No | No | No | No | No |
| Conversational config | Partial | No | Yes (core design) | No | No |
| Cost per query shown | Never | Never | Never | Never | Never |
| Spending cap for user | Never | Never | Never | Never | Never |
| Rate limit warning shown | Yes (when hit) | No | No | No | No |
| Privacy opt-in surfaced early | Yes | No | Yes (History) | Yes (opt-in) | Yes (training off) |
| Data import from Chrome | No | Yes | No | Yes | Yes |
| Audio/video intro | No | Yes | No | No | No |

**The universal pattern:** Every market leader abstracts cost entirely via subscription. Cost-control UX does not exist in any consumer AI browser as of June 2026. [FACT] Enterprise-level AI cost management tools (budget caps, anomaly alerts, spend dashboards) exist from vendors like MLflow, Google Cloud, Requesty, and Portkey, but they are API gateway/developer tools not consumer interfaces. [INFERRED] This is not an oversight — it is a deliberate product choice, because per-query pricing requires mental transaction cost design that nobody has yet cracked for consumers.

---

### 2. Cost-Control UX — A Market Failure Nobody Has Fixed

#### 2.1 The Enterprise Tooling That Exists (and Why It Doesn't Apply)

[FACT] The 2025-2026 enterprise AI cost management space is mature: MLflow AI Gateway supports budget policies, webhook alerts, and automatic request blocking at preset limits. [FACT] Google Cloud introduced project-level Spend Caps at Next '26, providing "absolute control over AI spending" with graduated alerts (green/yellow/red). [FACT] Requesty, Portkey, and similar API gateway tools support real-time token tracking, per-workspace caps, and email/Slack alerts.

These tools target CTOs and engineering teams. They operate at the infrastructure layer, not the user-facing UX layer. They assume users understand what a token is and can interpret a spend graph with multiple model cost lines. None of this translates to a casual browser user.

#### 2.2 The Gap in Consumer Products

[FACT] Research in 2025-2026 finds that 21% of larger organizations have no formal cost-tracking systems, and 85% of companies miss AI cost forecasts by more than 10% — and these are professionals with finance teams. For a casual user, the problem is worse.

The consumer-facing AI spend UX gap manifests as: (a) no warning before an expensive operation, (b) no running total of spend, (c) no budget cap interface, (d) discovery of overspend via a confusing error (ChatGPT rate limit, OpenAI 429 error). [INFERRED] The reason all AI products choose subscriptions over per-query pricing is precisely to avoid having to solve this UX problem. Subscriptions transform unpredictable per-query cognitive burden into a predictable monthly expense — the same reason most consumers prefer Netflix to paying per movie.

#### 2.3 Why This Matters for Hodos

Hodos cannot use a subscription to abstract BSV micropayment costs. The micropayment-per-query model is the architecture AND the differentiator. So Hodos must solve what the entire industry has deliberately avoided: a cost-control UX that (a) keeps the user informed and in control, (b) does not trigger decision fatigue on every query, and (c) remains comprehensible to a non-technical user.

[INFERRED] The closest consumer analogs are mobile data dashboards (your phone shows "you've used 8.4 GB of your 15 GB plan") and prepaid card balance notifications ("your balance is $4.20, low balance alert"). These are not AI-specific but demonstrate the pattern: a persistent low-salience indicator plus threshold alerts, not per-transaction prompts.

---

### 3. Edwin-Specific Failures — The Design Brief Written in $20 Bills

The following are [FACT] findings from hands-on field testing of EdwinPAI 1.0.0-beta.8 on Windows 11 + WSL2, documented June 26, 2026. Each is a concrete onboarding failure with a direct implication for Hodos's first-run design.

#### 3.1 Premium Model by Default — The $20 Burn

**What happened:** [FACT] The default model was `openai/gpt-5.5` — a premium frontier model. Approximately twelve questions burned $20 of OpenAI credit in roughly two weeks. The user discovered overspend via a cryptic `429 (out of quota)` error, not an in-app warning.

**Design implication:** The default model must be cheap. Hodos should default Edwin to the cheapest capable model (a small open-source or tier-1 model at fractions of a cent per query), with expensive models clearly labeled and requiring active opt-in. The user must never discover overspend via an error message.

#### 3.2 "Deep Workflow" On by Default

**What happened:** [FACT] `deepWorkflowEnabled: true` was set in the default config. This silently turns routine questions into full multi-step research runs ("full RLM pipeline"), generating `rlm/*-deep-result.md` artifacts. Each deep-workflow run costs significantly more than a simple query. The user had no idea this was happening.

**Design implication:** Any multi-step, agentic, or expensive workflow feature must default to OFF. Its cost implication must be stated in plain language before first use: "Deep research mode runs multiple searches and uses more AI credit. Use this for complex questions, not quick lookups." An icon or label on the query that will trigger it should appear before the user sends.

#### 3.3 "Think Level" — Opaque Cost Multiplier

**What happened:** [FACT] Edwin has a "think level" (off / low / medium / high) that controls reasoning depth and therefore token spend. No in-UI explanation of what each level means in cost or latency terms is provided.

**Design implication:** Any reasoning-depth selector must show the user what it means in plain language: "Quick (fast, near-free)," "Thorough (slower, uses more AI credit)," "Deep (multi-step research, uses the most credit)." If Hodos exposes this control at all, cost and time implications appear inline, not in a help document.

#### 3.4 Sources Tab — Empty State That Means "Misconfigured," Not "Empty"

**What happened:** [FACT] The Knowledge Sources tab reads `~/.shad/sources.yaml`, which the onboarding process never writes. The tab showed "0 sources / 0 collections / file not found" despite recall being correctly configured and working via a different surface. The user sees what appears to be a broken feature.

**Design implication:** Every status surface must show real, accurate state on first launch. Onboarding must populate the files and registry entries that the UI reads. An "empty" state must mean "you haven't added anything yet — here's how to add your first folder," not "the config file doesn't exist."

#### 3.5 Skills Panel — Shows "None" Despite 74 Loaded Skills

**What happened:** [FACT] The skills panel showed "none," but Edwin had 74 skills loaded with 31 ready. The panel reads a management endpoint that returns a different dataset than the runtime skill registry.

**Design implication:** Skills/capabilities panels must reflect actual loaded state. Showing "none" when 74 skills exist is worse than no panel at all — it creates the impression of a broken or incomplete product. Hodos's integration should either fix this by proxying the correct data, or suppress the panel until accurate data is available.

#### 3.6 Workflows Tab — Empty + Requires Hand-Written YAML

**What happened:** [FACT] The Workflows tab was empty because the workflows plugin was disabled AND because it expects the user to hand-write YAML into a specific folder. No casual user can or will do this.

**Design implication:** Workflows (and any feature requiring config-file editing) must be either (a) removed from the default UI until the user has demonstrated readiness, or (b) replaced with a guided, form-based, or conversational alternative. YAML should never appear in a first-run or beginner path.

#### 3.7 "Recall" Jargon

**What happened:** [FACT] The "recall" feature (index local files/folders so Edwin can search them) uses the term "recall" without explanation. The user correctly used the feature but found the word opaque.

**Design implication:** User-facing AI assistant terminology should use plain language: "Your files" not "recall," "AI memory" not "RAG," "research mode" not "deep workflow." Technical terms are acceptable in advanced settings screens, not in the default onboarding path.

#### 3.8 Windows Install — WSL + 9P Bridge

**What happened:** [FACT] Edwin on Windows runs inside WSL2, which communicates with Windows drive files over the 9P filesystem bridge. This bridge is slow — booting the gateway from `/mnt/c` took approximately 5 minutes; re-indexing repositories stalled and was killed by the OS. The experience felt broken even though Edwin was technically running.

**Design implication:** The Hodos bundle must run Edwin as a native Windows (and macOS) process, started and managed by Hodos on a localhost port — the same pattern used for the BSV wallet subprocess. No WSL, no Linux VM, no 9P bridge. This is a packaging/build problem, not an Edwin rewrite.

---

### 4. The Micropayment-Specific Onboarding Challenge

#### 4.1 Szabo's Warning — Mental Transaction Costs Dwarf Technical Costs

[FACT] Nick Szabo's 1999 paper "Micropayments and Mental Transaction Costs" identified the core barrier: the cognitive overhead of deciding "is this worth X?" on every micro-interaction often costs more cognitive effort than the payment itself. [FACT] This warning remains accurate 25+ years later: consumers strongly prefer bundled subscriptions at potentially higher total cost because they eliminate perpetual valuation decisions. The mental cost arises from three sources: uncertain cash flows (you can't budget for unpredictable per-query spend), quality assessment (you don't know if this answer is worth $0.003 before you receive it), and decision fatigue (hundreds of daily micro-approvals are unmanageable). [Source: [Bitcoin Magazine — Szabo 25 Years Later](https://bitcoinmagazine.com/technical/szabos-micropayments-and-mental-transaction-costs-25-years-later), [Nasdaq — Szabo Analysis](https://www.nasdaq.com/articles/szabos-micropayments-and-mental-transaction-costs-25-years-later)]

The implication for Hodos is not "don't use micropayments" — BSV micropayments are the core differentiator and the x402/BRC-103 signed-request architecture is the security backbone. The implication is: **the UX must eliminate per-transaction decision-making while preserving visibility and control.**

#### 4.2 The Correct Mental Model — Budget Caps + Invisible Execution

[FACT] Szabo himself identified the fix: automated agents that internalize user preferences and approve charges within preset budgets, removing conscious per-transaction decision-making. Payments become invisible; only the budget cap is visible and meaningful. [FACT] The Bitcoin Lightning wallet design community has reached the same conclusion independently. Phoenix wallet (non-custodial Lightning) achieves "the gold standard for sovereignty-preserving UX" by automating channel management so "users hold their own keys and manage their own channels — they just never have to think about it." [Source: [Bitcoin Design Guide](https://bitcoin.design/guide/daily-spending-wallet/first-use/)]

For Hodos, this translates to:
- **Budget cap is the primary UI element**, not the per-query cost
- **Per-query cost is logged and visible in a spend history**, but never interrupts a query
- **Low-balance warning** fires well before the cap is reached, not after
- **Auto top-up** option removes even the top-up decision from the user's regular path
- **The wallet feels like a prepaid card**, not a crypto wallet

#### 4.3 Introducing Wallet Funding to a Casual User

[FACT] The Bitcoin Design Guide recommends presenting funding information "at the time it becomes relevant to the user" rather than upfront. [FACT] The first-deposit moment is "a sensitive moment" — fees and mechanics should be explained clearly at deposit time, preventing the user from assuming initial behavior is abnormal. [Source: [Bitcoin Design — First Use](https://bitcoin.design/guide/daily-spending-wallet/first-use/)]

For Hodos, this means:
- **Don't explain BSV at install time.** Edwin just works with a starting balance.
- **Explain funding only when the balance approaches the cap threshold.** "Your AI wallet has $X remaining. Add more BSV to keep your AI running."
- **Never use the word "blockchain" in the default path.** Say "AI wallet," "AI credit," or "AI budget."
- **Show the first query cost only in retrospect** (spend history), not as a pre-query confirmation.

#### 4.4 The Szabo-to-Casual-User Translation Table

| Szabo's Problem | Hodos's Mitigation |
|---|---|
| Uncertain cash flows | Fixed monthly/weekly budget cap shown as a single number. User knows exactly how much they spend. |
| Quality assessment before payment | Payment happens silently after receiving the answer. User never approves individual queries. |
| Decision fatigue from many micro-approvals | Zero per-query prompts. The agent auto-pays within the cap. |
| Discovering empty wallet via error | Low-balance warning at 20% remaining. Hard pause with friendly message at 0%, not an error code. |
| Explaining micropayment rails to non-technical users | "AI wallet" language only. BSV/blockchain terminology reserved for advanced settings. |

#### 4.5 The Introductory-Balance Trick

[INFERRED] One approach used by prepaid consumer services (mobile SIM cards, app credit systems) is to include a small starting balance at install time — enough to have 10-20 meaningful AI interactions before needing to fund the wallet. This allows the user to experience the value of the AI before being asked to commit money. The cost to Hodos of subsidizing 10-20 queries is negligible at BSV micropayment rates. The trust built by "it works for free at first" is very high.

---

### 5. Proposed End-to-End First-Run Flow for Hodos — Option Set with Pros/Cons

The following are **options**, not a locked design. Each trade-off is noted honestly.

---

#### Option A — "Just Works" (Subsidized Start, Budget Revealed Later)

**Flow:**

1. **Install Hodos** — standard OS installer (Windows/macOS signed binary). No WSL, no Node visible. Hodos starts the Edwin gateway natively on localhost on first launch.
2. **First browser window opens** — Edwin is available immediately as a sidebar/panel. No account creation required yet.
3. **Starting balance pre-loaded** — Edwin begins with a small starter AI credit (~10-20 queries worth at cheap-model rates). No wallet funding prompt on day one.
4. **First interaction** — User asks a question. Edwin answers. No cost display. A small persistent indicator in the corner shows "AI credit: ◉◉◉◉◉◉◉◉◉◉" (a segmented gauge, not a dollar amount) decreasing slightly.
5. **Day 3-5 or when credit reaches 20%** — Gentle in-sidebar message: "You've used most of your starter AI credit. Add BSV to your AI wallet to keep chatting. [Tell me how] [Not now]"
6. **Funding flow** — Plain-language: "Your AI wallet is like a prepaid card for AI. Each question costs a tiny amount — about X cents per day if you chat normally. Add $5 to keep going for about a month." QR code + BSV address. No "blockchain" language.
7. **Budget cap set** — After first funding, user is prompted: "Set a monthly limit so your AI never costs more than you expect. Default: $5/month. [Change] [Keep this]"

**Pros:** Maximum day-one simplicity. User experiences Edwin's value before any money discussion. Removes "this costs money" anxiety from the discovery phase.

**Cons:** Subsidy cost to Hodos (small but non-zero). User may be surprised when the credit runs out if they didn't notice the gauge. Some users may feel tricked if the "free" starter credit is not disclosed upfront.

**Variant A1:** Disclose the starter credit on first use: "You have 15 free AI credits to start with. After that, tiny BSV micropayments keep Edwin running." This sacrifices some simplicity but removes the "gotcha" concern.

---

#### Option B — "Honest Meter" (Cost Visible from Day One, No Surprises)

**Flow:**

1. **Install & launch** — Same as Option A: native binary, Edwin starts automatically.
2. **First browser window** — A one-screen onboarding card: "Edwin is your AI assistant, built into Hodos. It uses a tiny AI credit for each response — usually less than a cent. We've loaded $2 of starter credit for you. [Start chatting]"
3. **Persistent spend gauge** — Always visible in the toolbar: "AI budget: $1.87 / $5.00" as a compact bar. Clicking opens spend history with per-conversation totals (not per-query in the default view).
4. **First interaction** — User asks a question. Subtle animation on the gauge as it decrements by $0.003. No interruption.
5. **Low-balance at 10%** — In-app notification (not modal): "Your AI budget is running low. [Top up your AI wallet]"
6. **Budget cap management** — Accessible from settings: "Monthly AI budget," with "pause AI when limit reached" toggle defaulted ON.

**Pros:** Full transparency. No deferred surprise. Teaches the pay-per-use model early. Spend history is immediately useful.

**Cons:** Showing "$1.87 / $5.00" may trigger Szabo friction on first use for cost-sensitive users. Requires the gauge to be consistently accurate (real-time BSV accounting). "Starter credit" requires explanation of source.

---

#### Option C — "Budget First" (Set Your Limit Before You Start)

**Flow:**

1. **Install & launch** — Same native process.
2. **First-run wizard (3 screens):**
   - Screen 1: "What's Edwin?" — One sentence + one demo. [Next]
   - Screen 2: "Set your AI budget" — Slider: $2 / $5 / $10 / custom per month. "Edwin will pause if you reach your limit." Default: $5. [Set my budget]
   - Screen 3: "Fund your AI wallet" — QR + BSV address. "You need a tiny amount of BSV to start. $2 is enough for a month of casual use." [I've sent BSV] [Remind me later]
3. **Edwin available once funded** (or after a grace period, if Option C is combined with a starter-credit variant).
4. **Interaction pattern** — Same as B: persistent gauge, spend history, low-balance alert.

**Pros:** User is in full control before spending anything. No "hidden" costs. Clear value proposition established. Budget setting reduces ongoing anxiety.

**Cons:** The setup wizard is a friction point — asking a new user to choose a budget before they know what Edwin does is asking them to value something they haven't tried. Funding step before first use is a known drop-off point for crypto applications ("you need crypto first" is historically the biggest casual-user barrier). Risk of users abandoning at step 3 if wallet funding is unclear.

**Variant C1:** Combined with Option A's subsidized starter credit — the wizard introduces the budget concept but Edwin works immediately. The funding step appears only when starter credit is nearly exhausted.

---

#### Option D — "Invisible Unless You Look" (Background Cost, Dashboard on Demand)

**Flow:**

1. **Install & launch** — Native binary, Edwin starts.
2. **No cost mention on first use** — Edwin is presented purely as an AI assistant. Starter credit pre-loaded silently. No gauge, no indicator.
3. **Spend history** — Accessible from a dedicated "Edwin settings" panel: "This month: $0.23 of AI credit used across 47 conversations." A per-conversation breakdown is available on tap.
4. **Budget cap** — Configured in settings, not during onboarding. Default: $5/month, Edwin pauses and notifies if reached.
5. **Low-balance notification** — System notification (OS-level): "Your Hodos AI credit is running low. Tap to add more."

**Pros:** Zero friction first-run. Most similar to Brave Leo's UX. Casual users who don't care about cost are never bothered by it. Cost information is available for those who want it.

**Cons:** Least educational about the micropayment model — users may not discover the spend history unless they notice the budget gauge. Risk that the first meaningful cost interaction is the "running low" notification, which may feel abrupt.

---

#### Recommended Approach for Discussion

[INFERRED — not a prescription] The strongest candidate for a casual-user starting point is **Option A1** (subsidized start with disclosed starter credit) combined with the persistent-but-unobtrusive spend gauge from **Option B**. This gives:
- Zero-friction day one (Edwin works immediately)
- Honest disclosure of the model without requiring upfront wallet setup
- A persistent visual anchor for "what this costs" that teaches the pay-per-use model without triggering decision fatigue
- Low-balance alerts before the cap is reached, never after

The budget-cap-before-first-use approach (Option C) is appropriate for a power-user mode or an opt-in "advanced setup" path, not the default. The YAML-free, form-based budget slider should appear as a gentle prompt during the low-balance notification flow, not as a gate before first use.

---

### 6. What This Means for Hodos — Options, Not a Pick

**On the platform/install problem:**

[FACT] The core WSL/9P failure means Edwin must be bundled as a native binary per OS (Windows x64, macOS arm64/x86_64), pre-built native modules included, started by Hodos on localhost — identical to how Hodos already manages the BSV wallet Rust subprocess. [INFERRED] This is the highest-priority single item: no amount of UX improvement matters if Edwin hangs for 5 minutes on every Windows install. This requires upstream coordination with Jake — a native Windows build target, pre-compiled modules shipped with the Hodos release — not a rewrite of Edwin's logic.

**On defaults:**

Option 1 (Minimal intervention): Ship Edwin with a small cheap model as default (e.g., Llama 3-class or equivalent), deepWorkflow OFF, think level "quick" by default. Expensive models and deep-workflow are surfaced only after the user has had 10+ interactions. Pros: least upstream change, immediate improvement. Cons: doesn't address the model-selection complexity for power users.

Option 2 (Hodos-managed config): Hodos writes the Edwin config file on first launch, ensuring safe defaults are set regardless of Edwin's upstream defaults. Pros: insulates Hodos users from upstream default changes. Cons: Hodos must maintain and update this config layer as Edwin evolves; risk of config drift.

Option 3 (Edwin PR): Submit a PR to Edwin's upstream that makes cheap-model-by-default and deepWorkflow-off the standard defaults. Pros: benefits all Edwin users, not just Hodos. Cons: requires Jake's approval and may not reflect Edwin's primary market (developers/power users for whom premium defaults make sense).

**On the status surfaces (Sources, Skills, Workflows tabs):**

Option A: Hodos proxies the Edwin API and fills the status tabs with accurate data at first run, creating a thin adapter layer. Pros: immediate UX fix. Cons: Hodos maintains an adapter that may break on Edwin updates.

Option B: Hodos hides status tabs entirely until the user has completed guided setup (which populates the files the tabs read). Tabs appear as "not yet set up" with guided links, not as empty/broken states. Pros: simpler, no adapter needed. Cons: removes visibility that some users may want.

Option C: Submit upstream PR to Edwin to fix the status surface data sources. Pros: correct fix. Cons: timeline dependent on Jake.

**On cost transparency and wallet UX:**

Option A: Persistent budget gauge in the Hodos toolbar (not Edwin's UI — Hodos wraps Edwin's API and tracks spend). The gauge and alerts are Hodos features, not Edwin features. Edwin is the engine; Hodos is the dashboard. Pros: Hodos controls the UX completely, no upstream dependency. Cons: Hodos must intercept and account for every Edwin API call in real-time.

Option B: Rely on Edwin's (future) cost reporting API. Pros: less code in Hodos. Cons: Edwin has no such API today; timeline unknown.

Option C: Estimate cost client-side from model + query length, show estimated cost rather than exact cost. Pros: immediate implementation, no API dependency. Cons: estimates will diverge from actual BSV spend; risky if wrong by a factor of 2x.

**On the micropayment introduction:**

Option A (Subsidized starter): Hodos pre-loads a small starting balance so Edwin works on day one without wallet funding. Best casual-user experience. Cost to Hodos is negligible at BSV micropayment rates. Requires Hodos to fund this balance on the user's behalf (operational cost model to establish).

Option B (Budget-first wizard): Surface the budget cap setup during onboarding, before first use. Best for users who want control. Risk of drop-off at wallet funding step. Appropriate if Hodos targets a slightly more technical early-adopter cohort.

Option C (Background cost, on-demand history): Most similar to existing AI browsers (Brave Leo). Lowest friction. Risks that casual users never learn the micropayment model, which is supposed to be a differentiator.

---

### Open Questions

1. **Native Edwin build for Windows:** Has Jake been approached about a Windows-native (non-WSL) build target? Is there a planned timeline? What upstream Node native modules need to be recompiled, and does Hodos have the CI infrastructure to do per-OS builds? [UNVERIFIED]

2. **Edwin's config API:** Does Edwin expose a first-run configuration API (set defaults programmatically at start), or does Hodos need to write config files directly? Which version first introduced the config file format used by Sources/Skills/Workflows tabs? [UNVERIFIED]

3. **Cost reporting:** Does Edwin expose per-query token usage and model-cost data at the gateway API level? If so, is this already documented, or is it observable only via logs? [UNVERIFIED]

4. **Starting balance economics:** At current BSV micropayment rates, what is the realistic per-query cost for a small model (e.g., 500-token query + 500-token response)? What is the cost to Hodos of subsidizing 15 "free" starter queries? Is this a marketing cost to absorb or a per-user operational cost that needs to be tracked?

5. **"AI wallet" vs. "BSV wallet" framing:** The BSV wallet is a separate Rust subprocess for financial sovereignty reasons. Should the AI credit UI look like a sub-wallet inside the main BSV wallet, or a completely separate "AI budget" concept? How does this interact with the signature vault (BRC-103 signed-request) architecture?

6. **Auto top-up UX and BSV custody:** If the user enables auto top-up, Hodos needs BSV on hand to transfer to the AI budget. Does this mean Hodos holds BSV on the user's behalf (custodial for the AI budget), or does it draw from the user's non-custodial wallet? The custody model affects the onboarding copy significantly.

7. **Onboarding discoverability of skills:** Edwin has 74 skills loaded; 31 are "ready." From a casual-user standpoint, should Hodos surface a curated "top 5 things Edwin can do" during first run, rather than exposing the skills panel? What is the right moment to introduce skill configuration?

8. **The YAML-editing workflow problem:** Can workflows be created via conversational input (like Dia does for preferences)? Is there an Edwin API endpoint for creating workflows programmatically, or is the YAML folder the only interface? [UNVERIFIED]

9. **Rate limits and model availability:** If Hodos routes through its own inference layer (to avoid exposing the user's API keys), what rate limits apply? Does Hodos plan to operate an Edwin-compatible inference proxy, or does each user bring their own API key?

10. **Trust onboarding for BSV payments:** The x402/BRC-103 signed-request architecture requires the user's wallet to sign AI queries. At what point in onboarding does the user authorize Edwin to spend from their wallet, what is the permission scope (per-session vs. budget-cap-based), and how is this revocable?

---

*Sources referenced in this document (complete list):*

- [Brave Leo Roadmap 2025](https://brave.com/blog/leo-roadmap-2025-update/)
- [How do I use Brave Leo — Brave Help Center](https://support.brave.app/hc/en-us/articles/20958609786637-How-do-I-use-Brave-Leo)
- [Brave Leo Automatic Mode](https://brave.com/blog/automatic-mode-leo/)
- [Brave Leo Pricing — eesel AI](https://www.eesel.ai/blog/brave-leo-pricing)
- [Brave Leo — Wikipedia](https://en.wikipedia.org/wiki/Brave_Leo)
- [Introducing Comet — Perplexity](https://www.perplexity.ai/hub/blog/introducing-comet)
- [Comet Guide — AI Chief](https://aichief.com/ai-tutorials/a-look-inside-comet-perplexitys-ai-powered-browser/)
- [Comet 101 Complete Guide — Substack](https://sidsaladi.substack.com/p/perplexity-comet-ai-browser-101-complete)
- [Comet Review — SwitchTools](https://www.switchtools.io/blog/perplexity-comet-ai-agent-browser)
- [Comet Wikipedia](https://en.wikipedia.org/wiki/Comet_(browser))
- [Dia Beta Launch — TechCrunch](https://techcrunch.com/2025/06/11/the-browser-company-launches-its-ai-first-browser-dia-in-beta/)
- [Dia Review — Caneraras](https://www.caneraras.com/learn/dia-browser-review-ai-first-browser)
- [The Hyped Dia Browser Has a Huge Problem — Medium](https://medium.com/@theo-james/the-hyped-dia-browser-is-out-but-with-a-huge-problem-42aabcc2e723)
- [Microsoft Edge Copilot Mode Launch — TechCrunch](https://techcrunch.com/2025/07/28/microsoft-edge-is-now-an-ai-browser-with-launch-of-copilot-mode/)
- [Microsoft Copilot in Edge](https://www.microsoft.com/en-us/microsoft-copilot/for-individuals/do-more-with-ai/ai-for-daily-life/ai-browser-innovation-with-copilot-in-edge)
- [Microsoft Killing Copilot Mode — Neowin](https://www.neowin.net/news/microsoft-is-killing-copilot-mode-in-edge-but-ai-features-arent-going-away/)
- [Getting Started with Atlas — OpenAI Help](https://help.openai.com/en/articles/12628555-getting-started-with-atlas)
- [Atlas Features & Security — Seraphic Security](https://seraphicsecurity.com/learn/ai-browser/openai-atlas-browser-features-pros-cons-security-and-privacy/)
- [Atlas Release Notes — OpenAI Help](https://help.openai.com/en/articles/12591856-chatgpt-atlas-release-notes)
- [ChatGPT Pricing 2026 — Fritz AI](https://fritz.ai/chatgpt-pricing/)
- [Szabo Micropayments 25 Years Later — Bitcoin Magazine](https://bitcoinmagazine.com/technical/szabos-micropayments-and-mental-transaction-costs-25-years-later)
- [Szabo Mental Transaction Costs — ResearchGate](https://www.researchgate.net/publication/2401801_Micropayments_and_Mental_Transaction_Costs)
- [Bitcoin Design Guide — First Use](https://bitcoin.design/guide/daily-spending-wallet/first-use/)
- [Budget Caps & Spend Alerts — Requesty](https://www.requesty.ai/blog/budget-caps-spend-alerts-never-blow-your-ai-budget-again-1751655724)
- [Budget Limits and Alerts in LLM Apps — Portkey](https://portkey.ai/blog/budget-limits-and-alerts-in-llm-apps/)
- [Google Cloud Spend Caps at Next '26](https://cloud.google.com/blog/topics/cost-management/introducing-spend-caps-ai-cost-visibility-next26)
- [AI Cost Visibility 2026 — Finout](https://www.finout.io/blog/ai-cost-visibility-in-2026-strategies-tools-and-best-practices)
- [x402 — Alchemy Explanation](https://www.alchemy.com/blog/how-x402-brings-real-time-crypto-payments-to-the-web)
- [x402 — Ledger Academy](https://www.ledger.com/academy/topics/economics-and-regulation/what-is-x402)
- [Fintech Onboarding Drop-off — eleken](https://www.eleken.co/blog-posts/fintech-onboarding-simplification)
- [Lightning Wallet Design Guide V2 — Bitcoin Design / Medium](https://bitcoindesign.medium.com/design-better-lightning-wallets-with-the-bitcoin-design-guide-v2-2669f610ebc7)
- [How Top AI Tools Onboard New Users — UserGuiding](https://userguiding.com/blog/how-top-ai-tools-onboard-new-users)
- [Agentic Browser Landscape 2026 — No Hacks](https://nohacks.co/blog/agentic-browser-landscape-2026)
- [AI Browsers Selection Guide 2026 — AI Multiple](https://aimultiple.com/ai-web-browser)
- [AI Browsers: Comet, Dia, Battle for the Web — Beam.ai](https://beam.ai/agentic-insights/ai-browsers-are-here-comet-dia-and-the-coming-battle-for-the-web)
- [Credit-Based Billing for AI — Flexprice](https://flexprice.io/blog/how-to-implement-credit-based-billing-for-ai-applications)
