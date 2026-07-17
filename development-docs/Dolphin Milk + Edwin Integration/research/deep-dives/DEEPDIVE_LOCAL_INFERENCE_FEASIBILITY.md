# On-Device Inference Feasibility in 2026 — What Edwin Could Run Locally

> 📁 Part of the **Dolphin Milk + Edwin Integration** doc set — see `README.md`. Forensic deep-dive companion to the main study docs.
> **Created:** 2026-06-28 by a research workflow (web-cited). **STUDY, not a decision** — options & trade-offs, no winner picked. Claim tags **[FACT]/[VISION]/[INFERRED]/[SPECULATION]/[UNVERIFIED]** preserved.

## On-Device Inference Feasibility for Edwin — A 2026 Study for Hodos Browser

### Purpose & Bottom Line

Edwin is a Node.js gateway process running as a Hodos-managed OS-level sidecar — not a browser extension, not a sandboxed script. That architectural fact is the single biggest enabler for local inference: Edwin can load native GPU libraries (CUDA, Metal, Vulkan) through `node-llama-cpp`, call Windows ML via a native addon, or spawn a bundled `llama.cpp` binary with full hardware access. In 2026, small language models (SLMs) in the 3–8B parameter range, quantized to Q4_K_M, are genuinely good enough for the tasks a browser assistant must perform most often — page summarization, entity extraction, quick Q&A, tab titling, and content classification — on the majority of 2026 consumer Windows laptops, which now ship with 16–32GB RAM and, in the Copilot+ tier, 40+ TOPS NPUs. The hard constraint is the low-spec tail: machines with 8GB RAM, CPU-only, or weak integrated GPUs will have a degraded experience with any model above 3B parameters at interactive speeds. The study below maps the model landscape, runtimes, hardware realities, download UX challenges, and the local embeddings story, then lays out four tier options without choosing among them.

---

### 1. The Model Landscape — What Is Actually Good Enough in 2026

#### 1.1 Quality Tiers by Model Family

The following models are all freely redistributable (Apache 2.0 or equivalent) and run via GGUF/llama.cpp or ONNX. Benchmark figures are from 2025 technical reports and third-party evaluations; treat MMLU as a general reasoning proxy and HumanEval as a coding proxy.

**Tier A — Sub-4B, fits any 2026 machine with headroom**

| Model | Params | MMLU | HumanEval | Q4_K_M size | Notes |
|---|---|---|---|---|---|
| Qwen3-0.6B | 0.6B | ~50% [UNVERIFIED] | — | ~0.4 GB | Bare minimum; instruction following only |
| Llama 3.2-1B | 1B | ~49% | — | ~0.7 GB | Acceptable for rewriting/summarization; very weak at Q&A |
| Qwen3-1.7B | 1.7B | ~58% [INFERRED] | — | ~1.0 GB | Better multilingual than Llama 1B |
| Llama 3.2-3B | 3B | 63.4% | — | ~2.0 GB | Meta's edge-optimized; strong summarization and tool-calling |
| Phi-4-mini | 3.8B | 67–73% | 68–74% | ~2.2–2.4 GB | **Best sub-4B overall**; matches Llama 3.1 8B on MMLU |
| Gemma 3-4B | 4B | 59.6% | 36% | ~2.5–3.0 GB | Multimodal/vision capable; 140+ languages; beats Gemma 2 27B |
| Qwen3-4B | 4B | ~70% [INFERRED] | — | ~2.5–3.0 GB | 131K context; dual thinking/non-thinking mode |

[FACT sources: [LocalAIMaster SLM Guide 2026](https://localaimaster.com/blog/small-language-models-guide-2026), [Phi-4-mini Hugging Face model card](https://huggingface.co/microsoft/Phi-4-mini-instruct), [Qwen3 Technical Report](https://arxiv.org/abs/2505.09388), [Gemma 3 Google blog](https://blog.google/innovation-and-ai/technology/developers-tools/gemma-3/), [Llama 3.2 Meta blog](https://ai.meta.com/blog/llama-3-2-connect-2024-vision-edge-mobile-devices/)]

**Tier B — 7–8B, needs a GPU or 16GB+ RAM with patience**

| Model | Params | MMLU | HumanEval | Q4_K_M size | Notes |
|---|---|---|---|---|---|
| Mistral 7B | 7B | 60.1% | 30.5% | ~4.5 GB | Fast inference, good instruction following |
| Llama 3.2 (8B class) | 8B | ~73% | — | ~5.0 GB | Strong tool use; 128K context |
| Qwen3-8B | 8B | strong [INFERRED from family] | — | ~5.0 GB | Top multilingual+code in class |

**Tier C — 12–27B, for users with 16GB VRAM or 32GB RAM**

| Model | Params | MMLU | Notes |
|---|---|---|---|
| Phi-4 | 14B | 84.8% | Best MMLU/HumanEval in open sub-20B as of 2025 |
| Gemma 3-12B | 12B | strong | Fits 8GB VRAM GPU |
| Gemma 3-27B | 27B | GPT-4-class | Fits 16GB VRAM; top-10 LM Arena as of Apr 2025 |

[FACT: [LocalAIMaster SLM Guide](https://localaimaster.com/blog/small-language-models-guide-2026), [Gemma 3 Hugging Face](https://huggingface.co/blog/gemma3)]

#### 1.2 Task Suitability — Where Small Models Succeed vs Fail

**Tasks where a 3–8B Q4 model is reliably good enough for Edwin's browser use-cases:**

- **Page summarization** (extractive + abstractive, up to ~8K tokens): Llama 3.2-3B and Phi-4-mini both score well on TLDR9+. Single-pass, bounded input.
- **Entity and fact extraction** (NER, structured JSON from page content): Phi-4-mini's JSON schema enforcement (via node-llama-cpp grammar) makes this reliable.
- **Tab/page titling and auto-labeling**: Sub-2B models handle this; trivial task.
- **Quick Q&A over a retrieved passage** (RAG answer, not open-domain): Strong for 4–8B models with context. Weak for questions requiring broad world knowledge.
- **Content classification** (spam, NSFW, sentiment, topic): Even 1–3B models achieve 85–95% on common categories.
- **Short rewriting** (tone adjustment, TL;DR, translate excerpt): Excellent for 3B+, particularly Llama 3.2-3B (distilled from larger model).
- **Code snippet explanation** (show me what this JS does): Phi-4-mini and Qwen3 strong here.

**Tasks where small models fail and a cloud escalation is warranted:**

- **Deep multi-step research and synthesis** (write a 2000-word report from 10 sources): 3–8B models hallucinate, lose coherence, and lack broad factual knowledge. Need 70B+ or frontier cloud.
- **Long context reasoning** (>32K tokens of page content): KV cache grows, quality degrades; 3B models with 32K context underperform 8B models with same context.
- **Coding assistance for complex tasks** (write a full React component, debug multi-file issues): Phi-4-mini is better than expected but below GPT-4o class.
- **Open-domain factual Q&A** (current events, obscure facts, math with many steps): Small models confabulate confidently. This is a serious UX risk if Hodos surfaces ungrounded answers.
- **Agentic multi-turn planning** (Edwin's current server-sent tool-calling loop): Sub-8B models often fail to follow complex tool-call schemas across many turns reliably.

[FACT/INFERRED: [LocalAIMaster SLM Guide](https://localaimaster.com/blog/small-language-models-guide-2026), [Phi-4-mini Technical Report PDF](https://arxiv.org/pdf/2503.01743), [Meta Llama 3.2 blog](https://ai.meta.com/blog/llama-3-2-connect-2024-vision-edge-mobile-devices/)]

---

### 2. Runtimes — What Edwin Can Actually Use

#### 2.1 llama.cpp (Foundation Layer)

llama.cpp is the de facto open-source inference engine for quantized GGUF models. It supports CPU, CUDA (NVIDIA), Metal (Apple), and Vulkan (cross-platform GPU). The GGUF format bundles weights plus metadata in a single file. All serious local inference tools build on top of llama.cpp or compete directly with it.

- **CPU performance on modern hardware:** ~14.2 tok/s for Llama 3.1 8B Q4_K_M at 12 threads (Ollama v0.5.7 benchmark). Smaller 3B models: ~15–25 tok/s CPU-only. [FACT: [Ollama CPU benchmark](https://markaicode.com/benchmarks/tool-cpu-benchmark/)]
- **GPU performance:** ~145 tok/s for 8B Q4 on RTX 4090; ~20–50 tok/s on consumer 8GB gaming GPUs. [FACT: [Ollama benchmark site](https://ai-ollama.github.io/benchmarks.html)]
- **KV cache quantization** reduces context VRAM overhead by ~50% (Q8 KV cache).

**Verdict for Edwin:** llama.cpp is the engine under every approach below. Edwin does not call it directly.

#### 2.2 Ollama

Ollama wraps llama.cpp in a user-friendly daemon with a REST API and a model registry (`ollama pull phi4-mini`). It auto-detects GPU backends and manages model files.

- **Strength:** Best-in-class setup UX; users already have it; REST API Edwin can call.
- **Weakness for Edwin:** Requires a *separate* Ollama installation by the user. Edwin cannot bundle or guarantee it. If Hodos targets non-technical users, "install Ollama first" is a friction point. Version mismatch and port conflicts are support surface.
- **Use case:** BYO tier — a power-user option where Hodos lets users point Edwin at a local Ollama endpoint.

[FACT: [Ollama](https://ai-ollama.github.io/benchmarks.html), [Running LLMs locally 2026 - daily.dev](https://daily.dev/blog/running-llms-locally-ollama-llama-cpp-self-hosted-ai-developers/)]

#### 2.3 node-llama-cpp — The Key Native Option for Edwin

**This is Edwin's most natural local inference path.** `node-llama-cpp` (v3.18.1 as of mid-2026, maintained by `withcatai`) provides native Node.js/Bun/Electron bindings directly to llama.cpp with zero-config defaults.

Key capabilities relevant to Edwin:
- **Auto-hardware detection:** `npx -y node-llama-cpp inspect gpu` reports available backends at install time; automatically selects CUDA > Metal > Vulkan > CPU.
- **Pre-built binaries** for Windows x64/ARM64, macOS (x86 and Apple Silicon), and Linux — no node-gyp, no Python, no cmake required for the common case. Falls back to source compilation with cmake if needed.
- **TypeScript-native** with full type safety — matches Edwin's likely TypeScript codebase.
- **JSON schema grammar enforcement** at the generation level — critical for reliable structured extraction without post-processing.
- **Function calling** for agentic tool use.
- **Embeddings** and reranking built in — directly usable with sqlite-vec.
- **Built-in model downloader** — can programmatically download GGUF files from Hugging Face with progress reporting.
- **Smart context shift** — manages context window overflow gracefully.
- **Windows on Arm support** — relevant for Snapdragon X Elite Copilot+ PCs.

It is already listed as an optional peer dependency in Edwin's ecosystem (`node-llama-cpp` is mentioned as Edwin's OPTIONAL peer dep per the task description). This means the integration pathway is already anticipated upstream. Hodos could contribute PRs to Edwin to activate this path.

- **Constraint:** Loading a model adds ~2–10 seconds cold-start for GPU model loading. Subsequent calls are fast.
- **Constraint:** Cannot call Windows ML/NPU directly — uses GPU via Vulkan/CUDA/Metal only. NPU path requires separate approach (see 2.5).

[FACT: [node-llama-cpp GitHub](https://github.com/withcatai/node-llama-cpp), [node-llama-cpp site](https://node-llama-cpp.withcat.ai/), [npm](https://www.npmjs.com/package/node-llama-cpp)]

#### 2.4 ONNX Runtime (Node.js / Web)

Microsoft's ONNX Runtime (`onnxruntime-node`) is the server-side Node.js package; `onnxruntime-web` targets browsers via WebAssembly/WebGPU.

- **Good fit for:** Classifier models, embedding models, and small task-specific models exported to ONNX. 2x GPU speedup over CPU WebAssembly in browser context; larger gains on dedicated models.
- **Not ideal for:** Large generative LLMs. ONNX lacks llama.cpp's GGUF quantization and KV cache management optimizations. Most GGUF models cannot be trivially converted to ONNX with equivalent quantization quality.
- **Windows ML integration:** Windows ML GA (Sept 2025) uses ONNX as its model format. `onnxruntime-node` can use Windows ML execution providers on Windows 11 24H2+ via the `windowsml` execution provider flag. Microsoft has Electron/WinML addon documentation (updated May 2026). [FACT: [Windows ML GA blog](https://blogs.windows.com/windowsdeveloper/2025/09/23/windows-ml-is-generally-available-empowering-developers-to-scale-local-ai-across-windows-devices/)]

**Verdict for Edwin:** ONNX Runtime Node.js is worth using for lightweight embedding generation (nomic-embed in ONNX) and classifiers, but `node-llama-cpp` is the better choice for generative tasks.

#### 2.5 Windows ML / DirectML / NPUs

Windows ML became generally available in September 2025 (Windows App SDK 1.8.1+, Windows 11 24H2+). It provides a hardware abstraction layer across CPU, GPU, and NPU, automatically dispatching to the best available execution provider per vendor (NVIDIA TensorRT-RT, AMD ROCm, Intel OpenVINO, Qualcomm HTP).

- **NPU access:** Qualcomm Snapdragon X Elite, AMD Ryzen AI 300/400 (XDNA 2), Intel Core Ultra 2/3. In 2026, all new Copilot+ PCs ship with 40+ TOPS NPUs. [FACT: [Windows Local AI AMD NPU](https://www.amd.com/en/developer/resources/technical-articles/2026/ai-model-deployment-using-windows-ml-on-amd-npu.html), [Windows ML intro](https://blogs.windows.com/windowsdeveloper/2025/05/19/introducing-windows-ml-the-future-of-machine-learning-development-on-windows/)]
- **Model format:** ONNX only. NPUs run small ONNX models; they are NOT designed for large generative GGUF LLMs (the TOPS rating is for matrix multiply throughput on vision/embedding/small classifier workloads, not autoregressive 7B text generation). [INFERRED from NPU architecture knowledge]
- **Node.js access:** Possible via native Node-API addons (Microsoft's Electron WinML addon docs, May 2026). Not a turnkey npm package today. Building and shipping this requires significant native code work and Windows-only build pipeline.
- **DirectML:** Now legacy; Windows ML is the recommended path.

**WSL 3 relevance:** At Build 2026 (June 2026), Microsoft previewed WSL 3 with near-native GPU/NPU passthrough (within 3–5% of bare-metal), Qualcomm and Intel supported at launch, AMD planned later. Edwin currently runs natively on Windows, not inside WSL, but this is relevant for future Linux-first builds. [FACT: [WSL 3 Build 2026 - TechTimes](https://www.techtimes.com/articles/317598/20260602/wsl-3-build-2026-near-native-gpu-npu-passthrough-brings-local-ai-windows.htm), [Neowin](https://www.neowin.net/forum/topic/1466958-wsl-3-at-build-2026-near-native-gpu-and-npu-passthrough-brings-local-ai-windows)]

**Verdict for Edwin:** Windows ML is compelling for NPU-accelerated embedding/classification on Copilot+ PCs, but adds significant build complexity. Not a Day 1 path. Possibly a future Edwin PR as a Windows-specific execution provider.

#### 2.6 transformers.js / WebGPU (Browser-Sandboxed)

Hugging Face's `transformers.js` v3 runs models in the browser via WebAssembly or WebGPU. It is the approach Firefox and Chrome AI integrations use for in-page inference.

**This is explicitly NOT what Edwin should use.** Edwin is an OS-level Node.js process, not a browser extension. The browser sandbox:
- Caps GPU memory access (crashes with >2GB models on integrated GPUs, error 77285704)
- Has no CUDA or Metal — WebGPU only (Chrome 113+, Edge, Firefox partial, Safari experimental)
- Is slower than native: a cloud API at 100ms vs 2–5 seconds for a complex sentence in WebGPU
- Has 70% browser support globally as of 2024 for WebGPU

`transformers.js` is the right choice if Hodos were building a browser extension. Edwin's OS-process architecture avoids all these constraints. This is the key competitive advantage Edwin has over browser-sandboxed competitors.

[FACT: [Transformers.js v3 blog](https://huggingface.co/blog/transformersjs-v3), [Sitepoint WebGPU vs WebASM](https://www.sitepoint.com/webgpu-vs-webasm-transformers-js/)]

#### 2.7 Apple Foundation Models Framework

At WWDC 2025, Apple released the Foundation Models framework (macOS 26 / iOS 26+). It exposes Apple's on-device ~3B parameter model (the same one powering Apple Intelligence) via Swift APIs — no API key, no internet required, free to use in apps.

iOS 27 additions: larger model, on-device fine-tuning, image input, full tool calling, and the new public `LanguageModel` protocol for third-party model providers. [FACT: [Apple developer docs](https://developer.apple.com/documentation/FoundationModels), [Apple newsroom Sept 2025](https://www.apple.com/newsroom/2025/09/apples-foundation-models-framework-unlocks-new-intelligent-app-experiences/)]

**Critical constraint for Edwin:** The Foundation Models framework is Swift-only. Edwin is Node.js. Accessing it from Edwin on macOS requires a platform-specific native addon that bridges Swift to Node-API. This is non-trivial. It is *possible* (similar to how apps call Swift frameworks from C via bridging headers and compile with cmake), but there is no existing npm package that does this. Any such bridge would be macOS 26+ only, not backward compatible.

**Verdict for Edwin:** Do not plan on AFM in the near term. If Hodos targets Mac users heavily, this is a future upstream Edwin PR to propose to Jake: a `node-llama-cpp`-style optional peer dep that wraps AFM via a Swift native addon. The quality-per-watt on Apple Silicon is exceptional.

---

### 3. Hardware Reality

#### 3.1 RAM and VRAM Requirements by Model + Quantization

The following covers weights plus KV cache at 8K context, which covers the majority of browser assistant tasks (a typical web page is 2–8K tokens).

| Model | Q4_K_M weight size | Total RAM for 8K ctx | Q8_0 weight size | Total RAM Q8 8K ctx |
|---|---|---|---|---|
| Qwen3 0.6B | 0.4 GB | ~0.8 GB | 0.7 GB | ~1.1 GB |
| Llama 3.2 1B | 0.7 GB | ~1.0 GB | 1.3 GB | ~1.6 GB |
| Qwen3 1.7B | 1.0 GB | ~1.4 GB | 1.9 GB | ~2.3 GB |
| Llama 3.2 3B | 2.0 GB | ~2.5 GB | 3.8 GB | ~4.3 GB |
| Phi-4-mini 3.8B | 2.2–2.4 GB | ~3.0 GB | 4.1 GB | ~4.8 GB |
| Gemma 3 / Qwen3 4B | 2.5–3.0 GB | ~3.5 GB | 4.5–5.5 GB | ~5.5 GB |
| Mistral 7B | 4.5 GB | ~6.0 GB | 7.5 GB | ~9.0 GB |
| Llama 3.x 8B | 5.0 GB | ~6.5 GB | 8.5 GB | ~10.0 GB |
| Phi-4 14B | 8.5 GB | ~10.5 GB | 14.5 GB | ~16.5 GB |
| Gemma 3 12B | 7.0 GB | ~9.0 GB | 12.5 GB | ~14.5 GB |
| Gemma 3 27B | 15–16 GB | ~17 GB | 27 GB | ~29 GB |

[FACT/CALCULATED: [GGUF Memory Calculator](https://ggufloader.github.io/gguf-memory-calculator.html), [llama.cpp VRAM guide](https://localllm.in/blog/llamacpp-vram-requirements-for-local-llms), [Sitepoint VRAM 70B guide](https://www.sitepoint.com/vram-requirements-70b-models-16gb-gpu-minimum-2026/)]

**Rule of thumb:** Dedicated VRAM is fastest (the whole model loads to GPU). If the model overflows VRAM, the excess loads to system RAM and runs on CPU — dramatically slower (sometimes 2–5x). A machine with 8GB dedicated VRAM can fit a 7–8B Q4 model comfortably but nothing larger.

KV cache quantization (Q8 KV or Q4 KV in llama.cpp) cuts context VRAM penalty by 50–75%, enabling longer-context inference on tighter hardware.

#### 3.2 CPU-Only vs GPU vs NPU Inference

| Path | Typical speed (8B Q4) | Typical speed (3B Q4) | Best use case |
|---|---|---|---|
| CPU-only (modern 12-core DDR5) | 8–15 tok/s | 15–25 tok/s | Low-end fallback; batch summarize |
| Integrated GPU (iGPU, 8–16GB shared) | 15–35 tok/s | 30–50 tok/s | Mainstream Copilot+ laptops |
| Discrete GPU 8GB VRAM | 20–60 tok/s | 50–100 tok/s | Gaming/creator laptops |
| Discrete GPU 24GB VRAM | 60–145 tok/s | 120+ tok/s | RTX 3090/4090 class |
| NPU (Copilot+ via Windows ML, 3–4B ONNX) | 30–80 tok/s [INFERRED] | 60–120 tok/s [INFERRED] | Copilot+ embedding/classifier |

[FACT: [Ollama benchmarks](https://ai-ollama.github.io/benchmarks.html), [Markaicode RTX 5090](https://markaicode.com/benchmarks/rtx-5090-tokens-per-second-benchmark/), [Markaicode CPU](https://markaicode.com/benchmarks/tool-cpu-benchmark/)]

**Usability thresholds:** Below ~8 tok/s, streaming text feels sluggish for interactive Q&A. For batch tasks (summarize this page while the user keeps browsing), even 5 tok/s is acceptable. For classifiers and embeddings, latency is measured in milliseconds, not tok/s.

#### 3.3 The 2026 Typical Consumer Windows Laptop

[FACT: [Copilot+ PC guide ACEMAGIC](https://acemagic.com/blogs/about-ace-mini-pc/microsoft-copilot-plus-pcs-explained), [Windows Forum AI PC 2026](https://windowsforum.com/threads/ai-pc-2026-guide-what-it-really-means-for-buyers-and-copilot.402716/), [NPU Comparison 2026](https://localaimaster.com/blog/npu-comparison-2026)]

Three tiers of hardware Hodos must plan for in 2026:

**Tier 1 — Copilot+ PC (growing rapidly, 2026 mainstream new purchase)**
- RAM: 16–32GB LPDDR5X (16GB minimum required by Microsoft for Copilot+)
- SoC/CPU: Snapdragon X Elite/X2 Elite, Intel Core Ultra 2/3, or AMD Ryzen AI 300/400
- NPU: 40–100+ TOPS (Intel, AMD, Qualcomm)
- GPU: No dedicated dGPU; powerful iGPU with GPU memory drawn from system RAM
- Can run: 3–8B models at 15–50 tok/s via iGPU; embedding models at low latency via NPU (via Windows ML)
- The Snapdragon X2 Elite ships with up to 128GB unified LPDDR5X — a 70B-class model fits in RAM

**Tier 2 — Mid-range gaming/creator laptop (2022–2025 vintage)**
- RAM: 16–32GB DDR5
- GPU: Dedicated 6–12GB VRAM (RTX 3060/4060/3070 class)
- NPU: None or 10–15 TOPS
- Can run: 7–8B Q4 at 30–60 tok/s; 14B Q4 with partial CPU offload

**Tier 3 — Low-spec or older machine (significant installed base)**
- RAM: 8GB DDR4
- GPU: Integrated only (Intel UHD, AMD Vega)
- NPU: None
- Can run: 3B Q4 at 15–25 tok/s on CPU (tight but workable); 7B Q4 is strained (4.5GB weights + system overhead = near-OOM on 8GB)
- This is the "low-spec problem" — see Section 3.4

#### 3.4 The Low-Spec Machine Problem

An 8GB RAM machine with no dedicated GPU is not rare in 2026. The Tier-3 population includes many corporate laptops, education machines, and older personal hardware.

For these machines:
- A 7B Q4 model requires ~6GB of RAM just for weights+cache, leaving 2GB for OS, browser, and Edwin itself. This causes OOM-induced quality degradation or crashes.
- A 3B Q4 model (~2.5GB total) runs safely, but quality noticeably drops for multi-hop reasoning, nuanced extraction, and anything requiring broader world knowledge.
- CPU-only 3B inference at 15–25 tok/s is borderline acceptable for summarization (not noticeable when shown at the end), but feels slow for interactive Q&A (streaming 1–2 seconds of latency before first token).

**Options for low-spec users:**
1. Auto-detect available RAM/VRAM at Edwin startup; select a smaller model (1B or 3B) automatically.
2. Gracefully route to cloud for tasks beyond the local model's capability.
3. Tell the user clearly: "Your device doesn't meet the recommended specs for full local AI. Edwin will use cloud inference." With a user-sovereign UX, this must be an *opt-in* to cloud, not a silent default.

Edwin can probe available memory and GPU via `npx -y node-llama-cpp inspect gpu` — which is a built-in CLI tool in node-llama-cpp — before attempting any model load.

---

### 4. Edwin's OS-Level Advantage vs Browser-Sandboxed Inference

This section is the "why Edwin is uniquely positioned" argument. It is stated as a structural fact, not marketing.

**What browser-sandboxed inference faces:**

Chrome's Built-in AI (the Gemini Nano integration) and any browser extension using transformers.js/WebGPU operate under:
- **WebGPU memory cap:** Browsers limit GPU memory per origin. Dense models above ~2GB crash with `GPUBufferOutOfMemory` (error 77285704) on integrated GPUs.
- **No CUDA/Metal:** WebGPU is a portable abstraction that cannot use CUDA TensorCores or Metal Performance Shaders. Performance is 2–5x lower than native.
- **Process sandbox:** The renderer process cannot access system RAM above the browser's tab allocation. On an 8GB machine, a renderer process typically has access to 1.5–2GB of memory.
- **No persistent model residency:** Each page load may need to reload the model unless careful SharedArrayBuffer/service worker tricks are used.
- **70% WebGPU support** globally as of 2024; Firefox support is partial; Safari is experimental.

[FACT: [transformers.js WebGPU issue #796](https://github.com/huggingface/transformers.js/issues/796), [Sitepoint WebGPU vs WebASM](https://www.sitepoint.com/webgpu-vs-webasm-transformers-js/), [Krisdigital NLP blog 2026](https://www.krisdigital.com/en/blog/2026/04/07/nlp-in-browser-webgpu/)]

**What Edwin gets as an OS process:**
- Full CUDA/Metal/Vulkan access through `node-llama-cpp` native bindings.
- Full system RAM access — a 16GB machine is a 16GB machine.
- Persistent model residency: load the model once at startup, keep it in memory for the session.
- GPU stays loaded between requests: no cold-start per query after initial load (~2–10s one-time warm-up).
- Can fork a child process (e.g., `llama.cpp` binary as a subprocess) or link natively — either works.
- Windows ML execution providers accessible via native addon.
- Cross-platform consistently: macOS, Windows, and Linux without browser compatibility matrix concerns.

This means Edwin's local inference throughput on the same machine is roughly **2–5x higher** than a browser-sandboxed approach on the same hardware, and can run models 3–4x larger. A Hodos user with a mid-range GPU gets a meaningfully better local AI experience than any browser-extension-based competitor.

---

### 5. Model Download UX — The Informed Consent Problem

Downloading a language model is a significant ask of a non-technical user. A 3B Q4 file is ~2GB; a 7B Q4 file is ~4.5GB. This is not a background update — it is a deliberate event requiring disclosure, consent, and progress visibility.

**What good UX looks like (derived from LM Studio and Jan's approaches):**

- **Size disclosure before download:** "Downloading the assistant model (Phi-4-mini, 2.3 GB). This is a one-time download on Wi-Fi. Estimated time: 4–8 minutes."
- **Hardware check first:** Edwin should inspect GPU/RAM before recommending a model tier. Suggest the smallest model that will give acceptable quality for the user's hardware.
- **Explicit opt-in:** For privacy-conscious users who chose Hodos specifically, the model download should feel like a feature, not a trap. Frame it as "your assistant stays on your device, never leaves." This aligns with Hodos's positioning.
- **Defer option:** "Use cloud inference for now, download your local model later." Avoids a blocking install step.
- **Storage location transparency:** Tell users where the model is stored (e.g., `%APPDATA%\Hodos\models\`) and how to delete it.
- **No auto-updates without consent:** Model updates change behavior. Users should know when a model changes. Show a diff-style summary ("Updated from Phi-4-mini 2.3 to Phi-4-mini 2.4 — improved reasoning") and require acknowledgment.
- **Progress indicator:** Download progress, verify hash/checksum after download. `node-llama-cpp`'s built-in model downloader supports progress callbacks.

**Model management footprint concern:** A user who has Phi-4-mini (2.3GB), Llama 3.2-3B (2.0GB), and a nomic-embed model (0.15GB) has ~4.5GB of model storage. This is manageable. Hodos should provide a model manager UI.

[FACT: [LM Studio UX](https://dev.to/lightningdev123/top-5-local-llm-tools-and-models-in-2026-1ch5), [Local LLMs Sept 2025](https://enclaveai.app/blog/2025/09/06/latest-advancements-local-llms-september-2025/)]

---

### 6. Local Embeddings and Recall — sqlite-vec Is Already There

Edwin lists `sqlite-vec` as an existing dependency. This is directly relevant to local RAG (retrieval-augmented generation) and context recall without cloud API costs.

#### 6.1 What sqlite-vec Provides

`sqlite-vec` by Alex Garcia is a SQLite extension written in C that adds vector operations (ANN search, cosine similarity, Hamming distance for binary vectors) directly to SQLite. It eliminates the need for a separate vector database process. Combined with SQLite's FTS5 full-text search and Reciprocal Rank Fusion (RRF), it implements a production-quality hybrid search system in a single embedded file.

- **Scale ceiling:** Works well to ~200K–500K documents. Beyond that, a dedicated vector DB (e.g., Chroma, Qdrant) is the upgrade path. For browser browsing history and bookmarks, 200K chunks is far beyond what most users accumulate.
- **Performance:** On a standard machine, embedding 182 documentation files (~640 words avg) takes ~25 minutes (one-time index build). Query latency is milliseconds.

[FACT: [sqlite-vec AI in Plain English](https://ai.plainenglish.io/embedded-intelligence-how-sqlite-vec-delivers-fast-local-vector-search-for-ai-de6d62936055), [Local-First RAG sitepoint](https://www.sitepoint.com/local-first-rag-vector-search-in-sqlite-with-hamming-distance/)]

#### 6.2 Embedding Model Options

The embedding model generates the vectors. Options for local use:

| Model | Params | Size | MTEB score | Notes |
|---|---|---|---|---|
| all-MiniLM-L6-v2 | 22M | ~23 MB | Good baseline | Fast, small, reasonable quality |
| nomic-embed-text-v1 | 137M | ~150 MB | Beats OpenAI ada-002 and text-embedding-3-small | Best quality/size ratio; 8192 context |
| mxbai-embed-large | 335M | ~670 MB | Top-tier | High quality; runs on any ONNX-capable device |

**Key fact:** As of April 2026, six open embedding models sit in the MTEB top 20, several above OpenAI `text-embedding-3-large`. `nomic-embed-text` outperforms both `ada-002` and `text-embedding-3-small` on short and long context benchmarks. [FACT: [Local vs OpenAI Embeddings 2026](https://localaimaster.com/blog/local-vs-openai-embeddings), [Elephas embedding models guide](https://elephas.app/blog/best-embedding-models)]

**For Edwin:** `nomic-embed-text` via `node-llama-cpp` (it supports embedding mode on GGUF models) is the natural choice. The 150MB download is negligible vs the generative model. This enables:
- **Semantic page memory:** "Find that article about X I read last month" — embedding-based recall from browser history.
- **Page-to-page similarity:** Surfacing related content from browsing history.
- **RAG over bookmarks and notes:** Give Edwin context from the user's personal knowledge base without cloud API.
- **Context injection at inference time:** Retrieve relevant chunks, prepend to prompt — keeps the generative model small while boosting effective knowledge.

Local embeddings also resolve a privacy concern: cloud embedding services (OpenAI, Cohere) see every chunk of text the user indexes. Local embeddings mean the user's browsing history and notes never leave the device.

---

### 7. Tier Options for Edwin's Local Inference Layer

The following four options are stated without a recommendation. Each has honest trade-offs relevant to Hodos's north star (casual user ease + privacy + x402 micropayment monetization).

#### Option A — Local-Only Default (Privacy Maximum)

**What it is:** Edwin ships with `node-llama-cpp` as an activated (not optional) dependency. On first launch, Edwin runs a hardware check and downloads the best-fit model from a Hodos-curated list. No cloud inference happens unless the user explicitly opts in.

**Pros:**
- Maximum privacy alignment — nothing leaves the device.
- No API key management for the user.
- Zero per-query cloud cost — aligns with x402's micropayment model (users pay once for the model download, then get unlimited local inference).
- Works offline (airplane mode, corporate firewall, restricted networks).
- Differentiates Hodos from "another AI browser that sells your data to train models."

**Cons:**
- Requires model download (2–5GB) on first use — friction for casual users.
- Quality cap at 7–8B for most hardware. Tasks requiring frontier-model reasoning will produce visibly worse output.
- Low-spec machines (8GB RAM, CPU-only) will have noticeably slower and lower-quality inference, potentially damaging the first-run impression.
- Harder cold-start: Edwin process must load model into memory (~2–10s) before first query.
- No path to "I need a better answer" without a UX redesign.

**Model footprint:** 2–5GB on disk for the generative model, ~150MB for embeddings.

#### Option B — Hybrid Local + Cloud Routing (Quality Preservation)

**What it is:** Edwin runs a local model by default for the majority of tasks (estimated 85–95% of browser assistant queries are simple: summarize, title, classify, short Q&A). Complex or long-context tasks are automatically routed to a cloud model (Claude, GPT-4o, or a Hodos-run endpoint). Users see a "local" or "cloud" indicator per response.

A routing layer (already a known pattern in 2026 production systems) decides by task complexity:
- Token count > threshold (e.g., 8K) → cloud
- Multi-step agentic task (tool chain > 3 steps) → cloud
- Sensitive data detected → local-forced
- Simple extraction/classification/summary → local

[FACT: [Hybrid Cloud-Local guide 2026](https://www.sitepoint.com/hybrid-cloudlocal-llm-the-complete-architecture-guide-2026/), [LocalAIMaster hybrid](https://localaimaster.com/blog/hybrid-local-cloud-ai), [Perplexity hybrid at Computex 2026](https://venturebeat.com/technology/perplexity-ai-unveils-hybrid-local-cloud-inference-system-at-computex-2026/)]

**Pros:**
- Best quality/privacy balance — most queries stay local.
- Graceful degradation: low-spec machines route more to cloud without a broken experience.
- Enables x402 micropayment monetization for cloud calls (user sees "this query used 2 BSV satoshis for cloud inference").
- Cloud fallback means users aren't stuck with degraded output for hard tasks.
- Routing logic can be tuned: privacy-first users can set "local always, even if slower/worse."

**Cons:**
- More complex to build and test (routing logic, latency budget, fallback chains).
- "Cloud" queries still require an API key or x402 payment — UX for first-time cloud use must be smooth.
- The "local vs cloud" indicator may confuse non-technical users or raise questions ("why did this go to cloud?").
- Network dependency reintroduced for the cloud path, even if infrequent.

**Typical cost in hybrid model:** 85–95% of queries stay local. Cloud overhead is small but real. At x402 micropayments, the per-query cost for cloud is payable frictionlessly — this is actually an argument *for* hybrid, because x402 turns the "cloud costs money" problem into a feature (pay only for what you need, instantly, without a subscription).

#### Option C — Cloud-Only, Local Optional (Lowest Complexity)

**What it is:** Edwin uses cloud APIs by default; a local model is an opt-in advanced feature for users who set it up. No model download on first run.

**Pros:**
- Easiest to implement and ship.
- Consistent quality regardless of hardware.
- No model management surface.

**Cons:**
- Privacy contradiction: Hodos is a privacy-focused browser, and a cloud-by-default AI assistant that processes your browsing content with a third-party API is a philosophical conflict.
- Ongoing API costs (user or Hodos absorbs them) — unless x402 micropayments are the mechanism, in which case every query costs BSV.
- Loses the differentiation that Edwin as OS-native sidecar provides.
- Users on restricted networks or offline lose all AI capability.

#### Option D — BYO (Bring Your Own Endpoint)

**What it is:** Edwin ships with no bundled model. Users configure their AI backend: a local Ollama URL, an OpenAI API key, an Anthropic API key, or a custom OpenAI-compatible endpoint. Edwin communicates over the same protocol regardless.

**Pros:**
- Maximum user control — aligns with "user-sovereign" philosophy.
- No model download imposed by Hodos.
- Power users already have Ollama running.
- Zero first-party AI hosting liability for Hodos.

**Cons:**
- Terrible experience for the casual user who has none of these and doesn't know what Ollama is.
- "Configure your AI backend" as a first-run screen is a non-starter for mainstream adoption.
- Cannot provide a coherent default AI experience.

**Most likely role:** BYO is an advanced setting available in all other tier options, not a stand-alone tier. Power users should always be able to override Edwin's backend.

---

### What This Means for Hodos — Options, Not a Pick

**The structural opportunity:** Edwin's OS-level sidecar architecture makes local inference meaningfully better than anything a browser extension or browser-built-in approach can deliver on the same hardware. This advantage is real and worth building on. The question is when and how, not whether.

**On model selection:** Phi-4-mini (3.8B, Q4_K_M, ~2.3GB) or Qwen3-4B are the natural "Hodos default local model" candidates in 2026 — best reasoning per gigabyte, fits all mainstream hardware, Apache 2.0 license, strong structured output via node-llama-cpp grammar. Llama 3.2-3B is the right fallback for low-spec machines. These are not locked-in; the field moves fast and the model can be swapped.

**On the monetization intersection:** Hybrid routing (Option B) creates the most natural fit for x402 micropayments. A user who wants a better answer for a complex query pays a few satoshis to route to a frontier cloud model. Local handles the free tier; cloud handles the premium tier. This is a genuinely novel monetization model — not a subscription, not ad-funded, but usage-priced at a granularity only BSV micropayments make practical.

**On the low-spec problem:** Hodos cannot ignore it. The casual user Hodos wants to reach may own an older 8GB RAM laptop. The system must auto-detect and gracefully handle this: smaller model tier automatically, or transparent cloud fallback with clear privacy disclosure, or a combination.

**On Windows ML/NPU:** This is the 2027 story, not 2026. The NPU ecosystem (Windows ML GA, Copilot+ PC installed base) is growing, but the Node.js native addon work required to use it from Edwin is non-trivial. Worth watching and contributing upstream to Edwin when the tooling matures.

**On Apple Foundation Models:** Not usable from Edwin today. If Hodos on macOS becomes important, a Swift bridge native addon is a worthwhile long-term Edwin PR. The quality and power efficiency on Apple Silicon would be excellent.

**On sqlite-vec + local embeddings:** This is low-hanging fruit. sqlite-vec is already a dependency. Adding nomic-embed-text as a local embedding model (~150MB download) enables semantic browsing history search, related-page discovery, and local RAG with no privacy cost. This does not require the large generative model and can be shipped independently as "smart search."

---

### Open Questions

1. **What is Jake's current plan for node-llama-cpp activation in Edwin?** It is listed as optional — is it fully wired as a code path, or still a stub? Hodos needs to understand the implementation state before drafting PRs.

2. **What is Edwin's cold-start tolerance?** Loading a 2–3B model takes 2–5 seconds; a 7B model takes 5–10 seconds. Does Edwin have a background warm-up strategy, or does it load on first query? Warm-up on browser launch vs warm-up on first AI request is a UX trade-off.

3. **What is the agreed "sensitivity detection" policy?** Hybrid routing requires a rule (local-forced for sensitive data). Who defines "sensitive"? Credit card patterns? Location? Medical terms? User-controlled? This has privacy and legal implications.

4. **How does the x402 micropayment flow integrate with model routing?** If a cloud query costs X satoshis, does Edwin initiate the payment silently or show a confirmation? At what cost threshold does confirmation appear? How does the user's BSV wallet (Rust subprocess) connect to Edwin's routing decision?

5. **Is there a model update cadence plan?** Models improve every few months. Hodos needs a policy: auto-update models (risk: behavior changes), notify and offer update (safer), or pin forever (model gets stale). Model update consent is a user-trust issue.

6. **How does model storage interact with Hodos's installer and uninstaller?** The model files (2–5GB) must be managed cleanly — not stranded on disk after uninstall, not deleted accidentally on update.

7. **What is the plan for Windows ARM (Snapdragon X Elite)?** node-llama-cpp supports Windows on ARM (documented feature), but Vulkan/ONNX performance on Snapdragon differs from x86. Testing matrix needs to cover this explicitly.

8. **Is there a target minimum spec for "supported" vs "best-effort" hardware?** Defining "Hodos supports 16GB+ RAM machines" vs "8GB is best-effort" helps scope the problem and set user expectations honestly.

9. **How does the privacy disclosure for hybrid routing get worded for a non-technical user?** "This response was generated locally" / "This response used cloud inference" is a binary that might need nuance (what data went to the cloud? was the page content included or just the user's question?).

10. **Can nomic-embed-text be shipped as a Hodos-exclusive feature independent of the generative model?** Local semantic search over browsing history is a concrete, low-risk capability that doesn't require solving the full local LLM story first. This may be the right first ship.

---

### Sources

- [LocalAIMaster Small Language Models Guide 2026](https://localaimaster.com/blog/small-language-models-guide-2026)
- [LocalAIMaster Phi-4-mini Guide](https://localaimaster.com/models/phi-4-mini)
- [Meta-Intelligence SLM Enterprise 2026](https://www.meta-intelligence.tech/en/insight-slm-enterprise)
- [Sitepoint Best Local LLM Models 2026](https://www.sitepoint.com/best-local-llm-models-2026/)
- [Phi-4-mini Technical Report (arXiv)](https://arxiv.org/pdf/2503.01743)
- [Phi-4-mini Hugging Face model card](https://huggingface.co/microsoft/Phi-4-mini-instruct)
- [Qwen3 Technical Report (arXiv)](https://arxiv.org/abs/2505.09388)
- [Qwen3 GitHub](https://github.com/QwenLM/qwen3)
- [Gemma 3 Google blog](https://blog.google/innovation-and-ai/technology/developers-tools/gemma-3/)
- [Gemma 3 Hugging Face](https://huggingface.co/blog/gemma3)
- [Gemma 3 hardware requirements](https://llmhardware.io/guides/gemma3-hardware-requirements)
- [Meta Llama 3.2 blog](https://ai.meta.com/blog/llama-3-2-connect-2024-vision-edge-mobile-devices/)
- [Mistral AI Review 2026](https://aitoolranked.com/blog/mistral-ai-review-open-source-performance-fine-tuning-deployment-options)
- [node-llama-cpp GitHub](https://github.com/withcatai/node-llama-cpp)
- [node-llama-cpp official site](https://node-llama-cpp.withcat.ai/)
- [node-llama-cpp npm](https://www.npmjs.com/package/node-llama-cpp)
- [Windows ML GA blog (Sept 2025)](https://blogs.windows.com/windowsdeveloper/2025/09/23/windows-ml-is-generally-available-empowering-developers-to-scale-local-ai-across-windows-devices/)
- [Windows ML intro blog (May 2025)](https://blogs.windows.com/windowsdeveloper/2025/05/19/introducing-windows-ml-the-future-of-machine-learning-development-on-windows/)
- [AMD Windows ML NPU article (2026)](https://www.amd.com/en/developer/resources/technical-articles/2026/ai-model-deployment-using-windows-ml-on-amd-npu.html)
- [NPU Comparison 2026](https://localaimaster.com/blog/npu-comparison-2026)
- [Copilot+ PC guide](https://acemagic.com/blogs/about-ace-mini-pc/microsoft-copilot-plus-pcs-explained)
- [Windows Forum AI PC 2026](https://windowsforum.com/threads/ai-pc-2026-guide-what-it-really-means-for-buyers-and-copilot.402716/)
- [WSL 3 Build 2026 — TechTimes](https://www.techtimes.com/articles/317598/20260602/wsl-3-build-2026-near-native-gpu-npu-passthrough-brings-local-ai-windows.htm)
- [WSL 3 — Neowin](https://www.neowin.net/forum/topic/1466958-wsl-3-at-build-2026-near-native-gpu-and-npu-passthrough-brings-local-ai-windows)
- [Ollama benchmarks](https://ai-ollama.github.io/benchmarks.html)
- [Markaicode RTX 5090 benchmark](https://markaicode.com/benchmarks/rtx-5090-tokens-per-second-benchmark/)
- [Markaicode CPU benchmark](https://markaicode.com/benchmarks/tool-cpu-benchmark/)
- [GGUF Memory Calculator](https://ggufloader.github.io/gguf-memory-calculator.html)
- [llama.cpp VRAM guide 2026](https://localllm.in/blog/llamacpp-vram-requirements-for-local-llms)
- [Transformers.js v3 blog](https://huggingface.co/blog/transformersjs-v3)
- [Sitepoint WebGPU vs WebASM](https://www.sitepoint.com/webgpu-vs-webasm-transformers-js/)
- [transformers.js GitHub issue #796 (GPU OOM)](https://github.com/huggingface/transformers.js/issues/796)
- [Apple Foundation Models developer docs](https://developer.apple.com/documentation/FoundationModels)
- [Apple newsroom Foundation Models Sept 2025](https://www.apple.com/newsroom/2025/09/apples-foundation-models-framework-unlocks-new-intelligent-app-experiences/)
- [Apple Foundation Models iOS 27 builder guide](https://chatforest.com/builders-log/apple-foundation-models-ios-27-on-device-llm-api-builder-guide/)
- [sqlite-vec AI in Plain English](https://ai.plainenglish.io/embedded-intelligence-how-sqlite-vec-delivers-fast-local-vector-search-for-ai-de6d62936055)
- [Local-First RAG with sqlite-vec — Sitepoint](https://www.sitepoint.com/local-first-rag-vector-search-in-sqlite-with-hamming-distance/)
- [Local vs OpenAI Embeddings 2026](https://localaimaster.com/blog/local-vs-openai-embeddings)
- [Elephas embedding models guide 2026](https://elephas.app/blog/best-embedding-models)
- [Hybrid Cloud-Local Architecture 2026 — Sitepoint](https://www.sitepoint.com/hybrid-cloudlocal-llm-the-complete-architecture-guide-2026/)
- [LocalAIMaster Hybrid Routing](https://localaimaster.com/blog/hybrid-local-cloud-ai)
- [Perplexity hybrid local-cloud at Computex 2026 — VentureBeat](https://venturebeat.com/technology/perplexity-ai-unveils-hybrid-local-cloud-inference-system-at-computex-2026/)
- [CPU challenges GPU paper (arXiv May 2025)](https://arxiv.org/html/2505.06461v1)
- [Running LLMs locally 2026 — daily.dev](https://daily.dev/blog/running-llms-locally-ollama-llama-cpp-self-hosted-ai-developers/)
- [Electron WinML addon — Microsoft Learn](https://learn.microsoft.com/en-us/windows/apps/dev-tools/winapp-cli/guides/electron-winml-addon)
