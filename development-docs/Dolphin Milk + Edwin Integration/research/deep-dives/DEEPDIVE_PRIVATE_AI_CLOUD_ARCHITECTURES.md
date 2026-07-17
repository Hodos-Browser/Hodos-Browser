# Private-AI-Cloud Architectures — How to Route AI Calls Without Surveillance

> 📁 Part of the **Dolphin Milk + Edwin Integration** doc set — see `README.md`. Forensic deep-dive companion to the main study docs.
> **Created:** 2026-06-28 by a research workflow (web-cited). **STUDY, not a decision** — options & trade-offs, no winner picked. Claim tags **[FACT]/[VISION]/[INFERRED]/[SPECULATION]/[UNVERIFIED]** preserved.

## Private-AI-Cloud Architectures: How to Route AI Calls Without Surveillance
### A Forensic Study for Hodos Browser — June 2026

---

**Purpose and bottom line.** A privacy-conscious browser that ships a built-in AI assistant must solve a paradox: cloud models are where the capability ceiling lives, but every cloud call is a potential surveillance event. Six distinct architectural patterns have reached production in 2025–2026 — each trading off cryptographic strength, engineering complexity, user experience, and cost in different ways. None of them is purely "trust a policy document." The leading ones combine hardware-enforced isolation (TEEs, Secure Enclaves), cryptographic unlinkability (blind tokens, OHTTP relay), and contractual backstop, stacked in depth. For Hodos and Edwin specifically, the local sidecar is already the strongest default-tier privacy position in the industry — the architecture question is what to do for the tier of tasks that genuinely require frontier-model cloud capability, where five real options exist (local-only, Hodos-operated proxy, OHTTP relay, TEE partnership, BYO-key), each with meaningful trade-offs that are detailed below. This document maps the mechanisms, not the decisions.

---

## 1. Architecture 1: Apple Private Cloud Compute (PCC)

### 1.1 Mechanism

PCC is a verifiable-private cloud inference system co-designed with Apple Silicon. The full mechanism, as published in Apple's security blog: [FACT — https://security.apple.com/blog/private-cloud-compute/]

**Step 1 — Transparency log seeding.** Every PCC software build is committed to an append-only, cryptographically tamper-proof transparency log before deployment. The log records the exact software measurements (hashes of the inference stack, OS, firmware) of every production node. Binary images are published publicly within 90 days of logging. This means external researchers can always verify what is actually running. Removal of any entry would be detectable, similar to Certificate Transparency.

**Step 2 — Client-side attestation check.** Before transmitting a request, the user's device consults the transparency log to enumerate which PCC nodes have software measurements matching published, audited releases. The device will refuse to send data to any node it cannot validate against this log. It does not simply trust the load balancer's list; it filters that list against the transparency log entries.

**Step 3 — Encrypted key exchange.** The client generates a one-time request payload encrypted to the verified PCC node's public key. That public key is itself part of the attested software measurement — meaning any tampering with the node's software would produce a different public key, and the client's request would not decrypt correctly. The Secure Enclave on the PCC node manages attestation keys; they are hardware-unextractable.

**Step 4 — OHTTP relay for IP decoupling.** The encrypted request travels through an OHTTP (RFC 9458) relay operated by a third party — Fastly and Cloudflare serve as the two relay operators. [FACT — https://arxiv.org/html/2605.24239v1] The relay forwards only ciphertext to PCC infrastructure. PCC nodes see only ciphertext from the relay's IP, not the originating device's IP. The relay sees only the connection metadata (who is connecting), not the request content.

**Step 5 — Stateless inference.** PCC nodes process the decrypted request in memory and return a response. User data is not retained after response delivery — no logs, no debugging state, no cross-request correlation. The PCC design explicitly prevents even Apple's own operations staff from observing queries: logs that leave a node must match a pre-approved, audited schema.

**Step 6 — RSA Blind Signatures for credential unlinkability.** The device uses RSA blind signatures to obtain single-use credentials for accessing PCC. The signing service cannot link a specific device to the credential it signs (the blinding ensures this), and the PCC node sees only a valid credential, not the device identity. [FACT — https://security.apple.com/blog/private-cloud-compute/]

**Step 7 — Independent audit path.** Apple maintains a Virtual Research Environment that allows independent security researchers to run and inspect PCC software. The Sigstore-backed transparency log (similar to certificate transparency) enables external parties to verify the complete chain from source code to running binary.

**June 2026 expansion:** PCC extended beyond Apple data centers to Google Cloud, using Intel TDX + NVIDIA Confidential Computing + Google Titan chips as the hardware root of trust. [FACT — https://security.apple.com/blog/expanding-pcc/]

**On-device routing tier.** PCC is not the first resort. Apple's on-device AFM 3 model (3B parameter, ~30 tokens/sec, ANE-accelerated) handles most queries. An on-device routing model decides whether to use AFM on-device, PCC, or the Gemini handoff, based on task complexity. PCC is the tier for tasks exceeding on-device capability but below the complexity requiring Gemini's world-knowledge. [FACT — https://machinelearning.apple.com/research/introducing-third-generation-of-apple-foundation-models]

### 1.2 What It Protects

- **IP address:** Hidden from PCC nodes via OHTTP relay; relay cannot read content.
- **Request content:** End-to-end encrypted to hardware-attested node; no intermediary can read it.
- **Usage correlation:** RSA blind signatures prevent linking requests to a specific Apple ID.
- **Operator snooping:** Even Apple SRE staff cannot read requests; stateless design prevents logging.
- **Software tampering:** Any change to the inference stack changes the software measurement, breaking attestation.

### 1.3 What It Leaks

- **Query timing and volume:** The relay operator (Fastly/Cloudflare) sees connection metadata — when the device connects and approximately how large the payload is, even without content.
- **Task-routing signals:** The on-device routing decision leaks which tier was used (on-device vs. PCC vs. Gemini), which reveals something about query complexity.
- **Gemini handoff privacy:** When PCC decides a task requires Gemini, the request goes to Google's standard infrastructure. Privacy properties at that point are governed by Apple's Gemini handoff agreement, the exact terms of which are not fully public. [UNVERIFIED — per BROWSER_AI_IMPLEMENTATION_STUDY.md §A]
- **Trust assumptions:** The transparency log mechanism itself requires trusting Apple to publish accurate measurements and not to selectively withhold entries. The audit window (90 days) means pre-publication attacks are theoretically possible but practically detectable.

### 1.4 Latency and Cost

- On-device AFM 3: ~0.6ms first-token on iPhone 15 Pro, ~30 tokens/sec. [FACT]
- PCC tier: adds OHTTP relay hop + attestation check latency. Attestation check is cached per software version, not per request, so marginal overhead is approximately one additional TLS round-trip to the relay. [INFERRED from architecture]
- Cost to Apple: enormous proprietary infrastructure. Replication cost for a small browser vendor: prohibitive without equivalent silicon and cloud partnership.

### 1.5 x402/BSV Micropayment Compatibility

Not applicable in Apple's closed system. Apple Intelligence is bundled with hardware purchase; PCC is not a metered service. The architecture is not designed for per-request billing. [INFERRED — Apple does not publish API pricing for PCC] However, the OHTTP relay pattern Apple uses is directly applicable to an Edwin routing architecture that is funded by x402 BSV micropayments — see §4 and §7 below.

---

## 2. Architecture 2: Brave Leo — Layered Anonymity + VOPRF + TEE

### 2.1 Mechanism

Brave Leo is the most comprehensively documented production example of a privacy-engineered browser AI proxy. Seven distinct layers operate in combination:

**Layer 1 — Anonymizing reverse proxy.** All Leo inference requests route through Brave's AWS-hosted reverse proxy. Model provider backends (Anthropic, Meta, Mixtral, etc.) never receive the originating user IP address. Brave substitutes its own proxy IP. As of June 2025, all models run on Brave's own AWS infrastructure (not third-party data processors). [FACT — https://brave.com/blog/automatic-mode-leo/]

**Layer 2 — Zero retention.** Conversations are discarded server-side immediately after the response is returned. No server-side usage logs are tied to any identifier. Model providers do not receive data that allows training on Leo interactions. [FACT — https://brave.com/blog/leo-launch/]

**Layer 3 — No-account-required free tier.** No email, no identity association for basic usage. This eliminates the account-as-persistent-identifier attack. [FACT]

**Layer 4 — VOPRF blind token scheme (the cryptographic core).** For premium Leo, Brave implements a Verifiable Oblivious Pseudorandom Function (VOPRF) scheme using the `challenge-bypass-ristretto` library (RFC 9497-compliant). The mechanism: [FACT — https://github.com/brave/brave-core/blob/master/docs/premium_account_privacy.md]

1. The browser locally generates a large batch (~500) of random tokens.
2. Each token is "blinded" using an elliptic curve blinding operation: `blinded_token = r * T` where `r` is a random scalar and `T` is the token point. The blinding factor `r` is known only to the client.
3. Blinded tokens are sent to Brave's Credential Redemption Server (CBR). CBR signs them with its private VOPRF key. Crucially, CBR **cannot see the original token values** — it signs blinded values only.
4. The browser receives signed blinded tokens and "unblinds" them: `unblinded_signed_token = r_inverse * (r * VOPRF_key * T) = VOPRF_key * T`. This produces valid signed credentials that CBR cannot link to the blinding event.
5. When accessing Leo, the browser presents only a token preimage + HMAC binding. No account ID. No email. The CBR cannot determine which user presented which token.
6. Double-spend tracking is maintained via the token hash, preventing reuse — but the tracker cannot learn which user produced the token.

The result: even if Brave is legally compelled to reveal which user made a specific request, they architecturally cannot — the cryptographic scheme breaks the link.

**Layer 5 — BYOM path (zero Brave visibility).** Users can configure any OpenAI-compatible local endpoint (e.g., `http://localhost:11434/v1/chat/completions` for Ollama). When BYOM is active, Brave has zero visibility into any inference call. [FACT — https://brave.com/blog/byom-nightly/]

**Layer 6 — Agentic isolation.** Brave's AI Browsing mode runs in a fully isolated browser profile with a separate cookie jar, cache, and site data. It has no access to the user's real authenticated sessions. An alignment-checker second model reviews proposed actions but never receives raw website content — it is firewalled from prompt injection. [FACT — https://brave.com/blog/ai-browsing/]

**Layer 7 — TEE attestation (Brave Nightly, November 2025).** For DeepSeek V3.1 on Brave Nightly, inference runs inside an Nvidia Hopper GPU TEE via NEAR AI. Brave's browser performs a cryptographic verification chain: from open-source code → GitHub Actions build measurements → Sigstore-signed attestation → hardware GPU attestation from Nvidia. The browser validates that the response was generated in a genuine Nvidia GPU TEE before trusting it. [FACT — https://brave.com/blog/browser-ai-tee/; https://www.theregister.com/2025/11/22/brave_leo_trusted_execution_environment/]

### 2.2 What It Protects

- **IP address:** Hidden from all model providers via reverse proxy.
- **Account linkability:** VOPRF scheme makes individual requests cryptographically unlinkable to the account.
- **Conversation content (cloud path):** Not logged; not used for training; discarded after response.
- **Conversation content (BYOM path):** Never leaves device.
- **Model integrity (TEE path, Nightly):** Hardware attestation ensures model and code are unmodified.
- **Agentic session data:** Isolated profile prevents cross-contamination with real authenticated sessions.

### 2.3 What It Leaks

- **Usage volume to proxy:** Brave's proxy knows how many requests per time period from a given IP, even without content. Traffic analysis is possible.
- **Prompt content to Brave's infrastructure:** Cloud path requests pass through Brave's AWS infrastructure plaintext after TLS termination. Brave is a trusted intermediary, not a cryptographically excluded party (except on the TEE path).
- **TEE path (Nightly only):** Available only for DeepSeek V3.1 on the Nightly build; not yet a default production feature.
- **Provider contracts are legal, not cryptographic:** The no-train and no-retention guarantees for cloud models depend on contractual enforcement, not hardware.

### 2.4 Latency and Cost

- Proxy hop: ~10–30ms additional latency depending on Brave AWS region proximity. [INFERRED]
- TEE inference: Brave reports near-zero overhead on Nvidia Hopper confidential computing. [FACT — https://startuphub.ai/ai-news/ai-research/2025/verifiable-ai-privacy-brave-leos-tee-powered-breakthrough]
- BYOM/local: Zero proxy latency; dependent on local hardware.

### 2.5 x402/BSV Micropayment Compatibility

High natural fit. Brave's VOPRF token scheme already implements a privacy-preserving payment mechanism — each token effectively functions as a pre-purchased usage credential. An x402/BSV variant would replace the VOPRF token with a BSV micropayment: the browser makes a per-request satoshi payment that serves as the access credential. The payment is pseudonymous on-chain (BSV UTXO model), auditable, and cryptographically unforgeable. This is directly analogous to NEAR AI's streaming-payment-enforced-by-enclave model. [INFERRED from Brave's VOPRF design and x402 architecture]

---

## 3. Architecture 3: DuckDuckGo Duck.ai — Contractual Privacy Broker

### 3.1 Mechanism

Duck.ai is the canonical "contractual privacy broker" — a simpler architecture than Brave's but well-documented and production-proven. [FACT — https://duckduckgo.com/duckduckgo-help-pages/duckai/ai-chat-privacy]

**Session token model.** Browser calls `GET /duckchat/v1/status` to obtain a session VQD (Verification Query Data) token. Subsequent requests use `POST /duckchat/v1/chat` with the `x-vqd-4` header. Responses stream via SSE. [FACT — https://github.com/benoitpetit/duckduckGO-chat-api]

**IP and metadata stripping.** DDG's proxy servers receive the request, strip the originating IP address entirely, and substitute DDG's own IP before forwarding to model backends (Anthropic, OpenAI, Mistral, Together.ai, and its own models). Metadata removed: IP address, user-agent string, browser/OS fingerprint data, all identifying HTTP headers. [FACT — https://duckduckgo.com/duckduckgo-help-pages/what-information-does-duckai-share-with-model-providers]

**What is forwarded.** Prompt text, today's date, user timezone, unit system preference (metric/imperial), optional city-level location (user-controlled). No PII, no cookies, no fingerprint. [FACT]

**Contractual no-train.** All model providers are contractually prohibited from training on Duck.ai interactions. Maximum retention: 30 days (subject to safety and legal exceptions). DDG represents this as "private by architecture" rather than "private by policy" — meaning the architecture constrains what can be shared, not merely the policy. [FACT — DDG help pages]

**Local-only chat history.** Chat history is stored only on the user's device. Optional E2E encrypted sync uses a client-held key — DDG cannot read synced content. [FACT]

**TEE tier (gpt-oss-120b via Tinfoil.sh).** For the `gpt-oss-120b` model, DDG routes through Tinfoil.sh, which runs inference inside Nvidia Hopper/Blackwell confidential computing mode + AMD SEV-SNP + Intel TDX. Prompts are encrypted in hardware-isolated memory; even Tinfoil's own operators cannot read data. [FACT — https://duckduckgo.com/duckduckgo-help-pages/duckai/ai-chat-privacy; https://tinfoil.sh/technology]

### 3.2 What It Protects

- **IP address:** Completely hidden from all model providers.
- **Persistent identity:** No account required; no cookies; no fingerprint forwarded.
- **Training data exposure:** Contractual no-train across all providers.
- **Chat history:** Local-only or client-side-encrypted sync.
- **Content at TEE tier:** Hardware-isolated from infrastructure operators.

### 3.3 What It Leaks

- **DDG as trusted intermediary:** DDG's servers receive plaintext prompts before forwarding. DDG knows usage patterns and content (they are the proxy, not a cryptographic escrow). This is a policy-layer protection, not a cryptographic one — except on the Tinfoil TEE tier.
- **Prompt content to providers:** Model providers receive the prompt (anonymized) and generate responses. Contractual no-train protects training datasets but not the in-flight request.
- **Volume metadata:** DDG knows when and how often a user makes requests (timing, frequency), even if not content.
- **Contractual enforcement risk:** No-train clauses depend on legal enforcement mechanisms; a subpoenaed provider might face conflict between the contract and legal compulsion.

### 3.4 Latency and Cost

- Proxy adds one hop: estimated 20–50ms depending on DDG server proximity. [INFERRED]
- TEE inference via Tinfoil: near-zero overhead reported by Tinfoil. [FACT — https://tinfoil.sh/technology]
- Free to end users; DDG absorbs infrastructure cost through search advertising revenue.

### 3.5 x402/BSV Micropayment Compatibility

Moderate fit. Duck.ai's architecture is purely contractual — it does not use cryptographic payment tokens. Adding x402/BSV micropayments to this pattern would provide a billing layer (user pays Hodos per request; Hodos pays provider per request) but does not strengthen the privacy properties beyond IP stripping. The upgrade would be: add VOPRF-style or BSV-payment-based unlinkability on top of the proxy layer. [INFERRED]

---

## 4. Architecture 4: Oblivious HTTP (OHTTP, RFC 9458)

### 4.1 Mechanism

OHTTP is an IETF standard (RFC 9458, ratified January 2024) that provides cryptographic separation between "who is asking" and "what is being asked" by splitting the knowledge across two non-colluding parties. [FACT — https://www.rfc-editor.org/info/rfc9458/]

**Three-party model:**

1. **Client** — the user's device (or Edwin sidecar).
2. **Oblivious Relay** — a relay server that sees the client's IP address and connection metadata but cannot read the request content (ciphertext only).
3. **Oblivious Gateway** — a gateway that decrypts the request and forwards to the Target Resource, but never sees the client's IP (the relay has already forwarded only ciphertext, so the gateway's view begins with the relay's IP).

**Encryption layer.** The client obtains the gateway's public key (published in a well-known configuration endpoint). It serializes the HTTP request in Binary HTTP format (RFC 9292), then encrypts this serialized request using HPKE (Hybrid Public Key Encryption, RFC 9180). The resulting ciphertext is addressed to the relay with a CONNECT-style request; the relay only sees: "forward this encrypted blob to this gateway address."

**No single party holds both identity and content:**
- Relay: knows who (client IP), not what (encrypted content).
- Gateway: knows what (decrypts the request), not who (sees only relay IP).
- Target Resource: processes the request, sees content, but receives it from the gateway — same privacy properties as gateway.

**Request unlinkability.** Because each OHTTP request uses a fresh HPKE ephemeral key pair, the gateway cannot link two requests from the same client even if both arrive at the same gateway via the same relay. [FACT — RFC 9458 §3]

**Key distribution integrity.** Apple addressed a subtle OHTTP attack vector by publishing advertised OHTTP keys to the PCC transparency log. This prevents a malicious relay from returning client-specific keys (which would allow the gateway to de-anonymize users). [FACT — https://arxiv.org/html/2605.24239v1]

**Production deployments.** OHTTP is used in production by: Apple PCC (IP decoupling, third-party relay operated by Fastly and Cloudflare), Apple Safari Highlights (URL → Apple servers), Firefox (DNS-over-OHTTP, Safe Browsing variant), Google Safe Browsing Enhanced Protection, Cloudflare's Oblivious DNS resolver. [FACT — various; https://support.mozilla.org/en-US/kb/ohttp-explained; https://developers.google.com/safe-browsing/ohttp/reference]

### 4.2 What It Protects

- **Client IP from gateway and target:** The gateway never learns the originating IP.
- **Request content from relay:** The relay never reads plaintext.
- **Request linkability:** Ephemeral HPKE keys prevent the gateway from linking requests from the same client.
- **Relay from key-distribution attack:** Publishing keys to a transparency log prevents malicious relay from inserting tracking keys.

### 4.3 What It Leaks

- **Relay sees volume and timing metadata:** The relay knows the client IP, connection frequency, and payload sizes.
- **Gateway sees plaintext after decryption:** The gateway decrypts the request and sees its full content — it cannot link it to an IP, but the content itself is visible.
- **Relay–gateway collusion:** If the relay and gateway collude (share IP timing with content timing), they can probabilistically link requests. OHTTP's threat model explicitly assumes the relay and gateway do not collude and are operated by different parties.
- **DNS leakage:** If the client resolves the relay's hostname via non-OHTTP DNS, DNS traffic could link the client to OHTTP relay usage.

### 4.4 Latency and Cost

- Adds one relay hop: 10–40ms depending on relay geography relative to client and gateway. [INFERRED]
- Encryption/decryption overhead: microseconds (HPKE is computationally inexpensive). [INFERRED]
- Relay operation cost: negligible per request (HTTP proxying); relay operators may charge for the service.
- Existing implementations: `ohttp-gateway` crate (Rust), Cloudflare Workers OHTTP, Apple's open-source implementation. [FACT — https://crates.io/crates/ohttp-gateway; https://divviup.org/blog/ohttp-now-available/]

### 4.5 x402/BSV Micropayments Compatibility

**High natural fit — one of the cleanest integrations in this document.** An OHTTP relay can be operated as a metered service: the client attaches a BSV x402 micropayment (or a pre-issued BSV-backed credential) to the OHTTP request envelope. The relay validates the payment before forwarding. Because the relay never reads the content, the payment for relay usage is decoupled from the content of the query — the relay knows "this client paid for one relay hop," not "this client asked about X." [INFERRED from x402 protocol design and OHTTP request format]

This maps to BROWSER_AI_IMPLEMENTATION_STUDY.md §H5 Option C ("OHTTP relay — third-party IP decoupling"), where the study notes: "BSV x402 micropayments could fund per-request relay usage (OHTTP relay as a paid BSV service — natural alignment with Hodos's x402 architecture)." [FACT — §H5 of local study]

---

## 5. Architecture 5: Confidential Compute / TEE Inference

### 5.1 Mechanism Overview

Trusted Execution Environments (TEEs) provide hardware-enforced isolation of a computation from the host operating system, cloud hypervisor, and even the infrastructure operator. Unlike contractual privacy ("we promise not to look"), TEEs provide hardware-attested evidence of what code is running and cryptographic enforcement that plaintext data is accessible only within the enclave. Three production-relevant implementations:

---

### 5.1a Nvidia Hopper/Blackwell Confidential GPU Computing

Nvidia's H100 (Hopper architecture) was the first GPU to support confidential computing, reaching general availability in 2024. [FACT — https://docs.nvidia.com/nvidia-secure-ai-with-blackwell-and-hopper-gpus-whitepaper.pdf]

**How it works:**

1. **Secure boot chain.** Firmware integrity is validated at power-on; the GPU measures its own firmware and configuration using hardware-embedded keys.

2. **Memory encryption.** GPU memory is encrypted using per-session hardware keys; the CPU and host OS cannot read GPU VRAM contents.

3. **CPU-GPU trust extension.** An attestation linkage from the CPU TEE (AMD SEV-SNP or Intel TDX on the host) to the GPU TEE is established via Nvidia's `local-gpu-verifier`, creating a transitive chain: "this specific hardware configuration running this specific firmware is processing this workload." [FACT — https://tinfoil.sh/technology]

4. **Remote attestation.** A third party (e.g., a browser verifying a remote inference endpoint) can request an attestation report from the GPU. The report is signed with Nvidia's attestation signing key (rooted in Nvidia's Root of Trust). The verifier checks this signature against Nvidia's published certificate chain. A valid attestation proves: (a) the hardware is a genuine Nvidia H100/H200/B100 in confidential mode, (b) the firmware version matches expectations, (c) the GPU-encrypted memory is not accessible to the host.

**Hopper limitations (important for production evaluation):** On H100, GPU memory is protected by access control rather than runtime encryption. RPC metadata and synchronization structures between CPU and GPU remain in plaintext in some configurations. Timing patterns in memory transfers could theoretically leak information. [FACT — https://www.corvex.ai/blog/confidential-computing-meets-nvidia-hgxtm-b200-secure-ai-without-the-performance-trade-off]

**Blackwell improvements.** B100/B200 (Blackwell architecture, 2025) extends confidential computing to cover GPU-to-GPU communication in multi-GPU systems (NVLink traffic encrypted). Purpose-built encryption engines for AI tensor operations reduce the overhead of encrypting large weight matrices. [FACT — Nvidia Blackwell whitepaper, August 2025]

**Performance overhead.** Nvidia reports near-zero throughput impact for Blackwell confidential mode (purpose-built encryption engines vs. general-purpose AES on Hopper). Hopper carries a small overhead for large batch operations. [FACT — https://docs.nvidia.com/nvidia-secure-ai-with-blackwell-and-hopper-gpus-whitepaper.pdf]

---

### 5.1b Tinfoil.sh

Tinfoil is a production confidential AI inference platform combining AMD SEV-SNP (host CPU TEE), Intel TDX (alternative host TEE), and Nvidia Hopper/Blackwell confidential GPU mode. [FACT — https://tinfoil.sh/technology; https://docs.tinfoil.sh/verification/attestation-architecture]

**Full attestation chain (forensic detail):**

1. **Build time.** `tinfoil-config.yml` commits firmware versions, CVM (Confidential VM) image, and model weights to a public GitHub repository.

2. **Measurement generation.** GitHub Actions builds the system and generates hardware measurements, signed via Sigstore's transparency log (a public, append-only, cryptographically auditable log of code signatures).

3. **Boot-time verification.** At enclave boot, the system verifies each stage: firmware → kernel → configuration hash → model weights, using `dm-verity` (Linux kernel block-device integrity verification). Any modification to any component breaks the measurement chain.

4. **Client verification (four steps):**
   - Fetch the enclave's attestation document, containing signed runtime measurements.
   - Validate the certificate chain back to AMD's hardcoded root certificate.
   - Retrieve the Sigstore bundle and verify it against Sigstore's root trust anchor.
   - Confirm that the TLS public key matches the key in the attestation document — binding the TLS connection to the attested enclave.

5. **Untrusted caching proxies.** Three proxy servers (Attestation Bundle Proxy, GitHub Proxy, AMD KDS Proxy) cache verification data for performance, but every piece of data they serve is independently verified by the client against hardcoded root certificates. The caching proxies cannot forge attestation data. [FACT — https://docs.tinfoil.sh/verification/attestation-architecture]

6. **Third-party comparison claim.** Tinfoil explicitly compares itself to Apple PCC on open-source auditability: Tinfoil's enclave code is open-source and verifiable by any party; Apple PCC's hypervisor and components are closed-source, requiring trust in Apple's transparency log publication. [FACT — https://tinfoil.sh/blog/2025-01-30-how-do-we-compare; https://tinfoil.sh/blog/2025-05-15-privacy]

**Production use:** DuckDuckGo routes `gpt-oss-120b` through Tinfoil. [FACT — §E Tier 3 of local study]

---

### 5.1c NEAR AI Confidential Inference

NEAR AI's private inference platform uses Intel TDX (host) + Nvidia TEE (GPU) to create confidential VMs for AI workloads. [FACT — https://docs.near.ai/cloud/private-inference/; https://near.ai/blog/decentralized-confidential-machine-learning]

**Mechanism:**

- Intel TDX creates a Confidential VM that isolates the AI workload from the host OS and hypervisor.
- TLS encryption protects data in transit; the TLS session **terminates inside the TEE**, meaning the TLS private key is generated and stored within the enclave — the host never sees decrypted traffic.
- Model weights are encrypted and can only be decrypted inside the secure enclave, protecting proprietary model weights from infrastructure operators.
- **Streaming payment enforcement:** The enclave can enforce payment terms — users attach proof of payment (e.g., a blockchain transaction) and receive a proof of compute used. The enclave is the enforcement point; it will not serve results without valid payment. [FACT — NEAR AI private inference docs]

**Brave Leo integration (Nightly):** Brave's Nightly build uses NEAR AI for DeepSeek V3.1 inference. The browser performs a cryptographic verification chain: open-source code → GitHub Actions measurements → Sigstore-signed attestation → Nvidia hardware attestation. The browser validates the chain before trusting the response. [FACT — https://brave.com/blog/browser-ai-tee/; https://www.privacyguides.org/news/2025/11/20/brave-announces-verifiable-and-transparent-tee-support-in-leo/]

---

### 5.2 Hardware Attestation vs. Contractual Trust — the Core Distinction

| Property | Contractual (DDG non-TEE, Kagi, Dia) | Hardware TEE Attestation |
|---|---|---|
| **Enforcement mechanism** | Legal agreement between parties | Hardware-enforced; violating requires breaking AMD/Nvidia signing keys |
| **Operator snooping** | Prevented by policy; technically possible | Prevented by hardware; technically not possible without breaking the TEE |
| **Legal compulsion** | Provider could be ordered to log; cannot comply if no logs exist | Even if ordered, operator cannot extract plaintext from active enclave |
| **Auditability** | Auditor must trust the audited party's logs | Auditor verifies independently against AMD/Nvidia/Sigstore public keys |
| **Verifiable by client** | No | Yes (attestation report, checked at request time) |
| **Overhead** | Negligible | Near-zero (Blackwell); small (Hopper) |

### 5.3 What TEE Inference Protects

- **Content confidentiality from infrastructure operator:** Even the TEE host (cloud provider, Tinfoil itself) cannot read prompts.
- **Model integrity:** Attestation proves the exact model version running is what was committed; no model-swapping attack.
- **Response integrity:** The response can be signed by the enclave's key, proving it came from an attested execution environment.
- **Legal compulsion to surveil:** Operator cannot comply with an order to produce plaintext because they genuinely cannot access it.

### 5.4 What TEE Inference Leaks

- **The network layer above the enclave:** IP addresses, timing, and payload sizes are visible to network-layer observers. TEE alone does not provide IP anonymity — requires OHTTP relay stacked on top.
- **Attestation to whom.** A client verifying an attestation must trust the attestation verification mechanism itself (AMD/Nvidia signing infrastructure). A compromise of Nvidia's signing keys would allow forged attestations. This is a narrow but real supply-chain risk.
- **Side-channel attacks.** Memory timing, cache behavior, power consumption, and other side channels can theoretically leak information from TEEs. Practical exploitation requires physical access or co-tenancy at the infrastructure level, making it a nation-state class attack rather than an infrastructure-operator attack.
- **Model selection visibility.** The TEE provider knows which model endpoint was invoked (TEE is model-specific; you request "run DeepSeek V3.1 inference" and the endpoint is identifiable even if content is not).

### 5.5 Latency and Cost

- Hopper confidential mode: small overhead for large batch operations, negligible for single-request inference. [FACT — Nvidia whitepaper]
- Blackwell: near-zero overhead (purpose-built AES engines). [FACT]
- Tinfoil pricing: not published; premium over standard cloud inference. [UNVERIFIED — no public pricing page found]
- NEAR AI pricing: tiered; confidential inference carries a premium. [FACT — https://docs.near.ai/cloud/private-inference/]

### 5.6 x402/BSV Micropayment Compatibility

**Strongest fit of any architecture.** NEAR AI's enclave design explicitly supports streaming payment enforcement: the enclave validates proof of payment before serving inference results and issues proof of compute in return. An x402/BSV micropayment (per-request satoshi payment with a transaction hash as proof) maps perfectly onto this model:

1. Edwin generates a BSV x402 payment (e.g., 100 satoshis) for one inference request.
2. Edwin sends the payment proof to the TEE endpoint.
3. The enclave validates the payment proof on-chain (or against a BSV SPV proof).
4. The enclave runs inference and returns a response + signed compute receipt.
5. The compute receipt is logged on BSV as an immutable audit trail.

This is documented as §H4 Option C and §H5 Option D in BROWSER_AI_IMPLEMENTATION_STUDY.md, and the study notes it is a differentiator: "no other browser offers agentic authorization with a cryptographic audit trail." [FACT — §H4 of local study]

---

## 6. Architecture 6: Self-Hosted / Local Inference — the Zero-Trust Baseline

### 6.1 Mechanism

Local inference runs on the user's own hardware. The model weights are stored on disk; inference executes in a local process. No query, no prompt, no response leaves the machine during inference. For Edwin specifically, the runtime is Ollama (llama.cpp-based, with CUDA/Metal/Vulkan GPU acceleration backends) running on localhost:11434.

**Edwin's structural advantage over browser-sandboxed local inference.** Edwin is an OS-level process — it is not constrained by browser process sandboxing. This distinction matters:

- Chrome Nano runs as a sandboxed utility process. GPU acceleration is available (>4 GB VRAM threshold), but the sandbox blocks certain GPU operations and limits the model to ~4 GB. [FACT — BROWSER_AI_IMPLEMENTATION_STUDY.md §A]
- Firefox's local models are CPU-only. GPU access is blocked by Firefox's process sandboxing constraints. A native ONNX runtime drop gave 10x speedup over WASM, but GPU acceleration remains unavailable. [FACT — https://blog.mozilla.org/en/firefox/firefox-ai/speeding-up-firefox-local-ai-runtime/]
- **Edwin on localhost has full GPU access** — CUDA, Metal, Vulkan — same as any native application. It can run Phi-4-mini, Llama 3.1 8B, Gemma 3, or Mistral 7B with full GPU acceleration, at quality levels unavailable to any browser-sandboxed solution.

**Capability ceiling (honest assessment).** Local inference with a 4–8B quantized model is meaningfully worse than frontier models at:
- Complex multi-step reasoning and research.
- Long document synthesis (context windows beyond ~32K effective tokens).
- Code generation for large, multi-file projects.
- Agentic planning requiring world-knowledge beyond the training cutoff.

Brave has been attempting to ship pre-configured local inference as a default since at least 2023. As of mid-2026, it remains a power-user configuration requiring Ollama installation, not a default-on feature. [FACT — BROWSER_AI_IMPLEMENTATION_STUDY.md §B, https://brave.com/blog/leo-roadmap-2025-update/]

**Verified zero-leakage status.** Chrome Nano's local inference was confirmed to produce zero outbound network traffic during inference (verified by network analysis). [FACT — BROWSER_AI_IMPLEMENTATION_STUDY.md §B] The same property applies trivially to any local Ollama process — network monitoring can confirm it.

### 6.2 What It Protects

- **Everything that doesn't leave the device.** Prompts, responses, session context, page content — none of it is ever transmitted.
- **No account, no cloud subscription required.**
- **Offline capability:** Works without any internet connection.
- **Auditability:** Users can verify zero outbound traffic with any packet inspector.

### 6.3 What It Leaks

- **Model download traffic.** Downloading the model weights requires a network request. The download URL reveals which model was chosen. Size reveals the model roughly (e.g., 4 GB download is obviously a 4B parameter model). Best practice: offer multiple sources, allow pre-installed models.
- **Nothing else** (during inference, confirmed by network analysis of equivalent architectures).

### 6.4 Latency and Cost

- No network round-trip. First-token latency is hardware-dependent: ~200ms on a modern GPU for 7B models; ~1–2s on CPU-only for the same.
- Per-token cost: zero beyond electricity.
- Quality ceiling is the trade-off, not a technical privacy failure.

### 6.5 x402/BSV Micropayments Compatibility

**No per-request cost to fund.** Local inference has no marginal cost structure that x402 payments address. However, BSV micropayments are still relevant as a **consent and authorization mechanism** for agentic actions — the local model proposes an action, the user authorizes it via a satoshi payment (on-chain audit trail, unforgeable, rate-limiting protection against prompt-injection-triggered autonomous actions). [INFERRED from BROWSER_AI_IMPLEMENTATION_STUDY.md §H4 Option C]

---

## 7. Comparative Summary Table

| Architecture | IP Hidden from Provider | Content Hidden from Proxy/Relay | Model Operator Cannot Read | Cryptographic (not just contractual) | Client Can Verify | Latency Overhead | x402/BSV Fit |
|---|---|---|---|---|---|---|---|
| **Apple PCC** | Yes (OHTTP relay) | Yes (E2E to attested node) | Yes (stateless, no-log) | Yes (attestation + blind sigs) | Yes (transparency log) | Low (~1 relay hop) | N/A (closed) |
| **Brave Leo proxy** | Yes (reverse proxy) | No (Brave decrypts at proxy) | Partial (self-hosted; no logs) | Partial (VOPRF for unlinkability; not content) | No (proxy-layer) | Low (~10–30ms) | High (VOPRF → BSV credential) |
| **Brave Leo TEE (Nightly)** | No (IP visible to NEAR AI infra) | Yes (TEE isolates from operator) | Yes (hardware enforced) | Yes (Nvidia attestation, Sigstore chain) | Yes (browser validates attestation) | Low | High (streaming payment in enclave) |
| **DuckDuckGo Duck.ai** | Yes (DDG substitutes IP) | No (DDG decrypts for forwarding) | Contractual (no-log, no-train) | No (contractual only, non-TEE tier) | No | Low | Moderate |
| **DDG + Tinfoil TEE** | Yes (DDG proxy) | Yes (Tinfoil TEE isolates content) | Yes (hardware enforced) | Yes (AMD SEV-SNP + Nvidia attestation) | Yes (client SDK verifies) | Low | High |
| **OHTTP relay** | Yes (relay sees IP, not content) | Yes (gateway decrypts, not relay) | Depends on endpoint | Yes (HPKE encryption + key distribution) | Partial (key published to log) | Low (~1 hop) | High (relay = paid BSV service) |
| **TEE inference only** | No (IP visible to TEE host) | Yes | Yes | Yes | Yes (attestation report) | Near-zero | Very high (enclave enforces payment) |
| **TEE + OHTTP** | Yes (relay decouples IP) | Yes (TEE decouples content) | Yes | Yes (both layers) | Yes | Moderate (2 hops) | Very high |
| **Local / Edwin on localhost** | N/A (no network) | N/A | N/A | N/A | Yes (packet inspection) | None (network) | Moderate (consent/authorization use) |

---

## 8. Edwin Routing Options — Grounded in §H5

BROWSER_AI_IMPLEMENTATION_STUDY.md §H5 ("The Privacy-Broker Pattern for Routing AI Calls") identifies five options for how Edwin routes AI calls. Below is a technically-grounded elaboration of each, integrated with the architecture analysis above.

---

### Option A: Local Inference Only (§H5 Option A)

**Mechanism.** Edwin routes all inference to `localhost:11434` (Ollama). No outbound cloud calls whatsoever. Users who want frontier quality configure a BYOM endpoint manually.

**What this achieves.** The strongest privacy claim in the industry. Stronger than Brave BYOM in one specific respect: Edwin manages the Ollama endpoint directly, so the user doesn't need to configure anything — the local tier is the factory default. [FACT — BROWSER_AI_IMPLEMENTATION_STUDY.md §H1 Option A]

**Technical constraints.** Edwin already runs as an OS-level process with full GPU access. A 7–8B quantized model (Phi-4-mini, Llama 3.1 8B) runs at ~20–40 tokens/sec on a mid-range NVIDIA GPU. Adequate for page summarization, Q&A, inline suggestions, and simple agentic tasks. Inadequate for complex multi-step research or deep document synthesis.

**Edwin-specific challenge.** Edwin is a Node.js gateway, not a native binary; Ollama is a separate process. The integration is `HTTP request → Edwin → Ollama:11434 → response`. This is already how Edwin works with local models; no architectural change needed. The transition to lean native binary (in progress per local docs) would make Edwin-to-Ollama communication even lower-latency.

**Honest user experience assessment.** The capability ceiling matters for a casual user. A user asking Edwin to "summarize this 200-page PDF with cross-references" will get a noticeably worse result from a 7B local model than from Claude 3.5 Sonnet. The local-only option is genuinely appropriate only if it is paired with an honest UI labeling the quality difference and offering a voluntary upgrade path to cloud.

**x402 role.** No per-request billing needed. BSV micropayments apply as consent-and-authorization tokens for agentic actions (see §H4 Option C). [FACT — §H4 of local study]

---

### Option B: Hodos-Operated Anonymizing Proxy (§H5 Option B)

**Mechanism.** When user opts into cloud inference, Edwin routes to a Hodos-controlled reverse proxy (AWS or equivalent). Proxy strips originating IP, substitutes Hodos IP, enforces contractual no-train/no-retention terms with providers, handles BSV x402 billing. Provider never sees user IP or device identity.

**What this achieves.** Brave Leo Tier 2 equivalent. IP anonymization + contractual protections. If Brave's VOPRF blind-token scheme is adapted, add cryptographic unlinkability. [FACT — BROWSER_AI_IMPLEMENTATION_STUDY.md §H5 Option B]

**Operational requirements for Hodos.** Hodos becomes a data processor for the content of cloud queries. This requires: (a) cloud infrastructure (AWS, GCP, or similar, estimated ~$200–500/month for small scale, scaling with usage); (b) legal agreements with model providers (Anthropic, OpenAI) including DPA and no-train clauses; (c) ongoing infrastructure management; (d) legal liability as a proxy operator.

**x402/BSV integration mechanism.** Edwin generates a BSV x402 payment (satoshi amount determined by model tier and token count) at request time. The payment goes to Hodos's BSV address. Hodos proxy validates the payment (either on-chain via SPV proof or via a BSV payment channel) before forwarding to the model provider. This replaces API key management entirely — each request is self-funding with a cryptographic receipt.

**Specific to Edwin's MCP architecture.** Edwin already exposes an MCP server. A Hodos proxy MCP tool could be registered: `hodos_cloud_inference(prompt, model, bsv_payment_proof)` — Edwin calls this tool, the MCP server validates the payment and routes to the provider. The payment proof is the authorization. [INFERRED from Edwin MCP architecture per local docs]

**Key risk.** If Hodos is subpoenaed, plaintext query content could potentially be compelled if logs exist. The mitigation: operate with a strict no-logging policy (technically enforced at the proxy layer, not just policy) and add TEE inference as the Option D upgrade path.

---

### Option C: OHTTP Relay (§H5 Option C)

**Mechanism.** Cloud AI requests from Edwin route through an OHTTP relay operated by a party independent of both Hodos and the model provider. The relay sees only ciphertext. Hodos cannot read request content. The model provider or gateway sees decrypted content but not the originating IP.

**What this achieves.** A genuine cryptographic separation that protects against both legal compulsion at Hodos (Hodos cannot produce plaintext they never had) and provider-side IP correlation (provider cannot see the originating IP). Unlike Option B, Hodos is not a data processor for content.

**Implementation specifics.** Edwin's cloud routing path encrypts the HTTP request (prompt + model parameters) using HPKE to the gateway's public key. The OHTTP-encapsulated request is sent to an independent relay. Existing open-source relay implementations: Cloudflare's OHTTP relay (available to Workers customers), `ohttp-gateway` Rust crate. [FACT — https://crates.io/crates/ohttp-gateway; https://divviup.org/blog/ohttp-now-available/]

**Who operates the relay?** This is the critical dependency. Options:
- Third-party relay service (Fastly, Cloudflare — both operate OHTTP relays for Apple/Mozilla). Adds a vendor dependency.
- Community-operated relay (analogous to Tor exit nodes, but simpler protocol). Risk: relay reliability and availability.
- A relay in the BSV ecosystem — a natural market opportunity: anyone can operate an OHTTP relay and charge BSV x402 micropayments per request. Edwin could discover available relays via a BSV-based relay directory (analogous to Tor's relay discovery).

**x402/BSV integration.** Edwin attaches a BSV micropayment proof to the OHTTP encapsulated request. The relay validates the payment (without reading the content, since payment proof can be in the outer OHTTP envelope) before forwarding. This is the cleanest x402 integration: relay is a paid commodity service, content is encrypted, relay never reads it. [INFERRED from OHTTP packet structure and x402 architecture]

**Latency.** One additional relay hop: estimated +15–40ms. Acceptable for non-interactive tasks; marginal for real-time autocomplete.

---

### Option D: TEE Inference Tier (§H5 Option D)

**Mechanism.** For highest-sensitivity queries, Edwin routes through a TEE inference endpoint (Tinfoil.sh, NEAR AI, or equivalent). Model runs in hardware-isolated enclave; even infrastructure operators cannot read prompts. BSV x402 micropayment funds the inference at a premium tier price.

**What this achieves.** "Trust but verify" — the gold standard for cloud inference privacy. No actor in the chain (Hodos, the TEE provider, the cloud host) can read the content. Verifiable by Edwin at request time via the attestation check.

**Edwin integration mechanics.** The NEAR AI TEE path (used by Brave Nightly) provides a blueprint:
1. Edwin fetches the current attestation report from the TEE endpoint.
2. Edwin validates the attestation chain: source code → Sigstore measurements → hardware attestation.
3. If valid, Edwin encrypts the prompt to the attested TLS key (which is enclave-bound).
4. Edwin attaches BSV x402 payment proof.
5. Enclave decrypts, runs inference, returns response + signed compute receipt.
6. Edwin logs the compute receipt TXID on-chain (optional audit trail).

**Model selection constraint.** Only models deployed in TEE environments are available. As of mid-2026, production-available TEE inference models: DeepSeek V3.1 (Brave/NEAR AI, Nightly), `gpt-oss-120b` (DDG/Tinfoil). Anthropic Claude and OpenAI GPT are not yet available in hardware-attested TEE environments for third-party access. [FACT — per research; UNVERIFIED on completeness of current TEE model catalog]

**Cost and latency.** Higher than standard cloud inference. Tinfoil and NEAR AI both report near-zero inference overhead from confidential computing itself, but TEE-capable hardware (H100/H200/B100) is priced at a significant premium over commodity cloud GPUs. [FACT — Nvidia whitepaper; UNVERIFIED specific pricing]

**Stacking with OHTTP.** For maximum privacy, stack TEE inference with OHTTP relay: relay decouples IP (relay does not know content), TEE decouples content from operator (operator does not know content). Combined: no single party holds both identity and query content with decryption capability. Apple PCC does exactly this combination. [FACT — Apple PCC architecture]

---

### Option E: BYO-Key (Per-Provider Direct, §H5 Option E)

**Mechanism.** User configures their own API keys (Anthropic, OpenAI, etc.) in Hodos settings. Edwin routes directly to provider endpoints. No Hodos intermediary.

**What this achieves.** Hodos has zero visibility. The user bears the full privacy relationship with their chosen provider directly.

**What it leaks.** The user's IP is directly visible to the provider. The API key is a persistent identity — the provider can link all requests. This is materially worse from a privacy standpoint than Options B, C, or D. It is equivalent to using the provider's web interface directly, with Edwin as a client-side convenience layer.

**Suitability.** Appropriate for users who already have an Anthropic/OpenAI subscription and understand they are trading privacy for model quality. Should be presented as the "advanced" path, not the default. [INFERRED from §H5 Option E of local study; FACT per BROWSER_AI_IMPLEMENTATION_STUDY.md §H1 Option D analysis]

---

## 9. What This Means for Hodos — Options, Not a Pick

The following presents the genuine architectural options Hodos faces in each dimension, with their trade-offs stated honestly. No option is selected here.

### 9.1 The Privacy Tier Stack

The architectures in §§1–6 naturally compose into a tier stack that Edwin could implement. Each tier is a distinct user opt-in level:

**Tier 0 — Local only.** Edwin routes to Ollama on localhost. No network traffic during inference. Verified by packet inspection. Quality ceiling: ~7B parameter models. Cost: electricity. Hodos infrastructure: none required. BSV role: agentic authorization (consent via satoshi payment), not billing.

**Tier 1 — Hodos proxy (Brave Leo pattern).** User opts into cloud capability. Edwin routes through Hodos-operated reverse proxy. IP stripped. No-train contract enforced. If VOPRF blind tokens (or BSV-payment equivalents) are implemented: request unlinkable to account. Hodos becomes a data processor. BSV role: per-request billing through the proxy.

**Tier 2 — OHTTP relay (Apple/Mozilla pattern).** Cloud requests route through an independent OHTTP relay. Hodos never holds plaintext content. Relay can be a paid BSV service. IP decoupled from content. Gateway sees content but not IP. Requires third-party relay operator (independent from Hodos and model provider). BSV role: relay funding via x402 micropayment attached to outer OHTTP envelope.

**Tier 3 — TEE inference (DuckDuckGo/Tinfoil/Brave Nightly pattern).** Inference runs inside hardware-attested enclave. Operator cannot read content. Model integrity verified by attestation at request time. Edwin validates attestation before transmitting. BSV role: enclave-enforced streaming payment (NEAR AI model). Requires partnership with Tinfoil.sh or NEAR AI; model selection currently limited.

**Tier 4 — TEE + OHTTP combined.** Stacks Tier 2 and Tier 3. No single party holds both originating identity and plaintext content. Closest to Apple PCC's privacy level achievable without Apple Silicon. Adds OHTTP relay latency (~15–40ms) on top of TEE overhead.

**Key trade-off the tiers surface.** Moving up the tier stack adds: (a) complexity for the user to understand, (b) latency, (c) cost, (d) dependency on third parties. The Hodos question is where to put the default and what triggers an upgrade. Current evidence from the industry: privacy-motivated users accept explaining the tier system; casual users need a clear default that "just works" with minimal friction.

### 9.2 The x402/BSV Payment Integration Points

x402/BSV micropayments can fund different parts of this architecture in different ways. The three clean integration points:

1. **Relay payment (Tier 2, OHTTP).** Edwin pays an OHTTP relay operator per forwarded request. Payment proof in the outer OHTTP envelope — relay validates, forwards. Relay never reads content. BSV transaction amount: 1–10 satoshis per request depending on payload size.

2. **TEE inference payment (Tier 3).** Edwin sends proof of BSV payment to the TEE endpoint. Enclave validates on-chain (SPV proof) before serving inference. Compute receipt returned. On-chain audit trail. NEAR AI's existing streaming payment architecture is directly compatible with BSV if an adapter is built. BSV transaction amount: 10–100 satoshis per inference call (model-dependent).

3. **Agentic consent (all tiers).** Each agentic action (form fill, navigation, cross-site research) is authorized by a BSV micropayment from the user's wallet. The payment IS the consent — cryptographic, unforgeable, on-chain audit trail. Prevents prompt-injection-triggered autonomous actions (attacker must spend real satoshis to trigger). This is the architecture described in BROWSER_AI_IMPLEMENTATION_STUDY.md §H4 Option C. [FACT — local study]

The broader x402 protocol ecosystem (Coinbase/Solana variant) has processed 75+ million transactions and been adopted by Cloudflare, Vercel, and Nous Research for per-inference billing. [FACT — https://www.allium.so/blog/x402-explained-the-internet-native-payments-standard-for-apis-data-and-agent-commerce/] The BSV variant has the significant advantage of sub-cent transaction fees (BSV fees are routinely under 1 satoshi per byte), making micropayments of 1–10 satoshis economically viable without fee overhead dominating the payment.

### 9.3 The User Communication Problem

The Chrome Nano conflation problem is the clearest anti-pattern: a locally-installed 4 GB model creating false user inference that all AI queries are processed locally, when in fact omnibox AI Mode queries go entirely to Google's cloud. [FACT — BROWSER_AI_IMPLEMENTATION_STUDY.md §B, §E Tier 4]

For Hodos, every Edwin surface must clearly label:
- "Local (Edwin + [model name])" when inference is on-device.
- "Cloud via Hodos proxy" when routing through Tier 1.
- "Cloud via OHTTP relay (IP-private)" when routing through Tier 2.
- "Cloud via verified TEE (hardware-attested)" when routing through Tier 3.
- "Direct to [Provider name] (your API key)" when routing through Tier 4.

The x402 payment event is itself a natural moment for this disclosure: the user sees the amount and destination before authorizing. A BSV payment to "Hodos Relay" vs "Tinfoil TEE Node" vs "Anthropic API" communicates the routing tier through the payment UI itself.

### 9.4 What Edwin Cannot Do That Requires a Hodos Decision

The five options in §8 all depend on Hodos making commitments Edwin cannot make:

| Decision | Why Edwin Cannot Make It | Why Hodos Must |
|---|---|---|
| Operate a proxy server | Edwin is a localhost sidecar, not a cloud service | Hodos controls the cloud infrastructure |
| Contract with model providers (no-train DPA) | Edwin has no legal entity | Hodos signs provider agreements |
| Partner with Tinfoil.sh or NEAR AI | Edwin has no external vendor relationship | Hodos negotiates the TEE partnership |
| Publish OHTTP gateway keys to a transparency log | Requires an external service and key management | Hodos owns the key management infrastructure |
| Define the default privacy tier | Edwin follows the configuration it receives | Hodos sets the factory default in the product |

Edwin's role is to implement the routing logic faithfully — it already exposes an MCP server and can route to any endpoint. The architectural decisions above are product and infrastructure commitments for Hodos.

---

## 10. Open Questions

The following questions are unanswered by this research and are relevant for the next phase of planning:

1. **Relay operator identity.** If Hodos uses OHTTP (Tier 2), who operates the relay? Cloudflare and Fastly both have existing OHTTP relay infrastructure. Is there a BSV-native relay operator in the ecosystem willing to accept satoshi payments? What happens to Hodos users if the relay operator is unavailable?

2. **TEE model catalog gap.** As of June 2026, Claude 3.5/4 and GPT-4o are not available through a hardware-attested TEE for third-party browsers. Is Anthropic or OpenAI pursuing TEE inference availability? What is the timeline? Until this gap closes, the TEE tier (Tier 3/4) means model quality is limited to open-weight models (DeepSeek V3.1, etc.).

3. **VOPRF vs. BSV-payment-as-credential.** Brave's VOPRF scheme provides cryptographic unlinkability. A BSV-micropayment credential provides on-chain auditability but different privacy properties (BSV transactions are pseudonymous, not anonymous). What is the right credential scheme for Hodos — adapt Brave's open-source VOPRF library, use a BSV UTXO-based credential, or both?

4. **Proxy cold-start timing.** If Hodos operates a proxy (Tier 1), the proxy is a persistent service that must be available 24/7 before users generate traffic. When should Hodos begin operating this infrastructure? What is the minimum viable scale for it to be cost-effective?

5. **Local model download consent and size.** A local inference default (Tier 0) requires downloading model weights (4–8 GB). Consent flow, source (Hugging Face vs. Hodos-hosted vs. IPFS/1Sat Ordinals), and fallback (what happens if download fails or user has insufficient disk) all need design. Chrome's undisclosed 4 GB download was the single largest privacy backlash event in browser AI in 2025. [FACT — BROWSER_AI_IMPLEMENTATION_STUDY.md §B]

6. **Edwin's MCP architecture as the abstraction layer.** Edwin already exposes an MCP server. Could the routing logic be implemented as MCP tools rather than hardcoded Edwin internals? E.g., `mcp:hodos_proxy`, `mcp:ohttp_relay`, `mcp:tee_inference`. This would make routing tiers pluggable and upgradeable without Edwin core changes, which matters given the constraint that Hodos does not fork Edwin.

7. **Attestation verification in Edwin (Node.js constraint).** Verifying Tinfoil/NEAR AI attestation reports requires cryptographic verification of AMD/Nvidia certificate chains and Sigstore transparency logs. Is there a Node.js library mature enough to do this correctly, or would this require a Rust/C++ native addon in Edwin? Brave implements this in their C++ browser layer, not the Node gateway layer.

8. **Legal status of the Hodos proxy for EU/GDPR users.** Operating a proxy that receives EU user prompts makes Hodos a data processor under GDPR. What processing agreements, DPAs, and data residency constraints apply? Microsoft Edge explicitly disables cloud AI features for EU users by default specifically for this reason. [FACT — BROWSER_AI_IMPLEMENTATION_STUDY.md §E Tier 5]

9. **BSV transaction fee stability.** Per-request BSV payments of 1–10 satoshis are economically viable today at current BSV pricing. What is the design tolerance for fee escalation? If BSV price increases 10x, does the micropayment UX become friction rather than convenience?

10. **Competitive timing.** Brave's TEE path is on Nightly (not production); DuckDuckGo's TEE tier is limited to one model; Apple PCC is Apple-only. The window for Hodos to establish a "TEE-native browser AI" position before these become default-on in major browsers is non-zero but finite. What is the minimum viable Hodos TEE implementation that establishes the positioning before Brave ships TEE to stable?

---

**Sources:**

- [Apple Private Cloud Compute — Security Research Blog](https://security.apple.com/blog/private-cloud-compute/)
- [Apple PCC Security Research (VRE, audit)](https://security.apple.com/blog/pcc-security-research/)
- [Apple PCC Expanding to Google Cloud, June 2026](https://security.apple.com/blog/expanding-pcc/)
- [Apple PCC Analysis — arxiv.org 2605.24239](https://arxiv.org/html/2605.24239v1)
- [Brave Leo — Verifiable Privacy and TEE announcement](https://brave.com/blog/browser-ai-tee/)
- [Brave Leo — VOPRF premium account privacy (GitHub)](https://github.com/brave/brave-core/blob/master/docs/premium_account_privacy.md)
- [Brave Leo — Android launch (proxy architecture)](https://brave.com/blog/leo-android/)
- [Brave Leo — Self-hosted models, automatic mode](https://brave.com/blog/automatic-mode-leo/)
- [Brave Leo — BYOM](https://brave.com/blog/byom-nightly/)
- [Brave Leo — TEE at The Register](https://www.theregister.com/2025/11/22/brave_leo_trusted_execution_environment/)
- [Brave Leo — TEE at Privacy Guides](https://www.privacyguides.org/news/2025/11/20/brave-announces-verifiable-and-transparent-tee-support-in-leo/)
- [DuckDuckGo Duck.ai — Privacy Help Pages](https://duckduckgo.com/duckduckgo-help-pages/duckai/ai-chat-privacy)
- [DuckDuckGo — What is shared with model providers](https://duckduckgo.com/duckduckgo-help-pages/what-information-does-duckai-share-with-model-providers)
- [RFC 9458 — Oblivious HTTP](https://www.rfc-editor.org/info/rfc9458/)
- [RFC 9458 — IETF Datatracker](https://datatracker.ietf.org/doc/rfc9458/)
- [Mozilla Support — OHTTP Explained](https://support.mozilla.org/en-US/kb/ohttp-explained)
- [ohttp-gateway Rust crate](https://crates.io/crates/ohttp-gateway)
- [Divvi Up — OHTTP available](https://divviup.org/blog/ohttp-now-available/)
- [Tinfoil.sh — Technology](https://tinfoil.sh/technology)
- [Tinfoil.sh — Privacy blog](https://tinfoil.sh/blog/2025-05-15-privacy)
- [Tinfoil.sh — Comparison with Apple PCC](https://tinfoil.sh/blog/2025-01-30-how-do-we-compare)
- [Tinfoil — Attestation Architecture (Docs)](https://docs.tinfoil.sh/verification/attestation-architecture)
- [NEAR AI — Private Inference](https://docs.near.ai/cloud/private-inference/)
- [NEAR AI — Decentralized Confidential ML](https://near.ai/blog/decentralized-confidential-machine-learning)
- [Nvidia — Secure AI with Blackwell and Hopper (whitepaper)](https://docs.nvidia.com/nvidia-secure-ai-with-blackwell-and-hopper-gpus-whitepaper.pdf)
- [Nvidia — Confidential Computing product page](https://www.nvidia.com/en-us/data-center/solutions/confidential-computing/)
- [Corvex — Blackwell Confidential Computing](https://www.corvex.ai/blog/confidential-computing-meets-nvidia-hgxtm-b200-secure-ai-without-the-performance-trade-off)
- [Red Hat — Confidential AI inference](https://next.redhat.com/2025/10/23/enhancing-ai-inference-security-with-confidential-computing-a-path-to-private-data-inference-with-proprietary-llms/)
- [x402 — allium.so explainer](https://www.allium.so/blog/x402-explained-the-internet-native-payments-standard-for-apis-data-and-agent-commerce/)
- [x402 — Whitepaper](https://www.x402.org/x402-whitepaper.pdf)
- [Mozilla — Local AI runtime speedup](https://blog.mozilla.org/en/firefox/firefox-ai/speeding-up-firefox-local-ai-runtime/)
- [Apple AFM 3 — Machine Learning Research](https://machinelearning.apple.com/research/introducing-third-generation-of-apple-foundation-models)
- [Google Safe Browsing OHTTP Gateway API](https://developers.google.com/safe-browsing/ohttp/reference)
- [Edera — Apple PCC vs Confidential Computing](https://edera.dev/stories/apples-private-cloud-compute-vs-confidential-computing)
- [Chutes — Confidential Compute for AI Inference](https://chutes.ai/news/confidential-compute-for-ai-inference-how-chutes-delivers-verifiable-privacy-with-trusted-execution-environments)
- [Confidential Inference Directory](https://confidentialinference.net/)
- [BROWSER_AI_IMPLEMENTATION_STUDY.md §E, §H4, §H5](C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\BROWSER_AI_IMPLEMENTATION_STUDY.md) — Local file, accessed 2026-06-28
