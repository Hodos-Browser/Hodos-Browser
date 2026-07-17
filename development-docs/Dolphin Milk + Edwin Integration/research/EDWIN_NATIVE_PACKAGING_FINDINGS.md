# Edwin Native-Packaging & Protected-Core Findings (code study)

> 📁 Part of the **Dolphin Milk + Edwin Integration** doc set (technical half) — see `README.md`.
> **Captured:** 2026-06-26, from a direct code study of the Edwin checkout (`~/edwinpai`, WSL), `@edwinpai/edwinpai` `1.0.0-beta.8` (origin now at `beta.9`).
> **Why this doc exists:** Decision 1 (how to bundle/run Edwin natively, no WSL) and Decision 2 (Edwin vs Dolphin Milk vs both) hinge on Edwin's actual native-dependency surface and how its crypto core is built. These are the hard facts, dug out of the source. **Study, not decision** — feeds the architecture session.

---

## 1. Headline findings

1. **The crown-jewel crypto/vault is a closed-source, per-platform NATIVE companion module — and there is NO JavaScript fallback.** `@edwinpai/identity-core` in the repo is only a TypeScript *interface + runtime loader*. The real implementation (envelope signing, BRC-103 signed HTTP requests, key derivation, verification) is a separate **restricted-publish, BSL-licensed, per-platform native package** loaded at runtime. If it isn't wired, `factory.ts` throws `IdentityCoreUnavailableError`. **Implication: native Windows/macOS Edwin requires Jake to build & publish a Windows/macOS native companion for identity-core (and shad-core). This is a hard upstream dependency.**

2. **…BUT the identity core can be backed by any async TRANSPORT, not just an in-process native addon.** `node-binding.ts` exposes `createNodeIdentityCoreBinding(transport)` — wrap *any* object that can `signHttpRequest` / `signEnvelope` / `verifyEnvelope` / `getPublicKey`, and Edwin uses it as its IdentityCore. `desktop-binding.ts` already does this for a **Rust/Tauri desktop backend** (snake_case `public_key`, `avatar_svg`, `short_id` betray a Rust struct). **Implication: Hodos's existing Rust wallet — which already does secp256k1, key custody (DPAPI/Keychain), BRC-100 — could implement the IdentityCore transport, and Edwin would sign *through Hodos*. This is an alternative to bundling Jake's native core, and it maps almost exactly onto the `ARCHITECTURE_TECHNICAL.md` plan (Hodos wallet does envelope issuance/verification).**

3. **The native dependency surface for the *rest* of Edwin is modest and mostly prebuilt-cross-platform.** The genuinely-native npm deps are `sharp` (image; libvips prebuilds for win/mac/linux), `@lydell/node-pty` (terminal; prebuilds), `@napi-rs/canvas` (peer; napi prebuilds), `sqlite-vec` (recall vector store; prebuilt loadable extension). Transitive: `@matrix-org/matrix-sdk-crypto-nodejs` (Rust/napi prebuilt, Matrix channel), `authenticate-pam` (**Linux-only**, irrelevant on Win/Mac). **`node-llama-cpp` is an OPTIONAL peer dependency** — local LLM inference is opt-in and fully avoidable with cloud models/embeddings (confirms the kickoff doc's note: the worst native offender is avoidable).

4. **`install.sh` is `curl … | bash` — Unix-only, and assumes Node is already installed** (it shells out to `node`). There is no Windows installer and no bundled runtime. This *is* the casual-user gap from `LESSONS_LEARNED`.

5. **Sizes:** `dist/` = 18 MB (the built gateway bundle, via tsdown/rolldown). `node_modules/` = 1.6 GB but that's the **full dev tree** (Playwright, every channel SDK, test tooling); a pruned production runtime tree would be far smaller. `packages/` (protected cores, source) = 6.9 MB. No compiled `.node`/`.dll`/`.dylib` are checked into this tree — native artifacts are produced/fetched at release time via the `*-platform-packages` scripts.

---

## 2. The protected-core architecture (how identity-core / shad-core actually load)

```
@edwinpai/identity-core (in-repo, open TS)
├── types.ts            IdentityCore interface (sign/verify/derive/getIdentity)
├── factory.ts          createIdentityCore(): DeferredIdentityCore
│                        - resolves an implementation lazily
│                        - if none wired -> throws IdentityCoreUnavailableError (NO fallback)
├── native-loader.ts    loads a per-platform native companion package:
│                        env-path override -> bundled -> staged -> resolve companion
│                        getIdentityCoreNativeRuntimeTriple() = napi-style platform triple
│                        loadNativeIdentityCore() returns null if nothing wired
├── node-binding.ts     createNodeIdentityCoreBinding(transport)  <-- TRANSPORT path
├── desktop-binding.ts  createDesktopIdentityCoreBinding(transport) <-- Rust/Tauri desktop today
└── binding.ts          createIdentityCoreFromBinding(...)

Actual native crypto impl  ->  SEPARATE restricted repo (BSL, publishConfig.access=restricted),
                                published as per-platform packages, loaded at runtime.
```

- Build pipeline already exists for multi-platform native artifacts: `identity-core:prepare-platform-packages`, `:smoke-platform-packages`, `:audit-platform-package-packs` (and the same for `shad-core`). So **the machinery to ship per-platform native cores exists** — the question is whether Windows targets are built/published, which is Jake's call.
- `EDWINPAI_REQUIRE_NATIVE_PROTECTED_CORES=1` + `EDWINPAI_IDENTITY_CORE_MODULE` env hooks let you force/override the native module — useful for a Hodos-controlled bundle.

---

## 3. What this means for the open decisions (options, not picks)

### Decision 1 — bundle/run Edwin natively (no WSL)
- **The non-protected ~95% of Edwin** (Node gateway + dist + sharp/node-pty/napi-canvas/sqlite-vec prebuilds) **bundles natively on Windows/macOS without exotic work** — these are standard prebuilt-binary npm packages. Ship a Node runtime + pruned `node_modules` + `dist/` as a Hodos-managed sidecar.
- **The protected cores are the gating item.** Two paths:
  - **(1a) Bundle Jake's native companion** — needs Jake to build/publish `identity-core` + `shad-core` Windows (x64/arm64) + macOS (arm64/x64) native packages. Cleanest fidelity to Edwin; hard dependency on Jake's release.
  - **(1b) Back IdentityCore via a Hodos transport** — implement `NodeIdentityCoreTransport` against the Hodos Rust wallet (already has the crypto). Edwin signs through Hodos. Less dependent on Jake's native build; but Hodos must faithfully implement the envelope/sign/verify semantics, and `shad-core` (recall) still needs a story.
- Single-binary (Node SEA / `pkg` / `bun build --compile`) is **hard** here: ESM + native addons + dynamic plugin/skill loading. The runtime-sidecar approach (bundled Node + files) is the realistic packaging.

### Decision 2 — Edwin vs Dolphin Milk vs both
- The transport-binding discovery blurs the line: Hodos's Rust wallet can be Edwin's identity/vault backend, which is conceptually what Dolphin Milk's SecureVault gate was meant to be. Worth studying whether **Hodos-wallet-as-vault + Edwin-as-assistant** collapses two of the three parties.

### For the Jake agenda (sharpens `ARCHITECTURE_TECHNICAL.md` §9)
- SecureVault extraction is really: *"will you publish Windows/macOS native companions for identity-core & shad-core, OR bless the transport-binding path where Hodos's wallet backs IdentityCore?"* — a concrete, answerable question now.
- The envelope schema lives in `types.ts` (`SignedEnvelope`, `SignEnvelopeInput`, `VerifyEnvelopeOptions`) — a real interface to standardize against (the possible BRC).

---

## 4. Raw facts (for reference)
- Version: `1.0.0-beta.8` local / `beta.9` origin. Node `>=22.12.0`. pnpm `10.23.0`. ESM (`type: module`). Build: tsdown + rolldown → `dist/`.
- Native npm deps: `sharp` ^0.34.5, `@lydell/node-pty` 1.2.0-beta.3, `sqlite-vec` 0.1.7-alpha.2; peers `@napi-rs/canvas` ^0.1.89, `node-llama-cpp` 3.15.1 (**optional**). `onlyBuiltDependencies`: node-pty, matrix-sdk-crypto-nodejs, napi-canvas, baileys, authenticate-pam (Linux), esbuild, protobufjs, sharp.
- Entry: `edwinpai.mjs` (bin), `dist/index.js` (main), gateway run via `scripts/run-node.mjs`.
- No Tauri/Electron/`.exe`/`Cargo.toml` in this repo; the macOS Desktop app (`dist/EdwinPAI.app`, referenced in vitest config) and the native crypto core are built elsewhere.
