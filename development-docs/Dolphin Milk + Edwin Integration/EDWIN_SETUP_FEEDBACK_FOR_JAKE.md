# Edwin Setup Feedback — for Jake

**Audience:** Jake (Edwin maintainer)
**From:** Matt Archbold (matthew.archbold@marstonenterprises.com)
**Status:** Living doc; appended as we hit friction. Will be folded into the post-meeting follow-up email and / or sent standalone.
**Tone:** Engineer-to-engineer, constructive. We want Edwin to succeed on Windows; this is what we hit and what would have saved us hours.
**Companion doc:** `INTEGRATION_PLAN_v1.md` (the Hodos↔Edwin↔Dolphin Milk plan, sent to you AND John).

---

## TL;DR

Edwin works well on Windows when installed correctly. The path-of-least-resistance install (clone to `/mnt/c`, run the gateway over the WSL 9P bridge) hits a stack of multi-minute hangs and silent config failures. Every issue we hit traces back to two roots:

1. **The Linux gateway should never read its own source over 9P.** Once we re-installed into `~/edwinpai` (ext4), boot dropped from ~5 min to seconds. This is the single most impactful change.
2. **The setup guide assumes a clean machine.** A check-first / detect-and-act runbook ("run `command -v X`; if found skip, else install") would have saved time both for fresh users AND when re-running steps to recover from earlier failures.

The rest of this doc is the friction we hit, in priority order.

---

## 1. High-impact items (where users will get stuck)

### 1.1 The default `qmd` does local embeddings; CUDA-OOM-crashes on consumer GPUs
- Published `@tobilu/qmd` 1.1.0 uses `embeddinggemma-300M` via `node-llama-cpp`.
- On a 6GB-ish consumer GPU it crashes with CUDA out-of-memory; the CPU fallback is slow.
- **Suggestion:** Make remote embeddings (OpenAI / Gemini) the default in the install path, and document the local option as advanced / opt-in. The fork at `jonesj38/qmd @ feat/openai-embeddings` (v2.5.2) already does this; merging it upstream would let `install.sh` set it directly.

### 1.2 Two memory subsystems both default to local embeddings
- The `qmd` backend (`memory.qmd.embeddingApiKey`) AND the `shad-context` plugin (its own embedding key) both default to local.
- Both need the remote key set independently or boot quietly degrades.
- **Suggestion:** A single `embedding.apiKey` at the root that both subsystems read by default, OR `edwinpai setup` interactive prompt that asks once and writes both.

### 1.3 Legacy → new install collision
- Older `edwin` binary + `~/.edwin/` config home coexist with the new `edwinpai` + `~/.edwinpai/`.
- `install.sh` is supposed to migrate, but the migration left stale paths on our machine — the `shad-context` plugin was still pointed at `~/.edwin/workspace` (non-existent).
- **Suggestion:** Make migration verify-and-fix on every `edwinpai setup` run, not just on first install. Surface a warning if any plugin config still references `~/.edwin/`.

### 1.4 JSON5 config + strict JSON tooling don't mix
- `edwinpai.json` allows comments (JSON5). Any tool that `JSON.parse` → mutate → `JSON.stringify` strips comments.
- We hit this when an automated agent tried to set `gateway.bind: lan` and the edit failed silently.
- **Suggestion:** Either commit to JSON5 (and document that tooling MUST use a JSON5 parser like `json5` or `JSON5.parse` from the lib) OR migrate the config to a format with first-class comment support (YAML, TOML). Pick one; today the gap creates silent failures.

### 1.5 Desktop app doesn't create `%APPDATA%\com.edwinpai.desktop\`
- On a fresh Windows machine, the Tauri app's `writeTextFile` calls flood with `os error 3` because the AppData dir doesn't exist yet.
- The app's `src/lib/config.ts` assumes Tauri auto-creates it; Tauri doesn't.
- **Fix:** Add `mkdir(BaseDirectory.AppData, { recursive: true })` before the first `writeTextFile` in `src/lib/config.ts`. We worked around it by creating the dir manually; a fresh user won't know to do that.

---

## 2. Documentation & UX gaps

### 2.1 Runbook should be idempotent / check-first
Today's setup guide reads top-to-bottom assuming nothing is installed. A more useful pattern for both fresh users and recovery scenarios is:

```
Step N — Install pnpm
  Check: `command -v pnpm && pnpm -v` → if version, skip to step N+1
  Action: `corepack enable && corepack prepare pnpm@latest --activate`
  Verify: `pnpm -v` prints a version → continue
```

Every step gets a `Check / Action / Verify` triad. Lets the user re-run the whole runbook to pick up where things broke without redoing work. Internal benefit: same script runs as both `install.sh` (action mode) and `edwinpai doctor` (check mode).

### 2.2 `bind: lan` is buried
The single most important config setting for Windows users (gateway reachability from the host) is in Part 4.3 of the setup guide and easy to miss. The default of `loopback` produces a silent timeout from the desktop with no useful error. **Suggestion:** Default new installs to `lan` on Windows / WSL detection; or surface a first-run warning ("`bind: loopback` + WSL detected — Windows desktop won't reach gateway; switch to `lan`?").

### 2.3 Hyper-V firewall is undiscoverable
If a user picks mirrored networking (recommended for Win11), the Hyper-V firewall blocks inbound by default — and Hyper-V firewall is **separate** from the normal Windows Defender Firewall (different cmdlets: `New-NetFirewallHyperVRule` vs `New-NetFirewallRule`). Most Windows admins won't know to look there. **Suggestion:** Add a `edwinpai doctor` check that runs `Test-NetConnection localhost -Port 18789` on Windows + WSL detection and, if False, emits the exact `New-NetFirewallHyperVRule` one-liner needed.

### 2.4 "Which Node" is non-obvious
On a machine with Windows Node installed, the WSL shell can inherit `/mnt/c/Program Files/nodejs/...` in `$PATH` and nvm-installed Linux Node gets shadowed. Native modules then break in confusing ways. **Suggestion:** `edwinpai doctor` checks `realpath $(command -v node)` and warns if it's under `/mnt/c/`.

### 2.5 The split between `EDWIN_SETUP_REPORT.md` and `WINDOWS_SETUP_GUIDE.md` is exactly right
For Hodos's own future installer docs, that split — **report** (what we changed + lessons + stack background) vs **runbook** (step-by-step) — is the model. Worth keeping as the canonical Edwin install docs shape.

---

## 3. Smaller items

### 3.1 The `which node` PATH check in §2.2 of the setup guide is correct but soft
The note buries the actual fix ("ensure the nvm lines are at the end of `~/.bashrc`"). Make this a hard check + auto-fix in the installer.

### 3.2 systemd `linger` is undocumented for non-power-users
`loginctl enable-linger "$USER"` is non-obvious; an end user reading the guide won't know why they need it. A one-line "without this, the gateway stops when you close the terminal" would help.

### 3.3 The Vite dev-script bash-only flag bug
`--port ${VITE_PORT:-1420}` doesn't expand under PowerShell. Already fixed in `edwin-desktop` commit `8063cf7`. Worth a regression test that runs `npm run dev` from a non-bash shell.

---

## 4. Cross-platform — Windows vs macOS

This whole doc is dominated by Windows because that's where we hit friction. macOS notes (for the integration plan):

- **No WSL, no 9P bridge.** The "install to ext4, not `/mnt/c`" issue doesn't exist on macOS. Edwin runs natively.
- **No Hyper-V firewall.** macOS has its own application firewall (System Settings → Network → Firewall) but it doesn't gate localhost-to-localhost the way Hyper-V firewall gates Windows-to-WSL.
- **launchd vs systemd.** `edwinpai daemon install` uses `launchd` on macOS, `systemd` on Linux. The user-experience is similar; the unit file format is different. Worth verifying the launchd path is as polished as the systemd path before recommending macOS to non-developer users.
- **Native modules.** macOS has the same `node-llama-cpp` / `sharp` / `@lydell/node-pty` story but with notarization / Gatekeeper as the gotcha instead of 9P. Apple Silicon vs Intel adds another axis.
- **Keychain access.** macOS Keychain is the natural place for Edwin to store the gateway token and API keys. Currently it lives in `~/.edwinpai/edwinpai.json` (plaintext on disk). Worth designing an OS-keychain-backed path for Mac, Windows Credential Manager on Windows. (Same issue Hodos solves with DPAPI on Windows / Keychain on macOS for its wallet.)

---

## 5. What we'd love to see (wishlist, not requirements)

- **`edwinpai doctor`** — single command that runs all the checks: filesystem location, Node origin, embeddings provider, network reachability, firewall rule, systemd / launchd state, AppData dir. Prints a checklist with ✓ / ✗ / how-to-fix.
- **First-run wizard.** `edwinpai setup` could do the interactive version of the doctor: detect what's missing, prompt for what's needed (API key, embedding provider), write the config in one pass.
- **Web installer.** `curl edwinpai.com/install | bash` is great; a Windows equivalent (a Tauri-bundled installer that creates the WSL Ubuntu instance + sets up the gateway in one click) would compress 90 minutes of work into 10. Long-term thing — flagging because it's the single largest UX gap.

---

## 5.10 Empirical 9P bridge measurement — Edwin via WSL is not chat-capable for Windows-side content (2026-06-08)

**Measurement.** With `shad context` searching a 500-file directory:

| Source | Files | Wall-clock |
|---|---|---|
| `/mnt/c/Users/archb/Hodos-Browser/` (Windows side, accessed via WSL's 9P bridge) | ~500 | **1m 43s** |
| `~/repos/BRCs/` (WSL ext4 native) | 33 | **0.53s** |

The 100-second cost is **purely Windows file I/O via the 9P bridge** — CPU and system time during the 9P call were ~6 seconds combined. The remaining ~97 seconds was waiting on file reads through Microsoft's virtio-9P implementation. Repeated searches showed no caching: every query re-walks the directory.

**Implication.** Standalone Edwin on Windows + WSL2, with the user's working content on the Windows side (the normal case — Hodos-Browser, business docs, pitch material), gives chat-driven recall queries that take **2 minutes to answer**. Not "slow." Structurally non-functional. The user reads their AI assistant as broken.

This isn't a 9P tuning problem — it's an architectural one. **`shad context` / `qmd` / shad-context's recall lane all need ext4-native paths to operate at chat speed.** The workaround we landed on locally: maintain a WSL-side mirror of the Windows content via a git-mediated sync layer (designed in our `WSL_HYBRID_WORKSPACE.md`), so Edwin reads from `~/repos/...` while the user edits on Windows. Works, but adds permanent infrastructure complexity that doesn't exist on macOS or native Linux.

**Recommendation:** the "Windows-native Edwin gateway build" referenced in our integration plan §4.1.1 Option C moves from *"recommend prioritizing"* to **"structural requirement for any Windows user above developer tier."** Without it, every Windows user either (a) sees Edwin as unusably slow, or (b) maintains a sync-mirror infrastructure that consumer users won't accept.

Compounding: the wizard-default setup the user lands on has Edwin reading from `~/.edwinpai/workspace/` (WSL ext4 — fast), but most useful Edwin recall is against the user's documents, source code, business material — all Windows-side. So the wizard's defaults paper over the problem until the user actually puts their content in front of Edwin, at which point chat becomes unusable.

## 5.9 Shad install footprint is heavy enough to be a Windows-audience barrier (2026-06-08)

The canonical Shad install (via `https://github.com/jonesj38/shad.git` `install.sh`) requires:

- Python 3.11+
- Docker + Docker Compose (for Redis + Shad API service stack via docker-compose)
- A Python virtualenv with FastAPI + dependencies
- 2 GB of local model files when running in default local-embedding mode (`embeddinggemma-300M` 313 MB + `qmd-query-expansion-1.7B` 1.22 GB + `qwen3-reranker-0.6b` 640 MB)

The OpenAI-embeddings path (your qmd fork at `feat/openai-embeddings` + `QMD_OPENAI=1`) eliminates the 2 GB local-model footprint but adds an OpenAI API key dependency with associated ongoing cost. Both modes still need Docker + Python + Redis.

For technical users this is fine. For the broader Windows audience the Hodos integration targets, this stack is a hard prerequisite cliff. The install order we ended up doing:

1. Confirm WSL2 + Ubuntu installed
2. `sudo usermod -aG docker $USER` + `newgrp docker` (Docker permission setup — friction point)
3. Clone Jake's qmd fork, `pnpm install`, `pnpm build`, symlink binary
4. Clone Jake's shad repo, run `install.sh` (Python venv + dep install)
5. Configure `~/.shad/.env` to route LLM calls through EdwinPAI gateway
6. Configure `~/.config/qmd/index.yml` collections
7. Configure `shad-context` plugin `collectionPaths` in `edwinpai.json`
8. Restart gateway

8 manual steps, each with platform-specific gotchas. **Each step is a potential drop-off point for a non-developer Windows user.**

**Recommendations for the Windows installer:**
1. **Prebundle the local model files** (or prefetch with progress UI at install time, not mid-conversation). A surprise 6-minute model download in the middle of a chat is a critical UX failure.
2. **Make Docker permission setup invisible.** A first-run installer can script `usermod -aG docker` cleanly.
3. **Default to OpenAI mode for Windows installs** — eliminates the 2 GB local-model footprint, leaves ongoing cost tied to the user's own OpenAI key. The local-model mode is appropriate for privacy-sensitive deployments but is a poor default for consumer install.
4. **Bundle Shad + qmd installation into a single `edwinpai install --with-shad`** flag rather than three separate clone-and-build steps.

This reinforces §5.10 — the install + runtime overhead together push Edwin standalone toward the developer/enthusiast user. Hodos's integration path can absorb this complexity, but only by making it invisible at install time.

## 5.8 Test Connection fails because the onboarding wizard and the desktop probe make incompatible assumptions about gateway auth (root cause + local resolution)

**Symptom (2026-06-07):** Edwin Desktop's Test Connection reports "Gateway not reachable" against a gateway that is verifiably running, reachable, and answering HTTP — token correct, port correct, network confirmed. Same failure on every URL the user tried.

**What we found.** The probe at `edwin-desktop/src-tauri/src/commands/gateway_real.rs::probe_gateway` (line 571) does a plain `GET /` against the supplied URL and marks success when the response is:

- 2xx with body containing `EdwinPAI` / `edwinpai` / `__EDWINPAI_`, OR
- 401, OR
- 403

We grep'd the full `edwinpai` repo (`src/` + `dist/`) for those magic body strings and found nothing returned from any HTTP route. **The 2xx-with-body branch is dead code** — the probe relies entirely on a 401 (or 403) at `/`.

The gateway's HTTP pipeline in `src/gateway/server-http.ts:439-577` runs this order:

1. WebSocket upgrade — skip
2. **`if (resolvedBsvAuth?.enabled)`** — top-of-pipeline BSV auth gate. When `allowUnauthenticated=false`, an unsigned request to any path gets `sendBsvAuthError(res, 401)` here.
3. `/v1/edwinpai/identity/certificate` handler
4. Hooks `basePath/*`
5. `/tools/list`, `/tools/invoke`
6. Plugin paths, `/v1/responses`, `/v1/chat/completions`, canvas paths (each conditional)
7. Fallthrough: `res.statusCode = 404`

Token auth (`gateway.auth.mode = "token"`) is checked **inside individual route handlers** (e.g., line 92 in `handleIdentityCertHttpRequest`). It is NOT a top-of-pipeline gate. Paths with no registered handler — including `/` — fall through to 404 regardless of whether token auth is on.

**Root cause.** The probe (commit `c82a3df`, Feb 14 2026, "fix: gateway probe treats 401/403 as 'running'") assumes the gateway gates `/` at the top of the pipeline and returns 401. Only `bsvAuth.enabled = true` + `allowUnauthenticated = false` produces that behavior. The `edwinpai onboard` wizard writes `gateway.auth = { mode: "token", token: ... }` with **no `bsvAuth` block** — the default config the wizard produces has `bsvAuth.enabled = false`, so `GET /` falls through to 404 and the probe rejects it.

This is not Windows-specific. Same wizard-default config produces the same 404 / probe-rejects on any OS. (Our 2026-06-06 install hit this on a Windows-WSL setup; we'd have hit the same thing on a clean macOS install.)

**Local resolution we applied (2026-06-07):** added a `bsvAuth` block to `~/.edwinpai/edwinpai.json`:

```json
"gateway": {
  "auth": { "mode": "token", "token": "..." },
  "bsvAuth": { "enabled": true, "allowUnauthenticated": false },
  "port": 18789,
  "bind": "lan"
}
```

After `systemctl --user restart edwinpai-gateway.service`, `GET /` returns 401 with `{"error":"Missing authentication headers: x-bsv-identity-key, x-bsv-signature, x-bsv-timestamp, x-bsv-nonce","code":"UNAUTHENTICATED"}`. Probe accepts, Test Connection passes, Save & Connect lands the desktop on Chat. End-to-end works.

**Recommendations** (any one of these resolves it for everyone — not asking for all four):

1. **Make `edwinpai onboard` write the `bsvAuth` block by default.** If BRC-103 identity is the architectural intent of EdwinPAI, having `bsvAuth.enabled = true` is presumably the baseline; the token-only config the wizard produces today is the edge case. One-line wizard change.
2. **Add a deterministic `GET /` info route to the gateway** returning `200 { "service":"edwinpai", "version":"..." }` regardless of auth mode. The probe's 2xx-with-body branch becomes alive; the route is useful for liveness checks generally; the assumption mismatch goes away architecturally.
3. **Patch the probe to hit a known-protected path** like `/v1/edwinpai/identity/certificate` (returns 401 under both token and bsvAuth modes; lives on a known stable URL). The probe becomes robust against gateway routing changes.
4. **Better error UX regardless of which fix lands.** "Gateway not reachable" with a confirmed-running gateway is opaque. Surface the actual response (`Got HTTP 404 from http://127.0.0.1:18789/`) so the user has a path to debug. `scan_gateways` already discovers the gateway under both modes — if `scan_gateways` finds it on the same port the probe rejected, surface that disagreement.

We'd lean toward (1) or (2). (3) preserves the assumption mismatch for the next person to trip over.

## 5.7 Desktop-on-Windows first-connect friction (2026-06-07)

The Edwin Desktop first-launch experience on Windows has at least three structural issues that prevent a non-expert user from getting connected — observed across two sessions (Matt's 6-02 with Jake, and the 6-07 follow-up).

**Gateway Mode is broken on Windows by design but isn't labeled that way.** The mode-picker UI offers "Gateway Mode" with the tagline "Run your own AI gateway and share access with others." On Windows, this mode tries to auto-spawn `edwinpai-gateway.exe` — which doesn't exist because Edwin's gateway is a Node app designed to run on Linux (in WSL on Windows boxes). Users who pick Gateway Mode are sent down a dead-end. **Suggestion:** detect `process.platform === 'win32'` at the mode picker and either (a) gray out / hide Gateway Mode with a tooltip "Gateway runs in WSL on Windows — use Connect Mode and point at your WSL gateway," OR (b) make Gateway Mode actually do the right thing on Windows by treating WSL as the runtime environment.

**Connect Mode's "Test Connection" can fail on `localhost` due to IPv4/IPv6 resolution.** The Connect Mode UI auto-populates the Gateway URL with `http://localhost:18789/`. On Windows, `localhost` typically resolves to IPv6 (`::1`) first. The WSL gateway listens IPv4-only (`0.0.0.0:18789`). Test-NetConnection confirmed this from PowerShell: `WARNING: TCP connect to (::1 : 18789) failed` while the IPv4 path succeeded. Note: this issue manifests cleanly in PowerShell's `Invoke-WebRequest`; the Tauri Rust HTTP client (`reqwest`) typically falls back IPv6→IPv4, but the default-`localhost` value is still a footgun (any HTTP client that doesn't fall back will silently fail). In our 2026-06-07 session we sidestepped this by using `http://127.0.0.1:18789` directly, and Test Connection still failed — the deeper cause was the bsvAuth/probe assumption mismatch detailed in §5.8. So both issues are real, but §5.8 is the one users will hit even after they switch to 127.0.0.1. **Suggestion:** either (a) default the URL field to `http://127.0.0.1:18789/` instead of `localhost`, OR (b) verify reqwest's IPv6→IPv4 fallback is working as expected and document it for any new HTTP client added, OR (c) make the gateway bind IPv6 as well (`[::]:18789`).

**The user must hand-grab the gateway token from a JSON5 file in WSL.** No flow exists for the desktop to discover / paste / generate-and-share the auth token on first connect. The user has to either `grep` it from `~/.edwinpai/edwinpai.json`, view the file in a text editor, or `cat` it — all WSL-side. Then paste into the desktop's token field. **Suggestion:** either (a) a "Pair with gateway" QR-code flow (gateway prints a QR with URL+token on `edwinpai gateway pair`, desktop scans), OR (b) WS handshake gives the desktop a way to request initial pairing via short-lived OTP printed on the gateway side, OR (c) save the token to the host's OS keychain on gateway start and have the desktop look there.

These three issues compound: a fresh Windows user picks Gateway Mode (wrong), gets dead-ended, retreats to Connect Mode, fails Test Connection because of IPv6 default, finally retypes `127.0.0.1`, then has to go fish a token out of a JSON5 file in another shell. Each step alone is small; together they make first-connect a 30+ minute exercise even with the gateway already running correctly.

## 5.6 Onboarding wizard observations (2026-06-06)

**`edwinpai setup` vs `edwinpai onboard` — naming creates discovery friction.** The wizard described in `WINDOWS_SETUP_GUIDE.md` Part 3.2 is `edwinpai setup`, but running that today writes a minimal stub (`{ agents: { defaults: { workspace: ... } }, meta: ... }`) and exits with no prompts. The real interactive wizard is `edwinpai onboard`. Took a `--help` to find. **Suggestion:** either rename `setup` to make it clear it's a non-interactive bootstrap (e.g., `edwinpai init` or `edwinpai bootstrap`), or have it print a one-line "for the full wizard, run `edwinpai onboard`" pointer.

**Default model is wrong-provider after picking OpenAI.** The model picker's "Keep current" option shows `anthropic/claude-opus-4-5` even when the immediately-prior prompt selected OpenAI as the provider. A user who hits Enter (accepting the default) ends up with an Anthropic model name and an OpenAI key — runtime failure on first call. **Suggestion:** when the provider is OpenAI, default to an OpenAI model (probably `gpt-5-mini` or whatever's both newest-and-cheap-tier).

**Codex-flavored model is the highlighted default for the OpenAI path.** Highlighting `openai/codex-mini-latest` as the default biases new users toward a code-tuned model when most Edwin use is general (memory, agent reasoning, channels). For a coding-specific install profile (which is one of Marston's intended use-cases per `EDWIN_INTERNAL_USE_PLANNING.md`), this is great. As a generic default, it's a footgun. **Suggestion:** default to the general-purpose mini (e.g., `gpt-5-mini`); offer codex-tuned models as an option when the user identifies as a developer in an earlier wizard step.

**Provider list is broader than the WINDOWS_SETUP_GUIDE suggests.** 16 providers including some unexpected ones (Cloudflare AI Gateway, Vercel AI Gateway, Z.AI GLM 4.7, MiniMax, Moonshot Kimi K2.5, Qwen, OpenCode Zen, Xiaomi, Synthetic, Venice AI). For users who don't recognize half the names, this is overwhelming. The guide says "Anthropic / OpenAI / Gemini" as the typical menu — the actual menu is much wider. **Suggestion:** either trim the visible default list to the 3–4 most common and offer "Show more providers" for the rest, or annotate each with a one-line "what it's for" hint. Cloudflare AI Gateway and Vercel AI Gateway are particularly interesting because they're router-style aggregators — worth their own category vs being mixed with model-provider names.

**`Skip for now` option for provider is healthy.** Good escape hatch; matches what we recommended in step prep. No friction here, just naming it as a thing the wizard gets right.

**Skills list shows macOS-only skills on Linux installs.** The skill registry includes apple-notes, apple-reminders, bear-notes, imsg, things-mac on a WSL Ubuntu install where none of those work. Confusing for the user (looks like options that just need a click); wasted scroll time. **Suggestion:** filter the skill list by `process.platform` (Linux installs skip the Mac skills; macOS installs skip the iOS-only or Windows-only skills). The skill manifests probably already declare their platform support — wire that to the picker.

**Default node manager mismatches Edwin's own internal tooling.** The wizard's `Preferred node manager for skill installs` defaulted to npm, even though Edwin's own build runs on pnpm (we just ran `pnpm install`/`pnpm build` to get here) and pnpm is the active package manager on this machine. **Suggestion:** detect what the host install used and default to that, OR default to whichever Edwin uses for itself (pnpm) for consistency.

## 5.5 Field notes from the 2026-06-06 fresh install

Re-running the install end-to-end after the 2026-06-02 partial install was deleted:

**The WSL-native install is dramatically faster than the report claimed it would be — `/mnt/c` is even worse than estimated.**
- Total install (`git clone` + `pnpm install` + `pnpm build` + `npm install -g`) finished in **under 60 seconds wall-clock.** Specifically:
  - `git clone` (186 MB): ~10 sec
  - `pnpm install` (993 packages): **3.6 sec** (lockfile cached, all from store)
  - `pnpm build` (4 entry points across protected cores + gateway): ~25 sec total across all build phases
  - `npm install -g .`: 546 ms
- The 6-02 setup report estimated `/mnt/c` install at 5+ minutes for gateway boot alone. Going ext4-native, the entire **install + build** is well under one minute. That's a ~10× delta with `/mnt/c` and an *order-of-magnitude* argument for the "install in WSL ext4" recommendation when explaining the trade-off to users.

**The "Ignored build scripts: koffi, node-llama-cpp" warning is alarming-looking but correct for the OpenAI-embedding path.**
- pnpm prints a yellow Warning box saying `Run "pnpm approve-builds" to pick which dependencies should be allowed to run scripts.`
- A fresh user reading this will assume something is broken or that they need to run `pnpm approve-builds`. They don't — and in fact running it for `node-llama-cpp` is the path to CUDA-OOM-crash territory the prior session documented.
- **Suggestion:** detect the embedding choice during `edwinpai setup` (Mode: OpenAI / Gemini / local) and suppress the warning when remote embeddings are picked. Or rewrite the warning to say "If you plan to use local embeddings, run `pnpm approve-builds`; remote-embedding users can ignore."

**`signed-envelope-*.js` is shipped in both the gateway and the plugin SDK build outputs.**
- Confirms the SecureVault envelope primitive (the cryptographic gate from the security model) is in-tree and ready for the Hodos integration. Worth confirming with Jake whether the gateway-side `signed-envelope` module is the stable API surface to target, or whether the plugin-SDK version is the contract.

---

## 6. What worked well

(Capturing the good as we go — easy to focus only on friction in writeups.)

- The `install.sh` philosophy (clone, build, configure in one shot) is solid; it just needs the idempotency + migration polish above.
- Splitting protected cores (`identity-core`, `shad-core`, `gateway-core`) is the right security move; the build separation makes the trust boundary explicit.
- `minimumReleaseAge` in pnpm config is a thoughtful supply-chain protection.
- The qmd OpenAI fork (`feat/openai-embeddings`) was a one-line repoint once we found it — and the embeddings were instantly fast at 1536-dim. Whoever did that work nailed it.
- The Tauri desktop ↔ WS gateway architecture is exactly the right shape — clean separation, network-only IPC, no shared filesystem assumptions.

---

## Appendix: where these notes came from

- `EDWIN_SETUP_REPORT.md` (2026-06-02 prior Claude session) — the source of items 1.1–1.4, the stack inventory, and the "WSL ext4 is dramatically better than `/mnt/c`" observation.
- `WINDOWS_SETUP_GUIDE.md` (same session) — the source of the runbook structure we'd like to make idempotent.
- The 2026-06-03 setup-from-scratch attempt that this doc is being written alongside.
