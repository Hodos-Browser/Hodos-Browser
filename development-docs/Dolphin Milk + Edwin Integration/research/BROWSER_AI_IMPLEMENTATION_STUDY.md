# Browser AI — Implementation & Rationale Study (HOW + WHY)

> 📁 Part of the **Dolphin Milk + Edwin Integration** doc set — see `README.md`.
> **Created:** 2026-06-26 by a 14-agent research workflow (architecture/rationale lens, web-cited). Companion to `UX_EDWIN_ASSISTANT_COMMUNICATION.md` §5 (which is the business/monetization lens). This doc is the **technical** lens: how each browser actually builds its AI and why. **Study, not decision.**
> Tags **[FACT]/[VISION]/[INFERRED]/[UNVERIFIED]** preserved. Raw per-player data in the appendix.

---

# AI-in-Browser: Industry Architecture Study for Hodos + Edwin

*Research synthesis from 13 player groups. Inline citations preserved. Claim tags: [FACT] = sourced from research data; [INFERRED] = reasoned from evidence; [UNVERIFIED] = stated but not independently confirmed.*

*Players covered: Perplexity Comet, OpenAI ChatGPT Atlas, Google Chrome/Gemini, Microsoft Edge/Copilot, Brave Leo, Opera Aria/Neon, The Browser Company/Dia, Mozilla Firefox, Apple Safari/Apple Intelligence, DuckDuckGo/Duck.ai, Kagi/Orion, Maxthon, Vivaldi/LibreWolf.*

---

## A. How the Industry Builds AI-in-Browser — the Patterns

Seven distinct architecture patterns emerge across the 13 players. Most ships use more than one simultaneously.

---

### Pattern 1: Cloud-API Bolt-On

**Who:** DuckDuckGo Duck.ai, Kagi/Orion, Maxthon AIChat/uuGPT

**How it works technically:** The browser is a thin shell. AI lives at a remote API endpoint; the browser contributes a UI surface (sidebar, toolbar button, address-bar toggle) and, optionally, page content forwarding. The browser operator runs a proxy server that anonymizes requests before forwarding to model providers.

- DuckDuckGo: Browser calls `GET /duckchat/v1/status` to get a session VQD token, then `POST /duckchat/v1/chat` with `x-vqd-4` header; responses stream via SSE. DDG's own servers strip IP before forwarding to Anthropic, OpenAI, Mistral, or Together.ai. [FACT — https://github.com/benoitpetit/duckduckGO-chat-api]
- Kagi/Orion: Users navigate to `kagi.com/assistant` in a browser tab or via the native sidebar panel rendering the Kagi web app. No AI code lives in Orion's core binary. [FACT — https://blog.kagi.com/orion]
- Maxthon: 2023 AIChat sidebar + 2025 uuGPT partnership; the sidebar is an embedded WebContents pointing at uuGPT.com. [FACT — https://blog.maxthon.com/2025/05/22/maxthon-announces-strategic-collaboration-with-uugpt-com/]

**Pros:** Low engineering cost; no local model management; rapid capability upgrades by swapping provider; frontier model quality day one.

**Cons:** All page content leaves the device to reach the model; privacy depends on contractual protections; cannot work offline; browser has no structural AI advantage over a standalone web app; deepest browser context (real session cookies, multiple tabs) requires extra integration work that is typically omitted.

---

### Pattern 2: Deeply Native Cloud

**Who:** Perplexity Comet, OpenAI ChatGPT Atlas, The Browser Company/Dia, Google Chrome Gemini sidebar/AI Mode

**How it works technically:** Cloud inference, but the browser is re-architected so AI has privileged access to internals that no extension could reach.

- **Perplexity Comet**: Three auto-updating Chrome extensions (comet-agent, Comet, Comet Web Resources); comet-agent uses `chrome.debugger` API to call `Accessibility.getFullAXTree`, serializes results to YAML, and dispatches RPC over a dual-channel SSE+WebSocket design. [FACT — https://labs.zenity.io/p/perplexity-comet-a-reversing-story]
- **OpenAI ChatGPT Atlas**: OWL (OpenAI's Web Layer) runs Chromium as a separate process (OWL Host); Atlas (OWL Client) communicates with it via Mojo IPC — custom Swift and TypeScript Mojo bindings give native SwiftUI code direct Chromium function access. [FACT — https://openai.com/index/building-chatgpt-atlas/]
- **Dia**: Native macOS shell (Chromium core + SwiftUI rebuild); AI sidebar has access to current tab DOM, up to 3 @-mentioned tabs, 7-day opt-in browsing history. Routing across Claude/GPT/Gemini. [FACT — https://www.diabrowser.com/security]
- **Google Chrome**: Gemini sidebar renders `cloud.google.com/gemini` inside a Chrome WebUI/WebContents frame; Gemini receives multi-tab context, history, and Connected Apps data when user permits. Distinct from Nano (see Pattern 3). [FACT — https://techcrunch.com/2026/01/28/chrome-takes-on-ai-browsers-with-tighter-gemini-integration-agentic-features-for-autonomous-tasks/]

**Pros:** Frontier model capability; deep browser context unlocks things extensions cannot (cross-tab synthesis, CDP automation, real session cookies); UX quality of a native app; can update AI capabilities without shipping browser updates (cloud-served sidecar UI).

**Cons:** Every AI invocation sends page content to a cloud server; significant attack surface (CometJacking: malicious URL → agent exfiltrates email metadata [FACT — https://tuta.com/blog/perplexity-comet-browser-security-privacy-risks]); supply chain risk if agentic code auto-updates from CDN; full blast radius of compromised agent runs in user's live authenticated session.

---

### Pattern 3: Local-Inference Hybrid

**Who:** Google Chrome/Gemini Nano, Microsoft Edge/Phi-4-mini, Mozilla Firefox (ONNX + wllama), Brave Leo (BYOM/Ollama path)

**How it works technically:** A small, specialized model runs on-device for bounded tasks; a cloud frontier model handles complex reasoning and agentic work. The boundary is enforced by task routing at the browser or sidecar layer.

- **Chrome Gemini Nano**: TFLite/LiteRT via ChromeML binary; sandboxed "On-Device Model Service" utility process; ~4GB download; GPU path (>4 GB VRAM) or CPU path (Chrome 140+, 16 GB RAM); network-blocked during inference. JavaScript `self.ai.languageModel.create()` API surface. [FACT — https://www.island.io/blog/looking-inside-chromiums-on-device-ai-stack]
- **Edge Phi-4-mini**: ONNX Runtime; NPU-first (Snapdragon X Elite), GPU primary, CPU fallback; `LanguageModel.create()` API. Aion-1.0-Instruct in Canary/Dev (June 2026). [FACT — https://blogs.windows.com/msedgedev/2026/06/02/expanding-on-device-ai-in-microsoft-edge-new-models-and-apis-for-the-web/]
- **Firefox ONNX + wllama**: Six registered backends (onnx-wasm, onnx-native C++, wllama WebAssembly, llama.cpp native, openai passthrough, static-embeddings). Native ONNX delivers 2–10x speedup over WASM; SmolLM2-360M (369 MB) for link preview via wllama; flan-t5-base distilled to t5-efficient-tiny (57 MB, INT8) for tab grouping. Currently CPU-only; GPU blocked by sandboxing constraints. [FACT — https://blog.mozilla.org/en/firefox/firefox-ai/speeding-up-firefox-local-ai-runtime/]
- **Brave BYOM**: User configures any OpenAI-compatible endpoint (e.g., `http://localhost:11434/v1/chat/completions` for Ollama). Brave has zero visibility. Not on-device by default; local if user sets it up. [FACT — https://brave.com/blog/byom-nightly/]

**Pros:** Interactive tasks (autocomplete, translation, inline suggestions) get sub-50ms latency; no per-request cloud cost for high-volume tasks; genuine privacy for the local tier; works offline; enables a credible "private AI" narrative.

**Cons:** Capability ceiling is real — a 4B quantized model cannot match Gemini 2.5 Pro on complex multi-step tasks; large downloads (4 GB for Chrome Nano) generate privacy/consent backlash if silent [FACT — https://www.alphapilot.tech/discover/google-chrome-quietly-deploys-4gb-gemini-nano-model-sparking-privacy-and-regulatory-concerns]; GPU sandboxing constraints prevent hardware-accelerated local inference inside browser processes (Firefox confirmed); requires device capability detection and graceful CPU fallback; model management (download, versioning, purging) is non-trivial engineering.

---

### Pattern 4: Privacy-Proxy-Broker

**Who:** Brave Leo (primary), DuckDuckGo Duck.ai (primary)

**How it works technically:** The browser operator runs a dedicated proxy/gateway that sits between the user's browser and all model providers. The proxy performs identity stripping, token unlinkability, and contractual no-train enforcement before forwarding to cloud inference.

- **Brave**: All requests route through Brave's AWS-hosted anonymizing reverse proxy. Provider backends never see user IP. Premium users get VOPRF (Verifiable Oblivious Pseudorandom Function) blind-token scheme via `challenge-bypass-ristretto`: browser generates ~500 tokens, blinds them locally, CBR signs blinded tokens (cannot see originals), browser unblinds → valid credentials unlinkable to the signing event. Double-spend tracking prevents reuse. When accessing Leo, browser presents only a token preimage + HMAC binding (no account ID, no email). [FACT — https://github.com/brave/brave-core/blob/master/docs/premium_account_privacy.md]
- **DuckDuckGo**: IP completely stripped; DDG substitutes its own IP. Data sent to providers: prompt text, today's date, user timezone, unit system preference, optional city-level location. No PII, no cookies, no fingerprint. Contractual 30-day max retention; no-train clause. For `gpt-oss-120b`: routed through Tinfoil.sh TEE (Nvidia Hopper/Blackwell confidential mode, AMD SEV-SNP, Intel TDX) — prompts encrypted in memory; even Tinfoil's own operators cannot read data. [FACT — https://duckduckgo.com/duckduckgo-help-pages/duckai/ai-chat-privacy; https://tinfoil.sh/technology]

**Pros:** Strongest practical privacy short of full local inference; no special hardware required on user's device; frontier model quality preserved; "IP stripping + no-train contracts" is auditable and understandable to users; VOPRF scheme provides cryptographic unlinkability, not just a privacy promise.

**Cons:** All prompt content still leaves the device and must be trusted to the proxy operator; proxy operator knows usage volume even if not content (network-layer necessity); contracts are legal instruments, not cryptographic guarantees for the content layer (Brave mitigates this with self-hosted models as of June 2025 [FACT — https://brave.com/blog/automatic-mode-leo/]); latency added by proxy hop.

---

### Pattern 5: Verifiable Private Cloud

**Who:** Apple Safari/Apple Intelligence (Private Cloud Compute)

**How it works technically:** Cloud inference with cryptographic verification of the server environment before any request is sent. Hardware attestation means users can independently verify what software runs on the cloud nodes processing their data.

- On-device routing model decides whether to use AFM on-device, PCC, or Gemini (WWDC 2026) based on task complexity. [FACT — https://machinelearning.apple.com/research/introducing-third-generation-of-apple-foundation-models]
- PCC flow: (1) Device evaluates which PCC nodes have published software measurements matching the public transparency log (append-only, cryptographically tamper-proof). (2) Request payload end-to-end encrypted to verified PCC node public key; intermediate load balancers see only ciphertext. (3) OHTTP relay operated by a third party strips device IP before request reaches PCC. (4) PCC nodes run stateless inference; memory periodically recycled; only pre-specified audited logs can leave the node. (5) Secure Enclave manages attestation keys; unextractable. (6) RSA Blind Signatures for single-use credentials. [FACT — https://security.apple.com/blog/private-cloud-compute/]
- All production software images published within 90 days; Virtual Research Environment available for independent researcher auditing. [FACT — https://security.apple.com/blog/pcc-security-research/]

**Pros:** "Trust but verify" — cryptographic attestation replaces contractual promises; third-party researchers can independently confirm what code runs; OHTTP relay means Apple cannot link IP to request; hardware root of trust extends even to Google Cloud expansion (Intel TDX + NVIDIA Confidential Computing + Google Titan). [FACT — https://security.apple.com/blog/expanding-pcc/]

**Cons:** Enormous engineering investment only Apple (and equivalently Google) can afford; requires controlling silicon, OS, and inference runtime simultaneously; still requires trusting the attestation mechanism and the transparency log infrastructure; Gemini handoff privacy details not fully public [UNVERIFIED per research data]; most browser vendors cannot replicate this without major cloud partnerships.

---

### Pattern 6: Agentic Automation

**Who:** Perplexity Comet, OpenAI ChatGPT Atlas, Opera Neon/Browser Operator, Brave AI Browsing, Google Chrome Auto Browse/Mariner, Microsoft Edge Copilot Actions

**How it works technically:** The AI doesn't just answer; it executes actions in the browser — clicking, form-filling, navigating, extracting data across multiple pages in a loop. See Section D for detailed mechanics.

**Pros:** Completes multi-step tasks without user re-engagement; access to authenticated sessions enables high-value workflows (booking, calendar management, research aggregation); CDP/accessibility-tree automation is more reliable than legacy Selenium-style scripting.

**Cons:** Highest attack surface in browser AI; prompt injection from adversarial pages can hijack agent running in user's live session; CometJacking class of attack is production-demonstrated; credential blast radius if agent is compromised; requires careful permission gating to avoid user trust violations; Amazon v Perplexity class of legal exposure when agents autonomously scrape and summarize content at scale — publisher lawsuits create architectural pressure to add consent mechanisms and content origin attribution before agents access protected content. [Architectural implication from CometJacking and CEO data-collection framing — FACT from research data; legal case context from training knowledge]

---

### Pattern 7: Deliberate Non-AI

**Who:** Vivaldi, LibreWolf

**How it works technically:** Active architectural exclusion. Vivaldi ships zero AI model, zero LLM integration. LibreWolf locks `browser.ml.*` pref stack to false/empty in `librewolf.overrides.cfg`; models cannot be downloaded because the ML stack is disabled before any fetch can be attempted. [FACT — https://codeberg.org/librewolf/issues/issues/2752; https://vivaldi.com/blog/keep-exploring/]

**Why this is an architectural pattern, not just a product decision:** These represent a coherent set of constraints that a pro-AI-but-private browser must address structurally, not merely rhetorically. See Section G for details.

---

## B. The Local-vs-Cloud Spectrum

### What's Actually Running On-Device in 2026

| Browser | Model | Runtime | Size | Hardware Path | Task Scope |
|---|---|---|---|---|---|
| Apple Safari/AI | AFM 3 Core (3B dense) + Core Advanced (20B sparse) | Apple Neural Engine + AXLearn | ~flash-resident, NAND streaming | ANE primary | Full AI: Writing Tools, Highlights, Siri |
| Google Chrome | Gemini Nano (2B/4B variants); Gemma 197M (Chrome 148+) | ChromeML/LiteRT via closed ChromeML binary | ~4 GB download | GPU (>4GB VRAM) or CPU | Safety signals, tab summaries, writing APIs, Summarizer/Translator |
| Microsoft Edge | Phi-4-mini (3.5–4B); Aion-1.0-Instruct (Canary, June 2026) | ONNX Runtime + Windows Copilot Runtime | Not specified | NPU (Snapdragon X Elite), GPU, CPU fallback | Prompt API, Writing Assistance APIs, simple tasks |
| Firefox | t5-efficient-tiny (57 MB, INT8); SmolLM2-360M (369 MB); Bergamot (~15 MB/language pair) | onnx-native C++; wllama/llama.cpp | 57 MB–369 MB per model | CPU only (GPU blocked by sandboxing) | Tab grouping titles, link preview summaries, page translation, PDF alt text |
| Brave Leo (BYOM) | Any Ollama-compatible model (user-selected) | llama.cpp via Ollama (user-managed) | User-determined | User's full GPU/CPU access | Full chat (user-configured) |
| Opera (Dev-only) | 150+ model families via Ollama | llama.cpp | User-determined | User-determined | Experimental, not default |

### Technical Trade-offs

**Latency:**
- Apple AFM 3 on-device: ~30 tokens/sec, ~0.6ms first-token on iPhone 15 Pro. [FACT — https://machinelearning.apple.com/research/introducing-third-generation-of-apple-foundation-models]
- Chrome Nano (GPU): sub-50ms for interactive tasks. [INFERRED from architecture and design intent]
- Firefox native ONNX: PDF alt-text dropped from 3.5s (WASM) to 350ms (native C++). [FACT — https://blog.mozilla.org/en/firefox/firefox-ai/speeding-up-firefox-local-ai-runtime/]
- Firefox WASM path penalty: 2–10x slower than native; "WASM SIMD can't beat NEON on Apple Silicon or AVX-512 on modern Intel chips." [FACT — Mozilla engineering blog]
- Cloud round-trips: 100–300ms minimum; perceptible in interactive UI contexts.

**Capability ceiling (2026 state):**
- Mozilla explicitly states: "small, specialized models work well on-device, but larger language models still need server-side compute." [FACT — Mozilla engineering blog]
- Agentic multi-step web tasks (the hardest use case) require frontier-class models; 4B quantized models cannot reliably complete them. Chrome Mariner requires Gemini 2.5 Pro. [FACT — https://localaimaster.com/blog/google-project-mariner-web-agent-2025]
- Brave explicitly lists "integrated, pre-configured client-side models" as incomplete on its 2025 roadmap after 2+ years of attempting this. [FACT — https://brave.com/blog/leo-roadmap-2025-update/]

**Cost:**
- On-device eliminates per-token cloud cost. At Chrome's scale (~3B installs), even routing tab-naming to Nano eliminates enormous infrastructure spend. [INFERRED — alphapilot.tech analysis]
- On-device requires 4–22 GB disk space; multi-GB downloads without user consent generate backlash and regulatory scrutiny. [FACT — https://www.alphapilot.tech/discover/google-chrome-quietly-deploys-4gb-gemini-nano-model-sparking-privacy-and-regulatory-concerns]

**Privacy (local vs cloud):**
- Chrome Nano: zero outbound network traffic during inference; confirmed by network analysis. Exception: Safe Browsing Enhanced Protection sends condensed signals (not raw content) to Google servers. [FACT — https://dev.to/jacquesgariepy/inside-chromes-edges-silent-4gb-ai-install-a-complete-hands-on-investigation-54g2]
- The conflation problem: Chrome deploys Nano locally but routes AI Mode omnibox queries entirely to cloud. Users inferring local processing from the on-device model's presence are incorrect. [FACT + analysis — research data]

**Hybrid strategies that work:**
- **Apple's three-tier**: on-device (default) → PCC (complex reasoning) → Gemini (world knowledge/real-time). Task routed by the device itself, with per-tier privacy disclosure.
- **Chrome's functional split**: Nano for safety signals + interactive writing features; cloud Gemini 2.5 Pro for Mariner/Auto Browse agentic work.
- **Firefox's task-specificity**: distilled/fine-tuned small models (57MB–369MB) handle scoped tasks (tab naming, link preview, translation) with local-only guarantees; cloud provider handles general chat.
- **Edwin's current and target architecture (Hodos position)**: Node gateway on localhost port → transition to lean native sidecar. This is architecturally the strongest local-inference position in the ecosystem: an OS-level process with full GPU access (no browser sandbox constraints), able to run Ollama/llama.cpp with CUDA/Metal/Vulkan while Chrome's Nano is CPU-only.

---

## C. How Page Context Reaches the Model

Six distinct mechanisms are in production. Each has distinct privacy implications.

### Mechanism 1: Accessibility Tree (AX/ARIA)

**Users:** Perplexity Comet (primary), OpenAI Atlas CUA (primary), Chrome Mariner (primary, with vision fallback), Opera Browser Operator (variant)

**Technical mechanics:**
- Comet: `chrome.debugger` API → `Accessibility.getFullAXTree` → YAML serialization; only interactable elements (links, buttons, textboxes) annotated with reference IDs. Model receives `click ref_32` rather than pixel coordinates. [FACT — https://labs.zenity.io/p/perplexity-comet-a-reversing-story]
- Atlas CUA: accessibility tree (ARIA roles, labels, semantic structure) + selective screenshots for visual grounding. [FACT — https://nohacks.co/blog/agentic-browser-landscape-2026]
- Opera Browser Operator: "textual representation" of DOM tree + layout data; NOT screenshots or pixel analysis. Can access non-visible elements (cookie banners, off-screen content) because it reads the DOM directly rather than rendering pixels. [FACT — https://blogs.opera.com/news/2025/03/opera-browser-operator-ai-agentics/]

**Token efficiency:** More compact than screenshots; layout-stable across page reflows; does not break when CSS changes.

**Privacy implications:** Exposes all semantic page content including ARIA labels, input field purposes, and element content. Structured data (form field names, button labels) is more semantically rich than raw screenshots. Does not capture visual design or images. Cannot see content not accessible to screen readers.

---

### Mechanism 2: DOM Text / HTML → Text Extraction

**Users:** Perplexity Comet (GetPageText = HTML → markdown), Dia (text extracted at request time via CEF/accessibility APIs), Firefox (page content + title for cloud sidebar), DuckDuckGo (Attach Page Content button), Edge (page text when EdgeEntraCopilotPageContext enabled)

**Technical mechanics:**
- Comet: `GetPageText` action serializes HTML to markdown for the cloud model. [FACT — research data]
- Dia: text extracted from page DOM at time user initiates request. No ambient background scraping. [FACT — https://www.diabrowser.com/security]
- Firefox cloud chatbot: selected text + page title (for queries); full page content + title (for "Summarize Page"). [FACT — https://support.mozilla.org/en-US/kb/ai-chatbot]
- DDG: explicit user action "Attach Page Content" button in sidebar. [FACT — https://duckduckgo.com/duckduckgo-help-pages/duckai]

**Privacy implications:** Full visible text of the page transmitted to inference provider. Includes any PII rendered on the page (account details, messages, health records, financial data). Does not include session cookies, but does include content that may be session-personalized. The key privacy variable is who receives it (local model vs cloud provider vs third-party provider behind a proxy).

---

### Mechanism 3: Screenshot / Vision / Computer-Use

**Users:** OpenAI Atlas CUA (primary), Chrome Mariner (when AX tree insufficient), Perplexity Comet ComputerBatch (fallback), Microsoft Copilot Studio System B (cloud VM screenshots)

**Technical mechanics:**
- Atlas CUA: GPT-4o vision + RL fine-tune; perceives rendered screen and issues virtual mouse/keyboard events. Inputs routed "directly to the web page renderer and never pass through the privileged browser layer," preserving Chromium sandbox integrity. [FACT — https://openai.com/index/building-chatgpt-atlas/]
- Chrome Mariner: Gemini 2.5 Pro receives screenshots and returns bounding boxes when elements are not in the accessibility tree. [FACT — https://arxiv.org/html/2511.19477v1]
- Comet ComputerBatch: raw pixel coordinate clicks, drags, scrolls, keystrokes. Used as fallback for inaccessible pages. [FACT — https://labs.zenity.io/p/perplexity-comet-a-reversing-story]
- Copilot Studio: CUA (OpenAI CUA or Anthropic Claude Sonnet 4.5/4.6) receives screenshots of a cloud VM desktop; runs on isolated Windows 365 Cloud PC. [FACT — https://learn.microsoft.com/en-us/microsoft-copilot-studio/computer-use]

**Privacy implications:** Highest disclosure of any context mechanism. Full visual rendering includes: any PII shown on screen, images, UI chrome, browser notifications, other app windows if visible. Screenshot-based AI sees everything the user sees. Atlas disables screenshots specifically during credential entry. [FACT — research data] Atlas also excludes agent-session-visited pages from browsing history. [FACT — research data] Vision-based approach is the most token-expensive and slowest in the inference loop.

---

### Mechanism 4: JavaScript Execution / DevTools Protocol

**Users:** Chrome DevTools AI assistance (primary), Chrome Mariner (CDP), Perplexity Comet (chrome.debugger API)

**Technical mechanics:**
- Chrome DevTools: Uses JS execution to dynamically determine context: DOM + computed styles (via `getComputedStyle`); allowlisted HTTP request/response headers (API keys and session tokens redacted); source file content (binaries excluded); serialized performance call trees. Deliberately chosen over "sending full HTML/CSS" because "dynamically determine what context data is important for the query at hand." [FACT — https://developer.chrome.com/blog/how-we-introduced-gemini-to-devtools]
- CDP: Chrome DevTools Protocol enables tab/DOM/screenshot/network interception at the browser process level. Comet's `chrome.debugger` API is the extension-accessible subset.

**Privacy implications:** More selective than raw DOM text extraction; redaction of credentials in DevTools integration is explicit. However, JS execution in page context has access to session storage, localStorage, DOM variables, and potentially sensitive runtime state. The selective extraction reduces token cost and exposure compared to full HTML capture.

---

### Mechanism 5: URL-Based / Server-Side Fetch

**Users:** Kagi Universal Summarizer, Firefox Link Preview, Apple Safari Highlights

**Technical mechanics:**
- Kagi: User pastes or triggers URL; Kagi's server-side Librarian agents fetch, extract, and process content. Browser does not inject DOM. [FACT — https://help.kagi.com/kagi/api/summarizer.html]
- Firefox Link Preview: Browser makes a credentialless HTTPS fetch of the linked URL (no cookies sent, no auth headers), parses HTML without executing scripts, extracts Open Graph tags + Reader View content. Sends `x-firefox-ai` custom header allowing publishers to opt out of AI summarization. Inference runs locally on SmolLM2-360M via wllama. [FACT — https://blog.mozilla.org/en/firefox/firefox-ai/ai-link-previews-firefox/]
- Apple Safari Highlights: Page URL sent to Apple via OHTTP relay; Apple retrieves pre-indexed highlight data. For summarization, page text processed on-device or via PCC. [FACT — https://www.asurion.com/connect/tech-tips/safari-highlights-apple-intelligence/]

**Privacy implications:** The content that leaves the device is controlled by the explicit URL trigger, not ambient page scraping. Firefox's credentialless fetch is a strong pattern: it strips identity from the request so the fetched page cannot be tied to the user's authenticated session. OHTTP relay in Apple's case further strips IP.

---

### Mechanism 6: Structured Tool Contracts (WebMCP / App Intents)

**Users:** Google Chrome (WebMCP, origin trial Chrome 149–156), Apple Safari/AI (App Intents)

**Technical mechanics:**
- WebMCP: Websites register callable tool contracts via `navigator.modelContext`. Agents invoke JSON Schema-defined functions (e.g., `checkout`, `filter_results`) rather than parsing UI visually. Same-origin policy; HTTPS required; `tools` Permissions Policy gating. [FACT — https://developer.chrome.com/docs/ai/webmcp]
- App Intents: Apps register typed, structured actions that Siri AI can invoke. AI orchestrates multi-step workflows by composing intents (Messages + Photos + AirDrop chained). Arbitrary web automation explicitly excluded. [FACT — https://developer.apple.com/documentation/AppIntents]

**Privacy implications:** This is the cleanest privacy model: the site or app explicitly declares what data the AI can access and what actions it can take. No unauthorized DOM scraping; no visual parsing of sensitive content. The trade-off is that adoption requires site/app cooperation — agents can only do what sites expose as tools. Early adopters: Expedia, Booking.com, Shopify, Credit Karma, TurboTax, Redfin, Etsy, Instacart, Target for WebMCP. [FACT — https://developer.chrome.com/blog/chrome-at-io26]

---

## D. Agentic Execution Architectures

### Four Automation Engine Approaches

**Approach 1: Chrome DevTools Protocol (CDP) via Extension API**

Used by Perplexity Comet.

Comet's `comet-agent` extension uses `chrome.debugger` API (the extension-accessible CDP surface). Full RPC action set dispatched via WebSocket: `BROWSER_OPEN_TAB`, `BROWSER_CLOSE_TABS`, `BROWSER_GROUP_TABS`, `GET_URL_CONTENT`, `ENTROPY_REQUEST`, `ComputerBatch`, `FormInput`, `Navigate`, `ReadPage`, `GetPageText`, `TabsCreate`, `CreateSubagent`. [FACT — https://labs.zenity.io/p/perplexity-comet-a-reversing-story]

Two action tiers:
- High-level (semantic): FormInput (set value by node ref), Navigate (URL), GetPageText (HTML→markdown). Layout-stable; breaks only if element is inaccessible to accessibility tree.
- Low-level (ComputerBatch): Raw pixel coordinate clicks, drags, scrolls, keystrokes. Fallback for poor accessibility pages.

Session model: User's real live session, full authenticated state. No isolation. Any action the user can take, the agent can take. No technical mechanism to distinguish agent-initiated from user-initiated network requests. [FACT — research data]

Security boundary: `isInternalPage` guard blocks `chrome://` and `comet://`. `isUrlBlocked` blocks `file://`, admin-managed domain blocklist, user domain blacklist. No isolation at the OS or browser-profile level. [FACT — research data]

**Approach 2: CUA (Computer-Using Agent / Vision)**

Used by OpenAI ChatGPT Atlas, Microsoft Copilot Studio System B.

- Atlas CUA: GPT-4o vision + reinforcement-learning fine-tune. Perceives screen; issues virtual mouse/keyboard events. NOT a DOM scripting engine. Inputs routed "directly to the web page renderer and never pass through the privileged browser layer," preserving Chromium sandbox integrity. Multi-round inference loop: plan → execute action → generate structured DOM observation → infer next step → repeat. Inherently slower than CDP automation but semantically general — works on any website without site-specific integration. [FACT — https://openai.com/index/computer-using-agent/; https://openai.com/index/building-chatgpt-atlas/]
- Copilot Studio System B: OpenAI CUA or Anthropic Claude Sonnet 4.5/4.6 receives screenshots of a Windows 365 Cloud PC (isolated VM). Credentials stored in Power Platform encrypted storage or Azure Key Vault. Access control whitelist. Runs entirely off user's personal device. [FACT — https://learn.microsoft.com/en-us/microsoft-copilot-studio/computer-use]

Two critical Atlas session modes:
- **Logged-in mode**: User's real browser session, live cookies. Sensitive financial sites trigger mandatory pause + user oversight.
- **Ephemeral mode (Chromium StoragePartition)**: Fresh, in-memory context, no cookies, no persistent state. All data discarded when session ends. Multiple ephemeral sessions can run simultaneously, isolated from each other and from user profile. [FACT — https://openai.com/index/building-chatgpt-atlas/]

**Approach 3: DOM-Text Native Access (non-CDP)**

Used by Opera Browser Operator/Neon.

Privileged Chromium browser/renderer process APIs rather than the public CDP surface. DOM tree + layout data as textual representation. Can access non-visible DOM elements. Faster than screenshot loops because no pixel rendering required. Can "read the entire page at once without needing to scroll." [FACT — https://blogs.opera.com/news/2025/03/opera-browser-operator-ai-agentics/]

Opera's stated rationale: "Faster because the Browser Operator doesn't need to 'see' and understand the screen from its pixels or navigate with a mouse pointer." [FACT — Opera blog]

Session model: Real authenticated user session, same cookies and auth state. Passwords not extracted. Agent acts on pages where user is already logged in. [FACT — research data]

**Approach 4: App Intents (Structured API Dispatch)**

Used by Apple Safari/Apple Intelligence.

Explicit non-adoption of CDP/computer-use. Every action Siri AI can take must be declared by a developer as a typed App Intent. AI calls the intent; the app executes it with its own credentials — AI never receives authentication tokens. SiriKit deprecated WWDC 2026. [FACT — https://ecorpit.com/ios-27-app-intents-siri-ai-developer-guide-2026/] Arbitrary web form-filling and link-clicking explicitly excluded.

### Isolation Models Compared

| Implementation | Session | Credential access | Profile isolation |
|---|---|---|---|
| Perplexity Comet | User's live real session | Inherited (all session cookies) | None |
| Atlas (logged-in mode) | User's live session | Inherited; agent cannot read saved passwords | None; financial sites pause |
| Atlas (ephemeral mode) | Isolated StoragePartition | No cookies, no passwords | Full in-memory isolation |
| Opera Browser Operator | User's live session | Inherited (session cookies); passwords not extracted | None |
| Brave AI Browsing | Isolated profile | No real session data; no cookies crossing profiles | Full: separate cookie jar, cache, site data |
| Copilot Studio System B | Isolated cloud VM | Credentials in Azure Key Vault; not in agent context | Full OS-level VM isolation |
| Chrome Auto Browse | User's real session | Google Password Manager triggers autofill; password plaintext not shared with Gemini | None; confirmation gates for sensitive actions |
| Apple App Intents | App's own process | App holds its own credentials | App-process-level; AI never touches tokens |
| Edwin (current) | Runs alongside user's real session | No native credential integration | Separate process on localhost |

### The Amazon v Perplexity Legal Constraint — Architectural Implications

Publisher litigation against agentic browsers (exemplified by the Perplexity content-scraping lawsuits) creates specific architectural pressure:

1. **Attribution requirements**: Agents that summarize web content at scale face copyright liability. Architecturally, this favors approaches that route to content via structured tool contracts (WebMCP, App Intents) rather than autonomous DOM scraping — sites consent to structured tool access. [Architectural inference from legal risk pattern; CometJacking data from research]

2. **Content origin tracking**: Agentic browsers need to track which domains they visited and what content they extracted — both for legal liability audit trails and for user transparency. Chrome marks visited-during-task sites with an action icon; Edge adds Purview audit logging. [FACT — research data]

3. **Crawl consent headers**: Firefox's `x-firefox-ai` opt-out header (analogous to robots.txt for AI summarization) represents an emerging norm. Hodos/Edwin that respects this header builds publisher goodwill and reduces legal exposure.

4. **Session-based vs crawl-based**: An agent acting in the user's real session (Comet, Opera) is acting as the user — different legal treatment than an autonomous crawler. This is a key reason agentic browsers prefer operating in live sessions rather than building headless crawlers.

5. **Human-in-the-loop requirements**: Perplexity Comet's "no security impact" response to CometJacking (where a single URL triggers unauthorized email exfiltration) is a litigation risk. Agentic browsers that require explicit user confirmation before accessing third-party services have a stronger legal and security posture.

---

## E. Privacy Architectures, Ranked

From data-hungry to verifiable-private. Focus on mechanisms most relevant to Hodos.

### Tier 6 (Most Data-Hungry): Perplexity Comet

**Mechanism:** No privacy design. Data collection is the product.

CEO explicitly: "one of the other reasons we wanted to build a browser is we want to get data even outside the app to better understand you." Planned targeted advertising using collected behavioral context. [FACT — https://tuta.com/blog/perplexity-comet-browser-security-privacy-risks]

What is collected: Page content, visited URLs, search queries transmitted to Perplexity's servers when AI features invoked. IP address collected always, including Incognito. Crash reports via Sentry, including Incognito. Browsing context stored up to 30 days. [FACT — https://www.cape.co/blog/perplexity-ai-data-privacy-policy]

Auto-updating extensions pull from remote CDN — silent supply chain update vector. [FACT — research data]

---

### Tier 5 (Cloud-First With Controls): OpenAI Atlas, Microsoft Edge Copilot

**Atlas mechanism:** Server-side summarization by default. Raw web content deleted immediately after summarization; privacy-filtered summaries retained up to 7 days. Diagnostics telemetry ON by default. On-device summarization exists as opt-in (macOS 26+) but is not the default path. No TEE or secure-enclave documentation. [FACT — research data]

**Edge mechanism:** Page text + browsing history extracted and sent to `copilot.microsoft.com` when `EdgeEntraCopilotPageContext` enabled. EU users: disabled by default (GDPR). Non-EU users: enabled by default. Copilot Memory ingested signals from Edge + Bing + MSN by default without prominent notice, drawing privacy criticism. [FACT — research data]

---

### Tier 4 (Cloud-First, Limited by UI Design): Google Chrome Gemini (Sidebar/AI Mode)

**Mechanism:** Nano is genuinely local (confirmed by network analysis). AI Mode omnibox queries go entirely to Google's cloud — but the UX conflates them. Users with ~4 GB of Nano on disk will incorrectly infer their queries are local. [FACT + analysis — research data]

Gemini sidebar: page content, tabs, history, Connected Apps data all cloud-processed with user permission. Conversation history in Google Account, reviewable and deletable. Auto Browse: Gemini receives personal and Connected Apps data.

The most significant privacy-communication failure: a locally-installed model creating a false sense of local processing for entirely cloud queries.

---

### Tier 3 (Privacy by Contractual Constraint): DuckDuckGo Duck.ai, Kagi/Orion, Dia

**DuckDuckGo mechanism:**
1. IP stripped (DDG substitutes its own IP to all providers). [FACT]
2. No PII, no cookies, no fingerprint data sent. [FACT]
3. Contractual: providers cannot retain >30 days, cannot use for training. [FACT]
4. Chat history local-only (optional E2E encrypted sync with client-held key). [FACT]
5. No account required. [FACT]
6. TEE tier: `gpt-oss-120b` via Tinfoil.sh — hardware isolation; Tinfoil operators cannot read data; open-source attestation (stronger than Apple PCC's closed-source hypervisor, per Tinfoil's own comparison). [FACT — https://tinfoil.sh/blog/2025-01-30-how-do-we-compare]

**Kagi mechanism:** Server-as-proxy with no unique user identifier for OpenAI API requests. Privacy Pass (VOPRF/RFC 9497) for search anonymization; not yet extended to Assistant. Assistant queries are not anonymous under Privacy Pass until extension is shipped. [FACT — research data]

**Dia mechanism:** Local-first storage (E2E encrypted sync); at request time, question + relevant page context forwarded to cloud providers contractually restricted from retaining or training. Chromium telemetry (UMA, Google Accounts, Reporting APIs) explicitly disabled. SOC 2 Type II 2025. [FACT — research data]

---

### Tier 2 (Privacy-Engineered Proxy + Cryptographic Unlinkability): Brave Leo

**Mechanism (seven layers):**
1. **IP anonymization**: All requests through Brave's AWS reverse proxy; provider backends never see user IP.
2. **Zero retention**: Conversations discarded immediately post-response; not used for training; no server-side usage logs tied to identifiers.
3. **No account**: Free tier requires no email or identity.
4. **VOPRF blind tokens**: challenge-bypass-ristretto library (RFC 9497-compliant). Browser generates + blinds tokens locally; CBR signs blinded tokens (cannot see original values); browser unblinds; resulting credentials are unlinkable to the signing event by the server. Double-spend prevention. HMAC binding to specific merchant+SKU. [FACT — https://github.com/brave/brave-core/blob/master/docs/premium_account_privacy.md]
5. **Self-hosted models**: All models on Brave's own AWS as of June 2025 — no third-party data processors. [FACT]
6. **BYOM**: Zero Brave intermediary; direct browser-to-local-endpoint; Brave has zero visibility. [FACT]
7. **TEE attestation (Nightly)**: Nvidia Hopper GPU TEEs for DeepSeek V3.1 via NEAR AI; hardware isolation; attestation validates model identity + execution code before response is trusted. [FACT — https://brave.com/blog/browser-ai-tee/]

What this achieves vs contractual-only: Layer 4 (VOPRF) means even if Brave is legally compelled to reveal which user made a query, they cannot — the cryptographic scheme prevents linkage. This is architecture-enforced unlinkability, not a promise.

**Agentic privacy (AI Browsing):** Completely isolated browser profile with separate cookie jar, cache, site data. No access to user's real authenticated sessions. Alignment checker (second independent model) receives system prompt + user prompt + proposed action but NOT raw website content — firewalled from prompt injection. [FACT — https://brave.com/blog/ai-browsing/]

---

### Tier 1 (Verifiable Private Cloud + On-Device): Apple Safari/Apple Intelligence

**Mechanism:** See Section A/Pattern 5 and Section B for full detail. Summary:
- Default: AFM 3 on-device, ANE, zero network traffic. [FACT]
- PCC: End-to-end encrypted to hardware-attested node; OHTTP relay strips IP; stateless inference; transparency log; Virtual Research Environment for independent auditing. [FACT — https://security.apple.com/blog/private-cloud-compute/]
- RSA Blind Signatures for unlinkable single-use request credentials. [FACT]

**What makes this uniquely strong:** The transparency log (append-only, cryptographically tamper-proof) means researchers can verify what software runs on PCC nodes. Privacy is hardware-enforced and independently auditable, not contractually promised. Extended to Google Cloud (Intel TDX + NVIDIA Confidential Computing + Titan) in June 2026. [FACT — https://security.apple.com/blog/expanding-pcc/]

**Limitation relevant to Hodos:** This requires Apple Silicon + Apple's inference infrastructure (and now Google Cloud PCC partnership). Replicating this for a third-party browser is not feasible without equivalent cloud infrastructure and silicon investment.

---

### Tier 0 (Fully Local / No AI): Mozilla Firefox local features, Edwin-on-localhost, Vivaldi, LibreWolf

**Firefox local features mechanism:** Tab grouping, link preview summaries, page translation, PDF alt text all run on-device via ONNX native or wllama. Zero outbound network traffic during inference. Models cached in OPFS. Telemetry collects only operational events (not prompt/response content), opt-out. [FACT]

**Edwin-on-localhost (current Hodos architecture):** All inference through Node gateway on localhost port. No data leaves device unless user explicitly routes to a cloud provider. OS-level process — no browser sandbox constraints on GPU access. This is the strongest privacy position available without specialized cloud infrastructure.

---

## F. The Crypto-Browser Prior Art: Maxthon — Forensic Analysis

### What Maxthon Actually Built (2020–2022)

**VBox (identity and wallet subsystem):** Built into Chromium fork C++ core — confirmed by two architectural facts: (1) it exposes a browser API that web pages call synchronously (impossible from a sandboxed extension without message-passing indirection), and (2) it handles custom URL schemes (`tx://`, `nb://`) at the browser's URL dispatch layer, requiring modification of Chromium's protocol handler registration in C++. [INFERRED from documented behavior; consistent with official technical blog]

**Signing flow:** Web page sends SHA-256 hash to VBox via browser API call → VBox performs double-SHA256 sign with stored private key → returns signature → page verifies with public key. Private keys stored locally with encryption; optional cloud sync (opt-in, encrypted). [FACT — https://blog.maxthon.com/2020/06/07/maxthon-6-blockchain-browser-part-1/]

**VPoint unit abstraction:** 1 VPoint = 100 satoshi. UI hides raw satoshi amounts from users. [FACT — https://coingeek.com/new-browsers-domain-systems-for-bitcoin-sv-powered-internet-debut-at-coingeek-live/]

**NBdomain (.b TLD):** Custom domain system on BSV blockchain. Browser natively resolves `.b` domains by querying external BSV nodes (thin client over HTTP/JSON; no local BSV node). Also handles `b://` and `d://` data protocols for reading BSV on-chain data blobs. [FACT — https://blog.maxthon.com/2020/07/27/maxthon-6-supports-nbdomain-protocol/]

**MCP connector pattern (2026):** Not present in Maxthon — Maxthon's API exposure was unidirectional (browser API to page) without the multi-tool MCP abstraction now used by Opera Neon (March 2026 [FACT]).

**AI (AIChat 2023 / uuGPT 2025):** 2023 launch claimed "all AIChat interactions occur locally and no personal data is sent to external servers" [UNVERIFIED — research data; claim contradicted by 2025 uuGPT cloud partnership]. The 2025 strategic partnership with uuGPT.com (cloud AI SaaS) reveals the true architecture: embedded WebContents pointing at a cloud service. No local inference runtime ever documented. [FACT — https://blog.maxthon.com/2025/05/22/maxthon-announces-strategic-collaboration-with-uugpt-com/]

**Developer portal:** v.maxthon.com/doc — now unreachable (connection refused as of June 2026). [FACT — research data]

---

### Why Maxthon Failed: Specific Technical and Adoption Reasons

**Failure 1: Chicken-and-egg content ecosystem.** VBox, `.b` domains, and NBdomain required a simultaneous two-sided bootstrap: browser users AND content on `.b` domains. Without `.b` content, no reason to adopt the browser. Without browser users, no developer reason to build `.b` content. Neither side materialized at meaningful scale. [INFERRED from market evidence; no official postmortem]

**Failure 2: BSV ecosystem isolation.** BSV was politically and technically rejected by most of the crypto/developer community — contentious fork from BCH, Craig Wright controversy. The pool of developers willing to build on BSV was tiny, making the content-side bootstrap nearly impossible regardless of browser quality. [INFERRED from market evidence]

**Failure 3: Micropayments-for-browsing never found PMF anywhere.** Not on BSV, not on Lightning Network, not on any blockchain. The core premise — that users want to pay per page or per content unit — has not found product-market fit despite multiple attempts across multiple ecosystems. This suggests the problem is the UX of micropayment friction, not the underlying technical protocol. [INFERRED from broader market evidence + absence of success stories]

**Failure 4: Privacy credibility destroyed by data exfiltration history.** 2016: Exatel researchers found Maxthon 4.4.5 transmitting browsing history, visited URLs, installed applications, and ad-blocker status to Beijing servers over unencrypted HTTP — vulnerable to MITM. [FACT — Wikipedia/security coverage] 2023: renewed reports of sensitive browsing data transmitted to Chinese servers. [UNVERIFIED — secondary reporting without named researchers] The "user-owned identity" pitch is unconvincing from a browser with documented exfiltration history. Exactly the demographic most interested in blockchain-native identity (privacy-conscious users) is also most likely to know about Maxthon's history.

**Failure 5: Declining pre-existing user base.** Maxthon peaked at ~100M users in 2012–2014 era. By 2020 the user base had already shrunk significantly, providing limited network effect to bootstrap a new ecosystem. A browser that was growing would have had a much better bootstrapping position.

**Failure 6: Developer portal abandonment.** When `v.maxthon.com/doc` (connection refused as of June 2026) goes dark, the developer ecosystem dies with it. No documentation → no new DApps → no new content → no new browser users.

**Failure 7: AI pivot was undifferentiated commodity.** The uuGPT integration is a cloud sidebar webview — identical in architecture to Edge Copilot, Opera Aria, and every other browser AI sidebar. No local inference advantage, no privacy advantage, no BSV-payment integration. The pivot provided no architectural differentiation.

---

### Key Precedent for Hodos

The Maxthon case is the most important precedent precisely because Hodos shares its BSV-native ambitions. The failures are mostly not technical — VBox's architecture was sound. They are ecosystem and trust failures:

1. **Target existing BSV infrastructure, not a new ecosystem.** Maxthon tried to bootstrap `.b` domains and MetaNet from zero. Hodos should integrate with 1Sat Ordinals, Sigma Auth, and existing BSV DApps. The payment primitive works immediately without new content.

2. **Privacy credibility must be architecturally demonstrable, not marketed.** Maxthon claimed local AI while deploying cloud AI. Edwin's localhost sidecar architecture is genuinely local — make this observable (network activity log, offline mode verification).

3. **BSV is implementation infrastructure, not the banner.** Maxthon's exclusive BSV identity made the browser politically unacceptable to most developers. Hodos's users care about privacy and capability; BSV is how you deliver cheap, private micropayments — the brand is the outcome, not the ledger.

4. **Sidecar process isolation keeps AI and wallet separated.** Maxthon didn't contemplate this because their AI (cloud sidebar) and wallet (native C++) were different systems. Edwin as a localhost sidecar maintains process isolation — AI compromise cannot directly reach wallet keys. Hodos proxies payment signing; Edwin never has private key access.

---

## G. The "Why NOT" Cases: Vivaldi and LibreWolf

### What They Protect and Why

**Vivaldi's architectural objections (all documented):**

1. **Hallucination/confabulation**: LLMs produce "plausible-sounding lies." Browser integration amplifies trust because outputs appear inside the user's primary information tool. [FACT — https://vivaldi.com/blog/technology/vivaldi-wont-allow-a-machine-to-lie-to-you/]
2. **Passive consumption harm**: PewResearch data shows users click traditional results ~half as often when AI summaries present, starving independent publishers. [FACT — https://vivaldi.com/blog/keep-exploring/]
3. **Agentic security risk**: Guardio Labs research shows agents running in live sessions (with real credentials and session cookies) are vulnerable to prompt injection — malicious pages issue fraudulent instructions that AI agent executes, including purchasing products and clicking phishing links. [FACT — https://cyberinsider.com/vivaldi-rejects-ai-integration-commits-to-human-centric-browsing/]
4. **Training data copyright/privacy**: LLMs "regurgitate copyrighted material" and leak "sensitive, private information" from training. [FACT — research data]
5. **User mandate**: ~95% of Vivaldi's user base opposed AI integration. [FACT — https://otontechnology.com/vivaldi-anti-ai-browser-4-million-users/]
6. **Business model insulation**: No VC, no ad network, no behavioral data requirement. Cloud AI integration creates a data-flow liability incompatible with zero-data-monetization positioning. [INFERRED from research data]

Vivaldi's one carve-out: **Vivaldi Translate** (Lingvanex, Iceland servers, no third-party, deterministic). This demonstrates the underlying principle: bounded, deterministic, no hallucination risk, no content summarization, no session access. [FACT — https://vivaldi.com/features/translate/; https://lingvanex.com/blog/cases/business-case-20/]

**LibreWolf's architectural objections (documented):**

1. **Cloud AI = surveillance vector**: Cloud LLM APIs require transmitting page content to third-party servers. Directly contradicts LibreWolf's core mission. [FACT — https://codeberg.org/librewolf/issues/issues/2037]
2. **Proprietary service incompatibility**: Two of Firefox's three AI providers (ChatGPT, Gemini) are proprietary; LibreWolf considers this ethically incompatible. [FACT — issue #2037]
3. **Defaults are the product**: If Firefox enables AI opt-out rather than opt-in, LibreWolf intervenes by locking defaults to disabled. "Most users never touch about:config." [FACT — maintainer statement in issue #2037]
4. **Maintenance overhead**: A small volunteer team cannot audit AI feature drift across every Firefox release. Disabling the entire `browser.ml.*` stack via configuration is lower-maintenance than patching code. [FACT — issue #1919]

LibreWolf's implementation: `browser.ml.chat.enabled`, `browser.ml.enable`, `browser.ml.linkPreview.enabled`, `browser.ml.pageAssist.enabled`, `browser.tabs.groups.smart.enabled`, `extensions.ml.enabled` — all locked to false/empty in `librewolf.overrides.cfg`. Models cannot download because the ML stack is disabled before any fetch is attempted. `browser.ml.chat.hideFromLabs=true` removes the entry point from Firefox Labs so users cannot even toggle it on. [FACT — https://codeberg.org/librewolf/issues/issues/2752]

---

### What a Privacy-Minded But Pro-AI Browser Must Do

To capture the benefit Vivaldi and LibreWolf avoid:

| Vivaldi/LibreWolf Objection | Architectural Response for Hodos |
|---|---|
| AI intermediates session with cloud provider | Edwin runs on localhost; page content stays on device by default; cloud is explicit user opt-in |
| Hallucination erodes trust | Local inference grounded on page content; confidence indicators; citations surfaced; AI never presented as authoritative fact |
| Agentic prompt injection | Isolated CEF profile for all agent tasks; user confirmation gate before every write action; page content sanitized/tagged before reaching model |
| Web ecosystem degraded | Edwin augments discovery (summarizes, connects); doesn't replace clicks; citations link back to original sources |
| Defaults expose users | AI features OFF by default; each feature requires opt-in; visible status indicator when Edwin is active |
| Configuration can be overridden by updates | Edwin's data-access permissions enforced at IPC/process layer, not just UI toggles |
| Proprietary model training on user data | Local inference by default; contractual no-train for any cloud tier; all context ephemeral unless user explicitly enables memory |
| Privacy claims unverifiable | Edwin sidecar is an OS-level process; network activity log shows exactly what leaves device; offline mode possible |

The Vivaldi Translate carve-out taxonomy is directly usable for classifying Edwin features. Every capability can be scored on four axes:
- (a) Data locality: local / proxied / third-party cloud
- (b) Hallucination risk: deterministic / grounded / open-ended LLM
- (c) Browser-context access required: none / page text / full DOM / credentials / multi-tab
- (d) Does it intermediate or augment user intent?

Features scoring low risk on all four can ship with minimal consent overhead. Features scoring high require explicit per-invocation user action.

---

## H. Implications for Edwin-as-a-Native-Sidecar in Hodos

*Options with pros/cons only. No winners selected. This is a study to inform a later decision.*

Hodos specifics: CEF-based browser shell (Windows/Linux primary), Node Edwin sidecar on localhost port (transitioning to lean native binary), BSV wallet, x402 micropayments, privacy-conscious audience, BSV DApp ecosystem integration. Edwin already exposes an MCP server.

---

### H1. Where Edwin's Model Calls Should Run (Local/Cloud/Hybrid)

**Option A: Local-only (Ollama + small model, default for all tasks)**

Mechanism: Edwin routes all inference to a local Ollama endpoint (`localhost:11434`). User selects model (Phi-4-mini, Gemma 2, Llama 3.1, etc.). No cloud call-home whatsoever.

Pros: Strongest privacy claim in the industry (stronger than Brave BYOM because Edwin manages it rather than requiring user configuration); works fully offline; no per-request cost; GPU-accelerated since Edwin is an OS-level process unbound by browser sandboxing (unlike Firefox's CPU-only constraint); directly addressable as a product differentiator.

Cons: Quality ceiling is real — 4–8B quantized models cannot reliably handle complex multi-step research, long document synthesis, or nuanced reasoning; Brave has been trying to ship this for 2+ years and hasn't made it the default; requires user to have compatible hardware; model download UX (explicit consent, size disclosure) must be handled carefully; local-only makes some features unusable on low-spec machines.

**Option B: Cloud-with-proxy-broker (all calls through Hodos-controlled proxy, no direct user-to-provider)**

Mechanism: Edwin routes cloud calls through a Hodos-operated anonymizing reverse proxy (similar to Brave Leo's model). Proxy strips IP, enforces no-train contractual terms, handles billing. x402 BSV micropayments could fund per-request relay usage.

Pros: Frontier model quality (Claude, GPT-4o, Gemini) available to all users regardless of hardware; Brave's model proves this is trusted by privacy-conscious users; simpler Edwin deployment (Node process with HTTP routing, no GPU management); BSV x402 maps naturally (each request is a paid micropayment, cryptographically auditable).

Cons: All prompt content leaves device — Hodos must be trusted as the proxy; requires Hodos to operate and maintain cloud infrastructure; data breach or legal compulsion at Hodos level could expose content; relies on contractual protections with model providers unless TEE partnership (NEAR AI / Tinfoil) is added; operational costs of running a proxy at scale.

**Option C: Two-tier hybrid — local for bounded tasks, cloud (proxied) for heavy reasoning**

Mechanism: Edwin uses a small local model (Ollama + Phi-4-mini-class) for: page summarization, tab grouping titles, inline writing suggestions, quick Q&A, privacy-sensitive classification, real-time page signals. Routes to cloud (via proxy) for: complex multi-step research, deep document synthesis, agentic task planning, code generation. Routing decision is explicit in the UI — local vs cloud is always labeled, with cloud requiring user confirmation and triggering an x402 payment event.

Pros: Captures quality of frontier models where needed; preserves genuine local privacy for high-frequency/sensitive tasks; x402 payment event serves dual purpose as consent mechanism and billing; matches proven industry patterns (Chrome Nano + cloud Gemini, Firefox local + cloud chatbot, Edge Phi-4-mini + cloud Copilot); Edwin's OS-level GPU access makes local tier more capable than browser-sandboxed equivalents.

Cons: More complex routing logic; users must understand two-tier model (though explicit UI labeling addresses this); local model still needs download management; routing decisions may be opaque if not carefully designed; cost of maintaining two code paths.

**Option D: User-configured routing (BYOM extended to all tiers)**

Mechanism: Edwin exposes an OpenAI-compatible API on the sidecar port. Users configure any combination: local Ollama endpoint, Hodos proxy for cloud, direct cloud API (their own keys). Factory default: local Ollama. Privacy-maximalist users never leave local. Users who want frontier quality configure cloud.

Pros: Maximum user agency; Brave's BYOM proves demand from technical users; Hodos doesn't need to operate proxy infrastructure if users bring their own cloud keys; future-proof as model landscape evolves; OpenAI-compatible interface means Edwin is compatible with every existing tool; makes Hodos's privacy position literally verifiable (users can audit traffic).

Cons: Complexity barrier for non-technical users; factory default quality depends on small local model; no revenue model from inference routing; users with their own cloud keys have no privacy protection beyond their direct relationship with the provider.

---

### H2. How CEF Should Feed Page Context to Edwin

**Option A: Accessibility tree snapshot on user request**

Mechanism: When user invokes Edwin for a page-aware task, Hodos calls CEF's DevTools Protocol (`Accessibility.getFullAXTree`) and serializes to JSON/YAML. Sent to Edwin over the localhost IPC channel. Edwin uses this as primary page context.

Pros: Same proven mechanism as Comet (production-deployed at scale); token-efficient; layout-stable; no screenshot bandwidth; CEF exposes same CDP surface as Chrome via `CefBrowserHost` DevTools interfaces; degrades gracefully when AX metadata is sparse (fall back to text extraction).

Cons: AX tree misses visually-rendered content that is not in the accessibility tree (images, canvas, some iframe content); some pages have minimal ARIA metadata; still exposes full semantic page content to Edwin — should be opt-in per invocation.

**Option B: DOM text extraction via JavaScript evaluation**

Mechanism: Hodos executes JavaScript in the active frame via `CefFrame::ExecuteJavaScript` to extract visible text content, page title, and Open Graph metadata. Result sent to Edwin as structured JSON.

Pros: Simpler implementation than AX tree serialization; captures what the user sees as text; faster than full AX tree for text-heavy pages.

Cons: Less structured than AX tree (loses element roles and interaction metadata needed for agentic actions); JS execution in page context creates slight XSS-class risk if malicious page can interfere with the extraction script; does not capture forms' semantic meaning.

**Option C: URL + credentialless fetch (Firefox link preview pattern)**

Mechanism: Edwin receives current page URL and performs its own credentialless HTTP fetch (stripping all auth cookies and headers, analogous to Firefox's approach). Edwin processes the public response. Hodos never forwards DOM content from the live authenticated session.

Pros: Strongest privacy boundary — authenticated session content never reaches Edwin; protects against any prompt injection vector in the live DOM; Edwin's output is based on the publicly-accessible version of the page.

Cons: Loses session-personalized content (user's email inbox view, authenticated dashboard state) — often the content the user actually wants Edwin to reason about; additional network request adds latency; fails for pages requiring authentication by design; publishers can deny credentialless fetches.

**Option D: Explicit content selection (Dia @-mention pattern)**

Mechanism: Edwin has zero tab access by default. User explicitly selects content to share with Edwin (via: selecting text and pressing shortcut, clicking "Share this page with Edwin," or @-mentioning specific tabs in the Edwin panel). Edwin receives only the explicitly shared content.

Pros: Strongest consent model in the industry; no ambient surveillance; lowest surface area for prompt injection (user-curated content only); aligns with Vivaldi/LibreWolf users' expectations about deliberate AI interaction; clear mental model.

Cons: Friction for casual use cases where the user expects Edwin to "just see" the current page; requires user behavior change; misses proactive suggestions that depend on ambient page awareness.

**Option E: Hybrid — URL+AX snapshot, sanitized, on explicit invocation**

Mechanism: When user explicitly invokes Edwin (keyboard shortcut, sidebar trigger, address-bar query), Hodos (1) extracts AX tree snapshot via CEF CDP, (2) strips invisible/hidden DOM elements (CSS `display:none`, `visibility:hidden`, `opacity:0`), (3) tags the content block clearly as `<untrusted_page_content>` in the system prompt, (4) sends to Edwin. No background DOM access. State clears after each session.

Pros: Addresses Opera's prompt injection vulnerability (hidden CSS text) via sanitization; structured context for agentic tasks; explicit invocation matches Vivaldi/LibreWolf expectations; tagging prevents model from treating page content as user instructions.

Cons: Still exposes full page text to Edwin on each invocation — local-only Edwin makes this acceptable; not appropriate for cloud routing without additional consent gate.

---

### H3. How the Assistant UI Should Surface

**Option A: Persistent native sidebar panel**

Mechanism: Dedicated CEF panel (separate from web content area) docked to the right or left of the browser window. Native OS panel — not an iframe, not an injected overlay. Edwin streams responses via SSE from the localhost port into this panel.

Pros: Comet, Chrome, Edge, Brave, Opera all validated this as the primary AI form factor; persists context across tab navigation; native rendering means no z-index battles, no extension blockers can hide it, trust signal to users who know what native UI looks like; SSE streaming from localhost port to native panel = standard HTTP, easy to implement.

Cons: Takes permanent screen real estate; must implement lazy-start (panel starts Edwin sidecar on first invocation, not browser launch) to avoid idle RAM overhead; Edge's lesson: dissolving the persistent sidebar in favor of ambient injection saved 18% idle RAM. [FACT — research data]

**Option B: Ambient injection points (address-bar + right-click + keyboard shortcut)**

Mechanism: No permanent sidebar. Edwin surfaces via: address-bar ML router (detects natural language vs URL vs query), right-click context menu on selected text, keyboard shortcut (opens transient panel that dismisses after interaction), toolbar button.

Pros: Edge May 2026 lesson — users treated Copilot as a separate thing they had to open; ambient integration normalizes AI as a browser primitive; no idle RAM from persistent panel; lower visual noise for users who want AI only occasionally.

Cons: Less discoverable for new users; no persistent conversation context visible across tab switches; harder to implement address-bar ML router (requires a small local classifier for query intent — though a heuristic approach works for early versions); higher engineering complexity for the multi-surface case.

**Option C: Localhost-hosted SPA served as side panel**

Mechanism: Edwin's Node/native sidecar serves a React SPA at `http://localhost:<port>/sidecar`. Hodos loads this URL in the native side panel via a dedicated embedded browser frame (separate from the user's main browsing profile).

Pros: Comet uses this pattern (loading from `perplexity.ai/sidecar`) and it enables rapid UI iteration without browser rebuilds; if hosted from localhost, page content never leaves device for the UI render itself; modern web UI tooling (React, Tailwind) available.

Cons: Comet loads from cloud (`perplexity.ai`) — loading from localhost instead is the key privacy win; but serving a React SPA from a Node/native sidecar adds complexity; embedded browser frame for the panel carries CEF renderer process overhead; same-origin considerations for the panel accessing Hodos browser APIs.

**Option D: CEF WebUI (Chromium-native first-party panel)**

Mechanism: Edwin's panel implemented as a Chromium WebUI (like Chrome's internal settings pages) — web technologies (HTML/CSS/JS) but running in a privileged first-party context with access to private Chromium APIs unavailable to web pages.

Pros: Brave's Leo uses this approach (confirmed by GitHub issues); first-party trust level; cannot be blocked by ad blockers; stable resize/pin/dismiss behavior from Chromium's Side Panel API; same toolchain as web development.

Cons: Requires deeper CEF integration — WebUI registration is not exposed as a simple CEF API; more complex build than serving from localhost; tighter coupling to Chromium version; less rapid iteration than a standalone React SPA.

---

### H4. How to Do Agentic Actions Safely Against a Wallet

**Option A: Isolated CEF profile for all agentic browsing, wallet actions require separate confirmation**

Mechanism: When Edwin initiates any agentic task (form-filling, navigation, web research), Hodos opens a separate `CefRequestContext` with in-memory storage (off-the-record profile). This profile has no cookies, no session data from the user's main profile, no wallet access. Web-based tasks complete in this isolated context. For any BSV payment, a separate confirmation dialog (native Hodos UI, not a web overlay) appears requiring explicit user approval, amount, and destination address.

Pros: Atlas's ephemeral StoragePartition design validated as the right trust primitive [FACT — research data]; Brave AI Browsing (isolated profile) validated at production scale [FACT]; prompt injection in isolated context cannot reach real session cookies or wallet; clear blast radius for compromise; BSV payment confirmation is native browser chrome — cannot be spoofed by page.

Cons: Agent cannot use user's real authenticated sessions (must re-authenticate or manually pass credentials); adds friction for tasks requiring login; requires session handoff mechanism if user wants to delegate a logged-in action.

**Option B: Live session with strict action confirmation tiers**

Mechanism: Agent runs in user's real session (same CEF profile). All actions require explicit user confirmation: read operations auto-approved (visibility is passive); suggestion display requires one-click accept; write actions (form fill, navigation to new domain) require per-action confirmation; any BSV payment or wallet action requires biometric/PIN confirmation with amount, address, and context displayed in native Hodos UI.

Pros: User friction eliminated for real-session actions (agent can see authenticated dashboards, logged-in services); maps to Comet's live-session model which users prefer for usability; Opera's real-session model confirmed as usable with hard gates on sensitive actions.

Cons: Prompt injection attack surface remains if malicious page content reaches the model and triggers unauthorized confirmation sequences; CometJacking demonstrated this is a real production threat; blast radius includes all real session data if permission gating is bypassed; wallet exposure risk is highest here.

**Option C: BSV micropayment as authorization token for agentic capability**

Mechanism: Each agentic capability tier is gated behind a BSV x402 payment from the user's wallet. Read-only page observation: free. Page text summarization: 1 satoshi. Form fill with confirmation: user explicitly authorizes a BSV payment for the action. Any transaction on behalf of the user: separate payment + TXID logged on-chain as immutable audit trail. No payment = no agentic action.

Pros: Payment IS the consent mechanism — cryptographic, auditable, unforgeable; no ambiguous "are you sure?" popups; every agentic action has an on-chain audit trail (useful for dispute resolution); aligns with x402 architecture natively; differentiator — no other browser offers agentic authorization with a cryptographic audit trail; rate-limits malicious use (attacker must spend BSV to trigger actions).

Cons: Micro-payments for every action adds friction for routine tasks; BSV price volatility affects UX (1 satoshi might become meaningful); requires wallet balance; may feel burdensome for power users doing many tasks; real-money consequences for testing/development use.

**Option D: Hodos-native "wallet proxy" — Edwin never has direct wallet access**

Mechanism: Edwin requests wallet actions from Hodos via a typed IPC call (not a raw API). Hodos owns all cryptographic wallet operations. Edwin can only request: "sign this payload", "broadcast payment to address X for amount Y satoshis." Hodos displays the request in native browser UI, user approves, Hodos signs, returns signed payload or TXID to Edwin. Edwin never sees private keys at any point.

Pros: Consistent with Chrome Password Manager pattern (browser owns credentials, AI never receives them [FACT — https://support.google.com/chrome/answer/16821166]); strongest key security possible — compromise of Edwin sidecar cannot lead to BSV theft; clean separation of concerns; matches Maxthon's VBox architecture (C++ in browser core, wallet separate from AI logic).

Cons: Requires Hodos to implement a complete wallet IPC interface; every wallet operation requires round-trip to Hodos main process; cannot support Edwin-initiated payments without user interaction (reduces autonomous payment utility); more complex initial implementation.

---

### H5. The Privacy-Broker Pattern for Routing AI Calls

**Option A: No proxy — local inference only**

Mechanism: Edwin only calls local model endpoints. No HTTP calls to external AI providers. Users who want cloud quality configure it themselves (BYOM pattern).

Pros: Strongest possible privacy statement — "no AI call from Hodos ever leaves your machine unless you configure it"; no operator infrastructure to maintain; zero liability for third-party data exposure.

Cons: Quality ceiling limits use cases; non-technical users cannot configure BYOM; no revenue model from inference routing; Brave BYOM is confirmed as a power-user feature only, not a default.

**Option B: Hodos-operated anonymizing proxy (Brave Leo pattern)**

Mechanism: When user opts into cloud inference, all requests route through a Hodos-controlled proxy server. Proxy strips IP, substitutes Hodos IP, enforces contractual no-train terms with providers, handles x402 BSV billing. User's IP never reaches model provider.

Pros: Brave Leo's model is trusted by privacy-conscious users at production scale [FACT]; IP stripping is the minimum viable privacy property; BSV x402 maps naturally (each request = paid micropayment through proxy); contractual no-train adds layer on top; VOPRF blind tokens (Brave's approach) can be adapted for Hodos.

Cons: Hodos is now a data processor — even without content logs, Hodos knows usage volume; must maintain cloud infrastructure; legal liability if proxy is subpoenaed; must be operated long-term (if Hodos shuts down the proxy, users lose cloud capability).

**Option C: OHTTP relay (Apple pattern) — third-party IP decoupling**

Mechanism: Cloud AI requests from Edwin route through an OHTTP (Oblivious HTTP, RFC 9458) relay operated by a party separate from both Hodos and the model provider. Relay sees only ciphertext of the payload (end-to-end encrypted to the model provider). Hodos cannot see request content; model provider cannot see IP; relay cannot see content.

Pros: Cryptographic separation of "who is asking" from "what is being asked"; even legal compulsion against Hodos cannot reveal request content; RFC 9458 is well-specified, existing implementations; BSV x402 micropayments could fund per-request relay usage (OHTTP relay as a paid BSV service — natural alignment with Hodos's x402 architecture).

Cons: Requires a third-party to operate the relay (adds dependency); relay must be trustworthy (cannot collude with model provider); OHTTP adds latency; fewer off-the-shelf implementations than standard HTTP proxy; model provider still processes plaintext after decryption.

**Option D: TEE routing tier (DuckDuckGo/Tinfoil pattern)**

Mechanism: For highest-sensitivity queries, Edwin routes through a TEE inference endpoint (Tinfoil.sh, NEAR AI, or similar). Model runs in hardware-isolated enclave; even infrastructure operators cannot read prompts. BSV micropayment (premium tier) funds the higher-cost TEE inference.

Pros: Cryptographic enforcement, not contractual promise; hardware attestation verifiable by client; DuckDuckGo/Tinfoil demonstrated this at production scale [FACT — https://tinfoil.sh/technology]; Brave's Nightly TEE implementation shows the direction [FACT — https://brave.com/blog/browser-ai-tee/]; BSV payment naturally differentiates premium TEE tier from standard proxy tier.

Cons: Higher cost than standard cloud inference; limited model selection (only models deployed in TEE environments); Hodos requires partnership with TEE provider (NEAR AI, Tinfoil); higher latency from enclave overhead (though reported as near-zero on Nvidia Hopper); requires user education on what TEE means.

**Option E: Per-provider direct (user holds API keys, Edwin routes)**

Mechanism: User configures API keys for chosen providers (Anthropic, OpenAI, etc.) directly in Hodos settings. Edwin routes requests directly to provider endpoints using user's own key. No Hodos intermediary for cloud calls.

Pros: Hodos has zero visibility into any cloud inference call; provider relationship is entirely between user and provider; user can leverage existing subscriptions; no Hodos cloud infrastructure required; most transparent about data flows.

Cons: User bears full privacy exposure to provider with no additional protection; no IP stripping; no contractual guarantees beyond provider's own policy; API key management creates UX complexity and security risk (Kagi's programmable button pattern showed this is too high-friction for casual users [FACT — research data]); providers can link usage to API key (persistent identity).

---

### Cross-Cutting Recommendations for the Hodos Architecture Decision

*These are framing observations from the research, not architecture picks.*

**The sidecar is already the strongest privacy position in the industry.** Edwin on localhost is more private than Brave BYOM (which requires user configuration), more private than Firefox local inference (which is CPU-only and browser-sandboxed), and comparable to Apple on-device AFM (which requires Apple Silicon). The OS-level GPU access Edwin has is a genuine hardware advantage over any browser-sandboxed inference.

**The UI conflation problem (Chrome) is a clear anti-pattern.** Never let users infer local inference when the actual call is cloud. Every Edwin surface must clearly label "Local (Edwin)" vs "Cloud (provider name)." This is a trust differentiator, not a minor UX detail.

**Prompt injection is the primary security threat for any page-context feature.** The Opera Neon (hidden CSS text, opacity:0 elements exfiltrating email via injected LLM instructions [FACT — research data]) and CometJacking (single URL → email exfiltration [FACT — research data]) demonstrations are production-confirmed. Minimum viable defenses: (1) strip invisible DOM elements before sending to model, (2) tag all page content as `<untrusted>` in system prompt, (3) require user gesture to initiate page context sharing, (4) implement an alignment-checker second model that never sees raw page content (Brave's pattern [FACT — https://brave.com/blog/ai-browsing/]).

**The x402/BSV payment event is a natural consent mechanism.** No other browser in this study has this. Used well, it can replace Brave's VOPRF scheme (payment pseudonymity on-chain) and provide a clearer user-confirmation primitive than "are you sure?" dialogs for consequential actions.

**The developer portal must survive.** Maxthon's v.maxthon.com/doc is dead. Edwin's developer documentation and API reference for BSV DApp integration must be durably hosted — on-chain via 1Sat if that aligns with the architecture, or on a static site with long-term stability commitment.

---

*Document compiled from research data covering 13 browser/AI implementations. All [FACT] claims are cited to specific sources in the research corpus. All [INFERRED] claims are reasoned from documented evidence. All [UNVERIFIED] claims reflect the research data's own uncertainty flags. No claims are fabricated. Accessed 2026-06-26.*

---

## Appendix — raw per-player implementation data (verbatim, cited)

> Captured 2026-06-26. Tags: **[FACT]** verifiable · **[VISION]** roadmap · **[INFERRED]** technical inference · **[UNVERIFIED]**. Sources per player.

### Perplexity Comet

**AI implementation architecture (where model runs, how browser talks to it).** **Pure cloud inference — no on-device model whatsoever.** [FACT]

The local footprint is: (1) a Chromium browser shell, (2) three auto-updating Chrome extensions, and (3) a Rust audio-preprocessing SDK for voice. Every AI inference call goes to Perplexity's cloud.

**Model roster (multi-model routing):** Perplexity Sonar and R1 (in-house, default search); Claude Opus 4.5 (Anthropic, used for Deep Research mode on Max/Pro tiers); GPT-4.1 / GPT-5 (OpenAI, user-selectable); OpenAI GPT Realtime 1.5 (voice mode only, via Realtime API); Gemini Pro and Grok 4 (user-selectable). [FACT — sourced from Beginners in AI 2026 article and OpenAI developer blog]

**Extension layer (three bundles, all auto-updated via GET /rest/browser/update-crx):**
- comet-agent (agents.crx, ~700 KB service worker): implements the full RPC dispatch system (dispatchRpcRequest routes backend commands to handlers).
- Comet (perplexity.crx): manages tab lifecycles, sidecar panels, PDF parsing via offscreen documents, idle/suspend monitoring, Sentry exception telemetry, browser history and top-sites access.
- Comet Web Resources (comet_web_resources.crx): minimal extension, no background scripts — makes /sidecar/* and /spa/* static assets web-accessible to perplexity.ai domains, functioning as a local CDN cache.

**Communication architecture (dual-channel):**
SSE stream at /rest/sse/perplexity_ask carries streaming reasoning tokens and citations to the sidecar UI. When browser automation is needed, the backend delivers an entropy_request message containing a base_url pointing to wss://www.perplexity.ai/agent. The sidecar unpacks this and forwards via Chrome's extension messaging API to comet-agent, which then opens a WebSocket directly to the backend for high-frequency bidirectional RPC. Both channels run simultaneously and independently — conversational UI never blocks automation execution.

**Voice audio pipeline:** a Rust SDK normalizes audio across Swift/TypeScript/Rust/C++ surfaces, resamples to 48 kHz mono, runs WebRTC APM (echo cancellation, automatic gain control, noise reduction, high-pass filtering), and encodes for transport. Audio is then streamed to OpenAI's Realtime API — not Perplexity servers. [FACT — OpenAI Developer Blog]

No local inference runtime (no llama.cpp, ONNX, WebGPU inferencing). The only on-device "intelligence" is WebRTC audio signal processing.

**Integration depth & browser access.** **Deeply native, not a bolt-on.** [FACT — Zenity reverse engineering writeup]

The comet-agent extension uses chrome.debugger API (CDP) to call Accessibility.getFullAXTree, generating a YAML-formatted accessibility tree. Only interactable elements (links, buttons, textboxes) are annotated with reference IDs, reducing token overhead. The model receives node reference IDs ("click ref_32") rather than pixel coordinates by default. This is more layout-stable and token-efficient than screenshot-based CV.

**What the AI can see and control:**
- Current tab DOM: full read via ReadPage (accessibility tree → YAML) and GetPageText (HTML → markdown). Write via FormInput (set element values by node ref) and ComputerBatch (pixel-coordinate clicks, drags, scrolls, keystrokes as a computer-use fallback for inaccessible pages).
- Tab lifecycle: create, close, group, ungroup tabs.
- Browser history and top sites: accessed by Comet (perplexity.crx) for contextual suggestions.
- Cookies and authenticated sessions: directly inherited — agent actions run in the user's live profile. No re-authentication for any service the user is already logged into.
- PDF content: parsed via offscreen documents.
- Multi-tab cross-synthesis: user can @-reference any open tab to include it in context.
- External services via MCP connectors: Gmail, Google Calendar, Slack, GitHub, Asana, Linear, Notion, Atlassian, Shopify — authenticated externally, with tool inputs/outputs displayed inline in the sidecar panel.

**What it cannot access (enforced by isInternalPage/isUrlBlocked guards):** chrome:// internal pages (settings, password manager), comet:// internal pages, file:// URLs, user-configured domain blacklist, admin-managed domain blocklist.

The Comet extension (perplexity.crx) separately provides browser history, bookmarks, and top-sites access to the model for proactive contextual suggestions independent of agent tasks.

**Form-factor mechanics.** **Three surface areas, all web-rendered except the chrome-level shell:**

1. **Persistent collapsible side panel (primary UI):** The Sidecar is a React SPA loaded from https://www.perplexity.ai/sidecar into Chromium's native Side Panel API. It is not rendered as native C++ UI — it is a web page inside a native Chrome frame. Assets are served by the Comet Web Resources extension as a local CDN (perplexity.ai-domain web-accessible resources), allowing the sidecar JS to update from the cloud without browser version bumps. The panel renders multi-step agent reasoning, tool invocations, decision points, MCP connector inputs/outputs, and citation cards in real-time as the SSE stream delivers them.

2. **Omnibox / address-bar handler:** Natural language commands typed in the address bar (e.g., "Summarize this page," "Open Gmail") are intercepted and routed to the assistant.

3. **Voice mode overlay:** Activated via Shift+Alt+V (Windows/Linux) or Shift+Option+V (Mac). Ambient by default — a "voice lock" feature inverts push-to-talk so the user can hold the floor during natural pauses (preventing premature model responses). Screen-aware: can reference what is currently visible. Context fed incrementally to GPT Realtime in 2,000-token chunks (discovered via experimentation that large all-or-nothing context updates fail; incremental chunks allow graceful truncation). [FACT — OpenAI Developer Blog]

4. **Background assistant / mission control (Max tier):** Async task execution while user browses other pages. Central dashboard for monitoring multiple parallel tasks. User receives notifications on completion and can intervene mid-task.

The sidecar receives task visualization updates via the SSE stream (renders reasoning steps as they arrive) while the WebSocket carries the actual automation RPC beneath it — the two are visually coupled but technically decoupled.

**Agentic execution mechanics.** **Automation engine: CDP via chrome.debugger API.** [FACT — Zenity reverse engineering writeup]

**Two action tiers:**
- High-level (semantic): ReadPage → Accessibility.getFullAXTree → YAML tree with annotated element reference IDs → model issues FormInput (set value by node ref), Navigate (URL with forward/back), GetPageText (HTML → markdown). More reliable than pixel methods; breaks only if an element is inaccessible to the accessibility tree.
- Low-level (ComputerBatch): Raw pixel coordinate actions — clicks, drags, scrolls, keystrokes. Used as fallback for pages with poor accessibility or for actions the semantic layer cannot handle. This is the "computer use" mode.

**Full RPC action set (dispatched via WebSocket):**
BROWSER_OPEN_TAB, BROWSER_CLOSE_TABS, BROWSER_GROUP_TABS, BROWSER_UNGROUP, GET_URL_CONTENT, ENTROPY_REQUEST, ComputerBatch, FormInput, Navigate, ReadPage, GetPageText, TabsCreate, CreateSubagent.

**Session model: user's real live session.** [FACT] No isolated profile or sandbox. Comet inherits all existing browser logins. The system has no technical mechanism to distinguish agent-initiated from user-initiated network requests. This gives maximal capability (no re-authentication friction) at the cost of the full blast radius of a compromised agent.

**Credential handling:** Standard Chrome extension storage APIs hold session state locally in the browser. No credential extraction to Perplexity servers is documented, but all navigation/form-fill actions occur in the authenticated session.

**Subagent spawning:** CreateSubagent opens nested automation tasks in new tabs, allowing parallel sub-workflows.

**Permission ladder (stated policy, not enforced by OS isolation):** read-only observation → proactive suggestions → draft actions → act with human confirmation → fully autonomous. Consequential actions (transactions, form submissions, data changes) require user confirmation; read operations (calendar, dashboards) are fully autonomous.

**External services:** MCP connectors authenticate to Slack, Gmail, GitHub, Asana, Linear, Notion, Atlassian, Google Calendar, Shopify. Tool call inputs/outputs are displayed inline in the sidecar. Authentication to these services goes through the sidecar/browser session, not a separate agent profile.

**Security boundaries enforced in comet-agent extension:** isInternalPage (blocks chrome:// and comet:// URLs), isUrlBlocked (blocks file://, view-source:file://, admin-managed domain blocklist stored in managed extension storage, user-configured domain blacklist).

**Background assistant:** Runs tasks asynchronously while user continues browsing. Mission control dashboard provides parallel task monitoring. Human-in-the-loop checkpoint at completion.

**Privacy / data architecture.** **Cloud-first, data-collection-motivated.** [FACT — CEO quote, Zenity analysis, Tuta research, Cape analysis]

**What is sent to cloud:**
- When AI features are invoked: page content, visited URLs, search queries transmitted to Perplexity's servers.
- Voice audio: local Rust SDK preprocessing → streamed to OpenAI's GPT Realtime 1.5 API. Audio goes to OpenAI, not Perplexity.
- Crash reports and exceptions via Sentry (Comet extension registers for this; sent even in Incognito).
- IP address (collected always, including Incognito).
- Browsing context stored on Perplexity servers for up to 30 days; enterprise queries deleted after 7 days.

**What stays local:**
- Browser session state (cookies, passwords, payment info in Chrome extension storage).
- Browsing data in Incognito Mode (not collected).
- Session activity logging disabled in Incognito.

**Stated business model (data angle):** CEO Aravind Srinivas explicitly stated: "one of the other reasons we wanted to build a browser is we want to get data even outside the app to better understand you" — and plans for targeted advertising using collected behavioral context. [FACT — confirmed multiple sources] This is architecturally significant: Comet's privacy architecture is designed to maximize data collection while maintaining plausible deniability, not to minimize it.

**No TEE or secure enclave** mentioned in any public documentation. [UNVERIFIED either way]

**CometJacking vulnerability (disclosed August 2025 by LayerX researchers):** [FACT — Tuta writeup] A single malicious URL can cause the agent to extract user memory and email metadata and exfiltrate it base64-encoded, with no additional user interaction beyond clicking a link. Perplexity classified the finding as "no security impact." This demonstrates the fundamental risk of running agents in the user's live authenticated session with unrestricted URL navigation.

**Auto-updating extensions:** The three extensions pull updates via GET https://www.perplexity.ai/rest/browser/update-crx. This is a supply chain risk vector — a compromised Perplexity CDN can push new agentic capability to all Comet browsers silently.

**Enterprise:** CrowdStrike partnership provides additional telemetry/security monitoring for enterprise Comet deployments (announced ~June 2026).

**Summary posture:** Comet is the inverse of a privacy-first browser. It is a data-collection product that happens to provide useful AI browsing features. Privacy mitigations (Incognito, local storage defaults) exist but the architectural intent is broad data capture.

**The WHY (strategic + engineering reasoning).** **Strategic rationale (stated where possible; inferred labeled):**

1. "Everything is Computer" framing [FACT — CEO Srinivas]: The browser is the universal authenticated observation point. An app-level agent (like Perplexity's standalone Computer product) sees only what each app's API exposes. A browser agent sees ALL authenticated sessions simultaneously — email, calendar, SaaS tools, internal apps — with no additional integration. The browser is the most leverage point for an agent platform.

2. Data collection moat [FACT — CEO quote]: Perplexity explicitly built Comet to capture behavioral data outside the search app, enabling a personalization and advertising model that mirrors Google Chrome's strategic position. The browser gives Perplexity a signal layer no pure-search product can match.

3. CDP + accessibility tree over pure pixel-based CV [INFERRED]: ComputerBatch (pixel coordinates) exists as a fallback, but the primary path uses element reference IDs from the accessibility tree. This is more token-efficient, layout-stable (doesn't break when page reflows), and reliable across dynamic SPAs. It also reduces screenshot bandwidth in the automation loop.

4. SSE + WebSocket dual-channel [INFERRED]: SSE is low-bandwidth, server-push-only, ideal for streaming progressive reasoning tokens to a UI. WebSocket is bidirectional, suitable for high-frequency RPC (send action → receive result → send next action). Separating them means UI streaming never competes with automation throughput, and the conversational context layer is never blocked by automation latency.

5. Chrome Extension architecture for the agentic layer [FACT — Zenity writeup, quoting Perplexity's own reasoning]: The Chrome Extensions API is "battle tested" and provides a "sound and secure framework for sensitive interactions with a webpage." It also inherits the full Chrome extension ecosystem (user-installed extensions continue working in Comet), lowering the switching barrier.

6. Web-rendered sidecar (React SPA from cloud) [INFERRED]: Enables rapid UI iteration and A/B testing without shipping browser binary updates. The Comet Web Resources extension caches assets locally, combining responsiveness with server-side control.

7. Multi-model routing [FACT]: Differentiation from ChatGPT Atlas (OpenAI lock-in). Perplexity routes per-task — Sonar for fast search, GPT Realtime for voice latency requirements, Claude Opus 4.5 for long-context deep research. User-selectable models reduces vendor lock-in as a purchase objection.

8. Running in user's live session [INFERRED strategic choice]: Reduces user friction (no re-authentication, no credential vaulting system to build). The trade-off (full blast radius if agent is compromised) was judged acceptable given the CometJacking response ("no security impact"). This is a calculated prioritization of capability and adoption over security hardening.

9. Voice using OpenAI Realtime (not Perplexity's own infra) [FACT]: GPT Realtime 1.5 is the best available low-latency voice model. Perplexity is a multi-provider aggregator, not an infra company; they choose best-of-breed per capability rather than building everything themselves. The Rust audio SDK abstracts platform differences to keep the integration clean.

**Lessons for Hodos/Edwin.** **Concrete architecture takeaways for Hodos/Edwin (CEF-native, privacy-first, BSV-micropayments):**

**1. CDP + Accessibility Tree is the proven page-perception primitive. Adopt it.**
CEF exposes the same CDP surface Chrome does (via CefBrowserHost DevTools interfaces). Implement Accessibility.getFullAXTree → serialize to YAML/JSON → feed to Edwin as page context. This is what Perplexity uses in production, it is token-efficient, and it degrades gracefully when accessibility metadata is sparse (fall back to GetPageText / HTML → markdown). Do not build a bespoke DOM scraper.

**2. Dual-channel is the right Edwin IPC pattern.**
Edwin's localhost port should expose: (a) an SSE endpoint for streaming reasoning tokens to the Hodos side panel, and (b) a WebSocket endpoint for bidirectional automation RPC (Hodos sends action specs, Edwin returns results). This exactly mirrors Comet's architecture and keeps UI streaming decoupled from automation execution latency.

**3. Serve the side panel from localhost — this is the single biggest privacy win over Comet.**
Comet loads its React SPA from perplexity.ai/sidecar. Page content then travels to cloud servers for every AI invocation. If Hodos's side panel loads from http://localhost:<Edwin_port>/sidecar, page context never leaves the machine. You get the same "rapid UI iteration without browser rebuild" advantage while eliminating the core privacy liability. This is a genuine architectural differentiator, not just marketing.

**4. Implement the permission ladder with real UX gates before any write action.**
Comet's CometJacking vulnerability (malicious URL → agent exfiltrates email metadata, no user click required beyond navigation) happened because the agent has unrestricted URL navigation + live session access. For Hodos: enforce explicit read → suggest → draft → act-with-confirmation tiers in Edwin's action executor. Display the pending action in the side panel and require user approval before form fills, navigation to new domains, or any external service writes. Especially important for BSV-payment actions.

**5. Context persistence: local SQLite/files, user-controlled retention. Never cloud.**
Comet retains context on Perplexity servers for 30 days (stated advertising data play). Edwin should persist all conversation context and page history to local storage (Hodos user profile directory — SQLite or JSON files). User sets their own retention window. This is the privacy positioning in a single architectural decision.

**6. Voice: local STT/TTS is the differentiator if you ship it.**
Comet routes voice to OpenAI Realtime — audio leaves the device. Perplexity's own Rust audio SDK (WebRTC APM, 48 kHz mono, Opus) is the right preprocessing model to copy: do the signal processing locally (echo cancellation, noise reduction, AGC), then send text to Edwin (not audio). Use Whisper.cpp for local STT. TTS can use a small local model (Coqui, Kokoro) or be deferred. This is harder but is the only way to claim voice is private.

**7. Multi-model routing is table stakes — x402/BSV micropayments map cleanly to per-call billing.**
Plan Edwin's routing layer to dispatch per task type: local Ollama for quick page summaries (zero cost, zero latency), Anthropic for deep research, OpenAI Realtime for voice if you take the convenience path. Each cloud call can be a BSV micropayment at the Edwin-to-provider level. This is a natural fit for Hodos's x402 architecture — users pay per inference, providers receive satoshis per call.

**8. MCP is the right external connector standard.**
Comet's Gmail/Slack/Notion connectors work through MCP. Edwin already exposes an MCP server. Extend Edwin's MCP server with external service connectors using the same standard. Users control which services Edwin can access; each action is explicit. This mirrors Comet's MCP pattern without Perplexity's cloud auth flow — credentials stay in a local vault.

**9. Avoid auto-updating agentic code from a remote CDN.**
Comet's extensions auto-update silently from perplexity.ai/rest/browser/update-crx — a supply chain risk. For Hodos, Edwin's agentic code is bundled with the CEF browser or installed as a signed local binary. Updates flow through Hodos's own signed release channel, not a remote extension CDN. This is a security property privacy-minded users will care about.

**10. What Comet proves about the live-session agentic model:**
Running agents in the user's real authenticated session works and is the correct UX choice for consumer adoption. The risk mitigation is not isolation (Comet doesn't isolate) but rather tight permission gating + domain blocklists + explicit user confirmation before consequential actions. Hodos should adopt the same live-session model (CEF in the user's real profile) with strict action confirmation UX rather than trying to maintain a parallel isolated profile — that path adds friction that kills adoption.

**Sources:** <https://labs.zenity.io/p/perplexity-comet-a-reversing-story — Primary reverse engineering of Comet's extension architecture, RPC system, dual-channel comms, CDP usage (accessed 2026-06-26)> · <https://developers.openai.com/blog/realtime-perplexity-computer — Perplexity's own engineering writeup on voice mode: Rust SDK, WebRTC APM, GPT Realtime 1.5 integration, context chunking strategy (accessed 2026-06-26)> · <https://dev.to/samwil007/how-perplexity-ais-comet-browser-actually-works-a-technical-deep-dive-on-the-future-of-the-57cp — Technical deep dive: hybrid architecture, context bus, DOM interpretation stages, background worker pool (accessed 2026-06-26)> · <https://tuta.com/blog/perplexity-comet-browser-security-privacy-risks — CometJacking vulnerability disclosure, data collection practices, CEO quotes on data strategy (accessed 2026-06-26)> · <https://www.cape.co/blog/perplexity-ai-data-privacy-policy — Data collection categories, local vs cloud storage, retention (accessed 2026-06-26)> · <https://www.mindstudio.ai/blog/perplexity-comet-browser-semantic-work-graph-strategy — Semantic work graph strategy, permission ladder architecture, MCP integration rationale (accessed 2026-06-26)> · <https://techcrunch.com/2025/10/02/perplexitys-comet-ai-browser-now-free-max-users-get-new-background-assistant/ — Background assistant launch, Max tier features, mission control dashboard (accessed 2026-06-26)> · <https://www.softwareseni.com/perplexity-comet-what-an-ai-native-browser-actually-does/ — Integration depth, DOM read/write, authenticated session inheritance, ChatGPT Atlas comparison (accessed 2026-06-26)> · <https://summarizemeeting.com/en/news/perplexity-voice-mode-comet — Voice mode upgrade details, GPT Realtime 1.5, screen-aware voice queries (accessed 2026-06-26)> · <https://beginnersinai.org/whats-new-perplexity-2026/ — Model roster: Deep Research on Claude Opus 4.5, multi-model routing overview (accessed 2026-06-26)> · <https://www.perplexity.ai/comet — Official Comet product page (accessed 2026-06-26)> · <https://www.perplexity.ai/comet/whats-new/releases-2.25.26 — Release notes including voice mode technical details (accessed 2026-06-26)> · <https://www.perplexity.ai/comet/whats-new — Comet changelog (accessed 2026-06-26)> · <https://www.superchargebrowser.com/library/perplexity-comet-vs-chrome-extensions/ — Tab context awareness, sidebar integration mechanics vs Chrome (accessed 2026-06-26)> · <https://comet-help.perplexity.ai/en/articles/12867415-comet-assistant-privacy-data-use — Comet assistant privacy & data use help article (accessed 2026-06-26)> · <https://www.techtimes.com/articles/318028/20260608/perplexity-raises-200-million-comet-ai-browser-agent-economy-front-door.htm — $200M raise, Perplexity Computer vs Comet distinction (accessed 2026-06-26)> · <https://ir.crowdstrike.com/news-releases/news-release-details/crowdstrike-and-perplexity-partner-deliver-enhanced-security/ — CrowdStrike enterprise security partnership (accessed 2026-06-26)>

### OpenAI ChatGPT Atlas

**AI implementation architecture (where model runs, how browser talks to it).** **Model runtime**: Primarily cloud-based. The main ChatGPT sidebar and Q&A pipeline calls OpenAI's server-side GPT-4o (or successor) via HTTPS API — no local LLM for conversation [FACT]. Agent mode is powered by CUA (Computer-Using Agent), which also runs inference cloud-side (GPT-4o vision + reinforcement-learning fine-tune) [FACT - https://openai.com/index/computer-using-agent/].

**On-device carve-out**: On-device summarization was added as an opt-in feature for macOS 26+ (2026), keeping web content local rather than sending it to OpenAI servers [FACT - https://seraphicsecurity.com/learn/ai-browser/openai-atlas-browser-features-pros-cons-security-and-privacy/]. The Jimmy Song technical teardown notes Atlas bundles "OptGuideOnDeviceModel" and "screen_ai" on disk, adding several gigabytes to the install [UNVERIFIED - secondary analysis, https://jimmysong.io/blog/chatgpt-atlas-architecture-analysis/; these are plausibly inherited Chromium internals (screen_ai is a Chromium component for accessibility)].

**OWL architecture (the key novelty)**: OpenAI built OWL (OpenAI's Web Layer) to decouple Atlas from Chromium. Instead of embedding Chromium inside the Atlas process (as most Electron/CEF apps do), Chromium runs as a separate background process (OWL Host) while Atlas is the OWL Client. They communicate via Mojo, Chromium's own inter-process message-passing system. OpenAI wrote custom Swift and TypeScript Mojo bindings so their native SwiftUI app can call Chromium functions directly without embedding a WebView [FACT - https://openai.com/index/building-chatgpt-atlas/].

**Browser-to-AI communication**: The sidebar and agent mode communicate with OpenAI's cloud API over standard HTTPS. There is no published detail on whether a local proxy or native IPC bridges requests — it is [INFERRED] that the Swift application directly calls the ChatGPT API, with page context serialized and injected as prompt context before sending.

**Process map (inferred from sources)**: Main Atlas process (SwiftUI/AppKit) → OWL Client → Mojo IPC → Chromium browser process (OWL Host) → Rendering processes (Blink). Separately: AI Runtime process → LLM Orchestrator → OpenAI cloud API. Agent Sandbox process handles execution isolation [FACT for Mojo/OWL; INFERRED for exact process graph beyond what OpenAI published - https://jimmysong.io/blog/chatgpt-atlas-architecture-analysis/].

**Integration depth & browser access.** **Deeply native — not a bolt-on**. Atlas is not a Chrome extension or sidebar add-on; the ChatGPT intelligence is woven into the browser's core. The OWL architecture gives Atlas direct Mojo-level access to Chromium internals that no extension could reach [FACT].

**What the AI can see and use**:
- Current page content: full DOM, read via accessibility tree (ARIA roles, labels, semantic structure) + selective screenshots for CUA visual grounding [FACT - industry analysis confirms Atlas's CUA uses accessibility tree + vision hybrid: https://nohacks.co/blog/agentic-browser-landscape-2026]
- Multiple tabs: agent mode can open, navigate, and compare across multiple tabs autonomously (confirmed in flight-comparison demos) [FACT]
- Cookies/session: in logged-in agent mode, user's real session cookies are accessible so the agent can act on sites where the user is already authenticated [FACT - https://skywork.ai/blog/chatgpt-atlas-agent-mode-automation-explained/]
- Browser memories: cross-session context from previously visited sites, stored as privacy-filtered summaries [FACT]
- Form fields: "cursor chat" can rewrite text at cursor position inline [FACT]

**What the AI cannot see**:
- Saved passwords and autofill data [FACT]
- Local filesystem or other applications [FACT]
- Browsing history entries created during agent sessions (excluded by design) [FACT]
- Pages visited in incognito mode (ChatGPT is signed out) [FACT]

**Page context delivery mechanism**: Page content is passed to the model as text extracted from the accessibility tree + DOM semantic representation, supplemented by composited screenshots for CUA visual tasks. The OWL architecture gives Atlas a "global context pipeline" that can aggregate context across pages/sessions in ways extensions cannot [FACT for accessibility tree approach; INFERRED for global pipeline details - https://www.searchenginejournal.com/the-accessibility-tree-is-how-ai-agents-read-your-site-its-breaking/578171/].

**Form-factor mechanics.** **Three primary surfaces** [FACT - https://openai.com/index/introducing-chatgpt-atlas/]:

1. **New tab page**: Replaces the browser's default new tab. Blends a ChatGPT chat input with real-time search results (links, images, video, news cards). The AI's first-touch surface — every new tab is a potential interaction point.

2. **Persistent sidebar ("Ask ChatGPT")**: Available on any webpage via a persistent trigger. User can summarize, analyze, compare, or ask questions about the current page without leaving it. Shows a split-screen view when the user clicks search results (webpage left, ChatGPT transcript right) [FACT].

3. **Cursor chat (inline editing)**: The AI can rewrite text at the cursor position inside web form fields — a truly in-page overlay capability that requires browser-level DOM write access, not achievable via extension [FACT].

**Rendering technology**: Atlas is built "almost entirely in SwiftUI and AppKit" — the browser chrome (tabs, nav bar, sidebar, new tab page) is native macOS UI, not a webview [FACT - OWL blog]. The ChatGPT conversation panel is [INFERRED] a native SwiftUI view backed by the ChatGPT API, not an embedded browser window pointing at chat.openai.com. Tab content renders via Chromium's CALayer system, exposed to Atlas through the CALayerHost API with GPU memory sharing for efficient compositing [FACT - OWL blog].

**Agent mode visual indicator**: When agent mode is active, a distinctive "blue UI highlighting" shows where the agent cursor is operating, giving the user continuous visual feedback [FACT].

**Platform trajectory**: macOS-only at October 2025 launch. Windows/iOS/Android "coming soon." By April 2026, Atlas was merged into OpenAI's unified desktop "super app" alongside ChatGPT and Codex, effectively making the browser a view within a larger AI shell [FACT - https://quasa.io/media/openai-is-building-a-desktop-super-app-merging-chatgpt-codex-and-atlas-into-one-unified-platform].

**Agentic execution mechanics.** **Automation engine: CUA (Computer-Using Agent)**. CUA combines GPT-4o's vision with reinforcement-learning fine-tuning to interact with GUIs the way a human does — it perceives a rendered screen and issues virtual mouse/keyboard events. It is NOT a CDP/Playwright-style DOM scripting engine [FACT - https://openai.com/index/computer-using-agent/]. The underlying input events are routed "directly to the web page renderer and never pass through the privileged browser layer," preserving Chromium's sandbox integrity even under automated control [FACT - OWL blog].

**Perception method**: Hybrid. CUA primarily uses the accessibility tree (ARIA roles, labels, semantic structure) with selective screenshots for visual grounding. Industry analysis confirms Atlas converged on this approach alongside Microsoft Playwright MCP and Perplexity Comet [FACT - https://www.searchenginejournal.com/the-accessibility-tree-is-how-ai-agents-read-your-site-its-breaking/578171/; https://nohacks.co/blog/agentic-browser-landscape-2026].

**Execution loop**: Plan → execute action → generate structured DOM observation → infer next step → repeat. This multi-round inference loop is inherently slower than script-based automation (Playwright/Selenium) but provides semantic reliability and safety checkpoints [FACT - https://jimmysong.io/blog/chatgpt-atlas-architecture-analysis/].

**Two session modes** — this is the critical architectural fork:
- **Logged-in mode**: Agent runs in the user's real browser session with live cookies. The agent can access sites the user is already authenticated on. Sensitive sites (financial institutions) trigger mandatory pause + user oversight [FACT].
- **Logged-out / ephemeral mode**: Agent runs in an isolated Chromium StoragePartition — a fresh, in-memory context with no cookies and no persistent state. All data is discarded when the session ends. Multiple ephemeral sessions can run simultaneously, fully isolated from each other and from the user's real profile [FACT - OWL blog].

**Credential handling**: The agent CANNOT read saved passwords or autofill data. If a task requires login, the agent pauses and hands control back to the user. During user-controlled credential entry, **screenshots are not captured** [FACT]. Cookies persist normally in logged-in mode; in ephemeral mode they are discarded.

**Hard limits**: No code execution in browser, no file downloads, no extension installation, no filesystem access, no other application access [FACT]. Pages visited in agent mode are excluded from browsing history [FACT].

**Privacy / data architecture.** **Default data flows (what goes to OpenAI's cloud)**:
- As you browse, page content is summarized server-side with safety + PII filters. Raw web content is deleted immediately after summarization. Privacy-filtered summaries are retained for up to 7 days then deleted [FACT - https://help.openai.com/en/articles/12574142 / confirmed via search].
- Diagnostics telemetry ("Help improve browsing & search") is **ON by default**. This shares diagnostic logs including "technical details and publicly known URLs" — not model training, but behavioral telemetry [FACT - OpenAI help docs].
- Browser memories (cross-session context): opt-in only; stores privacy-filtered summaries, not raw content [FACT].

**What stays local by default**: Nothing is explicitly local-only by default except the browsing session state itself.

**On-device option**: macOS 26+ introduced opt-in on-device summarization that keeps content local, avoiding server-side processing [FACT]. This is a late-added privacy escape hatch, not the primary path.

**Model training**: Web browsing content is NOT used for model training by default. Opt-in required. Even if opted in, pages that exclude GPTBot are not trained on [FACT].

**Agent session privacy**: Ephemeral Chromium StoragePartitions discard all cookies and site data when the session ends. Agent-visited pages are excluded from history [FACT - OWL blog]. Screenshots are withheld during credential entry [FACT].

**Incognito mode**: Signs the user out of ChatGPT entirely — the AI assistant is fully disabled in incognito [FACT].

**Privacy risks identified by independent researchers**:
- Continuous behavioral telemetry: "Atlas doesn't just register queries; it observes what you read, how long you stay, and what you do next" [FACT - Proton analysis, https://proton.me/blog/is-chatgpt-atlas-safe]
- Third-party data sharing with OpenAI partners [FACT - privacy policy]
- Prompt injection risk: malicious sites could embed hidden instructions in page content that CUA interprets as commands ("CometJacking" class of attack) [FACT - cited by Proton]
- Inference chains: deleting one memory does not erase what the model has already synthesized from it [FACT - Proton analysis]
- No TEE or secure-enclave documentation found [UNVERIFIED - not publicly documented]
- No end-to-end encryption details for the browser-to-OpenAI data channel published [UNVERIFIED]

**Posture summary**: Cloud-centric by default with opt-in local processing. Privacy controls exist but require user action to activate. The diagnostics-ON default and server-side summarization default create a structural privacy gap that is acknowledged by independent researchers.

**The WHY (strategic + engineering reasoning).** **1. Break distribution dependency [FACT - stated]**: ChatGPT's reach is mediated by browsers OpenAI doesn't own. Google controls Chrome integration, Apple controls Safari defaults, Microsoft bundles Copilot into Edge. Atlas is the structural answer — a distribution surface OpenAI controls completely. As framed by analysts: "The goal is not dethroning Chrome; the goal is ensuring ChatGPT's most engaged users reach the product through a surface OpenAI owns" [https://www.digitalapplied.com/blog/chatgpt-atlas-openai-ai-browser-strategy-guide].

**2. Context richness requires native depth [INFERRED]**: An extension is limited by the extension API surface. To build a "true super-assistant that understands your world," the AI needs page context, multi-tab context, session state, and the ability to act. These requirements collectively demand browser-level integration. A sidebar extension can summarize; a native browser integration can act across tabs and sessions. OpenAI explicitly stated this as the north star: "your browser is where all of your work, tools, and context come together" [OpenAI blog].

**3. OWL architecture rationale [FACT - OpenAI stated]**: The specific choice to run Chromium as a separate process (not embedded) was driven by three engineering requirements: instant startup, responsiveness under heavy tab load, and a stable foundation for agentic use cases. With Chromium as a separate process, its hangs and crashes cannot freeze or kill Atlas — a critical reliability property for an agentic assistant that may be running long tasks.

**4. SwiftUI/AppKit monostack [FACT - OpenAI stated]**: "One language, one tech stack, one clean codebase" — engineering discipline choice to avoid the complexity of mixing native and web UI frameworks. macOS-first allowed them to ship something polished before tackling cross-platform.

**5. Cloud-first model [INFERRED]**: GPT-4o is substantially more capable than any on-device model available in 2025-2026. The cost of cloud inference is offset by the quality gap. On-device summarization was added as a privacy feature late (macOS 26+), not as the primary path — confirming cloud capability was the design priority, privacy was secondary optimization.

**6. CUA visual/accessibility-hybrid [INFERRED]**: A vision-based agent that perceives screens like a human works on any website without requiring site-specific integration, DOM scraping APIs, or cooperative MCP endpoints. This makes the agent general-purpose by default, at the cost of latency (multi-round inference vs script execution).

**7. Ephemeral sessions as trust primitive [INFERRED]**: The StoragePartition isolation design for agent sessions is explicitly architected to build user trust in autonomous browsing. Without isolation, users would rationally refuse to let an AI browse on their behalf. The design makes the risk model legible: the agent gets a clean-room context, not your full identity.

**8. Super-app convergence [FACT]**: By April 2026, internal fragmentation between ChatGPT, Atlas, and Codex was identified as a "code red" — users didn't want three separate OpenAI apps. The merger into one desktop super app validates the thesis that the browser IS the AI platform, not a separate product.

**Lessons for Hodos/Edwin.** **1. OWL pattern validates the Hodos/Edwin sidecar model — with one critical nuance.**
OWL runs Chromium as a separate process controlled by a native app. Edwin as a localhost sidecar follows the same separation principle. The lesson: the native Hodos process (not CEF) should be the intelligence and UX orchestrator; CEF is the rendering engine it commands. Don't let CEF/Chromium own the main thread of the user experience. Hodos already has this right architecturally — Edwin on a localhost port is the OWL Client analog.

**2. Prefer accessibility tree + DOM snapshot over screenshots for Edwin.**
Atlas's CUA uses an accessibility tree + selective vision hybrid. For a local sidecar running on-device, sending screenshots to Edwin's model is expensive and slow. CEF exposes the accessibility tree natively — use it. DOM semantic snapshot + ARIA tree is lower bandwidth, faster to process locally, and doesn't require vision model capability. Screenshots become a fallback for visually complex pages.

**3. Hodos privacy posture should be the mirror image of Atlas's defaults.**
Atlas: server-side summarization by default, diagnostics ON by default, on-device as opt-in. For Hodos's privacy-conscious users, invert every default: local processing first, telemetry OFF by default (opt-in only), cloud as an explicit user choice for enhanced capability. This is not just an ethical choice — it is the product differentiator. Atlas has already claimed the cloud-capable-but-trust-us position; Hodos should own the local-first-verifiable position.

**4. Implement ephemeral CEF StoragePartition for all agent tasks.**
Atlas's isolation design for agent sessions is a non-negotiable trust primitive. Edwin-initiated agent tasks should run in an isolated CEF off-the-record profile (CEF supports this via CefRequestContext with in-memory storage) that discards all cookies and state on task completion. The user's real browsing profile is never exposed to agentic execution. Build this from day one, not as a retrofit.

**5. x402/BSV micropayments as a trust primitive Atlas doesn't have.**
Atlas pauses for sensitive actions and asks the user. Edwin can go further: let users authorize agentic tasks with a BSV budget ("spend up to X satoshis on this task"). This creates verifiable, auditable agent authorization with a cryptographic trail on-chain. No other browser agent offers this. It solves the consent problem for paid web actions (booking, purchasing) in a privacy-preserving way — the payment IS the authorization, not a pop-up prompt.

**6. Pause-for-sensitive-actions is load-bearing infrastructure, not a nice-to-have.**
Atlas's design explicitly pauses agents for login, payment, and financial sites; screenshots are disabled during credential entry. Edwin must implement the same pause gates. Without them, privacy-minded users will not delegate real tasks. Define a threat model (what triggers pause: login forms, payment forms, financial domains) and implement it before shipping agent mode.

**7. Native sidebar, not a webview.**
Atlas building its UI in SwiftUI/AppKit rather than embedding a web UI is the right call. For Hodos (CEF on Windows/Linux), Edwin's sidebar should render as a native panel (Qt or native CEF panel), not an iframe or embedded browser pointed at a local URL. Native rendering = faster, more secure, no same-origin leakage, and a trust signal to users who know what a real native app looks like versus a webview.

**8. Multi-profile support from day one.**
Atlas has a known limitation: only one host profile due to IPC architecture constraints. This was called out as a design pain point. Hodos should explicitly design for multiple user profiles each with independent Edwin context from the start. CEF's CefRequestContext is the mechanism.

**9. Super-app convergence lesson: browser IS the AI shell.**
OpenAI merging Atlas + ChatGPT + Codex into one desktop app by April 2026 validates the Hodos thesis: don't build the browser and the AI assistant as separate products. Hodos IS the AI shell; Edwin is not a plugin, it is the intelligence layer of the product. The browser chrome, the new tab page, the sidebar, and the agent should all be expressions of one coherent system with shared context.

**10. Watch installation size.**
Atlas's multi-process AI runtime pushes the installation to "several gigabytes." For Hodos, Edwin's transition from Node.js gateway to a lean native binary (as noted in the Edwin-Hodos integration memory) is architecturally correct — minimize the AI runtime footprint. Bundle only what runs locally; let cloud calls handle heavy inference. A lean binary also ships faster and feels trustworthy to privacy-conscious users who scrutinize what they install.

**Sources:** <https://openai.com/index/building-chatgpt-atlas/ — OpenAI engineering blog: OWL architecture, Mojo IPC, SwiftUI/AppKit, process isolation (primary source, accessed 2026-06-26)> · <https://blog.bytebytego.com/p/the-architecture-behind-atlas-openais — ByteByteGo technical breakdown of OWL, IPC, rendering pipeline (accessed 2026-06-26)> · <https://jimmysong.io/blog/chatgpt-atlas-architecture-analysis/ — Jimmy Song independent architecture analysis: multi-process map, AI runtime, on-device models, single-profile constraint (accessed 2026-06-26)> · <https://openai.com/index/introducing-chatgpt-atlas/ — OpenAI launch announcement: features, sidebar, new tab, agent mode, memory (primary source — 403 for direct fetch, content confirmed via search)> · <https://openai.com/index/computer-using-agent/ — OpenAI CUA description: GPT-4o vision + RL, GUI interaction (primary source — 403 for direct fetch, content confirmed via search)> · <https://seraphicsecurity.com/learn/ai-browser/openai-atlas-browser-features-pros-cons-security-and-privacy/ — Security analysis: data flows, on-device summarization, StoragePartition agent isolation (accessed 2026-06-26)> · <https://proton.me/blog/is-chatgpt-atlas-safe — Proton privacy analysis: telemetry defaults, inference chains, prompt injection risk, third-party data sharing (accessed 2026-06-26)> · <https://skywork.ai/blog/chatgpt-atlas-agent-mode-automation-explained/ — Agent mode mechanics: logged-in vs logged-out session, credential handling, pause gates (accessed 2026-06-26)> · <https://www.marktechpost.com/2025/10/21/openai-introduces-chatgpt-atlas-a-chromium-based-browser-with-a-built-in-ai-agent/ — Launch coverage: CUA, sidebar, safety constraints (accessed 2026-06-26)> · <https://en.wikipedia.org/wiki/ChatGPT_Atlas — Wikipedia: launch date, platform availability, memory system, super-app merger (accessed 2026-06-26)> · <https://nohacks.co/blog/agentic-browser-landscape-2026 — Comparative agentic browser landscape: Atlas CUA vs CDP vs accessibility tree approaches (accessed 2026-06-26)> · <https://www.searchenginejournal.com/the-accessibility-tree-is-how-ai-agents-read-your-site-its-breaking/578171/ — Accessibility tree as primary agent perception method confirmed for Atlas (accessed 2026-06-26)> · <https://quasa.io/media/openai-is-building-a-desktop-super-app-merging-chatgpt-codex-and-atlas-into-one-unified-platform — Super-app merger announcement (accessed 2026-06-26)> · <https://www.digitalapplied.com/blog/chatgpt-atlas-openai-ai-browser-strategy-guide — Strategic rationale analysis: distribution dependency, distribution surface ownership (accessed 2026-06-26)> · <https://9to5google.com/2025/10/22/chatgpt-atlas-is-yet-another-chromium-based-browser-with-clever-ai-features/ — 9to5Google launch coverage (accessed 2026-06-26)>

### Google Chrome + Gemini (Nano on-device + cloud Gemini Pro/3.x hybrid)

**AI implementation architecture (where model runs, how browser talks to it).** Chrome runs a two-tier hybrid: a local on-device model and cloud-powered premium features, with a sharp architectural boundary between them.

TIER 1 — On-Device: Gemini Nano [FACT]
- Runtime: TFLite/LiteRT via a proprietary ChromeML binary. ChromeML is a closed-source submodule not committed to the public Chromium git tree. By reverse-engineering the binary and inspecting the public ChromeML API, the inference stack uses LiteRT and MediaPipe LLM Inference pipeline. (Sources: island.io/blog/looking-inside-chromiums-on-device-ai-stack, dev.to/jacquesgariepy inside-chromes-edges-silent-4gb-ai-install)
- Model format: weights stored as `weights.bin` (not a standard .tflite file), located in `%LOCALAPPDATA%\Google\Chrome\User Data\OptGuideOnDeviceModel\<version>\` (Windows); `~/Library/Application Support/Google/Chrome/OptGuideOnDeviceModel/<version>/` (macOS). Observed version: 2025.8.8.1141. Model size: ~4,072 MB. [FACT]
- GPU path: ChromeML/MediaPipe handles GPU acceleration; reported backend "GPU (highest quality)" when VRAM is sufficient (>4 GB threshold). [FACT]
- CPU path: Added in Chrome 140 (rolling out 2025). No VRAM requirement, broader device support. (Source: developer.chrome.com/blog/gemini-nano-cpu-support)
- Process architecture: Nano inference runs in a dedicated sandboxed "On-Device Model Service" utility process, separate from both the browser process and renderer processes. OS-level sandbox blocks network access and limits filesystem access while allowing hardware access. If the process crashes, the browser spins up a new utility process. WebNN-based models (using DirectML/CoreML via the WebNN API) run in the GPU process instead. [FACT - Source: island.io]
- Two model sizes: Chrome estimates GPU capability via shader execution at first use and downloads either a 4B or 2B variant accordingly. (Source: developer.chrome.com/docs/ai/understand-built-in-model-management)
- Task-specific adaptations (Summarizer, Writer, etc.) download on-demand on top of the foundation model, tracked in chrome://on-device-internals. System prompts are stored as protobuf files (on_device_model_execution_config.pb). [FACT]
- Gemma 197M: A much smaller expert model deployed in Chrome 148+ to power task-specific APIs (Summarizer, Language Detector, Translator) on lower-spec hardware. (Source: developer.chrome.com/blog/chrome-at-io26)
- Download mechanism: Triggered when any `*.create()` call is made (e.g., `self.ai.languageModel.create()`). Model is shared across all origins; once downloaded it benefits all AI-enabled pages on that machine. Download resumes on reconnection, persists across browser restarts for 30 days. [FACT]
- Hardware requirements: Windows 10+, macOS 13+, Linux (not Android/iOS/ChromeOS non-Plus); ≥22 GB free disk space; >4 GB VRAM (GPU path) or 16 GB RAM / 4+ cores (CPU path). [FACT]
- JavaScript API surface: `self.ai.languageModel.create()` / `.prompt()` / `.promptStreaming()` / `.clone()` / `.destroy()`. Also Summarizer, Translator, Writer, Rewriter, Proofreader, Language Detector APIs. Multimodal inputs (audio, images, video frames) accepted; text-only output. [FACT - Source: developer.chrome.com/docs/ai/prompt-api]

TIER 2 — Cloud: Gemini Pro / Gemini 3.x [FACT]
- AI Mode in the omnibox (Chrome 147+): Every query sent to Google's cloud servers. Does NOT use on-device Nano. Requires Google account; premium features (full AI Mode) tied to AI Pro ($4.99/mo) or AI Ultra ($100/mo) subscriptions. [FACT]
- Gemini sidebar: Powered by cloud Gemini (Gemini 3 as of Jan 2026). Accesses multi-tab context, browser history, Google Connected Apps (Gmail, Calendar, Maps) via Google account — all cloud-processed.
- Auto Browse / agentic features: Cloud Gemini (powered by Gemini 2.5 Pro for Project Mariner; Gemini 3 for Chrome sidebar auto browse). Requires AI Pro or Ultra subscription. [FACT]
- DevTools AI assistance: Cloud Gemini API. [FACT - Source: developer.chrome.com/blog/how-we-introduced-gemini-to-devtools]

HYBRID EXCEPTION — Safe Browsing [FACT]:
- Nano runs locally, reads page content to extract security signals (not raw content), sends condensed signals to Safe Browsing servers. Active only for Enhanced Protection users. Use case: tech support scam detection triggered by signals like keyboard lock API usage. (Source: 9to5google.com/2025/05/08/chrome-enhanced-protection-gemini-nano)

**Integration depth & browser access.** Deeply native — not a bolt-on extension. Gemini Nano is integrated into Chrome's binary infrastructure at the Optimization Guide and Component Updater level. The integration gives the AI access to multiple browser-internal surfaces:

PAGE CONTENT ACCESS:
- Text selection: Highlights trigger a floating "Send to Gemini" button; selected text injected into sidebar context. [FACT]
- Full page DOM (Safe Browsing / scam detection): Nano reads the full page DOM to extract semantic security signals locally. [FACT]
- DevTools integration (cloud Gemini): Reads DOM + computed styles (via JS execution), allowlisted HTTP request/response headers (API keys and session tokens are redacted), source file content (binary files excluded), serialized performance call trees. The JS execution approach was deliberately chosen over full HTML/CSS sending — "dynamically determine what context data is important for the query at hand." [FACT - Source: developer.chrome.com/blog/how-we-introduced-gemini-to-devtools]

CROSS-TAB AND HISTORY ACCESS (cloud Gemini sidebar):
- Sidebar context includes all open tabs — "understands them as a context group." [FACT]
- Browser history accessible to cloud Gemini with user permission. [FACT]
- Google account Connected Apps (Gmail, Calendar, YouTube, Maps, Google Shopping, Google Flights) pulled into sidebar context. [FACT]

AGENTIC SURFACE ACCESS (auto browse / Project Mariner):
- Accessibility tree snapshots used as primary input for element identification. When elements are not in the AT, vision model (Gemini 2.5 Pro) receives screenshots and returns bounding boxes. [FACT - Source: arxiv.org/html/2511.19477v1]
- Google Password Manager integration: Agent can trigger Chrome's native autofill to sign into sites without passwords being exposed to Gemini. "Google Password Manager does not share your passwords with Gemini in Chrome." [FACT - Source: support.google.com/chrome/answer/16821166]

WEB PLATFORM API SURFACE (WebMCP):
- Proposed standard (origin trial Chrome 149-156): Websites register callable tool contracts via `navigator.modelContext`. Agents invoke JSON Schema-defined functions ("checkout", "filter_results", etc.) rather than parsing UI visually. Origin-isolated documents only; `tools` Permissions Policy gates access. [FACT - Source: developer.chrome.com/docs/ai/webmcp, developer.chrome.com/blog/webmcp-epp]

WHAT THE LOCAL NANO MODEL CANNOT ACCESS:
- The Prompt API is web-exposed but sandboxed — web pages call `self.ai.languageModel.create()` and get inference, but the model itself has no access to browser internals (history, other tabs, cookies) unless the calling code fetches that content and passes it in. [FACT]
- Not available in Web Workers; iframes require explicit Permission Policy. [FACT]

**Form-factor mechanics.** Chrome deploys AI across five distinct UI surfaces, each with different rendering architecture:

1. OMNIBOX AI MODE PILL [FACT - launched Chrome 147, 2026]:
- A prominent pill/button in the address bar opens "AI Mode" — a full-page cloud search experience (not a sidebar). Every query goes to Google's servers. Technically this is Chrome's native omnibox implementation triggering a cloud Search API, not using Prompt API or on-device Nano. Generates significant user confusion since Nano is installed locally. (Source: 9to5google.com/2026/02/21/chrome-address-bar-ai-mode, thatprivacyguy.com)

2. GEMINI SIDE PANEL [FACT - Jan 2026, replaced prior floating window]:
- Always-available right-side panel, persistent across tab navigation. Renders via Chrome's native Side Panel architecture (SidePanel API, stable since Chrome 114). The panel renders Google's Gemini web app (cloud-served content) inside a Chrome WebUI/WebContents frame — not a browser extension. Native Chrome UI shell wraps a webview pointing to Gemini's cloud app. (Source: techradar.com/chrome-gemini-side-panel, macrumors.com/2026/01/29)

3. FLOATING IN-PAGE OVERLAY [FACT]:
- Text selection on any webpage reveals a floating "Send to Gemini" button. Injected by Chrome's browser UI layer (not page JavaScript), appears as an overlay above page content. Clicking sends selected text + page context to the Gemini sidebar. Implemented natively in Chrome's selection handling code. [INFERRED architecture from behavior]

4. CHROME DEVTOOLS PANEL [FACT]:
- Gemini AI assistance embedded in Chrome DevTools as a dedicated panel. Uses ReAct (Reasoning + Acting) loop with cloud Gemini. DevTools JS APIs (DOM inspection, network panel data) are the context extraction mechanism. Renders within the standard DevTools iframe framework.

5. BACKGROUND TASK EXECUTION + PROGRESS UI [FACT - auto browse, 2026]:
- Auto browse runs invisibly in the current browser session. Chrome signals active execution via visible scrolling/clicking. Task progress shown in sidebar. High-stakes action gates pause execution and surface confirmation dialogs as native Chrome browser UI (not web overlays). (Source: support.google.com/chrome/answer/16821166)

WHAT IS NOT PRESENT: No injected content scripts into page DOM (Chrome's AI features operate at the browser chrome layer, not injected page scripts, preserving page integrity). No separate companion app or electron window. The sidebar is the primary persistent surface for all AI interactions.

**Agentic execution mechanics.** PROJECT MARINER (DeepMind research agent, cloud-deployed) [FACT]:
- Original form (2024): Chrome extension running in user's active tab, visibly controlling cursor in real-time via the extension APIs.
- Updated form (mid-2025): Moved to cloud VMs. Google runs Mariner in Chrome instances on virtual machines; user describes task via a dashboard, watches live preview, receives results. This removes the local resource constraint but puts execution on Google's infrastructure. (Source: allaboutai.com/ai-agents/project-mariner, techcrunch.com/2024/12/11)
- Model: Gemini 2.5 Pro with multimodal reasoning (text + screenshots + DOM).
- Browser control mechanism: Multi-layer — (a) CDP for low-level tab/DOM/screenshot access; (b) accessibility tree + ARIA hints + DOM text for element identification; (c) vision model for pixel-level analysis of screenshots when DOM alone is insufficient. Action graph engine maps page elements and branching dependencies dynamically.
- Credential handling in cloud VM path: Runs in isolated cloud session, NOT user's real session. User's actual credentials are not in the VM. [FACT - Source: humansecurity.com/ai-agent/google-mariner note this is from HUMAN Security's blocking perspective]

GEMINI AUTO BROWSE IN CHROME SIDEBAR (user-facing, Jan 2026) [FACT]:
- Runs in user's REAL browser session (same Chrome profile). Evidence: can see all open tabs, use Connected Apps data, and trigger Google Password Manager sign-in.
- Browser control: [INFERRED from behavior descriptions] Likely uses CDP internally via Chrome's own browser process (same underlying mechanism as DevTools). Gemini in the cloud receives page context and issues DOM interaction commands that Chrome executes locally.
- Credential integration: Opt-in Permission → Google Password Manager triggers Chrome's native autofill on the user's behalf. Password plaintext is NOT shared with Gemini. The Password Manager autofill API is invoked, not the password itself. (Source: support.google.com/chrome/answer/16821166, blog.google/chrome/gemini-3-auto-browse)
- Confirmation gates for high-stakes actions: Financial transactions, accepting ToS, account creation, sending communications, form submission, data modification, scheduling, and pages with sensitive financial/health data all require explicit user takeover or confirmation dialog. [FACT]
- Scope: Requires AI Pro or AI Ultra subscription ($4.99/mo or $100/mo). US English only at rollout.
- Audit trail: Task history recorded in Gemini Apps Activity (Google account). Chrome marks visited-during-task sites with an action icon.

WEBMCP (2026, emerging standard) [FACT]:
- Websites expose structured tools via `navigator.modelContext` API (JavaScript or HTML form annotation).
- Agents call JSON Schema-defined functions (e.g., `checkout`, `filter_results`) rather than visually parsing or DOM-scraping the page.
- Same-origin policy, HTTPS required, `tools` Permissions Policy gating. Origin trial Chrome 149-156.
- Gemini in Chrome will be the first consumer of WebMCP tools. Partners announced: Expedia, Booking.com, Shopify, Credit Karma, TurboTax, Redfin, Etsy, Instacart, Target. [FACT - Source: developer.chrome.com/blog/chrome-at-io26]

DEVTOOLS AGENT ACCESS (Chrome 149+) [FACT]:
- Agents (Antigravity and 20+ coding agents) get direct access to console logs, network traffic, and accessibility trees via new DevTools APIs. Real-time fix verification and automated debugging capability built into the browser's own dev surface. [Source: developer.chrome.com/blog/chrome-at-io26]

**Privacy / data architecture.** The privacy architecture is sharply split by tier and has a notable gap between marketing narrative and technical reality:

ON-DEVICE GEMINI NANO — GENUINELY LOCAL [FACT]:
- Inference generates zero outbound network traffic; prompt and response live entirely in process memory of the sandboxed "On-Device Model Service" utility process. [FACT - verified by network traffic investigation, Source: dev.to/jacquesgariepy]
- Sandbox blocks all network access from the inference process; limits filesystem access to what is necessary. [FACT - Source: island.io]
- No data sent to Google or third parties during inference. Subsequent use requires no network connection after initial download. [FACT - Google's stated position, consistent with network analysis]
- Telemetry exception: Chrome logs operational events (request made, response received, user reaction) but NOT content of prompts or responses. This telemetry is opt-out via standard Chrome settings. [FACT]
- Safe Browsing exception: For Enhanced Protection users, Nano extracts security signals from pages (not raw content) and sends CONDENSED SIGNALS to Safe Browsing servers. This is the only case where local Nano inference results leave the device. [FACT - Source: 9to5google.com/2025/05/08]
- One deleted privacy claim: Google removed a sentence from Chrome settings stating models run "without sending your data to Google servers." Google confirmed the local architecture hadn't changed but declined to explain the deletion. The claim removal caused documented public concern. [FACT - Source: decrypt.co/367193, progressiverobot.com/2026/05/09]
- Silent download controversy: Chrome downloads ~4 GB model without explicit user consent, triggered by Chrome auto-update. Users can inspect at chrome://on-device-internals and disable, but it's not opt-in. Generated privacy and EU regulatory concern. [FACT - Source: bogleheads.org forum, alphapilot.tech]

CLOUD GEMINI (SIDEBAR, AI MODE, AUTO BROWSE) — CLOUD PROCESSED [FACT]:
- All AI Mode omnibox queries sent to Google's servers. Page content, open tabs, browser history accessible by cloud Gemini with user permission.
- Connected Apps (Gmail, Calendar) content pulled into cloud processing when user enables integrations.
- Auto browse: Gemini (cloud) may receive personal information and Connected Apps data to complete tasks.
- Conversation history stored in Google account (Gemini Apps Activity), reviewable and deletable.
- Users have accepted Google's standard Gemini Apps privacy policy; data handling governed by it.

TEE / SECURE ENCLAVE: No public disclosure of TEE usage for Chrome's AI features. [UNVERIFIED — likely not implemented based on absence of any documentation]

KEY AMBIGUITY: A user seeing "AI Mode" in the omnibox in 2026 with 4 GB of Nano on their disk will reasonably infer queries are local. They are not — AI Mode is entirely cloud. This is the most significant privacy-architecture communication failure in Chrome's AI rollout. [FACT + analysis]

**The WHY (strategic + engineering reasoning).** The engineering and strategic choices form a coherent pattern once examined together:

WHY ON-DEVICE NANO AT ALL?

1. Infrastructure cost offloading [INFERRED with strong support]: At 3 billion+ Chrome installs, even moving low-stakes tasks (summarization, translation, proofreading, form suggestions) to Nano eliminates enormous cloud API spend. Google essentially distributes its CapEx and inference electricity costs to consumer hardware. At scale this is massive. (Source: alphapilot.tech analysis)

2. Real-time security requires local processing [FACT - stated by Google]: "The average malicious site exists for less than 10 minutes, so on-device protection allows us to detect and block attacks that haven't been crawled before." Cloud-based Safe Browsing cannot detect fast-rotating phishing and scam sites in time; local Nano can evaluate the page before the cloud is even queried. (Source: ghacks.net/2025/05/09)

3. Latency for interactive tasks [INFERRED]: Cloud round-trips (even at ~100-200ms) are perceivable in interactive UI contexts (type-ahead suggestions, real-time grammar correction). Nano provides sub-50ms inference for these cases without network dependency.

4. Privacy narrative vs Apple [INFERRED]: Apple Intelligence runs on-device as a privacy differentiator. Google needed a credible local AI story for Chrome to counter Apple's positioning, even though Chrome's most visible AI features (AI Mode, sidebar) are cloud-backed.

WHY CLOUD FOR AGENTIC/PREMIUM FEATURES?

5. Model capability ceiling [FACT - implied by architecture]: Agentic tasks (multi-step web navigation, reasoning over complex pages) require Gemini 2.5 Pro-class models. A 4B-parameter quantized model cannot reliably perform multi-step web tasks. The cloud tier is where the real reasoning happens.

6. Long-running tasks should not block the device [INFERRED]: Project Mariner's shift to cloud VMs in mid-2025 explicitly addresses this — background multi-hour research tasks should not pin the user's machine.

WHY THIS FORM FACTOR (SIDEBAR + OMNIBOX)?

7. Sidebar = persistent context surface [INFERRED]: Unlike tab-based AI tools (which lose context on navigation), a sidebar persists across all tab switches and maintains conversation history. This is the right UX pattern for a browsing assistant.

8. Omnibox = highest-friction, highest-intent surface [INFERRED]: The address bar is where users go to navigate. Capturing intent at that moment (before the user has even visited a page) is more valuable than post-page-load assistance. Google controls this surface natively in Chrome — a structural advantage no extension can replicate.

WHY WEBMCP?

9. Reliability gap of visual parsing [STATED - Chrome engineering]: OpenAI Operator's 2025 shutdown after failing on complex JavaScript flows, CAPTCHAs, and session management demonstrated that vision-only browser agents are brittle. WebMCP converts the web into structured, machine-callable APIs — orders of magnitude more reliable. Google is positioning Chrome as the infrastructure layer that makes the web agent-readable, locking in Chrome as the platform for agentic AI.

WHY THE UTILITY PROCESS ARCHITECTURE?

10. Security consistency [FACT + INFERRED]: Chrome's existing security model is multi-process with renderer sandboxing. Running Nano in a sandboxed utility process follows the exact same pattern as other untrusted-input processors in Chrome (e.g., GPU process, audio service). This is the correct security posture for a component that receives web-origin input — a malicious page calling the Prompt API should not be able to escape the sandbox into the browser process.

WHY SILENT INSTALLATION?

11. Model availability on demand [STATED]: Google wants any page calling `self.ai.languageModel.create()` to work without a wait. Pre-installing ensures the model is available when needed. This is the same reasoning behind Progressive Web App service worker pre-caching. The tradeoff (consuming 4 GB without consent) is a legitimate user objection that Google has not resolved. [ANALYSIS]

**Lessons for Hodos/Edwin.** Concrete architecture takeaways for Hodos/Edwin, a CEF-based, BSV-native, privacy-first browser with a native Node gateway sidecar evolving toward a lean local-process model:

1. THE SIDECAR-AS-UTILITY-PROCESS PATTERN IS VALIDATED [Apply directly]
Chrome's "On-Device Model Service" utility process is the exact architectural analog to Edwin's localhost sidecar. The pattern — separate process, OS-level sandboxed, hardware access permitted, network access blocked during inference, crash-isolated — is the right model. For Hodos: Edwin's sidecar should be a managed subprocess launched by the browser, not a user-installed daemon. Hodos owns the lifecycle (start, stop, restart), just as Chrome manages its utility process. The localhost port boundary provides the same isolation Chrome's IPC provides, with the added benefit of a standard HTTP interface.

2. TWO-TIER MODEL STRATEGY: DON'T FORCE EVERYTHING LOCAL OR EVERYTHING CLOUD [Apply directly]
Chrome's hybrid (Nano for interactive/security tasks, cloud Gemini for reasoning-heavy agentic tasks) reflects a real capability ceiling. For Hodos: Edwin should run a lean local model (Ollama + small model like Gemma 2 2B or Phi-4-mini) for interactive features (page summaries, quick Q&A, real-time signals), and route complex agentic tasks (multi-step research, form completion) to a user-configured cloud API (Anthropic, OpenAI, or x402-gated BSV-paid API). The routing decision should be explicit to the user, not hidden.

3. DO NOT CONFLATE LOCAL AND CLOUD IN THE UI — CHROME'S BIGGEST MISTAKE [Avoid]
Chrome's AI Mode sits in the omnibox next to Nano, and users cannot tell which is local vs cloud. Hodos's privacy-conscious users will be particularly sensitive to this. Design principle: every AI surface must clearly label "Local (Edwin)" vs "Cloud (provider name)". This is a differentiator vs Chrome, not a minor UX detail. Consider a persistent trust indicator (like HTTPS padlock) for local vs cloud inference.

4. CEF GIVES HODOS THE SAME DOM/AT ACCESS CHROME HAS NATIVELY [Apply directly]
Chrome DevTools integration uses JS execution to pull DOM, computed styles, accessibility trees, and network logs. Hodos/Edwin can replicate this entirely via CEF's CefFrame::ExecuteJavaScript and DevTools Protocol message routing (CefDevToolsMessageObserver). Edwin doesn't need to be baked into the browser binary — it can receive a serialized accessibility tree snapshot from Hodos over the localhost port and perform analysis externally. This is architecturally cleaner than Chrome's approach.

5. STRUCTURED TOOLS OVER VISUAL PARSING FOR AGENTIC TASKS [Watch WebMCP, start with AT]
The WebMCP lesson is that visual parsing (screenshots + bounding box detection) is brittle and expensive. For Hodos's early agentic features: use the accessibility tree as the primary grounding surface (same approach Mariner uses), fall back to screenshot+vision only when AT is insufficient. If WebMCP (navigator.modelContext) becomes widely adopted in the web, CEF can implement it as a browser extension or native WebUI feature — Hodos would benefit from the same structured tooling without needing to be Chrome.

6. CREDENTIAL PATTERN: BROWSER OWNS THE CREDENTIALS, NEVER EXPOSE TO SIDECAR [Apply directly]
Chrome's Password Manager integration is the right model — the browser's native credential store triggers autofill, and the password plaintext never reaches Gemini. For Hodos: when Edwin needs to authenticate on behalf of the user for agentic tasks, Hodos should inject credentials via CEF's network interception layer or native autofill; Edwin should never receive credential values. This is especially important for a BSV-native browser where wallet keys are in play — Edwin must never have direct access to the wallet private key. Hodos proxies payment signing, not Edwin.

7. CONFIRMATION GATE PATTERN IS MANDATORY FOR AGENTIC FEATURES [Apply directly]
Chrome requires explicit confirmation for: financial transactions, form submission, account creation, social posting. For Hodos, given the BSV payment context, add: any x402 micropayment authorization, any transaction signing, any BSV wallet action. The confirmation UI must be native browser chrome (not web overlay that could be spoofed by the page). Hodos implements this at the CEF layer.

8. SAFE BROWSING SIGNAL EXTRACTION = GOOD TEMPLATE FOR LOCAL BSV RISK SIGNALS [Novel application]
Chrome's Safe Browsing hybrid (Nano extracts signals locally → sends only signals to cloud) is a clean privacy-preserving pattern. For Hodos: Edwin could analyze page content locally and emit compact risk signals (e.g., "this page requests a BSV payment to an unknown address", "HTTPS certificate mismatch") that are shown to the user or logged locally — without the full page content leaving the device. This is better than either ignoring the risk or sending all page content to cloud.

9. THE SILENT DOWNLOAD BACKLASH IS A BRAND LESSON [Avoid at all costs]
Chrome's silent 4 GB install generated significant hostility from privacy-conscious users (the exact demographic Hodos targets). Edwin's model should be: (a) explicitly opt-in with clear explanation of size and purpose before first download, (b) visible in browser settings with storage usage displayed, (c) easily removable. The transparency is a feature for Hodos's brand, not a burden.

10. MODEL MANAGEMENT IS NON-TRIVIAL — USE EXISTING TOOLING [Engineering decision]
Chrome's Component Updater, hot-swap update mechanism, GPU performance estimation, storage pressure purging — this is significant engineering. For Hodos: do not reinvent this. Use Ollama as the local model runtime (it handles model download, storage, GPU detection, API serving). Edwin calls Ollama's OpenAI-compatible API on localhost. The sidecar's job is orchestration and Hodos integration, not bare-metal inference management. This is the pragmatic path for a small team vs Chrome's hundreds of engineers.

**Sources:** <https://developer.chrome.com/docs/ai/built-in — Built-in AI overview (official, accessed 2026-06-26)> · <https://developer.chrome.com/docs/ai/prompt-api — Prompt API technical spec (official, accessed 2026-06-26)> · <https://developer.chrome.com/docs/ai/understand-built-in-model-management — Model management (official, accessed 2026-06-26)> · <https://developer.chrome.com/docs/ai/webmcp — WebMCP spec (official, accessed 2026-06-26)> · <https://developer.chrome.com/blog/chrome-at-io26 — Chrome I/O 2026 engineering updates (official, accessed 2026-06-26)> · <https://developer.chrome.com/blog/gemini-nano-cpu-support — CPU inference expansion (official, accessed 2026-06-26)> · <https://developer.chrome.com/blog/how-we-introduced-gemini-to-devtools — DevTools Gemini architecture (official, accessed 2026-06-26)> · <https://developer.chrome.com/blog/webmcp-epp — WebMCP early preview (official, accessed 2026-06-26)> · <https://www.island.io/blog/looking-inside-chromiums-on-device-ai-stack — Chromium on-device AI reverse engineering (third-party technical, accessed 2026-06-26)> · <https://dev.to/jacquesgariepy/inside-chromes-edges-silent-4gb-ai-install-a-complete-hands-on-investigation-54g2 — Hands-on investigation of Chrome/Edge AI install (third-party technical, accessed 2026-06-26)> · <https://developers.googleblog.com/blazing-fast-on-device-genai-with-litert-lm/ — LiteRT-LM technical overview (official Google Developers, accessed 2026-06-26)> · <https://blog.google/products-and-platforms/products/chrome/gemini-3-auto-browse/ — Gemini 3 auto browse announcement (official, accessed 2026-06-26)> · <https://support.google.com/chrome/answer/16821166?hl=en — Auto browse user documentation (official, accessed 2026-06-26)> · <https://9to5google.com/2025/05/08/chrome-enhanced-protection-gemini-nano/ — Nano Safe Browsing integration (press, accessed 2026-06-26)> · <https://techcrunch.com/2026/01/28/chrome-takes-on-ai-browsers-with-tighter-gemini-integration-agentic-features-for-autonomous-tasks/ — Chrome agentic feature overview (press, accessed 2026-06-26)> · <https://techcrunch.com/2024/12/11/google-unveils-project-mariner-ai-agents-to-use-the-web-for-you/ — Project Mariner launch (press, accessed 2026-06-26)> · <https://localaimaster.com/blog/google-project-mariner-web-agent-2025 — Mariner technical overview (third-party, accessed 2026-06-26)> · <https://www.progressiverobot.com/2026/05/09/chrome-ai-privacy/ — Chrome AI privacy analysis (third-party, accessed 2026-06-26)> · <https://decrypt.co/367193/chrome-removes-privacy-claim-gemini-nano-google — Deleted privacy claim reporting (press, accessed 2026-06-26)> · <https://9to5google.com/2026/02/21/chrome-address-bar-ai-mode/ — AI Mode in omnibox (press, accessed 2026-06-26)> · <https://www.macrumors.com/2026/01/29/google-chrome-gemini-side-panel-ai-features/ — Side panel launch details (press, accessed 2026-06-26)> · <https://arxiv.org/html/2511.19477v1 — Browser agent architecture: accessibility tree + vision approach (research, accessed 2026-06-26)> · <https://nohacks.co/blog/agentic-browser-landscape-2026 — Agentic browser landscape comparison (third-party, accessed 2026-06-26)> · <https://www.alphapilot.tech/discover/google-chrome-quietly-deploys-4gb-gemini-nano-model-sparking-privacy-and-regulatory-concerns — Silent install regulatory concerns (press, accessed 2026-06-26)> · <https://www.ghacks.net/2025/05/09/scam-protection-google-integrates-local-gemini-ai-into-chrome-browser/ — Scam protection technical detail (press, accessed 2026-06-26)>

### Microsoft Edge + Copilot

**AI implementation architecture (where model runs, how browser talks to it).** DUAL-TRACK: cloud-primary for Copilot assistant + on-device for the Prompt API / Writing Assistance APIs.

CLOUD PATH (main Copilot assistant): The Copilot sidebar and ambient AI features call copilot.microsoft.com endpoints backed by Azure-hosted models. Microsoft does not publicly disclose the exact model for Edge Copilot; the wider Copilot ecosystem uses GPT-4o and family models via Azure OpenAI Service [FACT — from Microsoft 365 Copilot architecture docs]. The browser communicates via HTTPS to the Copilot web service; since the sidebar is itself a WebView2-rendered web app, these are standard web API calls from within a sandboxed renderer process [INFERRED from WebView2 architecture].

LOCAL PATH (Prompt API / Writing Assistance APIs): Edge ships Phi-4-mini (3.5–4B parameter SLM) downloaded and cached inside the browser on first use. As of June 2026, a developer preview of Aion-1.0-Instruct (smaller, faster, open-source on Hugging Face July 2026) is in Edge Canary/Dev, with CPU-fallback inference for devices without capable GPUs [FACT — Microsoft Edge Blog, June 2026]. Both models run via the Windows Copilot Runtime ONNX execution path: NPU-first if available (Snapdragon X Elite), GPU primary for Phi-4-mini today, CPU fallback via Aion. No llama.cpp; ONNX Runtime is the inference engine.

COMPUTER-USE PATH (Copilot Studio agentic): Entirely cloud-hosted. The Computer-Using Agent (CUA) model — choice of OpenAI CUA (GA) or Anthropic Claude Sonnet 4.5/4.6 / Opus 4.6 (experimental, GA May 2026) — runs in Azure. It takes screenshots of a target machine, reasons over the visual state, and emits virtual mouse/keyboard commands. Runs on Windows 365 Cloud PC pools, NOT on the user's local machine [FACT — Copilot Studio computer-use docs, May 2026].

EDGE COPILOT ACTIONS (browser-native, not Copilot Studio): In-browser automation that leverages the user's existing session. The automation model is not publicly documented as CUA; it uses a screenshot + reasoning loop to navigate approved websites within the user's real browser instance [INFERRED from Edge for Business agentic browsing docs, May 2026].

**Integration depth & browser access.** DEEPLY NATIVE — not a bolt-on extension. Copilot is compiled into Edge as a first-class browser component, governed by Group Policy (ADMX) keys baked into the browser binary.

PAGE CONTENT ACCESS: Controlled by EdgeEntraCopilotPageContext (Edge 130+, replacing obsolete CopilotCDPPageContext which was deprecated at Edge 132). When enabled, the browser extracts page text and browsing history and passes it as context to Copilot. Supported document types: public web pages, SharePoint/intranet, Outlook Web App, PDFs. Not supported: Office documents, EU users by default [FACT — EdgeEntraCopilotPageContext policy doc, updated June 25 2026].

DLP INTEGRATION: Microsoft Purview Endpoint DLP policies can block Copilot from accessing page content on protected pages — using the same block signals as Copy/Print/Save actions. This means Edge natively intercepts the AI context pipeline at the DLP enforcement layer [FACT — Copilot in Edge summarization docs].

MULTI-TAB CONTEXT: After retiring Copilot Mode (May 2026), the ambient features can reason across open tabs, browsing history, and past chats. Purview policies can exclude specific content categories from cross-tab reasoning [FACT — Windows Forum Edge for Business report, May 2026].

JAVASCRIPT API SURFACE (on-device): Web pages and extensions running inside Edge can call `LanguageModel.create()` (Prompt API), plus Summarizer, Writer, Rewriter, Language Detector, and Translator APIs. These directly invoke the locally-hosted Phi-4-mini / Aion model with zero cloud round-trip [FACT — Edge Dev Blog May 2025, June 2026].

COPILOT VISION: User explicitly shares a window/screen/tab. The browser captures frames and passes them to the Copilot cloud endpoint. Available for consumer MSA accounts only; blocked for Entra ID commercial profiles [FACT — Copilot Vision support page]. Vision data (inputs, images, page content) is not logged or stored; only Copilot responses are logged for safety monitoring [FACT — Microsoft support doc].

**Form-factor mechanics.** HISTORIC (pre-May 2026): Copilot Mode = a dedicated sidebar panel rendered as a WebView2 web view, not native C++/WinUI. The Copilot UI hosted inside the sidebar is the same web app as copilot.microsoft.com. This sidebar had its own renderer process that ran persistently even when collapsed, contributing to memory overhead.

MAY 2026 AMBIENT SHIFT: Microsoft retired dedicated Copilot Mode (announced May 13, 2026, rollout across desktop/iOS/Android). The sidebar renderer was dissolved. AI is now surfaced at four integration points: (1) address bar — natural language commands like "summarize this page" trigger Copilot inline; (2) right-click context menu — "Copilot" submenu for selection summarization/rewrite; (3) a remaining sidebar with permanent Insights and Compose tabs that dynamically update for the active page; (4) mobile — persistent slide-up panel overlay on iOS/Android, no full-screen mode [FACT — gHacks/WindowsNews May 2026 reporting].

PROCESS ARCHITECTURE: After the shift, the sidebar operates within the same browser UI process rather than a separate renderer, reducing idle RAM by ~18% [FACT — Windows News technical report]. Simple tasks (grammar, tone) run locally via compact on-device models, avoiding cloud round-trips; idle sidebar suspends its language model context [FACT — WindowsNews architectural note].

WINDOWS COPILOT APP (separate product): mscopilot.exe is a thin launcher wrapping a full private copy of Edge rendered via WebView2. RAM consumption 400–800MB active. This is NOT the same as the Edge browser sidebar; it is Edge-in-disguise shipped as a standalone app [FACT — WindowsLatest, Tweaktown April 2026].

LOCAL API SURFACE: The Prompt API and Writing Assistance APIs are exposed as standard JavaScript to any web page or extension running in Edge — same JavaScript environment as any other API, no special browser chrome required [FACT — Edge Dev Blog].

**Agentic execution mechanics.** TWO DISTINCT AGENTIC SYSTEMS coexist, with different isolation models:

SYSTEM A — EDGE COPILOT ACTIONS (in-browser, user session): Copilot Actions / Journeys, available to consumers and business users inside the Edge sidebar. The agent navigates pages within the user's real authenticated browser session, using their existing session cookies and logged-in state. It pauses when it encounters password entry or payment/credit-card fields, handing control back to the user for sensitive credential entry. Multi-step tasks: form filling, reservations, newsletter unsubscribes, multi-page comparisons. No published CDP integration — the mechanism appears to be screenshot-based loop rather than structured DOM access [INFERRED from Edge for Business agentic browsing docs; CDP is not mentioned in any official document]. The Edge for Business preview (launched May 20, 2026) adds IT governance: administrators define approved website allow-lists; Purview audit logging captures all agent actions; DLP/classification labels remain active during agent operation; user sees real-time visual indicators with pause/stop [FACT — Edge for Business agentic browsing, May 2026].

SYSTEM B — COPILOT STUDIO COMPUTER-USE (cloud-hosted, isolated machine): CUA model (OpenAI CUA or Anthropic Claude) receives screenshots of a dedicated Windows machine, emits virtual mouse/keyboard commands. Machine is isolated — a Windows 365 Cloud PC pool managed by Azure/Intune, fully separate from user's personal device. Credentials stored in either Power Platform internal encrypted storage or Azure Key Vault. Access control whitelist prevents actions on non-approved sites/apps (but does not prevent navigation to them). Human supervision: email notification + review gate for potentially harmful instructions [FACT — Copilot Studio computer-use docs, May 2026].

KEY DIFFERENCE: System A runs in the user's live browser session (cheaper, faster, privacy concern). System B runs on an isolated cloud VM (expensive, auditable, enterprise-safe). No hybrid is documented.

**Privacy / data architecture.** LOCAL MODEL PATH (Phi-4-mini / Aion / Writing APIs): Zero cloud transmission. All inference on-device. Model downloaded and cached by Edge, managed/updated by the browser transparently. Data not used to train models. No network dependency post-download [FACT — Edge Dev Blog, InfoWorld analysis].

CLOUD COPILOT PATH: Page text + browsing history extracted by the browser and sent to copilot.microsoft.com (Azure backend) when EdgeEntraCopilotPageContext is enabled. EU users: DISABLED by default (GDPR compliance). Non-EU users: ENABLED by default. Users and IT admins can override via toggle or Group Policy. DLP policies (Purview) can block specific pages from being included even when the global policy allows access [FACT — EdgeEntraCopilotPageContext policy doc, June 2026; Copilot page summarization docs].

COPILOT VISION: User inputs, images, and page content NOT logged or stored server-side. Only Copilot's responses are logged for safety monitoring. Data deleted after session ends. Conversation transcript retained in history but not used for training or personalization [FACT — Microsoft Copilot Vision support page]. No TEE/secure enclave disclosed [UNVERIFIED — not mentioned in any public doc].

MEMORY FEATURE: By default, Copilot Memory ingests signals from Edge browsing activity, Bing searches, and MSN — this was enabled by default for many users without prominent notice, drawing privacy criticism. Users can disable cross-product data sharing in Copilot privacy settings [FACT — WindowsForum privacy thread; Windows News report].

COMMERCIAL/ENTERPRISE DATA PROTECTION: Enterprise users with Microsoft 365 Copilot Business Chat get Commercial Data Protection (EDP) — Copilot chat data stays within the Microsoft 365 compliance boundary, not used for model training, subject to Purview audit [FACT — Microsoft 365 Copilot privacy docs].

NO TEE/LOCAL ENCLAVE: No public documentation of secure enclave or trusted execution environment for any Edge Copilot feature. Cloud path = standard HTTPS to Azure [UNVERIFIED — absence of disclosure, not confirmed absence of feature].

TELEMETRY: Windows Copilot app (WebView2 wrapper) spawns multiple sub-processes (GPU process, network service, Crashpad, PWA Identity Proxy) — a broader attack surface for telemetry vs a pure native binary [INFERRED from process enumeration in WindowsLatest report, April 2026].

**The WHY (strategic + engineering reasoning).** WHY WEB UI (WebView2) FOR COPILOT: Single codebase shared between copilot.microsoft.com web app and the sidebar — ship-velocity advantage; update the AI experience without an OS/browser patch cycle [INFERRED from architecture pattern]. The cost: 400–800MB RAM for the standalone app when it bundles full Edge. Microsoft accepted this tradeoff for development speed.

WHY RETIRE COPILOT MODE (May 2026): Three drivers: (1) Performance — sidebar renderer ran even when collapsed, consuming ~18% extra RAM; ambient injection is on-demand. (2) Product vision — Microsoft's stated strategy is "ambient AI" across all surfaces (M365, Windows, Bing, Edge); a discrete mode is anti-ambient. (3) UX friction — users treated Copilot as a separate thing they had to open; ambient integration normalizes AI as a browser primitive [FACT — ghacks/WindowsNews analysis, stated Microsoft reasoning].

WHY LOCAL PHI-4-MINI IN EDGE: Three reasons stated by Microsoft: (1) Privacy — page data doesn't leave device. (2) Zero per-token cost at browser scale — millions of users running summarization would be a massive Azure bill. (3) Developer ecosystem — exposing a standard JavaScript API (LanguageModel, Summarizer etc.) lets any website use on-device AI, creating an Edge-specific web platform advantage over Chrome [FACT — Edge Dev Blog May 2025 and June 2026].

WHY CLOUD CUA ON ISOLATED VMs: Enterprise requirement — IT cannot audit agents running in an employee's personal browser session; a dedicated Windows 365 Cloud PC gives a clean audit trail, Intune compliance, and zero risk of contaminating personal data. Also: current SLMs can't reliably drive GUIs autonomously for long tasks; the OpenAI CUA / Claude models require cloud compute [FACT — Copilot Studio computer-use docs reasoning].

WHY EDGE ACTIONS IN USER SESSION: Consumer product — isolated VMs cost Copilot Credits; running in the existing session is free and instant. The privacy tradeoff (AI sees your cookies) is acceptable for consumer use where speed matters more than enterprise auditability [INFERRED].

WHY DEEP POLICY/GROUP-POLICY INTEGRATION: Edge's commercial differentiator vs Chrome is enterprise governability; every AI feature needs a policy knob so IT can control it. This drives the Purview/Intune/ADMX depth [INFERRED — consistent with Microsoft's stated Edge-for-Business strategy].

**Lessons for Hodos/Edwin.** 1. DO NOT bundle a separate browser process for Edwin. The Windows Copilot app (mscopilot.exe = full Edge bundled as WebView2) consumes 400–800MB RAM. The Hodos sidecar-on-localhost architecture is correct — keep Edwin as a lean native binary talking to Hodos via local HTTP, not a full browser instance.

2. ADOPT THE AMBIENT SHIFT PATTERN. Edge's May 2026 lesson: a dedicated AI 'mode' creates friction and wastes RAM when idle. Edwin should surface from the address bar, right-click context menu, and keyboard shortcut — not behind a sidebar toggle. Make it instantaneous and then disappear.

3. LAZY-START THE SIDECAR. Edge's 18% RAM saving from making the sidebar on-demand applies directly to Edwin. The sidecar process should start on first invocation, not on browser launch. Bonus: better startup time perception for privacy-minded users who distrust background processes.

4. IMPLEMENT A DUAL-TRACK ROUTING LAYER. Edge uses local Phi-4-mini for cheap/fast/private tasks (summarize, rewrite, detect language) and cloud models for complex reasoning. Edwin should do the same: route simple tasks to a local SLM (llama.cpp/ONNX with Phi or Aion) and gate cloud calls behind an explicit user action + BSV micropayment (x402). This is a natural fit — the payment event IS the user's consent signal for cloud data transmission.

5. MAKE PAGE-CONTEXT ACCESS OPT-IN PER TAB. Edge's EdgeEntraCopilotPageContext is off-by-default in the EU; Microsoft still got privacy criticism for enabling it by default elsewhere. For Hodos's privacy-minded audience: default Edwin's page-content access to OFF on every tab, require a per-tab toggle that resets on navigation. Never silently read page content.

6. MATCH COPILOT VISION'S DATA HANDLING BAR. Microsoft's stated model for Vision: user inputs, images, and page content NOT logged or stored; deleted after session. If Edwin ever relays page content to a remote model (e.g. for GPT-4o quality tasks), it should match this: zero server-side logging of page content, ephemeral, not used for training. Publish this explicitly — privacy-minded users will check.

7. AVOID THE MEMORY OPT-IN MISTAKE. Microsoft quietly enabled cross-product data sharing for Copilot Memory (Edge + Bing + MSN signals) and faced backlash. Edwin must default all persistent memory/context features to OFF. Any long-term memory of browsing activity requires explicit, persistent user consent, not a buried toggle.

8. EXPOSE A LOCAL JS API TO HODOS PAGES. Edge's LanguageModel / Summarizer APIs let any web page use the local SLM with no cloud call. CEF exposes the same V8 environment. Edwin could register a `window.__edwin` or `LanguageModel`-compatible API in Hodos's renderer process, giving BSV-native sites (e.g. 1Sat dapps) a free local AI without a network call. Competitive moat vs Chrome/Edge for BSV-ecosystem developers.

9. FOR AGENTIC ACTIONS: PREFER CDP OVER VISION-BASED CUA. Edge's consumer actions run in the user's live session (risky but fast); the enterprise system uses vision-based CUA on isolated cloud VMs (safe but expensive). For Hodos, a middle path: use Chrome DevTools Protocol locally in a sandboxed Chromium profile (separate from user's main profile) to execute form fills and clicks — no cloud vision model needed, fully private, faster than screenshot loops, and auditable locally.

10. THE ENTERPRISE POLICY LAYER IS NOT YOUR AUDIENCE — BUT THE PRINCIPLE IS. Edge devotes enormous effort to ADMX/Purview/Intune knobs. Hodos's audience is individuals, not IT admins. But the underlying principle (every AI feature has an explicit on/off) is right — surface these as simple toggles in Hodos settings, not buried enterprise policy keys.

**Sources:** <https://www.ghacks.net/2026/05/15/microsoft-edge-retires-copilot-mode-and-integrates-ai-features-across-desktop-and-mobile/ (May 2026, accessed 2026-06-26)> · <https://windowsnews.ai/article/edge-retires-copilot-mode-ai-browsing-moves-into-default-experience.418244 (May 2026, accessed 2026-06-26)> · <https://windowsnews.ai/article/edge-retires-copilot-mode-ai-tab-voice-vision-tools-move-built-into-browser.418198 (May 2026, accessed 2026-06-26)> · <https://www.neowin.net/news/microsoft-is-killing-copilot-mode-in-edge-but-ai-features-arent-going-away/ (May 2026, accessed 2026-06-26)> · <https://blogs.windows.com/msedgedev/2026/06/02/expanding-on-device-ai-in-microsoft-edge-new-models-and-apis-for-the-web/ (June 2026, FACT — primary source)> · <https://blogs.windows.com/msedgedev/2025/05/19/introducing-the-prompt-and-writing-assistance-apis/ (May 2025, FACT — primary source)> · <https://learn.microsoft.com/en-us/deployedge/microsoft-edge-browser-policies/edgeentracopilotpagecontext (updated June 25 2026, FACT — primary source)> · <https://learn.microsoft.com/en-us/deployedge/edge-learnmore-copilot-page-summary-results (FACT — primary source)> · <https://learn.microsoft.com/en-us/microsoft-copilot-studio/computer-use (updated May 27 2026, FACT — primary source)> · <https://support.microsoft.com/en-us/topic/using-copilot-vision-with-microsoft-copilot-3c67686f-fa97-40f6-8a3e-0e45265d425f (FACT — primary source)> · <https://www.infoworld.com/article/4009190/taking-advantage-of-microsoft-edges-built-in-ai.html (technical analysis)> · <https://windowsforum.com/threads/edge-for-business-agentic-browsing-copilot-can-act-under-it-rules.419324/ (May 2026)> · <https://www.windowslatest.com/2026/04/05/new-copilot-for-windows-11-includes-a-full-microsoft-edge-package-uses-more-ram/ (April 2026)> · <https://www.tweaktown.com/news/110898/the-new-copilot-app-for-windows-11-is-really-just-edge-in-disguise/index.html (April 2026)> · <https://www.microsoft.com/en-us/microsoft-copilot/blog/copilot-studio/computer-using-agents-now-deliver-more-secure-ui-automation-at-scale/ (May 2026, FACT — primary Microsoft source)> · <https://windowsforum.com/threads/edge-copilot-actions-and-journeys-agentic-browsing-with-privacy-trade-offs.386068/ (2025)> · <https://siliconangle.com/2025/11/18/copilot-mode-makes-edge-business-enterprise-ready-agentic-browser/ (Nov 2025)> · <https://windowsforum.com/threads/microsoft-copilot-privacy-opt-out-of-cross-product-data-sharing.403441/ (privacy/memory discussion)> · <https://chatforest.com/builders-log/microsoft-build-2026-windows-ai-models-aion-local-inference-builder-guide/ (Build 2026 Aion model)> · <https://www.askvg.com/tip-disable-phi-4-mini-and-new-web-ai-apis-in-microsoft-edge/ (Phi-4-mini in Edge)>

### Brave Leo

**AI implementation architecture (where model runs, how browser talks to it).** **Model Runtime: Cloud-first with local opt-out and TEE path**

[FACT] Brave runs all models on its own AWS infrastructure — free tier models (Llama, Qwen, Gemma) are self-hosted; premium models (Claude Sonnet 4, DeepSeek R1) were previously served via AWS Bedrock but as of June 2025 Brave migrated all third-party models to its own hosting, eliminating Anthropic and other providers as data processors. Source: https://brave.com/blog/automatic-mode-leo/

[FACT] On-device local inference is NOT shipped as of mid-2026. The 2025 roadmap explicitly lists "integrated, pre-configured client-side models" as incomplete. WebLLM (browser-tab WebGPU inference) is under exploration but undeployed. Source: https://brave.com/blog/leo-roadmap-2025-update/

[FACT] BYOM (Bring Your Own Model, GA since August 2024 / Brave 1.69): Users configure any OpenAI-compatible endpoint. Local Ollama example: http://localhost:11434/v1/chat/completions. Remote examples: OpenAI, Grok. Brave is fully bypassed — zero visibility into the traffic. Source: https://brave.com/blog/byom-nightly/ and https://support.brave.app/hc/en-us/articles/34070140231821

[FACT] TEE path (Nightly, November 2025): In partnership with NEAR AI, Leo can route inference through Nvidia Hopper GPU Trusted Execution Environments. Hardware-isolated enclaves process queries with cryptographic attestation — Brave validates a hash chain (model identity + execution code) before delivering responses. Currently limited to DeepSeek V3.1 on Brave Nightly. Performance overhead approaches zero on Hopper architecture. Source: https://brave.com/blog/browser-ai-tee/ and https://www.theregister.com/2025/11/22/brave_leo_trusted_execution_environment/

**Browser-to-model communication flow (cloud path):**
Browser → Brave anonymizing reverse proxy → Brave's hosted LLM backend → streamed response → reverse proxy → browser. Internal browser IPC uses Chromium's Mojo mechanism via ai_chat.mojom (confirmed by PR references in brave-core: https://github.com/brave/brave-core/pull/25876). Protocol between browser and proxy: HTTPS + OpenAI-compatible chat completions format [INFERRED from BYOM's OpenAI compatibility and standard Chromium networking stack].

**Automatic model routing** (launched 2025): Server-side dynamic routing selects the optimal model per query. Currently routes image inputs to vision-capable models; planned expansion to language detection, code, and task-type routing. Source: https://brave.com/blog/automatic-mode-leo/

**Integration depth & browser access.** **Deeply native — compiled into brave-core, not an extension**

[FACT] Leo is compiled directly into Brave's Chromium fork (brave-core), not a browser extension or plugin. This is confirmed by the June 2026 Brave Origin launch: Origin offers a compile-out build that entirely removes Leo's code paths from the binary, and an "upgrade mode" that uses Chromium enterprise group policies to disable it. If Leo were an extension, neither mechanism would be necessary. Source: https://www.privacyguides.org/news/2026/06/07/brave-launches-paid-minimalist-brave-origin-browser/ and https://www.techtimes.com/articles/317922/20260606/brave-origin-browser-launches-60-compile-out-build-removes-leo-tor-wallet.htm

**Browser context Leo can access:**
- Current page content via DOM/accessibility tree extraction [FACT]
- Multiple open tabs (tab context aggregation) [FACT]
- PDFs loaded in the browser [FACT]
- YouTube video transcripts [FACT]
- Images on webpages (multimodal) [FACT]
- Google Docs content [FACT]
- Brave Search integration for real-time web results injected into responses [FACT]
- Browser settings (Leo can change them as an action) [FACT]
Source: https://brave.com/blog/leo-roadmap-2025-update/ and https://brave.com/blog/leo-real-time-results/

[INFERRED] Page context delivery mechanism: The browser process extracts page content from the DOM/accessibility tree (the same tree screen readers use), serializes it as text, and includes it in the payload sent to the model API. This happens in-process within the browser, not via content script injection. The accessibility tree approach is standard for Chromium-native features.

[FACT] Right-click context menu integration for selection-based queries. Omnibox ("Ask Leo" from address bar). Settings at brave://settings/leo-ai. Full-page interface at brave://leo-ai.

**What Leo cannot access in the default mode:**
- Credentials/cookies from the live session (explicitly firewalled in agentic mode)
- Extension data
- Private/incognito windows (unclear, [UNVERIFIED])

**Form-factor mechanics.** **Multi-surface native WebUI — sidebar-primary, full-page secondary**

[FACT] Primary surface: Sidebar panel adjacent to the active web page. Accessed via toolbar Leo icon, sidebar toggle, or keyboard shortcut. The sidebar is Leo's "companion mode" — the user can browse and chat simultaneously.

[FACT] Secondary surface: brave://leo-ai — a full-page dedicated conversation interface using Chromium's internal URL scheme (chrome:// equivalent). Treated as first-party browser UI.

[FACT] Omnibox integration: Typing a question in the address bar surface an "Ask Leo" affordance. Leo answers inline from the address bar context.

[FACT] Context menu: Right-clicking selected text surfaces Leo actions (explain, summarize, rewrite, translate). This is a native browser context menu addition, not a page injection.

[FACT - from GitHub issue refs] The UI is implemented as Chromium WebUI: https://github.com/brave/brave-browser/issues/49738 ("Add Leo AI WebUI feature flag on iOS") confirms WebUI as the rendering mechanism. Chromium WebUI means the interface is built with web technologies (HTML/CSS/JS, React-based) but runs in a privileged first-party context — it is NOT a content page, NOT an extension popup, and NOT an injected overlay. It has access to private Chromium APIs unavailable to web pages.

[INFERRED] The sidebar specifically uses Chromium's Side Panel API — the same infrastructure that powers Chromium's built-in Reading List, Bookmarks panel, and Google Lens sidebar in Chrome. This gives it native chrome-level integration with stable resize/dismiss/pin behaviors.

[FACT] Input focus: As of a 2024 update, the Leo sidebar input field auto-focuses on open. Source: GitHub issue #47796.

**What this is NOT:** Leo is not an injected `<iframe>` or DOM overlay into page content. It does not touch page z-index, does not inject scripts into web content. The sidebar is browser chrome, rendered outside the content area.

**Agentic execution mechanics.** **AI Browsing mode — isolated profile, dual-model safety, automation engine undisclosed**

[FACT] AI Browsing shipped to Brave Nightly in December 2025, expanded to all release channels for early testing in May 2026. It is opt-in and behind a feature flag. Source: https://brave.com/blog/ai-browsing/ and https://www.bleepingcomputer.com/news/artificial-intelligence/brave-browser-starts-testing-agentic-ai-mode-for-automated-tasks/

**Isolation architecture:**
[FACT] All agentic browsing runs in a brand-new, completely isolated browser profile. This profile has:
- Separate cookie jar — no access to user's logged-in sessions
- Separate cache and site data
- No access to browser settings pages
- Blocked from non-HTTPS sites
- Blocked from Chrome Web Store (prevents extension-downloading attacks)
- Blocked from Safe Browsing-flagged sites
Source: https://brave.com/blog/ai-browsing/

**Credential handling:**
[FACT] The agentic profile has no credentials — cookies, login state, saved passwords do not cross profiles. The agent cannot use or exfiltrate the user's real authenticated sessions. This is the primary security boundary.

**Primary agentic model:**
[FACT] Claude Sonnet is used as a key model for agentic tasks, cited for its prompt-injection resistance. Source: https://www.bleepingcomputer.com/news/artificial-intelligence/brave-browser-starts-testing-agentic-ai-mode-for-automated-tasks/

**Alignment checker (dual-model safety):**
[FACT] A second independent AI model ("alignment checker") receives the system prompt, user prompt, and the primary agent's proposed action — but critically does NOT receive raw website content. This firewall prevents a malicious page from poisoning the checker via prompt injection. The checker evaluates whether proposed actions match user intent. Source: https://brave.com/blog/ai-browsing/

**Automation engine:**
[UNVERIFIED] Brave has not publicly documented the specific mechanism (CDP, accessibility tree automation, DOM scripting) used to control browser actions in AI Browsing mode. [INFERRED] Given Leo's deep native integration in brave-core (not an external agent process), it most likely uses Chromium's internal accessibility tree or DevTools Protocol via an in-process binding rather than an external CDP connection. This is architecturally cleaner and avoids the overhead of external process communication.

**Basic (non-agentic) Leo actions:**
[FACT from roadmap] Leo can change browser settings, toggle dark mode, organize tabs, and execute web searches as simple tool calls within the regular chat interface — these run in the user's real session (not isolated) and are scoped to low-risk read/configure actions. Source: https://brave.com/blog/leo-roadmap/

**Privacy / data architecture.** **Layered privacy stack: proxy + zero-retention + blind tokens + self-hosting + TEE**

**Layer 1 — IP anonymization (reverse proxy):**
[FACT] All cloud Leo requests are routed through Brave's anonymizing reverse proxy. Brave's model backends never see the user's IP address. Brave's proxy cannot link the IP to the query content (the content is the request body, proxy strips the IP header). Source: https://brave.com/blog/leo-launch/

**Layer 2 — Zero retention:**
[FACT] Conversations are discarded server-side immediately after response generation. Not persisted, not used for model training. No server-side usage logs tied to identifiers. Chat history is stored locally in the browser only (user-controlled), with a "temporary chat" mode that avoids even local storage. Source: https://support.brave.app/hc/en-us/articles/20958609786637

**Layer 3 — No account for free tier:**
[FACT] No login required for free Leo. No email or identity collected.

**Layer 4 — Unlinkable tokens (premium):**
[FACT] Brave uses a Privacy Pass-based VOPRF (Verifiable Oblivious Pseudorandom Function) scheme via the challenge-bypass-ristretto library. Flow: after purchase, the browser generates random tokens and cryptographically blinds them locally before sending to Brave's Challenge Bypass Server (CBR). The CBR signs the blinded tokens (via blind signing — it cannot see the original token values) and returns a DLEQ batch proof. The browser unblinds the tokens, producing valid credentials that cannot be correlated back to the signing request by the server. When accessing Leo, the browser presents only a token preimage + HMAC binding (no account ID, no email, no payment info). Double-spend prevention: CBR tracks spent tokens per issuer. Cross-service prevention: HMAC is bound to specific merchant+SKU. Source: https://github.com/brave/brave-core/blob/master/docs/premium_account_privacy.md

**Layer 5 — Self-hosted models (as of June 2025):**
[FACT] Brave moved all model hosting in-house, eliminating third-party data processors (Anthropic, etc.). Data does not leave Brave's AWS infrastructure to any external API. Source: https://brave.com/blog/automatic-mode-leo/

**Layer 6 — BYOM (zero Brave involvement):**
[FACT] When using BYOM with a local model, requests go directly from the browser to the local endpoint. Brave is not an intermediary — zero visibility into BYOM traffic. Source: https://brave.com/blog/byom-nightly/

**Layer 7 — TEE with cryptographic attestation (Nightly, Nov 2025):**
[FACT] For the DeepSeek V3.1 model in Nightly, inference runs in NEAR AI's Nvidia Hopper GPU TEEs. Hardware isolation ensures even a compromised OS cannot access inference data. Attestation flow: TEE generates measurement hashes of the loaded model + execution code; Brave's browser validates a cryptographic proof chain before trusting the response. Users see a verified green label confirming the attestation. Source: https://brave.com/blog/browser-ai-tee/ and https://www.privacyguides.org/news/2025/11/20/brave-announces-verifiable-and-transparent-tee-support-in-leo/

**Telemetry:** [FACT per policy] Brave states it does not collect IP addresses or conversation data from Leo. [UNVERIFIED] Independent audit of actual telemetry not found in public sources.

**Agentic privacy:** [FACT] AI Browsing uses an isolated profile with no real session data — effectively an air-gapped browsing context for agent operations.

**The WHY (strategic + engineering reasoning).** **Strategic rationale: privacy is the product, not a feature**

[FACT - stated] Brave's core brand promise since 2016 has been privacy as the product. Adding an AI assistant that harvested conversation data would be brand-destroying. Leo had to be privacy-preserving at the architecture level, not just in policy. The reverse proxy was the minimum viable privacy engineering investment on day one.

**Why reverse proxy rather than on-device first:**
[INFERRED] Building and shipping a local inference runtime (llama.cpp/ONNX/WebGPU) integrated into a Chromium fork is a multi-engineer, multi-quarter engineering investment. The proxy approach achieves the key user-visible privacy property (IP anonymization, no-log) with minimal infrastructure work. Brave could launch Leo quickly and deliver the privacy story they needed without the runtime engineering burden. The proxy also provides the business model (serve cloud models at cost, monetize via premium tier).

**Why self-host all models (June 2025):**
[FACT + INFERRED] Eliminates the "third-party data processor" privacy gap. When using AWS Bedrock or direct Anthropic API, Brave's privacy promise depends on Anthropic's policies. Self-hosting collapses the trust surface to one party: Brave. Likely also provides cost efficiency at scale and full control over zero-retention enforcement.

**Why BYOM:**
[INFERRED] Solves the power user objection ("I don't trust any cloud, even Brave's proxy") without Brave having to build local inference runtime. Offloads model management complexity to the Ollama ecosystem. Also reduces Brave's compute cost for that user segment. Builds goodwill with technical users.

**Why TEE (the "trust but verify" arc):**
[FACT - Brave stated] The explicit rationale is moving from "trust me bro" to cryptographically verifiable privacy. As privacy-aware users become more sophisticated, policy promises are insufficient. TEE attestation provides hardware-enforced proof. The NEAR AI partnership means Brave didn't have to build the TEE infrastructure themselves. Source: https://brave.com/blog/browser-ai-tee/

**Why isolated profile for agentic:**
[INFERRED - safety-obvious] An AI agent with access to the user's real cookies and login state is a catastrophic attack surface: a malicious web page can prompt-inject the agent into exfiltrating sensitive accounts. The isolated profile is the minimal safe design. The alignment checker's firewall from raw page content is the same logic applied to the second model layer.

**Why Chromium WebUI (not extension) for UI:**
[INFERRED] First-party WebUI runs in a privileged context with full Chromium API access, cannot be blocked by extension blockers, and gets native side panel behavior. Extension popups are sandboxed and have to use messaging APIs to access browser state. Building Leo as WebUI gives it the same trust level as Chrome's own Settings page.

**Why on-device not shipped (2+ years in):**
[INFERRED from roadmap state] Small team. Local inference requires hardware detection (CPU/GPU/NPU), model download management (multi-GB), runtime library integration (llama.cpp or WebGPU shaders), fallback handling, and performance testing across a huge device matrix. The proxy approach + BYOM covers the use cases at lower engineering cost. WebLLM (browser-embedded inference) is being explored as a lower-complexity path but has its own limitations (browser sandbox prevents CUDA access, memory constraints).

**Brave Origin signal (June 2026):**
[FACT] The launch of a $60 "compile Leo out" browser confirms Brave recognizes that forcing AI on privacy-maximalist users is a brand risk. Modular architecture = right long-term decision. Source: https://www.ghacks.net/2026/06/05/brave-software-launches-origin-a-paid-bloat-free-version-of-brave-without-crypto-ai-and-rewards-features/

**Lessons for Hodos/Edwin.** **10 concrete architecture takeaways for Hodos/Edwin (CEF browser, native AI sidecar, BSV micropayments, privacy-first)**

**1. Edwin's localhost sidecar IS the better local inference story.**
[FACT] Brave has been trying to ship on-device inference for 2+ years and hasn't. Hodos has it today — Edwin running as a local process has access to the full OS (CUDA, Apple Silicon ANE, llama.cpp) with no browser sandbox constraints. Emphasize this architectural advantage. Do not try to replicate Brave's WebLLM exploration — that path is slower and weaker.

**2. Reverse proxy for any cloud fallback calls — from day one.**
If Hodos ever adds a cloud model option, route ALL requests through a Hodos-controlled anonymizing proxy before they reach any provider. This is the minimum viable privacy architecture. Never expose user IPs to model backends. Implement before launch, not as a retrofit.

**3. OpenAI-compatible endpoint on Edwin's localhost port.**
Brave's BYOM pattern works because Ollama speaks the OpenAI chat completions format. Edwin should expose the same interface (POST /v1/chat/completions) on its sidecar port. This makes Edwin compatible with every OpenAI-compatible tool, gives users familiar configuration UX, and is a well-understood API surface to audit.

**4. BSV x402 micropayments replace unlinkable tokens.**
Brave built a VOPRF blind-token scheme (Privacy Pass + challenge-bypass-ristretto) to decouple payment identity from usage identity for premium features. BSV micropayments via x402 achieve this natively and more elegantly: payments are pseudonymous on-chain with no Hodos account linkage required. Each query can be paid per-use rather than with a subscription credential. This is a genuine architectural advantage — Hodos gets cryptographic payment-usage unlinkability for free from the BSV protocol.

**5. Sidebar as native CEF panel — not page injection.**
Brave's sidebar is Chromium WebUI rendered in the native Side Panel. Edwin's UI should use CEF's equivalent native panel mechanism or an OS-level window docked to the browser — never a content-injected `<iframe>` or overlay. Native panel: first-party trust level, cannot be blocked by ad blockers, stable resize behavior, no z-index battles. This is the correct form factor.

**6. Isolated CEF profile for any agentic tasks.**
When Edwin does autonomous browsing (research, form filling, web scraping), run it in a completely separate CEF browser context with its own storage, cookies, and no shared state with the user's real profile. This is not optional — it is the primary security boundary that prevents prompt injection from compromising real user sessions. Block settings pages, non-HTTPS sites, and store/extension install endpoints in the agentic profile.

**7. Alignment checker pattern for prompt injection resistance.**
For agentic workflows, run a second validator model that receives only the task description + proposed action, NEVER the raw page HTML/content. This limits the blast radius of adversarial web content. The validator being firewalled from page content makes it very hard to simultaneously poison both models.

**8. TEE is the long-term "proof" story — watch NEAR AI + Nvidia Hopper.**
For Hodos's initial local-model story, Edwin eliminates the need for TEE (data never leaves the user's machine). But if Hodos ever offers cloud model options, TEE attestation is the right architectural direction. The NEAR AI + Nvidia Hopper GPU partnership that Brave uses is an accessible path — it does not require building TEE infrastructure from scratch. Monitor for NEAR AI's infrastructure becoming available to smaller browser vendors.

**9. Design Edwin as modular and compile-out clean.**
Brave Origin's June 2026 $60 "Leo-free" browser shows demand for AI-free options exists even among privacy-first users who otherwise love the browser. Hodos should architect Edwin so it can be disabled at the settings level cleanly, without leaving dead code paths. This is good engineering hygiene and also signals respect for user agency — aligned with Hodos's privacy north star.

**10. Do not replicate Brave's "Anthropic dependency then migrate" mistake.**
Brave launched Leo with direct Anthropic API calls, then spent engineering cycles migrating all models to self-hosted to close the privacy gap. For Hodos, design the cloud-optional path correctly from the start: Hodos controls the proxy and the model hosting, or the request goes through Edwin locally. Never design a path where a third-party model provider is a direct data processor for user queries.

**Closest architectural analog for Hodos:** Brave Leo's BYOM mode (OpenAI-compatible local endpoint, zero Brave intermediary) is the closest architectural parallel to Hodos/Edwin. The difference: Brave treats BYOM as an opt-in power-user feature on top of cloud defaults; Hodos treats local-first as the default and cloud as the fallback. Hodos should own that inversion as a differentiator.

**Sources:** <https://brave.com/blog/leo-roadmap-2025-update/ — 2025 Leo development progress and plans (primary technical source)> · <https://brave.com/blog/byom-nightly/ — BYOM technical announcement (OpenAI-compatible endpoint, Ollama integration, privacy architecture)> · <https://brave.com/blog/browser-ai-tee/ — TEE/NEAR AI/Nvidia Hopper implementation (primary)> · <https://www.theregister.com/2025/11/22/brave_leo_trusted_execution_environment/ — The Register TEE coverage (Nov 2025)> · <https://www.privacyguides.org/news/2025/11/20/brave-announces-verifiable-and-transparent-tee-support-in-leo/ — Privacy Guides TEE analysis> · <https://cyberinsider.com/braves-ai-assistant-leo-now-offers-cryptographically-proven-privacy/ — CyberInsider TEE/cryptographic privacy coverage> · <https://brave.com/blog/ai-browsing/ — AI Browsing agentic mode (isolated profile, alignment checker architecture)> · <https://www.bleepingcomputer.com/news/artificial-intelligence/brave-browser-starts-testing-agentic-ai-mode-for-automated-tasks/ — BleepingComputer agentic coverage (Claude Sonnet model confirmed)> · <https://brave.com/blog/automatic-mode-leo/ — Automatic model routing, self-hosted AWS infrastructure, model list> · <https://brave.com/blog/leo-launch/ — Original Leo launch, reverse proxy architecture> · <https://brave.com/blog/leo-roadmap/ — Original roadmap (agentic features, on-device plans, omnibox, WebUI references)> · <https://github.com/brave/brave-core/blob/master/docs/premium_account_privacy.md — VOPRF blind token scheme (Privacy Pass, challenge-bypass-ristretto, DLEQ proof)> · <https://support.brave.app/hc/en-us/articles/34070140231821-How-do-I-use-the-Bring-Your-Own-Model-BYOM-with-Brave-Leo — BYOM setup/config details> · <https://deepwiki.com/brave/brave-browser/1.2.3-brave-leo-ai-assistant — Architectural layer overview> · <https://en.wikipedia.org/wiki/Brave_Leo — Brave Leo Wikipedia (timeline, model history)> · <https://www.privacyguides.org/news/2026/06/07/brave-launches-paid-minimalist-brave-origin-browser/ — Brave Origin compile-out confirmation of Leo's native integration> · <https://www.techtimes.com/articles/317922/20260606/brave-origin-browser-launches-60-compile-out-build-removes-leo-tor-wallet.htm — Brave Origin compile-out vs group-policy architecture detail> · <https://kareemai.com/til/tils/2025-05-23-til.html — BYOM/Ollama localhost endpoint config details> · <https://github.com/brave/brave-core/pull/25876 — brave-core AIChat conversation data storage PR (Mojo IPC, AIChatService architecture reference)> · <https://github.com/brave/brave-browser/issues/49738 — Leo AI WebUI feature flag iOS (confirms WebUI rendering mechanism)> · <https://brave.com/blog/leo-real-time-results/ — Brave Search real-time integration with Leo> · <https://www.ghacks.net/2026/06/05/brave-software-launches-origin-a-paid-bloat-free-version-of-brave-without-crypto-ai-and-rewards-features/ — Brave Origin ghacks coverage>

### Opera Aria + Opera Neon

**AI implementation architecture (where model runs, how browser talks to it).** Opera runs a two-track AI architecture with no single local-inference path in production.

ARIA / OPERA AI (production, Opera One/GX/Air/Neon): Fully cloud-side through Opera's proprietary "Composer" multi-model routing engine [FACT]. Composer dispatches requests to OpenAI GPT-series (the original foundation) and Google Gemini models (added May 2024 via Google Vertex AI; Gemini 3 Pro available to Neon users as of late 2025) [FACT — press.opera.com/2024/05/28, press.opera.com/2025/12/01]. Image generation routes to Google Imagen 3 fast model; voice/TTS to Google's text-to-audio [FACT]. Opera also operates its own green-energy NVIDIA DGX supercomputing cluster in Iceland as a component of the inference stack [FACT — press.opera.com/2024/05/28]. The browser process communicates with the cloud AI over HTTPS; no local inference occurs in this path [FACT — help.opera.com/en/browser-ai-faq]. Opera Deep Research Agent (ODRA) is an additional specialized orchestration layer in Neon for parallel multi-agent research tasks [FACT — press.opera.com/2025/11/27].

LOCAL LLM PATH (experimental, Opera One Developer only): Launched April 2024 as part of "AI Feature Drops." Uses the Ollama framework (backed by llama.cpp) to run 150+ model variants from ~50 families (Llama, Gemma, Mixtral, DeepSeek R1) entirely on-device [FACT — press.opera.com/2024/04/03]. User selects the local model from a dropdown in the sidebar; it replaces Aria for the duration of that chat session. Remains experimental and distinct from the production Aria/Composer path [FACT].

NEON DO vs NEON MAKE split: Neon Do (task execution in the live browser) runs locally — the agent logic is cloud-side but DOM interaction happens in the browser process on-device [FACT — blogs.opera.com/news/2025/05/opera-neon-first-ai-agentic-browser]. Neon Make (generative / out-of-browser tasks: coding, document gen, dependency installation) runs entirely in Opera-hosted European cloud VMs [FACT].

**Integration depth & browser access.** Deeply native, not a bolt-on extension layer.

ARIA / OPERA AI: Integrated into the browser chrome as a first-party feature, not a browser extension. Page content access is opt-in per conversation: when the toggle is enabled at the top of the chat panel, the current tab's visible text (or the full Tab Island group of tabs) is serialized and sent to Opera's AI engine [FACT — help.opera.com/en/browser-ai-faq]. Opera AI does NOT access: general browsing history, cookies, password manager, cross-tab context outside the active Tab Island [FACT]. Sensitive sites (banking, payment processors, personal data handlers) are blocked from page-content sharing [FACT]. The AI can process multiple tabs simultaneously within a Tab Island, giving it multi-tab awareness within that scope [FACT].

BROWSER OPERATOR / NEON DO (agentic layer): Goes deeper. Uses DOM tree + browser layout data via a "textual representation" of the page — not screenshots, not video, not pixel analysis [FACT — blogs.opera.com/news/2025/03/opera-browser-operator-ai-agentics]. This is a native Chromium integration, not an injected content script [INFERRED from "runs natively inside the browser, on your device" and from the stated ability to access non-visible DOM elements]. The agent can interact with elements not visible to the user (cookie banners, hidden overlays, off-screen content) precisely because it reads the DOM directly rather than rendering pixels [FACT]. It operates in the user's live authenticated session, so all session tokens and auth state present in the browser are implicitly available to any page action it takes — though credentials are not extracted and sent to Opera servers [FACT].

MCP CONNECTOR (March 2026, Neon): Exposes browser internals to external AI clients via MCP protocol. Read tools (enabled by default): open tabs list, page content, screenshots, optionally browsing history. Write tools (disabled by default, user must enable): tab switching/closing, mouse clicks, keyboard input, page navigation, form filling, Google search [FACT — blogs.opera.com/news/2026/03/opera-neon-adds-mcp-connector-to-the-browser].

**Form-factor mechanics.** OPERA ONE / GX / AIR: The AI chat surfaces as a native left-sidebar panel within the browser chrome — rendered as part of Opera's proprietary browser shell built on top of Chromium, not as a separate WebView extension or injected overlay [INFERRED from architecture; Opera has long maintained a custom shell]. As of late 2025, "Opera AI" (the successor to "Aria") ships with a redesigned persistent side panel with better tab/context integration; conversations can be dragged out of the sidebar into a standalone browser tab [FACT — blogs.opera.com/news/2025/10/opera-one-upgraded-built-in-ai]. An address-bar shortcut was added in Opera Developer (April 2025) allowing quick queries from the omnibox [FACT — blogs.opera.com/news/2025/04/opera-developer-aria-ai-in-address-bar-ai-feature-drops]. A model-selector dropdown lets users switch between available LLMs mid-conversation while preserving context [FACT — press.opera.com/2025/11/27].

OPERA NEON: A purpose-built, AI-first browser with a redesigned shell. Key UI primitives: (1) Tasks — dedicated full-viewport workspaces that combine browser tabs, AI chat, notes, and context into a single isolated project view; (2) Cards — reusable, combinable prompt-instruction sets (analogous to IFTTT recipes), community-shareable from a Card Store; (3) Floating collapsible left sidebar for tools [FACT — operaneon.com, blogs.opera.com/news/2025/09/opera-neon-agentic-ai-browser-release]. The AI panel is always accessible in Neon's layout. External connections possible via MCP and CLI paths [FACT — operaneon.com]. All UI is Chromium-native browser chrome, not an injected overlay [INFERRED].

**Agentic execution mechanics.** Opera's agentic execution evolved from Browser Operator (Opera One preview, March 2025) into Neon Do (Opera Neon GA, September 2025), with the same core mechanism.

EXECUTION MECHANISM: DOM tree + browser layout data ingestion as a textual representation of the page [FACT]. NOT CDP screenshot-based, NOT computer-use/pixel pointer approach. The internal API used is not publicly disclosed; [INFERRED] it uses privileged Chromium browser/renderer process APIs rather than the public Chrome DevTools Protocol, given claims of speed advantage over screenshot approaches and the "native inside the browser" framing. The agent can read the entire page at once without needing to scroll, and can interact with non-visible elements [FACT].

SESSION MODEL: Runs in the user's real, live browser session — same cookies, same auth state, same tabs [FACT]. No isolated profile, no credential-sharing with cloud. Passwords are not extracted; the agent simply acts on pages where the user is already logged in [FACT — blogs.opera.com/security/2025/10/opera-neon-understanding-agentic-browser-security].

AUTONOMY LEVEL: L3 per MIT AI Agent Index — proposes a plan, user confirms before execution of complex/sensitive actions [FACT — aiagentindex.mit.edu/2025/opera-neon]. Hard pauses required before: transactions, file downloads, form submissions with sensitive data [FACT]. Blacklist prevents agent from accessing banking/payment/high-risk sites [FACT]. Input sanitization + output filtering applied [FACT]. A prompt injection vulnerability (hidden DOM text, CSS opacity:0) was demonstrated in 2025 that could exfiltrate user email [FACT — seraphicsecurity.com].

NEON MAKE (cloud VM path): Out-of-browser generative work (code execution, document generation, installing Python/JS dependencies) runs in Opera-managed European VMs. Described as "like employing a virtual computer on our secure European servers" [FACT — blogs.opera.com/news/2025/05/opera-neon-first-ai-agentic-browser].

MCP CONNECTOR (March 2026): Exposes Browser Operator capabilities to external MCP-compatible AI clients (ChatGPT, Claude, Lovable, OpenClaw, n8n demonstrated at launch). Authentication via OAuth2. A persistent proxy server maintains the MCP connection even when the Neon browser is closed. Write tools (form fill, clicks, navigation) default off; user must explicitly enable per client [FACT — blogs.opera.com/news/2026/03/opera-neon-adds-mcp-connector-to-the-browser].

**Privacy / data architecture.** Hybrid, with different data flows for different feature tiers. No TEE or secure enclave disclosed [UNVERIFIED].

ARIA / OPERA AI CHAT (cloud tier): Page content opt-in; when enabled, DOM text of the current tab or Tab Island is sent from browser → Opera servers → OpenAI and/or Google (routed by Composer). Data retention: Opera encrypts and stores chat history 30 days; OpenAI anonymizes and retains fragments 30 days; Google anonymizes and retains ≤24 hours [FACT — help.opera.com/en/browser-ai-faq]. Opera states no chat content or browsed page content is used to train models [FACT]. Uploaded files auto-deleted after 30 days [FACT]. General browsing history is NOT accessed [FACT].

BROWSER OPERATOR / NEON DO (agentic tier): No screenshots, no keystrokes sent to server. Only the user's natural-language instruction prompt + textual DOM representation of the relevant page travel to Opera's AI engine [FACT — blogs.opera.com/news/2025/03/opera-browser-operator-ai-agentics]. Session credentials are not extracted or transmitted. No cross-site or cross-session data leakage by design [FACT].

INFRASTRUCTURE: Opera's own Iceland green-energy NVIDIA DGX cluster + Google Vertex AI + OpenAI [FACT]. Neon Make VMs: European-hosted [FACT]. General browser telemetry (crash reports, anonymized usage) collected by default, separate from AI features and opt-outable [FACT — Opera forums/privacy policy].

LOCAL LLM PATH: Fully on-device via Ollama/llama.cpp. Zero data leaves the device [FACT — press.opera.com/2024/04/03].

KNOWN VULNERABILITIES: (1) CrossBarking (2024): malicious extension exploited overly permissive API access on Opera subdomains to take screenshots and hijack sessions [FACT — seraphicsecurity.com]. (2) Prompt injection (2025): hidden DOM content (CSS invisible text) caused Neon to exfiltrate user email to attacker server [FACT]. These are architectural attack surfaces inherent in the "native DOM access + cloud LLM" model.

MCP CONNECTOR: OAuth2 auth; proxy server for session persistence. No mention of E2E encryption between MCP client and browser [UNVERIFIED].

**The WHY (strategic + engineering reasoning).** STATED REASONING (from Opera's own publications):

1. DOM tree over screenshots [FACT/stated]: "Faster because the Browser Operator doesn't need to 'see' and understand the screen from its pixels or navigate with a mouse pointer, and can access the whole page at once without needing to scroll through." Also handles invisible DOM elements that screenshot-based approaches miss. Avoids video capture which would be a privacy liability.

2. Real session over cloud VM for "Do" tasks [FACT/stated]: "Your data — like browsing history, logins, and cookies — stays private and local." Cloud VM execution for user-authenticated tasks would require credential sharing. Opera explicitly contrasts this against competitors who "run a version of the browser in the cloud."

3. Neon Make in cloud VMs [FACT/stated]: Tasks "beyond the browser" (code execution, document generation, installing dependencies) require compute environment control that cannot run in a browser process. European servers cited for trust/compliance framing.

INFERRED REASONING:

4. Multi-model Composer engine [INFERRED]: Single-provider lock-in risk mitigation; task-to-model routing optimizes cost vs. capability (e.g., cheap models for simple queries, frontier models for reasoning). Enables Opera to instantly deploy new frontier models (GPT-5.1, Gemini 3 Pro added in 2025) as competitive table-stakes without rebuilding the client. Gives negotiating leverage with providers.

5. Cloud-first for AI reasoning [INFERRED]: Frontier model capability/scale is not achievable on-device; Opera is not an ML company and lacks training infrastructure. The "AI router" positioning is capital-efficient — add model providers rather than train models. Local LLMs kept experimental because quality gap is still significant.

6. Opera Neon as a separate product [INFERRED]: Allows aggressive UX and agentic experimentation (radical UI, no traditional tab bar, Cards/Tasks paradigm) without risking the ~350M user main browser. Neon can break things; Opera One cannot.

7. MCP Connector (March 2026) [INFERRED]: Repositions Neon as AI execution infrastructure, not just a browser — "the runtime for external AI clients." Aligns with MCP adoption momentum (Anthropic, OpenAI, others). Reduces pressure to build all agentic capabilities internally by letting Claude, ChatGPT, and Lovable use Neon's browser control as a platform.

**Lessons for Hodos/Edwin.** 1. DOM-TEXT AUTOMATION OVER COMPUTER-USE: Opera's Browser Operator proves that reading DOM tree + layout data in a textual representation is faster, more reliable, and more privacy-preserving than screenshot/pixel-based computer use. Edwin in CEF should use CEF's JavaScript evaluation bridge or Chromium's internal DOM APIs (via DevTools Protocol locally within the same process) to serialize page text — never screen-capture. This is especially important for Hodos's privacy positioning; sending screenshots to a cloud model is a hard sell.

2. THE SIDECAR-OVER-LOCALHOST PATTERN IS VALIDATED: Opera's MCP Connector (March 2026) is architecturally identical to Hodos's Edwin model: a sidecar (or server) exposes an authenticated endpoint, and the browser connects to it to gain AI capabilities. Opera's choice to use MCP protocol for this layer is worth adopting directly. If Edwin's localhost port speaks MCP natively, it becomes interoperable with Claude Desktop, ChatGPT, and any other MCP-capable host — this is a real force multiplier for a small team.

3. DO/MAKE TRIAGE LOGIC IS ESSENTIAL: Model after Opera's Do/Make split. "Do" = actions in the live browser session using DOM access, no credential sharing. "Make" = heavy generative or out-of-browser work handed off to a cloud backend. Edwin's sidecar should implement the same triage: DOM reads and page-scoped actions stay local (CEF process bridge); heavy synthesis or multi-step generation routes to a cloud model (or a local Ollama instance for privacy users). BSV x402 micropayments are a natural fit here — pay per "Make" call, free for local "Do" actions.

4. REAL SESSION + EXPLICIT HUMAN-IN-THE-LOOP GATES: Opera runs agents in the live user session (not isolated) for usability, but gates every sensitive action (form submit, purchase, file download) on explicit user confirmation. Edwin must implement the same. Do not use an isolated profile by default — users expect the agent to operate where they're already logged in. But every write action must surface a confirmation dialog with a clear description of what will happen.

5. PAGE CONTENT OPT-IN PER CONVERSATION: Opera AI requires an explicit per-chat toggle to share page content. Edwin must follow this model. For Hodos's privacy-conscious audience, do not auto-read DOM on every page load. Make DOM context access a deliberate, session-scoped opt-in with visible state in the UI.

6. SENSITIVE-SITE BLACKLIST IS NECESSARY, NOT OPTIONAL: Opera blacklists banking and payment sites from agent access. Hodos should ship Edwin with a default block-list for financial services, health portals, and email providers, with user-controlled exceptions. This is table-stakes for trust.

7. PROMPT INJECTION DEFENSE — SANITIZE DOM BEFORE SENDING: The 2025 Opera Neon exploit (hidden CSS text, opacity:0 elements exfiltrating user email via injected LLM instructions) is a real production-grade threat. Edwin must strip or filter invisible/hidden DOM elements before passing any page content to the model. Consider a whitelist of visible, semantic elements rather than raw innerHTML.

8. MULTI-PROVIDER ROUTING ENABLES BSV MICROPAYMENT MODEL: Opera's Composer picks between OpenAI and Google per task. Edwin can implement lightweight routing that selects provider based on task type and cost, paying per-call via x402. No upfront subscription or vendor lock-in. Start simple (two providers, hardcoded routing rules), evolve to dynamic routing.

9. LOCAL LLM AS A TRUST ANCHOR FOR POWER USERS: Opera's Ollama/llama.cpp integration validates demand. Hodos should provide a local inference path (Ollama sidecar alongside Edwin) that privacy-maximalist users can configure. Keep it optional/advanced — don't let local model quality ceiling bottleneck the default experience.

10. DO NOT REPLICATE OPERA'S PRIVACY THEATER: Opera markets "your data stays private" while page content flows through OpenAI and Google infrastructure. Privacy-conscious Hodos users will read the fine print. Edwin's local sidecar architecture is a genuine differentiator if the default path keeps inference on-device or pays per-call to vetted providers via x402 (giving users auditability). Be explicit in Hodos's UX about exactly what leaves the device and when.

**Sources:** <https://press.opera.com/2024/05/28/opera-google-cloud-aria-gemini/ — Opera x Google Cloud / Composer + Gemini announcement> · <https://blogs.opera.com/news/2025/05/opera-neon-first-ai-agentic-browser/ — Opera Neon first agentic browser announcement (DOM tree, Do/Make split, privacy claims)> · <https://blogs.opera.com/news/2025/09/opera-neon-agentic-ai-browser-release/ — Neon GA release blog> · <https://blogs.opera.com/news/2025/03/opera-browser-operator-ai-agentics/ — Browser Operator technical blog (DOM approach, no screenshots, real session)> · <https://press.opera.com/2025/03/03/opera-browser-operator-ai-agentics/ — Browser Operator press release> · <https://press.opera.com/2024/04/03/ai-feature-drops-local-llms/ — Local LLM / Ollama support announcement> · <https://blogs.opera.com/news/2024/04/ai-feature-drops-local-llms/ — Local LLMs blog (llama.cpp, model families, Developer-only)> · <https://blogs.opera.com/security/2025/10/opera-neon-understanding-agentic-browser-security/ — Neon security blog (Chromium multi-process, human-in-the-loop, blacklisting)> · <https://blogs.opera.com/news/2026/03/opera-neon-adds-mcp-connector-to-the-browser/ — MCP Connector announcement (OAuth2, read/write tool split, proxy server)> · <https://press.opera.com/2026/03/31/opera-neon-adds-mcp-connector/ — MCP Connector press release> · <https://press.opera.com/2025/11/27/opera-neon-presents-one-minute-deep-research/ — ODRA deep research agent + model selector> · <https://press.opera.com/2025/12/01/opera-new-ai-opera-one-gx-opera-neon-latest-gemini-models-by-google/ — Gemini 3 Pro integration, 20% speed improvement, engine rebuild> · <https://blogs.opera.com/news/2025/10/opera-one-upgraded-built-in-ai/ — Opera AI (successor to Aria) side panel upgrade> · <https://blogs.opera.com/news/2025/04/opera-developer-aria-ai-in-address-bar-ai-feature-drops/ — Address-bar Aria access point> · <https://help.opera.com/en/browser-ai-faq/ — Official Opera AI FAQ (data retention, opt-in model, training policy)> · <https://aiagentindex.mit.edu/2025/opera-neon/ — MIT AI Agent Index: Opera Neon (L3 autonomy, model-agnostic engine, execution architecture)> · <https://seraphicsecurity.com/learn/ai-browser/opera-ai-formerly-aria-key-features-pros-cons-and-security-concerns/ — Seraphic Security: CrossBarking + prompt injection vulnerabilities> · <https://layerxsecurity.com/generative-ai/opera-aria-risks-and-vulnerabilities/ — LayerX: data flows to OpenAI/Google, hidden DOM prompt injection exploit> · <https://www.ghacks.net/2026/04/02/opera-neon-adds-mcp-connector-to-let-external-ai-tools-safely-control-live-browser-sessions/ — gHacks: MCP Connector details> · <https://www.operaneon.com/ — Opera Neon product site (Cards, Tasks, MCP/CLI paths)> · <https://blogs.opera.com/news/2025/07/why-ai-agentic-browsers-will-create-massive-productivity-gains-opera-neon/ — Opera Neon vision piece>

### The Browser Company / Dia

**AI implementation architecture (where model runs, how browser talks to it).** **Cloud-only, multi-provider routing — no local inference.** [FACT] Dia's security page explicitly names three provider groups: Claude (Anthropic, Vertex, AWS), GPT (OpenAI Azure), and Gemini (Vertex). All inference is remote. When a user initiates a request, the page content and question travel through The Browser Company's own servers and are forwarded to whichever provider handles that request type; providers are contractually barred from retaining or training on that data.

The browser process and the AI assistant run as **separate processes**; one source describes the AI sidebar as "a separate process that talks to a hosted backend." [FACT] There is no llama.cpp, ONNX, WebGPU, or Apple Neural Engine path in any public documentation. [UNVERIFIED whether a small on-device model handles routing or short queries, but nothing in public sources confirms this.]

Dia's native shell is a macOS app (macOS 14+, Apple Silicon only as of mid-2026). The company explicitly sunset The Composable Architecture (TCA) and SwiftUI that Arc used, rebuilding for lighter memory footprint, faster startup, and a tighter security boundary between browser and AI. [FACT, from TBC substack + search results]

The **omnibox uses a custom ML classifier** to route keystrokes: URL → navigate; query → Google; question → LLM-only; question needing live data → LLM + web summary. This routing runs client-side or at TBC's edge — it is not disclosed precisely. [INFERRED: likely a small embedding-based classifier or heuristic, not a full LLM, given latency requirements.]

The **evaluation and prompt optimization** pipeline ("Jeba" technique) seeds, mutates, and auto-scores prompts across model variants. A "model behavior" discipline codifies desired LLM outputs; an internal dogfood version of Dia exposes every prompt, tool, context, and parameter for engineers. [FACT, from ZenML LLMOps source]

**Integration depth & browser access.** **Deeply native, not a bolt-on extension.** Dia is the entire browser (Chromium core + native macOS shell), so the AI has privileged access that a Chrome extension cannot match.

**What the AI can read (with user permission):**
- Current tab DOM/page content [FACT]: Dia extracts page text at request time and sends it (or a condensed derivative if the page is too long for the context window) to the model.
- Multiple tabs [FACT]: Users @-mention any open tab in the chat input; up to three tabs can be held in a multi-tab view with cross-tab synthesis. The `@all open tabs` command pulls all open tabs as context.
- Browsing history [FACT]: Opt-in "History" feature — 7 days, processed locally then selectively sent at request time.
- Bookmarks [FACT]: @-mentionable as context.
- Highlighted/selected text [FACT]: In-scope selection wraps the Skill prompt.
- Sensitive sites [FACT]: The assistant is designed to avoid automatically processing sensitive sites; if the user explicitly includes one, it processes it.

**What it cannot do by default:**
- The assistant starts with "no access to other tabs or ability to take write actions" [FACT, security page]. Access is granted progressively per user approval.
- No general DOM automation agent in public builds [FACT]: "Current public builds do not expose a general DOM automation agent capable of open-ended clicking and form submission across arbitrary sites."

**Page content extraction method [INFERRED]:** Likely Chromium's accessibility tree or a serialized text representation of the page DOM extracted via a native browser API — not screenshot-based computer use. This is consistent with the fast, text-only workflow and the absence of any computer-use or vision-based automation mention.

**Prompt injection defense [FACT]:** Untrusted web content is processed through "special tagging protocols" before it reaches the model, to prevent injected instructions from hijacking the assistant.

**Form-factor mechanics.** **Two primary surfaces: the unified omnibox and the right-hand Chat sidebar panel.**

**Omnibox [FACT]:** The address bar is the AI's primary entry point. Typing in it triggers the ML-based router described above. There is no separate AI search box — it is the navigation bar, reinterpreted. This is the deepest possible omnibox integration, handled natively, not via an extension content script.

**Right-hand sidebar Chat panel [FACT]:** A persistent panel pinned to the right of the content area. It displays the ongoing chat, accepts @-mentions of tabs/history/bookmarks, shows Skills slash-command completions, and streams model responses. Sidebar mode was added later (it arrived as part of inheriting Arc's "greatest hits" in November 2025) and co-exists with the original floating chat panel.

**Skills invocation [FACT]:** Typing `/skillname` in either the omnibox or the chat composer opens a completions picker. Built-in Skills include /Analyze, /Copy Edit, /Diagram, /Explain, /PR Description. Users create Custom Skills using natural language; the Skill body is a prompt template that wraps whatever is currently in scope (active tab, selection, @-mentioned tabs).

**Rendering [INFERRED]:** The sidebar panel is almost certainly a native macOS view (NSView/WKWebView or a custom Chromium renderer panel), not an injected in-page overlay. Dia rebuilt its UI layer away from SwiftUI/TCA specifically to gain rendering performance; the sidebar appears as part of the browser chrome, not inside the web content area.

**In-page experience [FACT]:** Dia is explicitly not an agentic executor of DOM actions on the live page. It reads and summarizes; any write actions (e.g., form fill) appear in a confirmation-gated step, not as invisible DOM injection.

**Platform [FACT]:** macOS 14+ on Apple Silicon only as of June 2026. No Windows or Linux build; waitlist only.

**Agentic execution mechanics.** **Dia is explicitly a high-context copilot, not a full autonomous agent — by deliberate design.** [FACT]

**What it does agentically:**
- Skills act as parameterized prompt chains over tab content (read-and-transform, not write-and-act).
- Write actions (e.g., autofill forms, send email drafts, insert text into composition fields) are possible but gated behind explicit user confirmation at each step [FACT, security page: "preventing form insertion without approval" and "irreversible actions without confirmation"].
- Memory: the browser surfaces a "Morning Brief" (post-Atlassian 2026) by pulling from calendar, Slack, and assigned tasks — this implies read-access integrations with SaaS tools via OAuth, not arbitrary DOM crawling.

**What it does NOT do:**
- No open-ended multi-step autonomous browsing [FACT].
- No general CDP/DevTools-driven DOM interaction on arbitrary sites [FACT].
- "Dia has no agent mode. It won't autonomously browse websites or complete multi-step tasks the way Comet or Atlas do." [FACT, from MarkTechPost comparison]

**Automation engine [INFERRED]:** For the limited write actions that ARE gated, Dia most likely uses Chromium's accessibility/automation APIs (the same underlying mechanism as CDP) rather than screenshot-based computer use. The security page's framing ("prevents automatic URL following, form insertion without approval") is consistent with Chromium native form-fill APIs with a confirmation layer on top.

**Credential/session handling [FACT]:** Dia runs in the user's live authenticated browser session. The security implications are acknowledged; the mitigation is explicit confirmation gates and no access granted by default. There is no evidence of an isolated sandbox profile for agent tasks.

**Prompt injection mitigation [FACT]:** "Untrusted web content processed through special tagging protocols." This is the primary agentic-safety mechanism — labeling content provenance so the model distinguishes user instructions from adversarial page instructions.

**Post-Atlassian enterprise direction (2026) [FACT]:** Atlassian Team '26 announced AI agents that can be "assigned work, mentioned in comments, and embedded directly into workflows and automations, with every action logged, auditable, and visible." This suggests a planned evolution toward structured agentic execution within the Atlassian product graph — not arbitrary web browsing.

**Privacy / data architecture.** **Local-first storage, cloud inference, minimal telemetry by design.**

**Local storage [FACT]:** Conversations, history, bookmarks, and files are encrypted and stored locally on the device by default. The Memory feature creates summaries on servers but stores the summaries locally.

**What goes to cloud [FACT]:** When a user initiates an AI request, "the data needed to fulfill your request (such as your question and relevant context)" travels through TBC's servers to the AI partner. Data sent = question + relevant page content (or condensed page content). Not continuously streamed; only at request time.

**Telemetry [FACT]:** TBC collects "certain content data (like questions you ask and answers you receive)" to improve Dia. This data is disassociated from user accounts before server processing and deleted after 30 days. Users can disable this in Settings > Privacy.

**Third-party model providers [FACT]:** Claude (Anthropic, Vertex, AWS), GPT (OpenAI Azure), Gemini (Vertex) are contractually restricted from retaining or using the data to train their own models.

**Chromium telemetry disabled [FACT]:** Dia disables Google Accounts integration, UMA metrics reporting, and Reporting APIs — actively stripping the default Chromium phone-home channels.

**Sync [FACT]:** End-to-end encrypted. TBC states: "our servers cannot read your synced data."

**No TEE or on-device model [UNVERIFIED/FACT]:** No mention of Trusted Execution Environment, secure enclave, or Apple Private Cloud Compute in any public source. The privacy model relies on contractual protections with providers rather than cryptographic guarantees.

**Compliance [FACT]:** SOC 2 Type II audit completed for calendar year 2025, report issued 2026. Enterprise version adds SSO, Chromium MDM support, and admin governance controls.

**Sensitive site heuristics [FACT]:** The assistant avoids automatically processing content from sensitive sites (banking, health, etc.) unless the user explicitly includes them in a request.

**The WHY (strategic + engineering reasoning).** **Why the browser layer?** [FACT, stated by Josh Miller] The browser is the only application that sees the user's entire web activity — all tabs, all SaaS tools, all documents, all communication. No standalone AI app can replicate this context without user copy-pasting. The browser IS the context layer for knowledge work, and 94% of knowledge workers spend half their working time in a browser.

**Why cloud-only, no local inference?** [INFERRED, consistent with stated quality-first philosophy] In 2025 on-device LLMs cannot match GPT-4 / Claude / Gemini on complex summarization, synthesis, and generation tasks. Dia targets mainstream knowledge workers, not privacy enthusiasts — quality matters more than provenance for this audience. Shipping a degraded on-device experience would undermine product credibility.

**Why multi-provider (Claude + GPT + Gemini)?** [INFERRED] Three likely reasons: (1) hedge against any single vendor's rate limits, pricing changes, or outages; (2) route different task types to the best specialized model (e.g., code to GPT-4, long-context synthesis to Claude); (3) negotiating leverage with providers. No public statement on routing logic.

**Why deliberately constrained autonomy?** [FACT + INFERRED] The stated position is "reading, learning, and writing workflows with strong local-first guarantees and explicit control over what information the model sees." [INFERRED] Full autonomous DOM agents (Comet, Atlas) create prompt-injection attack surface, credential leakage risk via CDP, and user trust problems from unexpected actions. Dia traded capability for predictability as a trust-building strategy with mainstream users and later enterprise buyers (Atlassian).

**Why rebuild from scratch vs. Arc?** [FACT] Arc used The Composable Architecture (TCA) and SwiftUI — performant for a power-user browser UI but too heavy for the fast, AI-response latency requirements of Dia. Rewriting allowed them to shed that weight, tighten the security boundary between browser and AI, and optimize for startup time and memory footprint.

**Why Skills-as-prompts instead of code?** [INFERRED] Prompt templates (not code execution) let any user create a Skill without programming knowledge, eliminate an entire class of code-injection security risk, and make the Skills Gallery shareable and remixable across the community. The "code" in "code-as-config" is really just prompt authoring, not executable code running on the client.

**Why Atlassian acquisition?** [FACT] TBC needed distribution and enterprise infrastructure to compete against OpenAI, Perplexity, and Google in the next 12–24 month window Josh Miller identified as decisive. Atlassian brings 300K customers (80% Fortune 500), enterprise sales, compliance infrastructure, and Jira/Confluence/Loom integration that makes the browser context story dramatically more valuable for knowledge workers.

**Lessons for Hodos/Edwin.** **1. The @-mention tab context UX pattern is worth cloning.** Dia's approach — AI has zero tab access by default, user @-mentions specific tabs into a request — is the right privacy-respecting UX. Edwin should adopt this as the model: explicit, per-request context attachment, not ambient continuous monitoring. This is also technically simpler: extract page content at request time via CEF's CefFrame::GetSource() or accessibility APIs, not a continuous background scraper.

**2. Prompt-template Skills are a powerful, low-risk primitive.** Dia proves that a Skill system built entirely on parameterized prompts (not code execution) delivers 80% of the value with near-zero security overhead. Hodos/Edwin could implement `/summarize`, `/compare`, `/extract` as user-editable prompt templates stored locally, invoked via slash command in the omnibox or sidebar. No sandboxed code execution required initially.

**3. Multi-provider routing is the right architecture, but start with one.** Dia routes across Claude/GPT/Gemini — sensible for quality and resilience. Hodos should architect Edwin's sidecar with a model-router abstraction from day one (provider-agnostic API call) even if launch uses only Claude or a local Ollama instance. This is the key difference: Hodos can offer local-first inference (Ollama/llama.cpp on the localhost sidecar) for privacy-sensitive users, which Dia explicitly does not do. That IS the differentiation Hodos should exploit.

**4. "Start with no access" as the permission primitive.** Dia's security architecture — assistant has no tab access or write capability until explicitly granted — is the right zero-trust default. Edwin should inherit Chromium/CEF's process isolation and expose a minimal API surface: read a tab's text content only when the user initiates a request. No background CDP listener, no continuous DOM watch.

**5. Disable Chromium telemetry channels explicitly.** Dia disables UMA, Google Accounts integration, and Reporting APIs. CEF-based Hodos should audit every Chromium phone-home channel and disable them in the CEF settings (CefSettings). This is a concrete, auditable privacy claim, not a marketing one.

**6. Prompt injection tagging is non-negotiable.** Dia's "special tagging for untrusted web content" is the right mitigation for the scenario where an adversarial page tries to hijack Edwin. In practice: wrap all page-extracted content in a clearly labeled `<page_content>` block in the system prompt, and instruct the model that instructions inside that block are data, not commands. This is a day-one requirement, not a v2 feature.

**7. Avoid autonomous agent mode until trust is built.** Dia's deliberate choice to be a copilot (not an autonomous executor) reduced attack surface and accelerated enterprise trust. For Hodos' privacy-conscious users, autonomous write actions (filling forms, clicking buttons in the live session with real credentials) require extreme care. If Hodos/Edwin does implement write actions, the Dia confirmation-gate model (explicit approval before each action, explicit confirmation before any irreversible step) is the minimum viable safety pattern.

**8. The omnibox-as-AI-router is the deepest possible integration point.** Because Hodos is a full browser (not an extension), it can, like Dia, make the address bar the primary AI entry point — routing naturally between URL navigation, web search, and Edwin. This is architecturally unavailable to browser extensions and is Hodos' structural advantage. Implementing an ML-based (or even heuristic-based) classifier that detects natural language questions vs. URLs vs. search queries in the omnibox input is a high-leverage early investment. For x402/BSV context: Edwin could also route micropayment-gated content discovery through this same surface, making the AI the natural mediator for paid web content.

**Sources:** <https://www.diabrowser.com/security> · <https://www.diabrowser.com/privacy> · <https://www.diabrowser.com/skills> · <https://techcrunch.com/2025/06/11/the-browser-company-launches-its-ai-first-browser-dia-in-beta/> · <https://techcrunch.com/2025/11/03/dias-ai-browser-starts-adding-arcs-greatest-hits-to-its-feature-set/> · <https://browsercompany.substack.com/p/the-strategy-behind-dias-design> · <https://www.zenml.io/llmops-database/building-an-ai-native-browser-with-integrated-llm-tools-and-evaluation-systems> · <https://www.marktechpost.com/2025/11/15/comparing-the-top-4-agentic-ai-browsers-in-2025-atlas-vs-copilot-mode-vs-dia-vs-comet/> · <https://github.com/aaronjmars/opendia> · <https://seraphicsecurity.com/learn/ai-browser/what-is-dia-browser-pro-cons-security-and-how-to-get-started/> · <https://www.atlassian.com/blog/announcements/atlassian-acquires-the-browser-company> · <https://usetandem.ai/blog/agentic-browsers-atlas-comet-dia-webmcp> · <https://nohacks.co/blog/agentic-browser-landscape-2026> · <https://blog.planetargon.com/blog/entries/solving-workflow-chaos-with-dia-browser> · <https://nand-research.com/atlassian-team-26-context-as-infrastructure/> · <https://www.primeproductiv4.com/apps-tools/dia-browser-review> · <https://www.ghacks.net/2025/06/12/dia-browser-beta-launched-with-ai-features/>

### Mozilla Firefox

**AI implementation architecture (where model runs, how browser talks to it).** Firefox runs a two-track AI architecture with a hard architectural split between cloud and local inference.

TRACK 1 — CLOUD CHATBOT SIDEBAR [FACT]:
The sidebar AI chatbot (shipped Firefox 130/133) connects to third-party cloud providers: Anthropic Claude, ChatGPT, Google Gemini, Le Chat Mistral, Microsoft Copilot (added Firefox 143). There is no local model for this track. The sidebar loads the provider's own web interface inside a sandboxed XUL browser element — architecturally, it is a panel containing the provider's website. Mozilla's own hosted solution "Orbit" (Mistral 7B on GCP, privacy-preserving, no account required) was shut down exactly June 26, 2025 as the sidebar rendered it redundant.

TRACK 2 — LOCAL INFERENCE RUNTIME [FACT]:
Firefox ships an on-device ML runtime ("Firefox AI Runtime") based on Transformers.js + ONNX Runtime. The original runtime used onnxruntime-web (WASM); in 2025 Mozilla vendored the ONNX Runtime C++ directly into Firefox, wrapped via a thin WebIDL layer. This is a native shared library fetched as a Taskcluster CI artifact, not distributed in the main Firefox binary. Results: 2–10× faster inference with zero WASM warmup overhead; PDF alt-text latency dropped from 3.5s → 350ms.

SIX BACKENDS ARE REGISTERED [FACT, from source docs]:
1. onnx (wasm) — DOM Workers, via Remote Settings ml-onnx-runtime collection
2. onnx-native — pre-compiled C++ shared lib, CI artifact
3. wllama — WebAssembly bindings for llama.cpp (shipped Firefox 142, July 2025)
4. llama.cpp — native C++ vendored in third_party/llama.cpp/
5. openai — API-compatible remote endpoint passthrough
6. static-embeddings — single-threaded embedding-only backend

WHICH MODELS RUN LOCALLY [FACT]:
- Smart Tab Grouping title suggestions: flan-t5-base → distilled to t5-efficient-tiny (57MB, INT8 quantized) via ONNX
- Smart Tab Grouping tab suggestions: MiniLM embedding model + logistic regression
- Link Preview AI summary: SmolLM2-360M (369MB) via wllama/llama.cpp
- PDF.js alt text generation: local ONNX vision-language model
- Page translation: Bergamot (Marian NMT C++ compiled to WASM, separate from the main ML runtime, with language-pair model files ~15MB each)

GPU STATUS [FACT]: Currently CPU-only. The source docs explicitly state GPU support requires "additional sandboxing to safely and securely interact with the underlying hardware." WebGPU is available in Firefox but not yet wired into the inference process due to sandboxing constraints. WebNN is on the roadmap (W3C CR status Jan 2026) [VISION].

PROCESS COMMUNICATION [FACT]:
- Models: HuggingFace CDN (restricted to Mozilla and Xenova orgs) → parent process download → OPFS cache (mlRuntimeFiles/) → transferred to inference child process
- Runtime binaries: Remote Settings CDN → OPFS cache
- IPC: JSActor IPC between parent (MLEngineParent) and inference process (MLEngineChild), which runs a SpiderMonkey JS engine and ChromeWorker threads
- Model metadata (ETag, size, revision) tracked in IndexedDB "modelFiles" database for freshness validation without re-download

**Integration depth & browser access.** CLOUD CHATBOT (shallow integration) [FACT]:
Firefox injects into the AI only: selected text + page title (when user selects text and prompts), or full page content + page title (when "Summarize Page" is triggered). Firefox itself explicitly states it has "no access to conversations or any information about webpages." No history, cookies, bookmarks, multiple tabs, DOM tree, or accessibility tree are exposed to the cloud provider. The sidebar loads the provider's web app, so the provider sees only what its own site would normally receive.

LOCAL INFERENCE FEATURES (deeper but task-scoped) [FACT]:
- Tab Grouping: receives tab titles, URLs, and group names — no DOM content, no cookies
- Link Preview: performs a credentialless HTTPS fetch of the linked URL (no cookies sent), parses HTML without executing scripts, extracts Open Graph tags + Reader View article content + reading time. The x-firefox-ai custom header is sent to the target server allowing opt-out.
- Translation: full page DOM content parsed locally
- Alt text: image pixel data from PDFs, processed locally

BROWSER INTERNALS NOT EXPOSED [FACT]:
There is no API surface giving any AI feature access to: browsing history, saved passwords, cookie store, arbitrary tab DOM, or the accessibility tree of active pages. Page content reaches the AI only through explicit user action (summarize, select text) or through scoped fetches.

EXTENSIONS API SURFACE [FACT]:
Firefox 142+ exposes browser.trial.ml (experimental) to extensions with the optional trialML permission. 21 supported task types. Models restricted to Xenova/Mozilla HuggingFace orgs. Multiple simultaneous engines not permitted. The API surface is inference-in, results-out with no browser access primitives attached.

**Form-factor mechanics.** CLOUD CHATBOT SIDEBAR [FACT + INFERRED]:
The chatbot sidebar is rendered using Firefox's existing sidebar mechanism — a XUL `browser` element with id="sidebar", configured with disablehistory="true", autoscroll="false". [FACT: the sidebar panel mechanism exists and is used for AI; INFERRED: the AI chatbot uses the same panel infrastructure, as no separate rendering path is documented publicly]. The provider's full web application loads inside this element. This is NOT a native UI — it is a sandboxed web content frame loading e.g. claude.ai or chatgpt.com. The user switches providers via a dropdown in the sidebar header (native XUL UI element).

LOCAL FEATURE UI [FACT]:
- Tab Grouping: native browser chrome UI, the AI runs silently and populates the suggestion. The dialog to create/name a group is standard Firefox native UI.
- Link Preview: triggered via context menu (right-click > Preview Link) or hover; displays as a card overlay. The "Want to use an AI summary?" consent popup appears before first model download.
- Translation: the existing Firefox translation bar (native UI) handles the UX; AI runs beneath it.
- Alt text: invisible to user, runs as part of PDF.js rendering pipeline.

AI WINDOW (2026 experimental) [VISION]:
Mozilla opened a waitlist in 2025-2026 for an "AI Window" — described as "a new, intelligent space" that persists as a sidebar or dedicated pane. Technical rendering details are not publicly documented; it likely builds on the same sidebar XUL panel mechanism. Functionally described as combining the chatbot interaction with local context awareness.

ABOUT:INFERENCE DEBUG UI [FACT]:
Firefox Nightly exposes about:inference as a developer UI for inspecting the local inference runtime, model state, and performance. Not exposed in release builds.

**Agentic execution mechanics.** Firefox is largely NON-AGENTIC as of June 2026 [FACT].

WHAT IT DOES NOT DO:
- No CDP/WebDriver automation of user sessions
- No form-filling, click, or navigation automation by AI
- No computer-use or screenshot-based action loop
- No multi-step task execution in the user's live browser profile
- The cloud chatbot sidebar can answer questions about a page but cannot interact with it

WHAT EXISTS:
- "Firefox DevTools MCP" [FACT]: A Model Context Protocol server for AI agents to interact with Firefox during DEVELOPMENT (testing, debugging). This is a developer tool (agents testing the browser), not a consumer agentic browsing feature.
- Extensions with browser.trial.ml can run inference on inputs they collect themselves, but the ML API has no browser-automation primitives bundled — extensions must use existing WebExtensions APIs separately.

CREDENTIALS/SANDBOX [FACT]:
- Local inference runs in an isolated process with minimal privileges (separate from content and parent processes)
- The link preview credentialless fetch explicitly drops all cookies/credentials before fetching linked URLs — a deliberate privacy boundary
- No separate isolation profile for agentic tasks because there are no agentic tasks in consumer Firefox

FUTURE DIRECTION [VISION]:
Mozilla's partnership with Mila (announced 2025) explicitly mentions "private memory architectures for AI agents" as a research focus. The "AI Window" described for 2026 suggests more contextual awareness. Whether this extends to autonomous action is not technically documented.

CONTRAST: Firefox is meaningfully behind Chromium-based competitors (Arc, Perplexity Comet, Opera) in agentic capability. This appears deliberate — Mozilla's stated philosophy is user-controlled AI, not ambient autonomous AI.

**Privacy / data architecture.** STRONG LOCAL PRIVACY (verified features) [FACT]:
- Smart Tab Grouping: 100% on-device. No tab data sent to Mozilla or any server. Training used synthetic GPT-4 data and Common Crawl, not user browsing history. Models open-sourced on HuggingFace and GitHub (mozilla/smart-tab-grouping).
- Page Translation (Bergamot): 100% on-device. Page content never leaves the machine.
- PDF Alt Text: 100% on-device. Image pixels processed locally.
- Link Preview (SmolLM2-360M via wllama): Credentialless fetch of linked URL (no cookies), local inference. Sends x-firefox-ai header to the target server. No inference data leaves the machine. 369MB model downloaded on opt-in only.

CLOUD CHATBOT (weak by design) [FACT]:
- Selected text, page titles, and page content (for Summarize) go to the chosen third-party provider
- The provider's privacy policy applies — Mozilla disclaims responsibility
- Firefox collects interaction telemetry: which provider is selected, how often suggested prompts are used. Conversation content is NOT collected by Firefox.

TELEMETRY CONTROVERSY [FACT]:
Firefox 138 (2025) temporarily required telemetry to be enabled in order to access Firefox Labs (where experimental AI features live). Community backlash was immediate. Mozilla reversed this within days (bug 1972647), confirming Firefox Labs will not require telemetry. This reveals organizational tension between data collection needs and the privacy-first brand.

AI KILL SWITCH [FACT]:
Firefox 148 (Feb 24, 2026) added a master "Block AI enhancements" toggle that disables all AI features — both local inference and cloud chatbot sidebar. Individual per-feature toggles also exist under a dedicated AI Controls page.

NO TEE / SECURE ENCLAVE [FACT from absence]:
No Trusted Execution Environment is mentioned in any Firefox AI documentation. Local inference runs in an unprivileged inference process, not hardware-isolated. This is appropriate for the task (protecting data from network, not from local compromise).

MODEL DOWNLOAD SECURITY [FACT]:
Models are restricted to Mozilla and Xenova organizations on HuggingFace. This is an allowlist enforced at the API level, not cryptographic signing. Model files cached in OPFS, metadata (ETag/size) in IndexedDB. No mention of model hash verification in public docs [UNVERIFIED — may exist in source but not documented publicly].

**The WHY (strategic + engineering reasoning).** WHY LOCAL FOR SMALL SPECIALIZED TASKS [FACT + INFERRED]:
Mozilla's engineering blog explicitly states: "small, specialized models work well on-device, but larger language models still need server-side compute." The specific tasks chosen for local execution (translation, tab naming, alt text, link preview summary) all have two properties: (a) they benefit from strict privacy guarantees (no page content leaves device), and (b) they are narrow enough for a small fine-tuned model (57MB–370MB) to handle adequately. [INFERRED: the economics also matter — no per-request inference cost for features that run on millions of tabs daily.]

WHY CLOUD FOR CHATBOT [FACT]:
Large frontier models (GPT-4, Claude, Gemini) cannot run locally in a browser. Rather than building and hosting their own cloud LLM (Orbit proved expensive to maintain), Mozilla lets users choose their provider and lets providers bear the compute cost. Mozilla avoids picking winners commercially. Orbit's shutdown on June 26, 2025 confirms this reasoning — once the sidebar approach existed, maintaining a hosted Mistral 7B was redundant overhead.

WHY NATIVE ONNX OVER WASM [FACT]:
The WASM limitation is fundamental: "WASM SIMD is great, but it can't beat hardware-specific instructions such as NEON on Apple Silicon or AVX-512 on modern Intel chips." The 2–10x speedup from native ONNX made features like PDF alt text practical (3.5s → 350ms). The engineering investment (vendoring C++, CI build artifacts, WebIDL wrapper) was justified by this multiplier.

WHY SEPARATE INFERENCE PROCESS [FACT]:
Mirrors Firefox's existing multi-process architecture (e10s). Isolation prevents inference crashes from killing the browser, contains ML runtime exploits, enables OS-level resource management (the process can be killed when idle). SpiderMonkey in the inference process enables running Transformers.js without modifications.

WHY WLLAMA FOR LINK PREVIEW (NOT ONNX) [INFERRED]:
SmolLM2-360M is a generative LLM requiring autoregressive token generation — ONNX is better suited for encoder-only or seq2seq tasks with fixed-length outputs (T5 for tab titles). GGUF-quantized models via llama.cpp are the idiomatic path for generative LLMs. Firefox exposed both ONNX and wllama as backends precisely to handle this split.

WHY OPEN SOURCE MODELS ONLY [FACT]:
Mozilla's open source AI strategy paper frames open models as "a more accessible, transparent and secure form of technology." Commercially, it avoids licensing entanglements. The allowlist to Xenova/Mozilla HuggingFace orgs is a security measure but also enforces this principle — only models Mozilla has vetted or Mozilla's own community has published.

WHY NO GPU YET [FACT]:
The inference process sandbox was not designed for GPU access. Adding GPU requires new IPC privilege grants and sandboxing rules — the same class of work that took years for Firefox's GPU process (for WebGL/WebGPU). The team explicitly acknowledges this is the blocker, not lack of desire. [INFERRED: Mozilla is waiting for WebNN to mature as a cleaner GPU acceleration path, rather than building their own CUDA/Metal bindings.]

WHY PRIVACY-FIRST BRAND DRIVES ARCHITECTURE [INFERRED]:
Firefox's user base is disproportionately privacy-conscious. The telemetry controversy (Firefox 138) caused outsized backlash vs. what a Chrome user base would produce. Every architectural decision — local inference, opt-in model downloads, no conversation logging, the kill switch — is simultaneously a genuine engineering preference and a brand necessity for retaining this audience.

**Lessons for Hodos/Edwin.** 1. THE SIDECAR ADVANTAGE OVER FIREFOX'S APPROACH [INFERRED]:
Firefox runs inference inside the browser sandbox (inference process = sandboxed content process subtype). This is why GPU is blocked — the sandbox cannot safely talk to GPU hardware. Hodos's Edwin sidecar is an OS-level process with no browser sandbox constraints, giving it access to GPU (CUDA/Metal/Vulkan) immediately. This is a concrete architectural advantage: Edwin can run quantized 7B+ models with GPU acceleration while Firefox is limited to CPU-only local models. Do not replicate Firefox's in-process design.

2. THE DUAL-TRACK SPLIT IS CORRECT [FACT-DERIVED]:
Firefox proves the right split: local for small specialized tasks (tab naming, translation, link preview), cloud for frontier LLMs. Hodos/Edwin should implement the same split explicitly: Edwin handles small scoped tasks (tab summaries, page digests, privacy-sensitive operations) locally; the x402 micropayment layer routes to cloud APIs for heavy lifting. The BSV x402 mechanism makes the cloud side financially self-sustaining in a way Firefox's sidebar (relying on free provider tiers) does not.

3. ONNX FOR CLASSIFICATION/EMBEDDING, LLAMA.CPP FOR GENERATION [FACT-DERIVED]:
Firefox uses ONNX for MiniLM embeddings and T5 seq2seq tasks, wllama/llama.cpp for SmolLM2-360M generative tasks. Edwin's model hosting layer should follow the same convention: use ONNX Runtime (native, not WASM) for embedding/classification pipelines, llama.cpp (via node-llama-cpp or llamafile) for text generation. Do not use WASM inference — the 2–10x native penalty matters at the user latency level.

4. KNOWLEDGE DISTILLATION MAKES LOCAL VIABLE [FACT-DERIVED]:
Mozilla's flan-t5-base → t5-efficient-tiny reduction (1GB → 57MB, INT8, modest accuracy loss) is the right template for any task-specific local model Hodos deploys. Fine-tuned small models beat general models for narrow tasks. If Edwin needs to do something like tab naming or page classification on-device, commission or fine-tune a distilled model rather than shipping a 7B parameter general model.

5. OPFS + INDEXEDDB CACHE PATTERN [FACT]:
Firefox uses OPFS for binary model files (raw bytes, fast random access) and IndexedDB for metadata (ETag, size, revision). In Hodos's CEF context the equivalent is: filesystem for model weights (a dedicated directory under the Edwin sidecar's data dir), SQLite for metadata tracking. Same separation of concerns, simpler implementation since Hodos controls the OS process.

6. OPT-IN MODEL DOWNLOAD WITH EXPLICIT USER CONSENT IS THE CORRECT UX [FACT]:
Firefox shows "Want to use an AI summary? OK/Cancel" before downloading the 369MB SmolLM2 model. This pattern — never downloading AI models silently, always surfacing size and purpose — is the right default for Hodos's privacy-minded audience. Edwin's first-run setup should show: what model, how big, what it will do, stored where, deletable how.

7. THE AI KILL SWITCH IS TABLE-STAKES FOR PRIVACY AUDIENCES [FACT]:
Firefox added a master "Block AI enhancements" toggle under community pressure. Hodos should ship this from day one. Technically trivial: if the kill switch is on, the Edwin sidecar process is not started. No hooks, no background processes, no network calls.

8. CREDENTIALLESS FETCH FOR LINK PREVIEW IS AN ELEGANT PRIVACY PATTERN [FACT]:
Firefox's link preview makes a credentialless HTTPS request (no cookies, no auth headers) to fetch linked pages, preventing credential leakage and session correlation. Hodos implementing any link preview or pre-fetch feature should adopt the same: strip all credentials from AI-context fetches, and optionally send an x-hodos-ai header allowing publishers to opt out of AI summarization.

9. DO NOT REPLICATE THE TELEMETRY CONTROVERSY [FACT-DERIVED]:
Firefox's brief mandatory-telemetry-for-labs requirement caused severe backlash from exactly the user demographic Hodos is targeting. Any Hodos telemetry must be: (a) explicitly opt-in, (b) granular (per-feature), (c) never a gate to functionality. Edwin's BSV/onchain identity model actually provides a better telemetry alternative — aggregate usage signals can be gathered without identifying individuals, and users can verify what is collected.

10. MODEL ALLOWLISTING AS SECURITY LAYER [FACT]:
Firefox restricts local models to Xenova/Mozilla HuggingFace orgs. For Hodos, the equivalent is a curated Edwin model registry (possibly BSV-anchored for tamper-evidence) that Edwin's sidecar will load. Do not allow arbitrary HuggingFace URLs as model sources without vetting — GGUF model files can contain embedded scripts in some loaders.

11. THE SIDEBAR-AS-PROVIDER-WEB-APP HAS POOR BROWSER INTEGRATION [INFERRED]:
Firefox's cloud chatbot sidebar is architecturally just a browser window loading claude.ai or chatgpt.com. The AI gets no structured browser context — just what the user pastes or the page text that Firefox explicitly forwards. Edwin with a defined JSON API on a localhost port has far richer integration potential: it can receive structured tab metadata, selected DOM nodes, page accessibility trees, etc., fed by Hodos's CEF layer directly. This is a structural advantage over Firefox's sidebar approach.

**Sources:** <https://blog.mozilla.org/en/firefox/firefox-ai/speeding-up-firefox-local-ai-runtime/> · <https://firefox-source-docs.mozilla.org/toolkit/components/ml/index.html> · <https://firefox-source-docs.mozilla.org/toolkit/components/ml/architecture.html> · <https://firefox-source-docs.mozilla.org/toolkit/components/ml/extensions.html> · <https://blog.mozilla.org/en/mozilla/ai/ai-tech/ai-tab-groups/> · <https://github.com/mozilla/smart-tab-grouping> · <https://blog.mozilla.org/en/firefox/firefox-ai/ai-link-previews-firefox/> · <https://blog.mozilla.org/en/firefox/firefox-ai/running-inference-in-web-extensions/> · <https://blog.mozilla.org/en/firefox/ai-window/> · <https://blog.mozilla.org/en/firefox/firefox-ai/ai-browser-features/> · <https://blog.mozilla.org/en/mozilla/mozilla-open-source-ai-strategy/> · <https://blog.ziade.org/2025/12/05/two-years-of-ai-at-mozilla/> · <https://support.mozilla.org/en-US/kb/ai-chatbot> · <https://support.mozilla.org/en-US/kb/orbit-frequently-asked-questions> · <https://www.phoronix.com/news/Firefox-142-Extensions-AI-LLMs> · <https://www.ghacks.net/2025/07/22/firefox-141-introduces-local-ai-to-help-with-tab-management/> · <https://www.quippd.com/writing/2025/06/16/mozilla-turns-firefox-away-from-open-source-towards-spyware-firefox-labs-now-requires-data-collection.html> · <https://www.quippd.com/writing/2025/06/18/mozilla-backs-off-on-data-collection-firefox-labs-to-not-require-telemetry-or-studies-in-future-updates.html> · <https://blog.tomayac.com/2025/02/07/playing-with-ai-inference-in-firefox-web-extensions/> · <https://hacks.mozilla.org/2022/06/training-efficient-neural-network-models-for-firefox-translations/> · <https://browser.mt/> · <https://github.com/ngxson/wllama> · <https://serverhost.com/blog/mozillas-orbit-the-ai-add-on-for-firefox-shutting-down-this-month/> · <https://firefox-source-docs.mozilla.org/dom/ipc/process_model.html>

### Apple Safari + Apple Intelligence

**AI implementation architecture (where model runs, how browser talks to it).** **Hybrid on-device / PCC / third-party cloud, three-tier routing [FACT]**

Apple runs a three-tier model stack:

**Tier 1 — On-device (default path):** AFM-on-device, a ~3B-parameter dense language model [FACT, Apple ML Research: https://machinelearning.apple.com/research/introducing-apple-foundation-models]. As of WWDC 2026 this has been replaced by AFM 3 Core (3B dense) and AFM 3 Core Advanced (20B sparse, activates 1–4B parameters per token), where the full sparse model is stored in NAND flash and a lightweight dense routing block decides which expert to load [FACT: https://machinelearning.apple.com/research/introducing-third-generation-of-apple-foundation-models]. Quantization is mixed 2-bit/4-bit, averaging 3.5–3.7 bits-per-weight. Generation on iPhone 15 Pro achieves ~30 tokens/sec with ~0.6ms first-token latency [FACT].

**Tier 2 — Private Cloud Compute (PCC):** Apple silicon servers (now also NVIDIA Blackwell GPUs on Google Cloud as of June 2026) running AFM 3 Cloud / AFM 3 Cloud Pro. Apple extended PCC to Google Cloud infrastructure using Intel TDX + NVIDIA Confidential Computing + Google Titan root-of-trust [FACT: https://security.apple.com/blog/expanding-pcc/].

**Tier 3 — Gemini (announced WWDC 2026):** For "world knowledge," creative generation, and real-time information queries, Siri now seamlessly hands off to a customized Gemini model (~1.2T parameter class) running on Google Cloud PCC infrastructure. No user confirmation required — Gemini is a native system process [FACT: https://letsdatascience.com/news/apple-unveils-gemini-powered-siri-and-ios-27-at-wwdc-2026-b757953c].

**Inference runtime:** Apple Neural Engine (ANE) for on-device. The Foundation Models framework (Swift, beta iOS 26 / macOS 26) is the developer-facing API [FACT: https://developer.apple.com/documentation/FoundationModels]. It exposes a LanguageModelSession API that is provider-agnostic — apps can route to on-device AFM, PCC, Gemini, Claude, or any conforming LanguageModel protocol implementation with no session-logic changes [FACT: https://developer.apple.com/videos/play/wwdc2026/241/].

**Task routing:** The device itself decides tier. Simple tasks (proofread, summarize short text) → on-device. Complex reasoning or long-context → PCC. World knowledge / real-time → Gemini. The routing is opaque to users but the privacy rules for each tier are disclosed [FACT with routing logic [INFERRED] from Apple support docs].

**Training framework:** AXLearn (open source, JAX/XLA) for distributed training on TPUs/GPUs [FACT]. LoRA adapters (rank-16, 10s of MB each) enable task-specific fine-tuning loaded dynamically at runtime [FACT].

**Integration depth & browser access.** **Deeply native — kernel-level OS integration, not a bolt-on [FACT]**

Apple Intelligence is not an extension or a third-party addon. It is built directly into UIKit, AppKit, and WebKit:

**Writing Tools in WebKit/WKWebView:** WKWebView has a first-class `writingToolsBehavior` property on `WKWebViewConfiguration` (iOS 18+/macOS 15+). Default is `.limited` (panel-only experience); can be set to full inline rewriting. The WKWebView-level API exposes `isWritingToolsActive` and automatically ignores `<blockquote>` and `<pre>` content. Text is extracted from the rendered page's text storage and returned mutated in-place [FACT: https://developer.apple.com/videos/play/wwdc2024/10168/].

**Safari Highlights — page content access:** When a user opens Highlights, Safari surfaces key page facts (locations, summaries, links to related apps). Technically, Safari sends the page URL to Apple via OHTTP to retrieve pre-indexed highlight data; for summarization, page text content is processed (on-device or via PCC depending on complexity) [FACT: https://www.asurion.com/connect/tech-tips/safari-highlights-apple-intelligence/]. The model does NOT receive raw DOM — it receives extracted text content from the rendered page [INFERRED based on described behavior and Writing Tools text extraction pattern].

**Personal context index:** On-device semantic index covers Photos, Messages, Mail, Calendar, Contacts, Notes — this is available to on-device models for personal context queries but is NOT exposed to web content or Safari page context directly [FACT from Apple support documentation].

**App Intents for agentic depth:** Browser-adjacent automation goes through App Intents (not CDP or DOM scripting). Siri AI can compose multi-step actions across apps — "find the photos from Sunday, make a poster, AirDrop it" — by calling structured App Intent APIs that each app registers [FACT: https://developer.apple.com/documentation/AppIntents/Integrating-actions-with-siri-and-apple-intelligence]. SiriKit was deprecated at WWDC 2026 [FACT: https://ecorpit.com/ios-27-app-intents-siri-ai-developer-guide-2026/].

**What the AI cannot see in Safari:** The AI does not have access to cookies, session tokens, full browsing history, or multiple tab contents simultaneously via any public API [FACT — no such API exists]. It operates on selected text or the current page's rendered text content only.

**Form-factor mechanics.** **All-native UI — no injected HTML, no sidebar webview [FACT]**

**Writing Tools form factor (Safari + system-wide):**
- macOS: Appears in the right-click context menu as "Writing Tools" submenu; also in the Edit menu; macOS 15+ shows a hover affordance over selected text to open the panel
- iOS/iPadOS: Appears in the text selection callout bar (same bar as Cut/Copy/Paste) and above the keyboard
- Processing shows animation indicators in-line; long rewrites delivered in chunks with animation
- The panel is rendered in native AppKit/UIKit — no web UI [FACT: https://developer.apple.com/videos/play/wwdc2024/10168/]

**Safari Highlights form factor:**
- A purple "Apple Intelligence glow" scan animation runs over the page when activated
- Key information surfaces in a native overlay panel above the page — not injected into page DOM
- Accessible via the Page Menu button in Safari's toolbar [FACT: https://support.apple.com/guide/mac-help/use-apple-intelligence-in-safari-mchl62d5873e/mac]
- Highlights shows: page summary, quick facts, nearby locations, links to other apps, relevant suggestions

**Page Summarization form factor:**
- Accessed via Page Menu icon in Safari toolbar → "Summarize" option
- Shows summary in a distinct native panel; user can dismiss to return to page [FACT: Apple support docs]

**Siri AI form factor (WWDC 2026):**
- Now a dedicated standalone app (similar to ChatGPT app UI), not just the system overlay
- Maintains conversation history, syncs via iCloud
- On-screen context awareness for in-browser actions uses visual understanding of current screen, not DOM APIs [INFERRED from available descriptions, confirmed not DOM scripting]

**No sidebar panel in Safari specifically:** Unlike Chrome's Gemini sidebar or Edge's Copilot sidebar, Apple has no persistent AI sidebar in Safari. AI surfaces contextually on selection or explicit page menu invocation [FACT].

**Agentic execution mechanics.** **App Intents framework — structured API dispatch, not screen scraping or CDP [FACT]**

**Automation engine:**
Apple explicitly chose NOT to implement computer-use / CDP-style screen automation. Instead, the architecture relies on App Intents — a structured, typed API that apps register to declare what actions AI can invoke [FACT: https://developer.apple.com/documentation/AppIntents]. Siri AI orchestrates multi-step workflows by composing these intents across apps (e.g., Messages intent + Photos intent + AirDrop intent chained together) [FACT: https://www.mindstudio.ai/blog/apple-wwdc-ai-strategy-siri-app-intents-mcp].

**In Safari specifically:** There is no CDP-driven browser automation via Apple Intelligence. Writing Tools and Highlights operate on text/content already rendered, not by scripting DOM interactions. Safari itself exposes no App Intents for AI to control navigation or form filling as of the researched period [FACT — no such public API documented].

**Session isolation:** Agentic actions taken via App Intents run in the user's real session — they use the user's actual app state, credentials, and data. There is no sandboxed or isolated profile for AI task execution [FACT by design — App Intents execute within app process].

**Credential handling:** Because App Intents run within the registered app's process (which already holds the user's credentials), no credential handoff occurs — the AI calls the intent, the app executes it with its own credentials. The AI never touches authentication tokens [FACT by architectural design].

**MCP support announced:** WWDC 2026 included native support for Model Context Protocol (MCP), letting Siri AI reach external tools and APIs through a standard protocol [FACT: https://www.mindstudio.ai/blog/apple-wwdc-2026-ai-strategy-what-it-means-for-builders]. Details of in-browser MCP execution are not yet public [UNVERIFIED].

**Key constraint:** This architecture means AI agents can only do what apps explicitly expose as App Intents. Arbitrary web automation (fill arbitrary forms, click arbitrary links) is not supported and appears intentionally excluded [INFERRED from the App Intents-only strategy + SiriKit deprecation narrative].

**Privacy / data architecture.** **Privacy-by-architecture, not privacy-by-policy — verifiable via transparency log [FACT]**

**On-device (default):**
No data leaves the device. The 3B (or 20B sparse) model runs on Apple Neural Engine with locally stored LoRA adapters. Zero network calls [FACT].

**Private Cloud Compute (when cloud needed):**
1. Device evaluates which PCC nodes have published software measurements matching the public transparency log (append-only, cryptographically tamper-proof) — only those nodes receive encrypted requests [FACT: https://security.apple.com/blog/private-cloud-compute/]
2. Request payload is end-to-end encrypted to verified PCC node public keys; intermediate load balancers never see plaintext [FACT]
3. OHTTP relay (operated by a third party, not Apple) strips the device's IP before the request reaches PCC infrastructure [FACT]
4. PCC nodes run stateless inference: no persistent data storage, memory address spaces periodically recycled, no general-purpose logging — only pre-specified, structured, audited logs/metrics can leave the node [FACT]
5. Secure Enclave manages attestation keys; keys that decrypt requests cannot be extracted or duplicated [FACT]
6. RSA Blind Signatures enable single-use credentials that authorize requests without identifying the user [FACT]
7. All production software images published within 90 days; Virtual Research Environment available for independent security researchers to audit PCC on Apple Silicon Macs [FACT: https://security.apple.com/blog/pcc-security-research/]

**PCC expansion to Google Cloud (June 2026):**
Intel TDX (Trust Domain Extensions) + NVIDIA Confidential Computing + Google Titan chip. A cryptographically verifiable ledger of all hardware in the PCC fleet is maintained. Software attestation rooted in at least two independent vendor roots of trust [FACT: https://security.apple.com/blog/expanding-pcc/].

**Safari Highlights specifically:**
Page URL sent to Apple via OHTTP to retrieve pre-indexed highlights. Webpage address not associated with Apple Account or other personal data [FACT: https://www.asurion.com/connect/tech-tips/safari-highlights-apple-intelligence/]. Page text content goes through on-device or PCC path depending on complexity.

**Telemetry:**
Aggregated, non-identifiable: approximate request size, which feature used, duration only. No request content, no result content, no Apple Account linkage. Users can enable transparency logging in Settings to view PCC requests [FACT: https://www.apple.com/legal/privacy/data/en/intelligence-engine/].

**Gemini handoff:**
Complex queries to Gemini go to Google Cloud PCC infrastructure (not standard Google Cloud). Same privacy architecture applied. [FACT but full implementation details of Gemini-path privacy stack not fully public — [PARTIALLY UNVERIFIED]].

**ChatGPT (legacy, now being superseded by Gemini):** Was opt-in, required explicit user confirmation per request. Requests logged to OpenAI per their policy [FACT from Apple legal page].

**The WHY (strategic + engineering reasoning).** **Apple's engineering choices are driven by brand survival, silicon advantage, and platform leverage [FACT + INFERRED]**

**1. Privacy as existential brand requirement [FACT + INFERRED]:**
Apple's core brand equity is privacy. Adding AI via a cloud API that processes user data would have directly contradicted years of "what happens on your iPhone stays on your iPhone" messaging. The on-device-first architecture and PCC are not engineering curiosities — they are what made AI commercially possible for Apple without brand damage. This is why Apple published the PCC source code and transparency log: the architecture had to be independently verifiable, because marketing claims alone would be insufficient [INFERRED from public statements and engineering investment].

**2. Apple Silicon as moat [FACT + INFERRED]:**
The Neural Engine in Apple Silicon (A-series, M-series chips) is what makes a 3B (now 20B sparse) parameter model practical on a phone. No other phone maker can do this at scale because they don't control silicon, OS, and runtime together. Apple's AI strategy is partly a validation of its vertical integration investment — AI is the proof-of-value for ANE [INFERRED].

**3. Deep OS integration as developer lock-in [FACT]:**
By making Writing Tools automatic in UITextView/NSTextView/WKWebView, Apple created a zero-effort developer experience — apps get AI text editing for free. This drives ecosystem adoption and makes it hard for third-party AI tools to compete at this UX level [INFERRED from API design].

**4. App Intents (not computer-use) is about safety + auditability [INFERRED]:**
Choosing App Intents over CDP/computer-use for agentic actions means every action an AI can take must be explicitly declared by a developer. This is slower to build out, but it gives Apple (and users) a clear audit surface — no AI can click arbitrary buttons or scrape arbitrary sessions. This tradeoff prioritizes safety and platform control over raw agentic power [INFERRED].

**5. Gemini deal: pragmatic recognition of frontier model limits [FACT]:**
Apple reportedly cannot build a frontier reasoning/knowledge model competitive with Gemini at this scale. Rather than ship a worse product, Apple licensed Gemini for ~$1B/year and built a privacy architecture wrapper around it. The bet is that users will trust Apple's privacy layer more than trusting Google or OpenAI directly [FACT from deal reporting; strategic reasoning INFERRED].

**6. PCC transparency log: accountability without opening infrastructure [FACT + INFERRED]:**
Publishing software measurements to a public log (same pattern as certificate transparency) lets researchers verify what runs on PCC without Apple needing to grant direct server access. This is engineering-driven trust design — the system is designed to not require trusting Apple's word [INFERRED from Apple Security Research blog framing].

**Lessons for Hodos/Edwin.** **Seven concrete architecture takeaways for Hodos/Edwin:**

**1. The OHTTP relay pattern is directly adoptable — and costs almost nothing [FACT basis]:**
Apple routes every PCC request through a third-party OHTTP relay to decouple the device IP from the request payload. When Edwin makes any external API call (x402 payment routing, cloud model fallback, Highlights-style lookup), Hodos should insert an OHTTP or CONNECT-proxy hop operated by a party separate from the model provider. This is a well-specified protocol (RFC 9458) and cleanly separates "who is asking" from "what is being asked." BSV micropayments could fund per-request relay usage, aligning with x402 architecture naturally.

**2. Local-first routing with explicit escalation disclosure [INFERRED from Apple pattern]:**
Apple's three-tier routing (device → PCC → Gemini) with per-tier privacy disclosure is the right mental model for Edwin. Edwin's Node gateway today is cloud-first; the lesson is to invert: handle locally, then escalate explicitly. Show users a visual indicator when Edwin is leaving the local sidecar (similar to Apple's privacy labels per request tier). Privacy-minded users need to see the escalation, not just trust it.

**3. Attestation of the sidecar binary via BSV [INFERRED, novel for Hodos]:**
Apple publishes PCC software measurements to a cryptographically tamper-proof transparency log. Hodos could publish Edwin sidecar binary hashes (build-time SHA-256 + signing key fingerprint) to BSV as unspent OP_RETURN outputs — creating a public, permanently auditable record of what version of Edwin any Hodos installation is running. Users (or auditing tools) could verify the running binary matches the published hash before trusting it. This directly maps Apple's PCC transparency model to a decentralized ledger. Cost: trivial in BSV.

**4. Flash-storage sparse model loading is the right architecture for a lean sidecar [FACT basis]:**
Apple's AFM 3 Core Advanced stores 20B parameters in flash, activates only 1–4B per prompt. For Edwin's sidecar, this means: don't keep a large model fully resident in RAM. Use a small always-hot routing/embedding model (~1–3B) in memory; keep specialized adapter weights on disk and load on demand. This keeps Edwin's baseline memory footprint compatible with a browser co-existing with a user's normal workload.

**5. LoRA adapter hot-swap for domain specialization [FACT basis]:**
Apple's LoRA adapters (10s of MB, loaded at runtime) enable task-specific behavior without separate models. Edwin could ship one base model plus BSV-specific, web-browsing, and payment-assistant adapter packs — downloaded from a Hodos-operated endpoint (BSV-payable per adapter) and hot-swapped without restarting the sidecar. This avoids monolithic model updates and enables community-contributed adapters.

**6. Do NOT emulate the App Intents-only agentic approach for a browser [INFERRED with caveats]:**
Apple's App Intents architecture is elegant for iOS but depends on every app registering intents — a years-long ecosystem build. For Hodos/Edwin, users want AI to operate on arbitrary web pages (fill forms, extract data, navigate flows). CDP (Chrome DevTools Protocol) is the right tool for this — it is what Apple deliberately avoided to maintain platform control, but Hodos does not have the same platform-control incentive. Use CDP for browser automation; use a structured permission model to require user approval for sensitive actions (form submission, navigation away from page), mirroring Apple's spirit without Apple's structural constraints.

**7. Provider-agnostic inference protocol (LanguageModel protocol pattern) [FACT basis]:**
Apple's Foundation Models framework WWDC 2026 expansion lets apps swap between on-device AFM, PCC, Gemini, and Claude via a single LanguageModelSession interface. Edwin's Node gateway should implement the same abstraction: a single session interface that routes to llama.cpp local, Anthropic API, or OpenAI API transparently, with BSV micropayments (x402) covering cloud tier costs automatically. Users pick their privacy/cost tradeoff; the interface stays constant. This is also how Edwin avoids vendor lock-in as the local model landscape evolves.

**Sources:** <https://security.apple.com/blog/private-cloud-compute/ — Apple Security Research: PCC architecture and attestation (primary source)> · <https://security.apple.com/blog/expanding-pcc/ — Apple Security Research: PCC expansion to Google Cloud, June 2026> · <https://security.apple.com/blog/pcc-security-research/ — Apple Security Research: Virtual Research Environment and transparency log> · <https://machinelearning.apple.com/research/introducing-apple-foundation-models — Apple ML Research: AFM-on-device and AFM-server architecture (2024)> · <https://machinelearning.apple.com/research/introducing-third-generation-of-apple-foundation-models — Apple ML Research: AFM 3 Core, Core Advanced, Cloud Pro (2026)> · <https://developer.apple.com/videos/play/wwdc2024/10168/ — WWDC 2024: Get started with Writing Tools (WKWebView APIs, UITextView integration)> · <https://developer.apple.com/videos/play/wwdc2026/241/ — WWDC 2026: What's new in the Foundation Models framework> · <https://developer.apple.com/videos/play/wwdc2026/339/ — WWDC 2026: Bring an LLM provider to the Foundation Models framework> · <https://developer.apple.com/documentation/FoundationModels — Apple Developer: Foundation Models framework documentation> · <https://developer.apple.com/documentation/AppIntents/Integrating-actions-with-siri-and-apple-intelligence — Apple Developer: App Intents for Siri and Apple Intelligence> · <https://support.apple.com/guide/mac-help/use-apple-intelligence-in-safari-mchl62d5873e/mac — Apple Support: Apple Intelligence in Safari (Highlights, Page Summary UI)> · <https://www.apple.com/legal/privacy/data/en/intelligence-engine/ — Apple Legal: Apple Intelligence data collection and privacy policy> · <https://www.asurion.com/connect/tech-tips/safari-highlights-apple-intelligence/ — Asurion: Safari Highlights OHTTP relay and privacy details> · <https://letsdatascience.com/news/apple-unveils-gemini-powered-siri-and-ios-27-at-wwdc-2026-b757953c — Let's Data Science: WWDC 2026 Gemini-powered Siri technical details> · <https://www.mindstudio.ai/blog/apple-wwdc-2026-ai-strategy-what-it-means-for-builders — MindStudio: WWDC 2026 App Intents + MCP strategy analysis> · <https://ecorpit.com/ios-27-app-intents-siri-ai-developer-guide-2026/ — EcorpIT: SiriKit deprecation and App Intents migration> · <https://pdpspectra.com/blog/apple-foundation-models-languagemodel-protocol-2026/ — PDP Spectra: LanguageModel protocol provider-agnostic design> · <https://dev.to/arshtechpro/wwdc-2026-apple-just-opened-the-foundation-models-framework-to-any-llm-provider-5ejn — DEV Community: Foundation Models framework opened to any LLM provider>

### DuckDuckGo / Duck.ai

**AI implementation architecture (where model runs, how browser talks to it).** Duck.ai is a PURE CLOUD proxy model with zero on-device AI inference. All model computation runs on third-party cloud infrastructure: Anthropic (Claude models), OpenAI (GPT models, accessed via Azure OpenAI and directly), Together.ai (Meta Llama and Mistral), and Tinfoil.sh (gpt-oss-120b in a TEE). [FACT — DDG help pages, privacy-terms]

DuckDuckGo operates a privacy-preserving HTTP proxy/gateway that sits between the user's browser and each model provider. The browser communicates with DDG's gateway via two endpoints: GET /duckchat/v1/status with header "x-vqd-accept: 1" to obtain a session VQD token, then POST /duckchat/v1/chat with the "x-vqd-4" header carrying the current session token. Each provider response returns a new x-vqd-4 token that must be used for the next turn, creating a lightweight stateless conversation chain. Responses stream via Server-Sent Events (SSE). [FACT — reverse-engineering community, blek.codes, github.com/benoitpetit/duckduckGO-chat-api, github.com/aaronsbytes/DuckDuckGO-ChatAPI]

No local inference runtime is used. There is no llama.cpp, ONNX, WebGPU, or Apple Foundation Model involvement. [FACT — no public evidence of any local inference layer]

Search Assist (previously DuckAssist) is a second AI surface: it calls Anthropic and OpenAI models to summarize DuckAssistBot's real-time web crawl results, inserting a two-sentence answer above organic results. This feature launched in 2023 using OpenAI Davinci and Anthropic Claude; it has since been updated to pull from the broader web rather than just Wikipedia. [FACT — spreadprivacy.com/duckassist-launch, computerworld.com/duckduckgo-ai-search]

Voice chat (launched February 2026) uses WebRTC with an encrypted relay: audio streams through DuckDuckGo's relay server but the relay is end-to-end encrypted such that DDG cannot decrypt it — only the endpoint OpenAI model receives the audio. For dictation (non-realtime), audio is recorded locally then transmitted to OpenAI via DDG's server for transcription. [FACT — duckduckgo.com/duckduckgo-help-pages/duckai/is-duckai-voice-chat-private, helpnetsecurity.com]

Current free-tier models (as of June 2026): Claude 4.5 Haiku (Anthropic), Mistral Small 4 (Mistral AI), GPT-5.4 nano and GPT-5.4 mini (OpenAI), gpt-oss-120b (OpenAI-trained, Tinfoil.sh hosting). Paid Plus/Pro plans add GPT-5.4, Claude Sonnet 4.6, Claude Opus 4.8. [FACT — duckduckgo.com/duckduckgo-help-pages/duckai/chat-models]

**Integration depth & browser access.** Duck.ai is best described as a bolt-on integration with a narrow, explicit browser hook rather than deep browser-native AI. Key access boundaries:

WHAT THE AI CAN SEE: Only what the user explicitly provides. There is one native page-context mechanism — the "Attach Page Content" button in the Duck.ai sidebar, available in all DDG browsers (iOS, Android, Mac, Windows). When triggered, page content is passed into the chat as context. The exact extraction mechanism is not publicly documented; [INFERRED] given that DDG browsers are open-source (github.com/duckduckgo/apple-browsers for iOS/macOS, github.com/duckduckgo/Android), they most likely extract the page's visible text or accessibility tree via their native app layer (WKWebView on iOS/Mac, WebView2 on Windows) and inject it as a text block prepended to the user's message. This is explicit user action, not ambient awareness.

WHAT THE AI CANNOT SEE (documented): browsing history, bookmarks, cookies/session tokens, tabs other than the current one, real-time URL changes, any persistent profile of the user's behavior. [FACT by omission — no documentation of these capabilities exists]

Chat history is stored locally on device (browser localStorage or native app storage), not on DDG servers by default. Optional "Sync & Backup" stores encrypted history on DDG servers with a client-held master decryption key; synced chats auto-delete after 18 months of inactivity. [FACT — duckai/privacy-terms]

Search Assist integrates at the search results layer, not the page layer — it reads the query and DuckAssistBot's crawled content, never the user's current browser page.

**Form-factor mechanics.** Duck.ai surfaces through five distinct form factors, only some of which are deeply native:

1. STANDALONE WEB APP (duck.ai): A full-page SPA at duck.ai, accessible from any browser. Rendered as a standard web application on DDG's servers. No browser integration required. This is the primary surface for non-DDG-browser users.

2. NATIVE APP SIDEBAR: In DDG's own browsers (iOS/macOS/Windows/Android), Duck.ai appears as a resizable native sidebar panel built into the app shell. This is a native UI component (Swift/UIKit on Apple platforms, C#/XAML or equivalent on Windows), not a browser extension or injected web overlay. It coexists with the current page without replacing it. The "Attach Page Content" button appears in this sidebar and triggers page context extraction. A secondary button on the browser tab bar opens this sidebar without leaving the current tab. Keyboard shortcut Ctrl+Alt+C activates it. [FACT — duckduckgo.com/updates, duckduckgo.com/duckduckgo-help-pages/duckai]

3. ADDRESS BAR TOGGLE: In DDG's browsers, the address bar includes a toggle allowing users to switch between traditional search mode and Duck.ai chat mode. When toggled to Duck.ai, recent conversations surface as suggestions (analogous to search history suggestions). Available across Chrome, Safari, and other browsers when DDG is the default search (presumably through the search suggest endpoint). [FACT — duckduckgo.com/updates, ghacks.net]

4. SEARCH RESULTS INLINE (Search Assist): AI-generated summaries appear inline within the SERP, above or within organic results. Not a sidebar or overlay — a native SERP component. Frequency is user-configurable (often / sometimes / on-demand / never). [FACT — duckduckgo.com/duckduckgo-help-pages/results/ai-assisted-answers]

5. BANG COMMANDS: !ai and !chat from any browser route to Duck.ai, leveraging DDG's existing bang redirect system.

Third-party community extension (yookoala/duck-ai-chat-sidebar on GitHub, also on Firefox Add-ons) opens duck.ai in Firefox's native sidebar panel — this is NOT an official DDG product but shows community demand.

Browser engine context: DDG browsers use OS-provided web views rather than their own Chromium fork — WKWebView on iOS/macOS (WebKit), WebView2 on Windows (Blink via Edge). The app itself is built natively (Swift on Apple, proprietary Windows code) with web-view embedding for page rendering. This means the AI sidebar is native app UI, not a web-rendered overlay. [FACT — en.wikipedia.org/wiki/DuckDuckGo_Private_Browser, spreadprivacy.com/windows-browser-open-beta]

**Agentic execution mechanics.** Duck.ai has NO agentic execution capabilities as of June 2026. It is a conversational chatbot interface with no browser automation, task execution, or tool use exposed to users.

Specifically absent: no CDP/WebDriver automation, no computer-use, no DOM scripting on behalf of the user, no form filling, no navigation automation, no web browsing by the AI, no file system access, no credential handling beyond the ephemeral x-vqd-4 session token. [FACT — no documentation of such features; confirmed by agentic browser landscape analysis at nohacks.co/blog/agentic-browser-landscape-2026 which explicitly contrasts DDG with agents like Perplexity Comet]

The "Attach Page Content" feature is the closest thing to browser integration but is entirely passive: the user explicitly attaches a page's text to the chat context; the AI then reasons over that text but cannot interact with the page.

Session isolation: Each Duck.ai conversation is identified by the x-vqd-4 session token chain. There is no persistent user profile or cross-session identity on DDG's servers. The session exists only for the duration of the conversation, with the chain of VQD tokens preventing replay or cross-session contamination. [FACT — technical API analysis, benoitpetit repo]

DDG's stated philosophy is deliberately non-agentic: "our approach to AI extends our privacy strategy by integrating private, useful, and optional AI features." The "optional" framing explicitly rules out ambient or autonomous behaviors. [FACT — duckduckgo.com/duckduckgo-help-pages/duckai/approach-to-ai]

**Privacy / data architecture.** Duck.ai operates a three-tier privacy stack, differentiated by model choice:

TIER 1 — ANONYMIZED PROXY (Claude, GPT, Mistral via Together.ai):
- IP address completely stripped; DDG substitutes its own IP so requests appear to come from DDG, not the individual user. [FACT — help pages]
- Data sent to providers: prompt text, today's date, user's timezone, preferred unit system (metric/imperial based on region), and optionally city-level approximate location if user opts in. No PII, no cookies, no fingerprint data. [FACT — duckduckgo.com/duckduckgo-help-pages/what-information-does-duckai-share-with-model-providers]
- Contractual no-train agreement: providers cannot use prompts or outputs to train or improve their models. [FACT — privacy-terms]
- Provider data retention: maximum 30 days, with limited exceptions for safety/legal compliance. [FACT — privacy-terms]
- Chat history: stored locally on device only. Optional E2E encrypted server sync with client-held master key; 18-month inactivity deletion. [FACT — privacy-terms]
- No account required; no login; no persistent user identity at DDG. [FACT — help pages]

TIER 2 — ZERO PROVIDER VISIBILITY via TEE (gpt-oss-120b via Tinfoil.sh):
- Same proxy anonymization as Tier 1 PLUS cryptographic isolation of the compute environment.
- Tinfoil runs on NVIDIA Hopper/Blackwell GPUs in confidential mode, AMD SEV-SNP (hardware memory encryption), and/or Intel TDX (hardware-isolated trust domains). [FACT — tinfoil.sh/technology]
- Prompts and responses are encrypted in memory inside the enclave; Tinfoil's own operators cannot read the data. This is cryptographically enforced, not merely contractual. [FACT — tinfoil.sh/technology]
- Open-source attestation: clients can cryptographically verify the software running in the enclave matches published code. This is stronger than Apple Private Cloud Compute, which has closed-source hypervisor code. [FACT — tinfoil.sh/blog/2025-01-30-how-do-we-compare]
- DDG labels this model "zero provider visibility" in the Duck.ai UI. [FACT — help pages]

TIER 3 — VOICE CHAT (E2E encrypted WebRTC relay):
- Audio streamed via WebRTC through DDG's relay server, but the relay is encrypted end-to-end: DDG cannot decrypt the audio stream. Only OpenAI's endpoint decrypts it. [FACT — is-duckai-voice-chat-private help page]
- No audio retention by either DDG or OpenAI after session ends. [FACT — help page]
- For dictation (non-realtime): audio recorded locally up to 5 minutes, then sent to OpenAI via DDG server for transcription, then deleted. [FACT — help page]

DDUCKDUCKGO'S OWN TELEMETRY GAP: The privacy policy states DDG does not store chats or search history. However, DDG as a proxy necessarily knows that a given session (identified by IP at the network layer) made AI requests, even without content. DDG's own server logs would capture request timestamps and volumes. Whether they retain these at aggregate or individual level is not clearly stated in public documentation. [INFERRED — architectural necessity; policy is silent on this specific question]

The x-vqd-4 session token does NOT identify the user across sessions — it is a per-conversation stateless chaining token for maintaining conversation context at the proxy layer. [INFERRED from reverse-engineering analysis]

Duck.ai's browsers are open-source (Apache 2.0): github.com/duckduckgo/apple-browsers, github.com/duckduckgo/Android. This allows independent verification of what data the apps transmit. [FACT]

**The WHY (strategic + engineering reasoning).** Six primary engineering and strategic rationales, reconstructed from stated positions and inferred from architecture:

1. PROXY BROKER AS NATURAL EXTENSION OF EXISTING MODEL [STATED]: DDG's core search product already proxies user queries — they replace user IPs with their own when calling partner data sources. Duck.ai directly applies this existing pattern to AI model calls. No new architectural pattern required; same anonymization infrastructure, same contractual approach with providers. [FACT — spreadprivacy.com/ai-feature-upgrade]

2. CLOUD MODELS OVER LOCAL = FRONTIER ACCESS WITHOUT HARDWARE GATE [INFERRED]: DDG's user base is broad (privacy-conscious mainstream users, not hardware enthusiasts). Shipping local models would require distributing large weights, GPU requirements, and complex install flows. Cloud proxy lets them offer GPT-5, Claude Opus, and Mistral to all users regardless of device capability — including mobile users. Local AI would have fragmented their offering.

3. WEBVIEW2/WKWEBVIEW NOT CEF FORK = LEAN SHIPPING [STATED by DDG engineers]: "It's not a fork of any other browser code." Using OS-provided web views (WKWebView on Apple, WebView2 on Windows) means DDG does not maintain a Chromium fork — no upstream merge burden, smaller binary, inherits OS security patches. The trade-off accepted is less rendering engine control. This was explicitly a prioritization of shipping speed and maintenance cost over customization depth. [FACT — spreadprivacy.com/windows-browser-open-beta]

4. CONTRACTUAL + TEE = TRUST SPECTRUM WITHOUT OWN GPU INFRA [INFERRED]: DDG cannot plausibly build and operate their own LLM inference cluster. Contracts handle the mainstream providers; TEE partnerships (Tinfoil) handle the ultra-private tier without DDG owning the hardware. This layered approach lets them offer genuinely differentiated privacy tiers (labeled clearly in UI) while outsourcing compute entirely.

5. OPTIONAL AI DESIGN = SERVE BOTH AUDIENCE SEGMENTS SIMULTANEOUSLY [STATED]: DDG explicitly acknowledges "not everyone wants AI in their lives." Making all AI features optional and clearly labeled allows the same brand and product to serve both the no-AI crowd (whose numbers jumped 30% in 2026 as users flee Google's forced AI) and privacy-conscious AI adopters. This is a strategic bet that optional + private AI beats forced + surveillance AI, even in the short term. [FACT — techcrunch.com/2026/05/26, duckduckgo.com/duckai/approach-to-ai]

6. VOICE CHAT AS E2E RELAY (NOT STORAGE) = ARCHITECTURAL CONSISTENCY [INFERRED]: The decision to make DDG's voice relay cryptographically unable to read audio content (rather than just promising not to) reflects a philosophical consistency: the brand cannot afford to be a voice data intermediary. WebRTC E2E encryption for voice is technically standard; applying it here eliminates a category of trust question entirely rather than requiring users to trust another DDG promise. [INFERRED from design choice + brand positioning]

**Lessons for Hodos/Edwin.** Ten concrete takeaways for Hodos/Edwin as a CEF-based, BSV-native, localhost-sidecar AI assistant:

1. THE PRIVACY PROXY PATTERN IS PROVEN AND DIRECTLY APPLICABLE: DDG demonstrates at scale that stripping IP + substituting provider identity + contractual no-train agreements is a viable and trusted privacy architecture. Edwin-as-sidecar on localhost is actually STRONGER than DDG's model: the sidecar makes usage invisible to any remote party including Edwin's operator. Where DDG is the proxy (and therefore knows usage volume), Edwin running on localhost means nobody outside the user's machine knows they made any AI request at all. This is a significant advantage to highlight to users.

2. THE x402/BSV PAYMENT CHAIN CAN REPLACE THE x-vqd-4 SESSION TOKEN: DDG's per-conversation token chain (each response yields the next request's token) is a stateless session mechanism. For Hodos/Edwin with x402 micropayments, each paid request could embed a BSV transaction ID that chains to the next request — the payment IS the session token. This creates a payment-as-authentication model with natural rate limiting (you pay per turn) and no persistent identity.

3. "ATTACH PAGE CONTENT" IS THE RIGHT UX FOR PRIVACY-CONSCIOUS USERS: DDG made page context an explicit user action, not ambient browser surveillance. Hodos/Edwin should default to the same — page context is opt-in per-conversation, not always-on DOM scraping. CEF gives Hodos deeper DOM access than DDG has (DDG must go through WebView2/WKWebView APIs), but that capability should be exercised only on user intent, not automatically.

4. TEE ROUTING TIER IS A DIFFERENTIATOR WORTH BUILDING TOWARD: DDG's "zero provider visibility" label for Tinfoil-hosted gpt-oss-120b resonates with privacy users. Hodos could offer a similar routing tier: for highest-sensitivity queries, route through a TEE inference partner (Tinfoil or equivalent) via Edwin, with the user explicitly choosing this tier. The BSV payment per request naturally accommodates premium pricing for the TEE tier.

5. OS WEBVIEW (WebView2/WKWebView) WAS CHOSEN SPECIFICALLY TO AVOID CHROMIUM FORK BURDEN: DDG's choice is instructive even though Hodos chose CEF. CEF is correct for Hodos because Hodos NEEDS deep protocol hooks (BSV custom schemes, x402 interceptors, custom MIME handling) that OS webviews don't support. But the lesson is: never maintain a Chromium fork deeper than necessary. Keep the CEF integration layer thin and upstream-trackable.

6. VOICE E2E RELAY REQUIRES PROVIDER COOPERATION: DDG's encrypted voice relay works because OpenAI supports WebRTC endpoints. For Edwin, voice AI routing through localhost is inherently private (no relay needed), but if Edwin needs to forward to a cloud voice model, check whether that provider supports WebRTC or requires audio transcription at Edwin's layer (which DDG handles for dictation — local record, then forward). The localhost sidecar model is cleaner: audio goes from browser to Edwin (localhost), never to the cloud at all if using a local whisper/TTS stack.

7. DIVERSIFY MODEL PROVIDERS FROM DAY ONE: DDG routes across Anthropic, OpenAI, Mistral, Together.ai, Tinfoil — never locked to one. Edwin's routing layer should be provider-agnostic from the start. The x402 payment flow makes this easier: different providers can have different per-token prices expressed in BSV, and Edwin can select based on user preference or lowest cost.

8. SESSION ARCHITECTURE: STATELESS CHAINING, NO PERSISTENT IDENTITY: DDG's x-vqd-4 per-turn token demonstrates a clean approach: no user accounts, no persistent session IDs, conversation state chains through tokens that expire. For Edwin: conversations can be keyed by BSV transaction chain (payment nonce → next nonce), with no server-side session state. Local history storage (like DDG's localStorage approach) is correct; optional encrypted BSV-keyed cloud backup is the upgrade path.

9. "NO AGENTIC FEATURES YET" IS A SHIPPING STRATEGY, NOT A FAILURE: DDG has shipped a successful, growing AI product with zero agent capabilities. The lesson for Edwin: conversational AI with excellent privacy guarantees is a complete product. Ship that first. Add CDP/computer-use automation later when the privacy model for credentialed agent actions is worked out. DDG's constraint is also Hodos's opportunity — when Hodos does add agentic features (via Edwin's CEF access), it can do so with tighter isolation than any extension-based competitor.

10. OPEN-SOURCING BROWSER CODE BUILDS VERIFIABLE TRUST: DDG open-sources all their browser code (Apache 2.0 for iOS/macOS/Android) specifically to allow independent verification of privacy claims. Hodos should consider open-sourcing the Edwin sidecar integration layer (if not the full browser) so privacy-focused users can audit exactly what Edwin sends and receives. This is especially important for the BSV payment flow — users should be able to verify that Edwin does not exfiltrate payment keys or conversation content.

**Sources:** <https://duckduckgo.com/duckduckgo-help-pages/duckai/ai-chat-privacy — DDG official: anonymization architecture, IP stripping, TEE, provider agreements> · <https://duckduckgo.com/duckduckgo-help-pages/duckai/chat-models — DDG official: current model list, providers, Tinfoil hosting> · <https://duckduckgo.com/duckduckgo-help-pages/what-information-does-duckai-share-with-model-providers — DDG official: exact data fields sent to providers> · <https://duckduckgo.com/duckai/privacy-terms — DDG official: full privacy terms including Sync/Backup E2E encryption, 18-month deletion> · <https://duckduckgo.com/duckduckgo-help-pages/duckai/is-duckai-voice-chat-private — DDG official: WebRTC relay architecture, dictation vs live voice> · <https://duckduckgo.com/duckduckgo-help-pages/duckai — DDG official: Duck.ai overview, sidebar, address bar toggle, Attach Page Content> · <https://duckduckgo.com/duckduckgo-help-pages/results/ai-assisted-answers — DDG official: Search Assist (DuckAssist) technical overview> · <https://duckduckgo.com/duckduckgo-help-pages/duckai/approach-to-ai — DDG official: strategic rationale for optional/private AI> · <https://duckduckgo.com/updates — DDG official: changelog showing Attach Page Content, voice chat, address bar toggle, image support> · <https://spreadprivacy.com/duckassist-launch/ — DDG engineering blog: DuckAssist launch, OpenAI+Anthropic models, Wikipedia grounding, IP anonymization> · <https://spreadprivacy.com/windows-browser-open-beta/ — DDG engineering blog: WebView2 architecture rationale, not a Chromium fork> · <https://spreadprivacy.com/ai-feature-upgrade/ — DDG blog: AI feature philosophy, proxy model, optional design> · <https://tinfoil.sh/technology — Tinfoil: AMD SEV, Intel TDX, NVIDIA Hopper/Blackwell confidential mode, cryptographic attestation> · <https://tinfoil.sh/blog/2025-08-05-gpt-oss-120b-privacy — Tinfoil: gpt-oss-120b launch on Duck.ai, Apple PCC comparison context> · <https://tinfoil.sh/blog/2025-01-30-how-do-we-compare — Tinfoil: technical comparison vs Apple Private Cloud Compute, open-source attestation advantage> · <https://github.com/duckduckgo/apple-browsers — DDG open-source: iOS/macOS browser source (Swift, WKWebView)> · <https://github.com/duckduckgo/Android — DDG open-source: Android browser source (Blink via WebView)> · <https://en.wikipedia.org/wiki/DuckDuckGo_Private_Browser — Platform engine breakdown: WKWebView (iOS/Mac), WebView2/Blink (Windows/Android)> · <https://github.com/benoitpetit/duckduckGO-chat-api — Reverse engineering: /duckchat/v1/status and /duckchat/v1/chat endpoints, SSE streaming, session management> · <https://github.com/aaronsbytes/DuckDuckGo-ChatAPI — Reverse engineering: x-vqd-4 header, x-vqd-accept, session token chain> · <https://helpnetsecurity.com/2026/02/10/duckduckgo-duck-ai-voice-chat-feature/ — Voice chat launch: WebRTC E2E encryption details, no storage guarantee> · <https://9to5mac.com/2026/02/09/duckduckgo-adds-free-encrypted-real-time-ai-voice-chat-to-duck-ai/ — Voice chat: relay architecture, OpenAI contractual limits> · <https://techcrunch.com/2026/05/26/duckduckgo-installs-are-up-30-as-users-reject-being-force-fed-googles-ai-search/ — Strategic context: 30% install growth from anti-forced-AI sentiment> · <https://techcrunch.com/2026/06/01/duckduckgo-makes-its-no-ai-search-engine-easier-to-access-as-its-traffic-booms/ — Strategic context: no-AI product as separate offering> · <https://techcrunch.com/2023/03/08/duckassist/ — DuckAssist launch history: OpenAI Davinci + Anthropic Claude, Wikipedia grounding> · <https://nohacks.co/blog/agentic-browser-landscape-2026 — Agentic browser landscape: DDG explicitly non-agentic vs Perplexity Comet/ChatGPT Atlas> · <https://github.com/yookoala/duck-ai-chat-sidebar — Community Firefox extension showing demand for native sidebar integration>

### Kagi Assistant + Orion Browser

**AI implementation architecture (where model runs, how browser talks to it).** Cloud-API-only; no on-device inference. Kagi's backend acts as an anonymizing proxy that forwards requests to 30+ third-party frontier model APIs — Anthropic (Claude 4.8 Opus, 4.6 Sonnet), OpenAI (GPT 5.5), Google (Gemini 3.5 Flash, Gemini 2.5 Pro), Mistral, DeepSeek V4 Pro, xAI Grok, and a tier of open-weight models for free plans. No llama.cpp, ONNX, WebGPU, CoreML, or Apple Foundation Model inference is used [FACT — official docs].

The browser-to-AI communication path has two layers:
(a) Primary assistant access: users navigate to kagi.com/assistant — a full web-page app — either by typing the URL, using address-bar bangs (!ai, !assistant, !research, !quick), or via a browser toolbar shortcut that opens the web app. The AI itself is fully remote; Orion is just a WebKit shell rendering a web app.
(b) Programmable JS buttons (a native-but-user-scripted feature): Orion lets users create toolbar buttons containing arbitrary JavaScript executed in the current page's context. Example scripts in Orion's documentation call the OpenAI API or Kagi's Universal Summarizer using user-supplied API keys. Output is printed to Orion's native sidebar panel. This is user-authored automation, not a Kagi-managed AI integration [FACT — official Orion blog].

Research Assistant is a server-side multi-agent pipeline: ~15 specialized sub-model roles (research agents, librarian agents, final-response agents), up to 5 sequential steps per query, ~15 tool calls per query. Tool set includes Kagi Search, Librarian (URL analysis), sandboxed Python interpreter, Wolfram Alpha, image generation, Maps Search. This pipeline runs entirely on Kagi's infrastructure [FACT — Kagi Research docs].

**Integration depth & browser access.** Bolt-on via web navigation; deliberately NOT native. The definitive statement from Orion's 1.0 release: "Orion ships with no built-in AI code in its core." [FACT — blog.kagi.com/orion]

What the browser CAN see or access for AI:
- Current page URL: sent to Kagi's Universal Summarizer (server-side fetches and extracts content); browser itself does not inject DOM into AI prompts natively.
- Page DOM: accessible only through user-written JS programmable buttons that execute in-page JavaScript — the user must author the extraction script. This is not a Kagi-managed pipeline.
- Page summarization feature (confirmed in iOS release notes v1.3.26, Jul 2025): appears to send the current URL to kagi.com/summarizer; content extraction is server-side, not browser-DOM-level.
- Kagi Assistant (web app): can fetch any URL the user pastes (up to 50 MB), but the browser does not automatically inject current-tab content.

What the AI cannot see at browser level: browsing history, bookmarks, cookies/session state, multiple tabs, page accessibility tree — none of these are exposed to AI via any Kagi-managed channel. Privacy Pass is the deepest native integration (browser-level token management for anonymous auth), but that covers auth, not AI content access.

Integration categorization: shallow bolt-on for AI content; deep-native only for Privacy Pass authentication infrastructure.

**Form-factor mechanics.** Three surface mechanisms, in order of depth:

1. Full-page web app (primary): kagi.com/assistant renders in a standard browser tab. Address-bar bangs (!quick, !research, !ai etc.) and the `?` prefix for quick answers route queries to this webapp. This is the canonical form factor [FACT].

2. Native sidebar as AI output channel (secondary, user-scripted): Orion has a native macOS sidebar panel (built in Cocoa, not a WebView injection). JavaScript programmable buttons can "print output" to this sidebar. AI-powered examples in Orion's documentation include an "Unbiased News" rewriter and a page summarizer, both calling external AI APIs. The sidebar rendering is native; the AI call is remote via user-authored JS [FACT — blog.kagi.com/orion-new-features].

3. Contextual menu / toolbar action for page summarization: Orion surfaces a "summarize page" action (iOS confirmed; macOS present but platform-inconsistent per release notes). This opens or populates the Kagi Summarizer with the current page URL [FACT — release notes v1.3.26].

Rendering: Orion is 100% native Cocoa on macOS/iPadOS/iOS (not Electron, not CEF). The sidebar is a native NSPanel/split-view area. The AI assistant UI itself is a WebView rendering kagi.com. There is no injected in-page overlay. There is no omnibox AI-answer inline panel (queries go to a new page/tab). Windows and Linux versions are in development (GTK4/libadwaita on Linux) [FACT — release notes, blog].

**Agentic execution mechanics.** Minimally agentic at the browser level by deliberate design philosophy.

Server-side agentic pipeline (Research Assistant): Kagi's Research Assistant is the most agentic component, but it runs entirely on Kagi's servers. It executes multi-step web research (up to 5 steps, ~15 tool calls), runs sandboxed Python for computation, calls Wolfram Alpha, generates images, and runs Librarian agents for deep URL analysis. Automation engine: Kagi's proprietary orchestration; no CDP, no browser-level automation [FACT — Kagi Research docs].

Browser-side JavaScript buttons: These execute JavaScript in the USER'S real browser session (with real cookies, real DOM, real page state). This is technically agentic capability — a user-authored script could theoretically extract form data, read session storage, etc. — but Kagi provides no managed agent harness on top of it. The user writes the script; the user provides API keys at the top of the code [FACT — blog.kagi.com/orion-new-features]. No isolated profile or sandbox is used.

Explicit non-features: No CDP-based computer-use. No always-on ambient agents. No credential management for AI sessions beyond user-typed API keys. Kagi explicitly cites security research (Brave researchers found prompt injection attacks that hijacked AI assistants to steal credentials in competing AI browsers) as the reason for this restraint [FACT — Orion 1.0 blog, InfoQ article].

The stated roadmap position: "As AI matures and security models improve, they'll continue to evaluate thoughtful, user-controlled ways to bring AI into your workflow." Future Kagi-native AI integration in Orion is marked [VISION], not shipped as of June 2026.

**Privacy / data architecture.** Multi-layer privacy architecture combining browser-native controls and service-level anonymization:

BROWSER LAYER (Orion):
- Zero telemetry: no analytics, no identifiers, no usage data collected. Business model (paid subscriptions) removes financial incentive for tracking. Kagi invites independent verification via Proxyman or mitmproxy [FACT — official blog].
- Orion is proprietary code on open-source WebKit: privacy claims cannot be independently code-audited, a noted limitation [FACT — InfoQ article].
- Data stored locally in SQLite (history), plist (bookmarks), WebKit cache — all in ~/Library on macOS; no cloud sync of browser internals without explicit user action [FACT — technical docs].
- Profile isolation: work/personal/hobby profiles with separate cookies and extensions [FACT].

PRIVACY PASS (most technically novel component):
- Protocol: VOPRF-based (Verifiable Oblivious Pseudorandom Function, RFC 9497), implementing privately verifiable tokens (PrVT) using 2HashDH-NIZK construction [FACT — Kagi Privacy Pass docs].
- Two-phase separation: (1) Token GENERATION — user authenticates with session cookie to prove subscription eligibility; browser generates ~500 tokens (batch DLEQ proofs prevent per-user key attacks). (2) Token REDEMPTION — tokens used for actual searches without any session cookie. Server cannot link generation to redemption [FACT].
- Native Orion integration (v0.99.131+ macOS, v1.3.17+ iOS): at the browser process level, Orion strips deanonymizing HTTP headers and cookies from requests. This is deeper than extension-level (extensions cannot strip all headers on all browsers; Safari's model prevents it entirely) [FACT — Kagi Privacy Pass docs, Privacy Guides article].
- Extension (Chrome/Firefox): open-sourced at github.com/kagisearch/privacypass-extension; Rust library used for WASM; source-verifiable builds [FACT].
- Current scope: covers Kagi Search only. Privacy Pass for Kagi Assistant, Summarizer, Maps, Translate — PLANNED but not shipped [FACT — Kagi Privacy Pass docs state "forthcoming"].
- Limit: 3,000 tokens/month; personalizations (lenses, custom bangs) disabled when active to prevent deanonymization.
- Known gap: same-session generation+redemption can create correlation patterns despite cryptographic unlinkability [INFERRED from HN discussion technical critique].

ASSISTANT LAYER:
- No unique user identifier sent for OpenAI API requests [FACT — LLMs privacy docs].
- Data retention by provider: 24h (Google), 30d (Anthropic, OpenAI, xAI), not stored (Fireworks.ai, Groq, Together.ai, DeepInfra) [FACT].
- No training on user data via API (contractual with all listed providers) [FACT].
- Thread auto-deletion: 24h default [FACT].
- Account info not shared with LLM providers [FACT].
- All queries leave device: no local processing. This is the fundamental limitation of Kagi's privacy model [FACT].

**The WHY (strategic + engineering reasoning).** WHY NO AI IN BROWSER CORE [FACT — stated]:
Security over features: Kagi explicitly cites documented prompt injection attacks in competing AI browsers where malicious page content hijacked AI agents to exfiltrate credentials and financial data (research attributed to Brave's security team). Their position: "Your browser should be a secure gateway, not an unvetted co-pilot wired into everything you do." The 6-person dev team cannot also maintain a secure, always-on agent harness.

WHY CLOUD-ONLY AI (no local inference) [INFERRED]:
A 6-person team maintaining a native macOS/iOS browser cannot additionally maintain quantized model variants, platform-specific inference runtimes (CoreML, llama.cpp, Apple Neural Engine), and update cycles for local models. Cloud API routing is operationally tractable. Additionally, Kagi's paid-subscription business model means the assistant IS a premium product — it must be best-in-class quality, which requires frontier models that won't run locally at comparable quality for years.

WHY 30+ MODELS / MULTI-MODEL ROUTING [FACT + INFERRED]:
Stated: continuous benchmarking selects the best model per task; "unpolluted private LLM benchmarks." Also: model-agnosticism future-proofs the product as model landscape evolves rapidly. Unstated but evident: offering every major model is a competitive differentiator against ChatGPT/Claude/Gemini which each lock users to one provider.

WHY WEBKIT NOT CHROMIUM [FACT — stated]:
"Deliberate choice against the growing Chromium monoculture." Practical: WebKit is deeply optimized for Apple's platforms (macOS/iOS are the exclusive initial target market). Smaller/more auditable codebase for a small team. The zero-telemetry privacy stance aligns with Apple's own platform direction, reducing friction for the target audience.

WHY PRIVACY PASS AT BROWSER LEVEL [FACT + INFERRED]:
Safari's extension model cannot intercept and strip all request headers — which is why Safari is explicitly unsupported for Privacy Pass. By building Orion natively, Kagi gained the ability to implement Privacy Pass at the network request layer (deeper than any extension). The VOPRF scheme was chosen over blind RSA because Kagi is the sole verifier — no need for public verifiability, simpler implementation, lower computational cost. Open-sourcing the extension and Rust library was necessary to claim cryptographic auditability given Orion's closed source.

WHY THE ASSISTANT IS A WEB APP, NOT NATIVE UI [INFERRED]:
The assistant UI at kagi.com/assistant is maintained by one team serving all Kagi users (web, mobile, extension). Building a separate native macOS/iOS UI for the assistant would require duplicating the React/web UI development effort. Web delivery also enables instant updates without browser app releases. The tradeoff is that the UI cannot deeply integrate with local browser state (history, tabs, etc.) — but given the philosophy of keeping AI separate, this constraint is welcome rather than a problem.

**Lessons for Hodos/Edwin.** 1. BOUNDARY ARCHITECTURE AS A TRUST FEATURE: Kagi turned "AI is separate from browser core" into a positive selling point rather than a limitation. For Hodos/Edwin, "Edwin runs as an auditable local sidecar process — it cannot access your browser without your explicit permission" is a stronger privacy story than Kagi's cloud-proxy model. Make the architecture boundary explicit and user-visible. This is what Kagi got right.

2. PRIVACY PASS AT THE BROWSER PROCESS LEVEL: The single biggest lesson. Kagi's Privacy Pass native integration works better than extensions precisely because the browser process can intercept and strip HTTP headers that extensions cannot touch. CEF gives Hodos the same capability — the browser network stack (via CefRequestHandler/ResourceRequestHandler) can inject payment tokens, strip fingerprinting headers, and handle x402 BSV micropayment headers at a layer BELOW any extension. This is architecturally superior to any web extension approach for payment auth. Build x402 payment token management directly into the CEF layer, not as a WebExtension.

3. MULTI-MODEL ROUTING FROM DAY ONE: Kagi's multi-model design means Edwin should not be architected as "the Claude sidecar" or "the GPT-4 sidecar" — it should route to whichever model the user or task calls for, with user-selectable defaults. The BSV x402 payment layer actually enables per-inference micropayment to different model endpoints, which is a natural fit for model-agnostic routing.

4. URL-BASED CONTEXT INJECTION, NOT DOM SCRAPING: Kagi's Universal Summarizer approach (send URL to server-side fetcher) is simpler and safer than DOM-level extraction. For Edwin: the browser sidecar communicates with Edwin's localhost endpoint; the browser sends the current page URL (and optionally a DOM text snapshot) to Edwin, which fetches and processes content. Edwin does not need deep DOM access via CDP; a simple "send URL + optional clipboard text" message over the IPC channel is sufficient for most use cases. This avoids the security complexity of giving Edwin JS execution in the page context.

5. NATIVE SIDEBAR OUTPUT CHANNEL: Orion's pattern of using a native sidebar panel as AI output (rather than injecting content into the page DOM) is correct. The AI response stays sandboxed from page content, preventing any cross-contamination. In CEF: implement the Edwin panel as a dedicated browser-side panel view (a separate CEF frame or a native OS panel), not an injected overlay into the active tab's DOM.

6. SIDECAR TELEMETRY AUDITABILITY: Kagi explicitly tells users to run Proxyman or mitmproxy to verify zero-telemetry claims. Hodos should do the same for Edwin: document that Edwin's sidecar process only opens outbound connections to explicitly listed endpoints, and publish that list. This costs nothing and builds substantial trust with the privacy-conscious target audience.

7. WHAT TO AVOID — THE "BRING YOUR OWN API KEY" TRAP: Orion's programmable buttons require users to paste their OpenAI API key into script code. This is too high-friction for casual users and exposes key management risk. Edwin's design (managed sidecar on localhost port, Hodos handles key/credential management) is architecturally superior. Do not require users to configure AI credentials manually.

8. WHAT KAGI GETS WRONG FOR HODOS: Kagi's Privacy Pass does not yet cover the Assistant (only Search) — a gap that means assistant queries are not anonymous even when search is. For Edwin/Hodos, if the goal is privacy-first AI, the payment/auth layer (x402 BSV) should cover Edwin queries from the start, not be added later. Don't ship a partially-private AI assistant.

9. LOCAL MODEL AS OPTIONAL TIER: Kagi has no local inference. For Hodos's audience (privacy-conscious, BSV-native), advertising an optional local model path (even if lower quality) would be a genuine differentiator that Kagi cannot match. The sidecar architecture (Edwin on localhost) is already the right shape for this: Edwin can route to either a local llama.cpp instance or a remote API, with the same browser-facing interface. Label clearly which mode is used.

10. AGENTIC RESTRAINT IS CORRECT: Kagi's security research-driven decision to keep AI non-agentic in the browser is well-founded. The prompt injection threat is real. Edwin's browser agentic features (if any) should be explicitly user-triggered, sandboxed in an isolated CEF profile (not the user's real session), and should never have ambient access to cookies or active tab sessions. The "secure gateway" framing is the right north star.

**Sources:** <https://blog.kagi.com/orion — Orion 1.0 launch blog, official architecture statement (accessed 2026-06-26)> · <https://help.kagi.com/kagi/ai/assistant.html — Kagi Assistant official docs: models, features, browser access (accessed 2026-06-26)> · <https://help.kagi.com/kagi/ai/llms-privacy.html — LLMs privacy docs: data retention, proxy model, no-training commitments (accessed 2026-06-26)> · <https://help.kagi.com/kagi/privacy/how-does-privacy-pass-work.html — Privacy Pass technical architecture: VOPRF, 2HashDH-NIZK, DLEQ proofs (accessed 2026-06-26)> · <https://help.kagi.com/kagi/privacy/privacy-pass.html — Privacy Pass deployment: native Orion support, token limits, planned expansion (accessed 2026-06-26)> · <https://blog.kagi.com/kagi-privacy-pass — Privacy Pass announcement: Rust library, RFC 9497, IETF standards compliance (accessed 2026-06-26)> · <https://help.kagi.com/kagi/ai/kagi-research.html — Kagi Research Assistant: 15 agent roles, tool set, multi-step pipeline (accessed 2026-06-26)> · <https://blog.kagi.com/kagi-assistants — Kagi Assistants blog: multi-model benchmarking, Quick vs Research tiers (accessed 2026-06-26)> · <https://blog.kagi.com/announcing-assistant — Original Assistant announcement: proxy architecture, privacy design (accessed 2026-06-26)> · <https://blog.kagi.com/orion-new-features — Orion programmable buttons: JS-in-page, sidebar output, OpenAI button examples (accessed 2026-06-26)> · <https://help.kagi.com/orion/misc/technical.html — Orion technical info: data storage locations, WebKit cache, extension APIs (accessed 2026-06-26)> · <https://help.kagi.com/kagi/why-kagi/ai-philosophy.html — Kagi AI philosophy: contextual limitation, enhancement-not-replacement principles (accessed 2026-06-26)> · <https://www.infoq.com/news/2025/12/orion-ai-proof-privacy-browser/ — InfoQ Orion 1.0 review: AI-proof security rationale, prompt injection threat model (accessed 2026-06-26)> · <https://orionbrowser.com/updates/orion-release-notes.html — Orion macOS release notes v0.99.133–1.1.1: no AI features in core confirmed (accessed 2026-06-26)> · <https://orionbrowser.com/updates/orion-iOS-release-notes.html — Orion iOS release notes: summarize page (v1.3.26), Privacy Pass (v1.3.17) (accessed 2026-06-26)> · <https://news.ycombinator.com/item?id=43040521 — HN discussion: temporal correlation attack on Privacy Pass, WASM reproducibility, multi-device token sync gap (accessed 2026-06-26)> · <https://www.privacyguides.org/articles/2025/04/21/privacy-pass/ — Privacy Guides Privacy Pass explainer: Private State Tokens API vs extension model (accessed 2026-06-26)> · <https://github.com/kagisearch/privacypass-extension — Open-source Kagi Privacy Pass extension (Chrome/Firefox) (accessed 2026-06-26)> · <https://help.kagi.com/kagi/api/summarizer.html — Universal Summarizer API: content extraction, supported types, pricing (accessed 2026-06-26)>

### Maxthon

**AI implementation architecture (where model runs, how browser talks to it).** Maxthon's AI and crypto implementation evolved in two distinct eras, neither involving local inference.

BSV ERA (2020-2022): VBox is the identity and wallet subsystem. It manages a private/public keypair natively in the browser (almost certainly compiled into the Chromium-fork C++ code rather than a separate process or extension, based on the fact that it exposes a browser API to web pages and handles custom URL protocols). The signing flow: a web page sends a SHA-256 hash to VBox via a browser API call; VBox performs a double-SHA256 sign with the stored private key and returns the signature; the page then verifies with the public key. Private keys are stored locally with encryption; optional cloud sync is offered but described as opt-in. No local BSV node: resolution of .b domains and blockchain queries is done by querying external BSV nodes (users can pick the fastest node), meaning the browser acts as a thin client to the BSV network over HTTP/JSON. [FACT from official blog, 2020]

AI ERA (2023-present): AIChat launched 2023. The 2023 blog post claimed "all AIChat interactions occur locally and no personal data is sent to external servers" [UNVERIFIED — no technical basis provided; the architecture is undocumented and the claim contradicts later cloud partnerships]. In May 2025 Maxthon announced a strategic partnership with uuGPT.com, adding a uuGPT sidebar icon and toolbar entry. This is clearly a cloud API integration: uuGPT.com is an external SaaS platform providing "globally leading advanced models." No on-device inference runtime (llama.cpp, ONNX, WebGPU) has ever been mentioned. The underlying model providers used by uuGPT are not disclosed publicly. The browser itself runs on Chromium (Blink engine since v5; fully Chromium by v6 in 2020, continuing through v7 as of 2026). No evidence of any sidecar process, local port, or native inference engine. The AI integration is a thin embedded panel pointing at a cloud service. [FACT: uuGPT partnership from official blog May 2025; AI architecture labeled INFERRED from available evidence]

**Integration depth & browser access.** Two layers of integration depth, one deep (VBox/wallet), one bolt-on (AI).

VBOX — DEEP NATIVE INTEGRATION: VBox is built into the browser core, not a Chrome extension. Evidence: it exposes a 'browser API' that web pages call synchronously (impossible from a sandboxed extension without message-passing indirection that would be documented differently); it handles custom URL schemes (tx://, nb://) at the browser's URL dispatch layer, which requires modifying Chromium's protocol handler registration in C++. The API surface exposed to web pages allows: (1) requesting identity signatures (SHA256 hash in, double-hash ECDSA signature out), (2) initiating payments via VPoint units (1 VPoint = 100 satoshi), (3) binding to NBdomain names for identity resolution. Web pages can call these APIs after the user confirms a consent dialog. The browser also understands the b:// and d:// data protocols natively (for reading BSV on-chain data blobs). Full page DOM is NOT accessible to VBox — VBox provides a signing/payment primitive, not content awareness. Developer documentation was hosted at v.maxthon.com/doc (now unreachable, domain refuses connections) [FACT: confirmed via search, now dead]. [FACT: architecture from official 2020 blog and CoinGeek reporting]

AI (AICHAT / UUGPT) — BOLT-ON: The AIChat and uuGPT sidebar have no evidence of DOM or page-content access. Maxthon's blog describes general browsing assistance (news, weather, translation) rather than page-aware summarization. The AI cannot see tab contents, history, bookmarks, or cookies. It is a general conversational assistant embedded in the sidebar, not a browser-aware copilot. Page summarization may exist as a feature where users manually paste or copy content to the chat, but no automatic page context injection is documented. [INFERRED from absence of any documentation of DOM access combined with the cloud-embedding model]

**Form-factor mechanics.** Two distinct UI surfaces for AI vs wallet.

WALLET/VBOX UI: A native browser dialog/popup is triggered when a web page calls the VBox API. This is a standard browser-chrome modal — native C++ rendered UI, not a webview — presenting the signing or payment request for user confirmation. The address bar natively intercepts .b TLD inputs and tx:// / nb:// protocol strings, dispatching them to the blockchain resolver instead of DNS. This is implemented at the Chromium browser process level (URL scheme registration and omnibox handler). [INFERRED as native C++ from the protocol-handler integration pattern; no source code available]

AICHAT / UUGPT UI: Left sidebar panel, accessible via sidebar icon or from the omnibox. In Maxthon 7 (2025-2026) the uuGPT integration adds a sidebar icon and quick-access toolbar entry. The panel is almost certainly an embedded WebContents (a Chromium BrowserView or WebView2-equivalent) pointing to the uuGPT.com web application. This is the standard pattern for all browser AI sidebars (Copilot in Edge, Opera's Aria) — a privileged webview with extra browser permissions. No evidence of native C++ rendering for the AI UI. Users can access it from the left sidebar or address bar area. Available on PC, Mac, iOS, and Android. [INFERRED: sidebar webview pattern from UX description and uuGPT.com partnership structure; no source code available]

MINING (LIVESTOKEN): Implemented as a separately downloadable 'Mining Go' browser extension (not built-in), visible via an orange icon. Uses behavioral tracking (clicks, browsing duration, content interactions) rather than CPU/GPU hash computation. Proof-of-Value consensus, ERC-20 token (LVT), operated by Symbiosism Economy Foundation (Singapore). This is a bolt-on, not native. [FACT: official Maxthon blog Oct 2024]

**Agentic execution mechanics.** Maxthon has no meaningful agentic AI execution capability. The AIChat/uuGPT integration is purely conversational — no evidence of CDP-based automation, DOM scripting, multi-tab orchestration, or computer-use capability. Maxthon is not listed among agentic browsers (OpenAI Atlas, Perplexity Comet, Dia, Opera's browser agent) in 2025-2026 industry surveys. [FACT: confirmed by absence from agentic browser roundups]

The closest Maxthon comes to 'doing things' is VBox's payment and signing flow — when a web page calls the VBox browser API, the browser can initiate a BSV transaction on behalf of the user (after confirmation). This is a narrow, constrained action: spending BSV from the built-in wallet. The credential model: private keys live in the browser's local storage (encrypted), never shared with the web page. The web page receives only the signature or transaction confirmation, not the key material. User session isolation: VBox operates in the user's actual browser session (same profile), not an isolated sandbox. [FACT: from 2020 technical blog and CoinGeek interview]

Magic Fill is a form autofill feature (recognizes form field types automatically) but this is not AI-driven automation in the agentic sense. [FACT: from browser feature descriptions]

No isolation of AI from user session, no sandboxed profile for AI-driven actions, no permissions model for agentic actions beyond the VBox consent dialog for individual signing/payment events.

**Privacy / data architecture.** Maxthon's privacy posture has a significant documented gap between marketing claims and technical reality.

DOCUMENTED INCIDENT (2016): Security researchers at Exatel discovered Maxthon 4.4.5 was surreptitiously transmitting browsing history, visited URLs, installed applications, and ad-blocker status to Beijing servers over unencrypted HTTP connections. This was vulnerable to MITM attacks. Maxthon's initial response was non-responsive to the researchers; they only acted after public disclosure. Maxthon claimed it was a 'bug' they fixed. [FACT: Wikipedia/security coverage]

2023 REPORTS: Renewed reports from researchers and watchdogs of sensitive browsing data being transmitted to Chinese servers without user consent or proper disclosure. No independent technical audit was publicly cited — these reports appear based on traffic analysis. [UNVERIFIED: secondary reporting without named primary researchers or CVEs]

CREDENTIAL MANAGER RISK: The built-in password manager uses reversible AES encryption with the key stored on Maxthon's servers. Without an optional master password explicitly set by the user, credentials could be extracted by Maxthon or via legal compulsion. [FACT: cited in security review coverage]

VBOX LOCAL KEY STORAGE: The private/public keypair for blockchain identity is stored locally with encryption. Optional cloud sync is explicitly opt-in and the data is described as encrypted. This is structurally better than the password manager's approach — but no independent audit of the encryption implementation exists publicly. [FACT from official blog; encryption strength UNVERIFIED]

AI PRIVACY CLAIM: The 2023 AIChat blog claimed all interactions are local with no personal data sent externally. This claim is contradicted by the 2025 uuGPT partnership (cloud service) and is architecturally implausible without a local model. [UNVERIFIED/CONTRADICTED]

MINING EXTENSION: LivesToken mining tracks clicks, browsing duration, registrations, purchases, and ad interactions as 'Proof of Value.' This is extensive behavioral telemetry sent to the Symbiosism Economy Foundation — far more privacy-invasive than the browser's core features. [FACT: official mining blog]

OVERALL POSTURE: Maxthon markets itself as a privacy browser but has a documented history of data exfiltration, a weak password manager security model, and ongoing concerns about Chinese data routing. It is not audited by independent third parties.

**The WHY (strategic + engineering reasoning).** STRATEGIC BET ON BSV (2019-2022): Jeff Chen (CEO/founder) made a deliberate strategic alignment with the BSV/MetaNet vision articulated by Craig Wright and nChain. The stated engineering rationale: BSV is the only public blockchain capable of massive microtransaction throughput at sub-cent fees, making it the only viable infrastructure for a micropayment-based internet. The browser was conceived as the natural gateway for this new internet — just as Netscape Navigator was the gateway to the web. VBox was designed to be the universal user identity so that nobody would need a separate wallet app, lowering the friction to zero. NBdomain was designed to make blockchain-hosted content as accessible as typing a URL. The engineering choice to build VBox natively (not as an extension) was correct for the threat model: browser-extension wallets (like MetaMask) can be compromised by malicious extensions; a kernel-level wallet is harder to attack. [VISION stated by Jeff Chen in CoinGeek interviews; INFERRED for native vs extension rationale]

WHY THE LOOP FAILED [INFERRED from market evidence, no official postmortem exists]: (1) Chicken-and-egg failure: virtually no content existed on .b domains; without content, there was no reason for users to adopt the browser; without browser users, no developer reason to build .b content. (2) BSV ecosystem isolation: BSV was politically and technically rejected by most of the crypto/developer community (contentious fork from BCH, Craig Wright controversy), meaning the pool of developers willing to build on BSV was tiny. (3) Micropayments-for-browsing never found product-market fit anywhere — not on BSV, not on Lightning, not on any blockchain — suggesting the core premise that users want to pay per-page was wrong regardless of technical quality. (4) Maxthon's existing privacy scandals (2016 data exfiltration) made the 'user-owned identity' pitch unconvincing to the privacy-conscious users who would most value it. (5) Maxthon's global user base had already shrunk significantly from its peak (~100M users in 2012-2014 era) so there was limited network effect to bootstrap an ecosystem.

AI PIVOT RATIONALE (2023-present): With BSV stalled, Maxthon followed the industry trend of adding AI chat to browsers. The uuGPT partnership is a fast-follower move requiring minimal engineering: embed a webview, add sidebar icon. The why: lowest engineering cost to check the 'AI browser' marketing box. No stated engineering rationale for choosing uuGPT over OpenAI/Anthropic directly — likely cost and/or relationship driven. [INFERRED]

**Lessons for Hodos/Edwin.** 1. NATIVE WALLET IN BROWSER CORE IS THE RIGHT CALL: Maxthon proved that building VBox as a deeply native C++ component (not an extension) is the correct architecture for a wallet that web pages need to call. Hodos should wire Edwin's BSV wallet the same way — native code in the CEF host process, not a Chrome extension. Extension wallets (MetaMask) have a documented attack surface; native wallet code is far harder to compromise. [EMULATE]

2. URL PROTOCOL INTERCEPTION AT DISPATCH LAYER: Maxthon's tx:// and nb:// handlers are exactly the right pattern for x402. CEF exposes scheme handler registration (CefSchemeHandlerFactory) that lets Hodos intercept custom URLs before they hit the network stack. This is where x402 payment negotiation logic should live — intercept the HTTP 402 response or the x402:// scheme at CEF protocol level, handle in native C++/Node bridge, complete payment, then retry the request. The browser user never sees a break in navigation. [EMULATE]

3. JAVASCRIPT API INJECTION PATTERN: Maxthon's 'browser API' that web pages call for signing and payment is the right model — analogous to window.ethereum for MetaMask. Hodos should inject a window.hodos or window.bsv object into page JS context via CEF's V8 extension mechanism (CefV8Handler), exposing methods like hodos.sign(hash), hodos.pay(satoshis, address), hodos.getIdentity(). This lets BSV DApps (1Sat Ordinals, Sigma Auth) work in Hodos without requiring MetaMask or a separate wallet extension. [EMULATE]

4. AVOID THE CHICKEN-AND-EGG TRAP BY TARGETING EXISTING ECOSYSTEMS: Maxthon tried to bootstrap a new content ecosystem (.b domains, MetaNet sites) from zero — this requires both a browser and a content ecosystem to emerge simultaneously, which is nearly impossible. Hodos should instead target EXISTING BSV infrastructure: 1Sat Ordinals, existing DApps, Sigma Auth sites. The wallet/payment primitive works immediately without needing new .b content. Don't build a new internet; unlock existing BSV internet. [AVOID Maxthon's mistake]

5. LOCAL AI IS THE ACTUAL DIFFERENTIATOR: Maxthon's AIChat/uuGPT is a cloud sidebar webview — identical to Edge Copilot, Opera Aria, and every other browser AI. This is a commodity feature providing zero privacy benefit. Edwin running locally as a sidecar process (localhost port, no cloud call-home) is genuinely differentiated for privacy-conscious users. Maxthon's cloud AI approach should be treated as a floor, not a ceiling. [AVOID — Edwin's local architecture is the moat]

6. PRIVACY CREDIBILITY REQUIRES AUDITABLE ARCHITECTURE: Maxthon claimed 'local AI' while later deploying cloud AI, and had documented data exfiltration history. Privacy-conscious users have seen this pattern. Hodos must make Edwin's privacy architecture observable: show users that AI queries go to localhost:PORT (not a cloud), provide a network activity log, allow Edwin to run fully offline. Claims without mechanism are worthless after Maxthon. [AVOID Maxthon's trust deficit]

7. MICROPAYMENT UNIT ABSTRACTION (1 VPOINT = 100 SATOSHI): Maxthon's VPoint abstraction is worth studying — hiding satoshi amounts behind a friendlier unit reduces cognitive friction for users unfamiliar with BSV. For Hodos x402 flows, displaying prices as 'credits' or a stable-valued unit rather than raw satoshis may improve conversion. [CONSIDER adapting the UX pattern]

8. DO NOT COUPLE BROWSER IDENTITY TO A SINGLE BLOCKCHAIN NARRATIVE: Maxthon's exclusive BSV identity made the browser politically unacceptable to most developers and users outside the BSV community. Hodos is BSV-native but should not require users to understand or endorse BSV. Frame Edwin's payments as 'fast, cheap, private micropayments' — the BSV implementation is an infrastructure choice, not a banner. [AVOID Maxthon's positioning mistake]

9. SIDECAR PROCESS ISOLATION IS SAFER THAN IN-PROCESS AI: Maxthon's VBox is in-process (native C++), which is correct for key management. But an AI model running in-process would be a security risk (prompt injection could theoretically target the wallet). Edwin as a separate sidecar process (localhost) maintains process isolation — AI compromise cannot directly reach wallet keys. This is an architectural advantage Maxthon's design did not contemplate because their AI and wallet were separate systems. [HODOS-SPECIFIC ADVANTAGE — maintain Edwin/wallet separation]

10. DEAD DEVELOPER PORTAL IS A DEATH SIGNAL: Maxthon's v.maxthon.com/doc developer portal is now unreachable (connection refused). When a browser's developer portal goes dark, the ecosystem dies with it. Hodos/Edwin developer docs must be hosted durably (static site, GitHub Pages, or on-chain via 1Sat if that aligns with the vision). [OPERATIONAL LESSON]

**Sources:** <https://blog.maxthon.com/2020/06/07/maxthon-6-blockchain-browser-part-1/ (Maxthon official blog: VBox architecture, signing flow, key storage — June 2020)> · <https://www.prnewswire.com/news-releases/maxthon-announces-worlds-first-bitcoin-sv-bsv-powered-internet--blockchain-browser-300997572.html (PR Newswire: first BSV browser announcement — Feb 2020)> · <https://www.prnewswire.com/news-releases/maxthon-6-the-browser-for-the-next-generation-internet-built-on-bitcoin-sv-bsv-301080919.html (PR Newswire: Maxthon 6 launch announcement — Nov 2020)> · <https://coingeek.com/maxthon-6-the-browser-for-the-next-generation-internet-built-on-bitcoin-sv-bsv/ (CoinGeek: Maxthon 6 technical feature breakdown — Nov 2020)> · <https://coingeek.com/maxthon-6-enables-every-website-to-conduct-bitcoin-transactions/ (CoinGeek: tx:// and nb:// protocol details, VBox payment API — 2020)> · <https://blog.maxthon.com/2020/07/27/maxthon-6-supports-nbdomain-protocol/ (Maxthon official: NBdomain .b protocol browser integration — July 2020)> · <https://coingeek.com/nbdomain-officially-launches/ (CoinGeek: NBdomain launch with VBox/DotWallet integration details — 2020)> · <https://coingeek.com/new-browsers-domain-systems-for-bitcoin-sv-powered-internet-debut-at-coingeek-live/ (CoinGeek Live: VPoint 1=100 satoshi, VBox dual-purpose wallet+identity — 2020)> · <https://blog.maxthon.com/2023/07/29/aichat-on-maxthon-for-intelligent-browsing-maxthon-browser/ (Maxthon official: AIChat launch, 'local processing' claim — July 2023)> · <https://blog.maxthon.com/2024/03/18/optimise-browsing-with-maxthons-aichat/ (Maxthon official: AIChat 2024 update — March 2024)> · <https://blog.maxthon.com/2025/05/22/maxthon-announces-strategic-collaboration-with-uugpt-com/ (Maxthon official: uuGPT cloud AI partnership — May 2025)> · <https://blog.maxthon.com/2024/10/22/mining-cryptocurrency-with-maxthon-browser-today/ (Maxthon official: LivesToken/LVT mining extension, ERC-20, behavioral PoV — Oct 2024)> · <https://en.wikipedia.org/wiki/Maxthon (Wikipedia: 2016 Beijing server data exfiltration incident, rendering engine history)> · <https://umatechnology.org/maxthon-browser-caught-sending-sensitive-personal-data-to-china/ (UMA Technology: 2023 data-to-China reports — note: lacks technical primary sourcing)> · <https://coingeek.com/maxthon-ceo-jeff-chen-reveals-bsv-features-of-maxthon-6/ (CoinGeek: Jeff Chen interview on BSV rationale — 2020)> · <https://blog.maxthon.com/2025/01/20/maxthons-crypto-friendly-browser-2/ (Maxthon official: 2025 crypto/Web3 feature overview)> · <https://nbdomain.medium.com/nbdomain-an-updatable-datastore-for-blockchain-37e84d9af16e (NBdomain Medium: blockchain datastore model — 2020)> · <https://v.maxthon.com/doc/ (Maxthon Developer Docs — now unreachable, connection refused as of June 2026)>

### Vivaldi + LibreWolf (joint "why NOT" case study)

**AI implementation architecture (where model runs, how browser talks to it).** Neither browser runs any LLM or ML inference of their own. This is the defining architectural fact.

VIVALDI: Built on Chromium (~92% open-source Chromium code, ~5% closed-source proprietary HTML/CSS/JS UI layer, ~3% open-source Vivaldi code). The browser ships zero AI model, zero LLM inference runtime, zero cloud AI API integration as of August 2025. The one exception is Vivaldi Translate: a Lingvanex-powered machine-translation service hosted on Vivaldi's own Iceland servers — server-side neural MT, NOT an LLM, NOT a general-purpose assistant. The text you select goes to Vivaldi's Iceland infrastructure and comes back translated; no third-party cloud provider is involved. [FACT — confirmed from https://vivaldi.com/features/translate/ and https://lingvanex.com/blog/cases/business-case-20/] The daily telemetry ping (anonymized unique ID, version, CPU arch, screen resolution, time-since-last-ping) goes to Iceland servers; the final IP octet is dropped before any lookup. This is the only regular outbound data stream. [FACT — https://vivaldi.com/privacy/browser/]

LIBREWOLF: A hardened Firefox fork that achieves its AI-free posture through configuration, not code deletion. LibreWolf tracks upstream Firefox releases, typically shipping within days. [FACT — https://librewolf.net/docs/faq/] Firefox 130+ ships with browser.ml.* ML inference capabilities including an ML Chat Sidebar (cloud LLM via ChatGPT/Gemini/HuggingFace) and on-device ML features. LibreWolf disables this entire stack via librewolf.overrides.cfg and a settings repository (Codeberg settings repo PR #98 and subsequent fixes). Specific prefs locked to false/empty: browser.ml.chat.enabled, browser.ml.enable (note: ml.enable was found to be accidentally left true in one version and corrected in 146.0.1-1), browser.ml.linkPreview.enabled, browser.ml.pageAssist.enabled, browser.ml.chat.hideFromLabs, browser.tabs.groups.smart.enabled, extensions.ml.enabled. [FACT — https://codeberg.org/librewolf/issues/issues/2752, https://codeberg.org/librewolf/issues/issues/1919] The code itself still exists in the binary (unlike Ungoogled Chromium which surgically removes code at build time); LibreWolf achieves privacy-equivalent posture via preference locks that prevent the code from executing or downloading models. [FACT/INFERRED — explicit statement from LibreWolf maintainer in issue tracker: 'thoroughly disabling should be equally effective and sufficient']

**Integration depth & browser access.** VIVALDI: The browser is deeply integrated at the Chromium-engine layer for security and rendering, but the AI-integration depth is zero by design. The proprietary UI layer (HTML/CSS/JS, closed-source, obfuscated) implements tab tiling, workspaces, Quick Commands, mail client, and other power-user features — none of which expose DOM content, history, or session data to any AI service. Vivaldi Sync exposes bookmarks/history/passwords to Vivaldi's servers in zero-knowledge-encrypted form only (AES-256 with user-held key, never transmitted). [FACT — https://vivaldi.com/privacy/sync/] The browser explicitly does NOT feed current-tab DOM, browsing history, or page content to any AI. The UI HTML/JS overlay has access to all browser internals (it runs as privileged Chromium UI code), but there is no AI endpoint to forward that access to.

LIBREWOLF: Firefox's internal ML stack has deep browser access by design — browser.ml.* hooks can read page content for summarization (browser.ml.pageAssist), link preview generation (browser.ml.linkPreview), and sidebar chat context. LibreWolf's contribution is blocking ALL of these access points via preference locks before they can be exercised. [FACT — issue tracker discussions at codeberg.org/librewolf/issues]. The result: zero AI depth of integration. uBlock Origin (pre-installed, strict mode) DOES have deep access to network requests and DOM, but it is a content blocker not an AI system. ResistFingerprinting (from Tor Uplift project) operates at the browser engine level to spoof/normalize hardware-observable signals — this is the deepest non-AI integration and it works in the opposite direction (reducing information leakage outward). [FACT — https://librewolf.net/docs/features/]

**Form-factor mechanics.** VIVALDI: No AI UI form factor exists. The browser's UI layer is an HTML/CSS/JS web application running as Chromium's UI (similar to how Chrome's NTP and settings are web-rendered, but Vivaldi's entire chrome is this way). The sidebar, which in Chrome/Edge houses Copilot, in Vivaldi houses user-configurable web panels (any URL the user chooses), mail, calendar, and feeds — but no AI panel and no AI omnibox interceptor. Users can manually add an external AI service (ChatGPT, Claude) as a web panel by typing a URL, but this is not a native integration — it is a generic iframe/webview with no browser API hooks. [FACT — https://forum.vivaldi.net/topic/94624/ai-assistant-to-vivaldi, confirmed by forum discussion of user workarounds] Vivaldi 7.8's new features focused on tab tiling drag-and-drop, domain-restricted pinned tabs, and cross-window mail — engineering investment went entirely to power-user UI primitives rather than AI surfaces. [FACT — https://vivaldi.com/blog/vivaldi-7-8-launches-with-message-to-big-tech-humans-dont-need-ai-babysitters/]

LIBREWOLF: Firefox 130+ introduced an AI sidebar panel rendered via the browser's native XUL/WebComponents sidebar architecture. LibreWolf hides this panel from the UI via browser.ml.chat.hideFromLabs=true, which removes the entry point from Firefox Labs settings so users cannot even toggle it on. [FACT — https://codeberg.org/librewolf/issues/issues/2037] The underlying sidebar code remains in the binary but is unreachable through normal UI flows. The form factor that does exist in LibreWolf is the about:config editor (full access) and the librewolf.overrides.cfg file — these are the surfaces through which the privacy posture is configured.

**Agentic execution mechanics.** VIVALDI: Zero agentic capability. No CDP automation, no DOM scripting on behalf of AI, no form-filling automation, no navigation agent. Vivaldi's CEO explicitly cited Guardio Labs research showing that agentic browsers (specifically Perplexity Comet) can be hijacked via prompt injection — malicious pages issue fraudulent instructions that the AI agent executes, including purchasing products and clicking phishing links. [FACT — https://cyberinsider.com/vivaldi-rejects-ai-integration-commits-to-human-centric-browsing/] This security argument is stated as a technical reason, not just a philosophical one: agentic AI running in the user's real browser session, with access to real credentials and session cookies, is a prompt-injection attack surface. Vivaldi's implicit architectural lesson: the user's real authenticated session must not be delegated to an AI agent.

LIBREWOLF: Zero agentic capability. Firefox's browser.ml.pageAssist and similar features could theoretically assist with page interaction but are all disabled. LibreWolf's hardening philosophy — derived from the principle that 'defaults matter more than options' — means no automation of any kind runs without explicit user initiation. Form autofill is disabled. Password manager is disabled in favor of third-party KeePass-style tools. [FACT — https://librewolf.net/docs/faq/] The architectural implication: credentials never flow through browser-native automation pipelines; they are held in external tools the user controls independently.

**Privacy / data architecture.** VIVALDI — Technical privacy posture:
- Browsing history, typed URLs, downloaded content: local-only, never transmitted. [FACT]
- Daily telemetry: unique installation ID + version + CPU arch + screen resolution + time-since-last-ping → Iceland HTTPS servers. Final IP octet stripped before geoIP lookup. NOT linked to browsing activity. [FACT — https://vivaldi.com/privacy/browser/]
- Sync: AES-256 client-side encryption with user-held passphrase (never transmitted). Zero-knowledge — Vivaldi servers hold ciphertext only. Self-described 'government subpoena resistant.' [FACT — https://vivaldi.com/privacy/sync/]
- Crash reporting: opt-in only. When enabled, full crash dumps (potentially containing page content/passwords) sent; Vivaldi extracts stack traces, stores for 60 days, deletes full dump. [FACT]
- Translate: page-selected text → Vivaldi Iceland servers → Lingvanex MT → translated text returned. No third party. No log retention stated. [FACT]
- Google Safe Browsing: enabled by default (hashed URL lookups to Google). Can be disabled. [FACT]
- AI data flows: none, because no AI is integrated.
- Business model insulation: no VC, no ad network, no behavioral data monetization. [FACT — confirmed via multiple sources]

LIBREWOLF — Technical privacy posture:
- Telemetry: completely zeroed. Crash reports disabled, Normandy disabled, studies disabled, extension recommendations disabled. No infrastructure exists server-side to receive data. [FACT — https://librewolf.net/docs/features/]
- Tracking: uBlock Origin strict mode pre-installed. dFPI (Total Cookie Protection/dynamic First-Party Isolation) enabled. Third-party cookies blocked universally. Cookies/site data cleared on browser close. [FACT]
- Fingerprinting: Resist Fingerprinting (RFP) from Tor Uplift project. WebGL disabled (fingerprinting vector). User-agent language hardcoded to en-US for external sites. Referer headers trimmed cross-origin. Canvas/AudioContext access normalized. [FACT — https://librewolf.net/docs/features/]
- Network: DoH disabled (to avoid centralized DNS logging). Speculative connections disabled. Link prefetching disabled. WebRTC IP leak protection active. [FACT]
- Sync: Firefox Sync available but opt-in only. Client-side encryption. Self-hosting supported. [FACT]
- Preference locking: librewolf.overrides.cfg and policies.json lock critical settings such that extensions, updates, and accidental user changes cannot override them. [FACT — https://codeberg.org/librewolf/issues/issues/1767]
- AI data flows: blocked at the preference layer. browser.ml.* models cannot be downloaded because the ML stack is disabled before any model fetch can be attempted. [FACT — confirmed from issue #2752 discussion: 'ml.enable is set to false and the needed models are blocked from being downloaded anyway']

**The WHY (strategic + engineering reasoning).** VIVALDI — Stated and engineering reasons (sourced from primary blog posts):

1. [FACT] Hallucination / confabulation: LLMs produce 'plausible-sounding lies.' Training data contains misinformation; models cannot be auditorily checked for every output. Browser integration amplifies trust in these outputs because they appear inside the user's primary information tool. Source: https://vivaldi.com/blog/technology/vivaldi-wont-allow-a-machine-to-lie-to-you/

2. [FACT] Passive consumption harm to web ecosystem: PewResearch data cited — users click traditional results ~half as often when AI summaries are present. This starves independent publishers and creators (the content sources AI is trained on), creating a feedback loop that degrades the web. Source: https://vivaldi.com/blog/keep-exploring/

3. [FACT] Agentic security risk: Guardio Labs research shows agentic browsers are vulnerable to prompt injection — phishing pages can hijack the agent running in the user's real session to make purchases and click scam links. Source: cited in https://cyberinsider.com/vivaldi-rejects-ai-integration-commits-to-human-centric-browsing/

4. [FACT] Copyright and privacy violations in training: LLMs 'regurgitate copyrighted material' and leak 'sensitive, private information' from training sets. Source: https://vivaldi.com/blog/technology/vivaldi-wont-allow-a-machine-to-lie-to-you/

5. [FACT] Energy consumption: 'Vast amounts of energy' and GPU demand are cited as resource costs that Vivaldi considers unjustified at current quality/accuracy levels.

6. [INFERRED] Business model alignment: Vivaldi has no VC, no ad business, no behavioral data requirement. Cloud AI integration typically requires sending user data to an API provider — this creates a data-flow liability that conflicts with Vivaldi's zero-data-monetization positioning. The Iceland server model (Translate) shows they will invest in infrastructure to avoid third-party data flows when they do add a service.

7. [FACT] User polling: ~95% of Vivaldi's user base opposed AI integration. The anti-AI stance is also a successful product differentiation play in 2025-2026 (4M users). Source: https://otontechnology.com/vivaldi-anti-ai-browser-4-million-users/

LIBREWOLF — Stated and engineering reasons:

1. [FACT] Cloud AI = surveillance vector: Cloud LLM APIs require transmitting page content, queries, and context to third-party servers (ChatGPT/Gemini/HuggingFace). This directly contradicts LibreWolf's core mission. Source: https://codeberg.org/librewolf/issues/issues/2037

2. [FACT] Proprietary service concern: 2 of the 3 Firefox AI providers (ChatGPT, Gemini) are proprietary, which LibreWolf considers ethically incompatible with an open-source privacy project. Source: issue #2037 discussion.

3. [FACT] Opt-in vs opt-out defaults: LibreWolf's maintainers treat defaults as the real product — if Firefox enables AI opt-out rather than opt-in, LibreWolf intervenes by locking the default to disabled. This reflects a technical philosophy that 'most users never touch about:config.' Source: maintainer statement in issue #2037.

4. [FACT] Maintenance burden argument (team constraint): A small volunteer team cannot audit AI feature drift across every Firefox release. Disabling the entire browser.ml.* stack via configuration is lower-maintenance than patching out code (unlike Ungoogled Chromium's approach). This is an explicit engineering practicality argument, not just ideology. Source: issue #1919 maintainer discussion.

5. [INFERRED] GPL/copyright consistency: Training data copyright concerns (GPL code in training sets) are cited by community contributors. For an open-source project built on GPL-licensed Firefox, bundling services trained on possibly-GPL code creates legal/ethical consistency issues.

**Lessons for Hodos/Edwin.** Hodos is PRO-AI-done-privately, with Edwin as a local sidecar process. These two contrarian browsers teach the following concrete architecture lessons:

1. THE SIDECAR IS THE CORRECT ANSWER TO VIVALDI'S OBJECTION. Vivaldi's core worry is that AI intermediates the user's browsing session — accessing page content, history, and credentials via browser APIs, and forwarding that to a cloud provider. Edwin as a localhost sidecar process, invoked explicitly by the user, does NOT sit between the user and the web. The browser remains an unmediated tool; Edwin is a separate tool the user consciously invokes. This sidesteps the 'passive consumption' and 'surveillance' objections entirely. [Lesson: explicitly position Edwin as a user-invoked peer tool, not a browser intermediary.]

2. DEFAULT-OFF FOR BROWSER-CONTEXT ACCESS. LibreWolf's lesson is that defaults are the product — users don't change them. Hodos should default Edwin to having ZERO access to current-tab DOM, history, or session cookies. If the user wants to share page content with Edwin, it should be an explicit gesture (e.g., a 'share this page with Edwin' button that makes a single copy of the DOM text and sends it over the localhost port). This mirrors LibreWolf's preference-lock philosophy applied to AI.

3. AGENTIC ISOLATION IS NON-NEGOTIABLE. Vivaldi cited Guardio Labs on prompt injection in agentic browsers specifically: the AI agent runs in the user's real session with real credentials, and a malicious page can hijack it. If Edwin ever gains agentic capabilities (clicking, form-filling, navigation), it MUST run in an isolated sandboxed profile — NOT in the user's live authenticated session. The sidecar architecture helps here: Edwin on localhost can receive instructions from the browser, but executing them via CDP in a separate profile (not the user's main session) closes the prompt-injection attack surface.

4. THE TRANSLATE PATTERN: SERVER-SIDE OK IF NO THIRD PARTIES AND JURISDICTIONALLY CONTROLLED. Vivaldi's Translate uses Iceland servers, Lingvanex, no data retention, and calls this 'private.' For Edwin's optional cloud-augmentation (x402/BSV paid API calls), the same principle applies: if a call goes to a cloud model, it should go through Hodos-controlled infrastructure (or a user-explicitly-chosen provider), not through an undisclosed third-party chain. The user should know exactly where their text goes.

5. ZERO-KNOWLEDGE SYNC PATTERN. If Edwin ever persists conversation history, preferences, or memory to a cloud sync service, implement Vivaldi's pattern: AES-256 client-side encryption with user-held passphrase, never transmitted. The server holds ciphertext only. This is achievable with standard Web Crypto or libsodium.

6. CONFIGURATION LOCK > CONFIGURATION OPTIONS. LibreWolf locks critical privacy settings via policies.json and librewolf.overrides.cfg so that extensions and updates cannot override them. Hodos should apply the same discipline: Edwin's data-access permissions should be enforced at the process/IPC layer, not just surfaced as UI toggles that an auto-update or a compromised extension could override.

7. ACCEPTABLE USE CASE FRAMING (VIVALDI'S CARVE-OUT). Vivaldi accepts Translate (bounded, deterministic, no hallucination risk, no content summarization, no session access) while rejecting LLM chatbots and summarizers. This is a useful taxonomy for Edwin's feature roadmap: classify every Edwin capability by (a) data locality, (b) hallucination risk, (c) browser-context access required, and (d) whether it intermediates or augments user intent. Features scoring low risk on all four can ship without extensive user consent flows; features scoring high risk require explicit per-invocation user action.

8. THE PROMPT INJECTION THREAT MODEL IS REAL FOR CEF BROWSERS. Hodos uses CEF (Chromium Embedded Framework). If Edwin ever reads DOM content from the active page automatically (not user-triggered), any page the user visits can attempt prompt injection — crafting hidden text that redirects Edwin's behavior. The mitigations: (a) never auto-read DOM without user gesture, (b) treat page content as untrusted input (sanitize/sandbox before passing to model), (c) separate the 'page content' context from the 'user instruction' context in the prompt structure.

Sources: Vivaldi blog https://vivaldi.com/blog/keep-exploring/, Vivaldi translate https://vivaldi.com/features/translate/, Vivaldi privacy https://vivaldi.com/privacy/browser/, LibreWolf features https://librewolf.net/docs/features/, LibreWolf issue tracker https://codeberg.org/librewolf/issues/issues/1919 and https://codeberg.org/librewolf/issues/issues/2037 and https://codeberg.org/librewolf/issues/issues/2752

**Sources:** <https://vivaldi.com/blog/keep-exploring/ — Vivaldi 'Keep Browsing Human' official statement, August 2025> · <https://vivaldi.com/blog/vivaldi-7-8-launches-with-message-to-big-tech-humans-dont-need-ai-babysitters/ — Vivaldi 7.8 release blog, engineering priorities> · <https://vivaldi.com/blog/technology/vivaldi-wont-allow-a-machine-to-lie-to-you/ — Vivaldi technical objections to LLMs (hallucination, copyright, energy)> · <https://vivaldi.com/privacy/browser/ — Vivaldi browser privacy policy, telemetry technical details> · <https://vivaldi.com/privacy/sync/ — Vivaldi Sync zero-knowledge architecture> · <https://vivaldi.com/features/translate/ — Vivaldi Translate Iceland server architecture> · <https://lingvanex.com/blog/cases/business-case-20/ — Lingvanex partnership for Vivaldi Translate> · <https://cyberinsider.com/vivaldi-rejects-ai-integration-commits-to-human-centric-browsing/ — Guardio Labs prompt injection research cited by Vivaldi> · <https://librewolf.net/docs/features/ — LibreWolf official feature documentation> · <https://librewolf.net/docs/faq/ — LibreWolf FAQ, build architecture, configuration approach> · <https://codeberg.org/librewolf/issues/issues/1919 — 'LibreWolf should delete all AI code' issue tracker thread> · <https://codeberg.org/librewolf/issues/issues/2037 — 'Remove or disable AI Chatbot' issue tracker thread> · <https://codeberg.org/librewolf/issues/issues/2752 — 'LibreWolf uses AI to read the beginning of the page' incident thread (browser.ml.enable left true accidentally)> · <https://otontechnology.com/vivaldi-anti-ai-browser-4-million-users/ — Vivaldi 4M user count, 95% anti-AI polling data> · <https://news.itsfoss.com/vivaldi-stance-on-ai/ — Summary of Vivaldi stance> · <https://www.ghacks.net/2025/08/29/vivaldi-says-no-to-ai-features/ — gHacks coverage of Vivaldi AI stance> · <https://help.vivaldi.com/desktop/privacy/is-vivaldi-open-source/ — Vivaldi open-source architecture breakdown (92%/3%/5%)> · <https://discuss.privacyguides.net/t/questions-about-the-closed-source-nature-of-vivaldi/13891 — Privacy Guides community discussion on Vivaldi closed-source UI>

