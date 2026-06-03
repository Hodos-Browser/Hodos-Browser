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
