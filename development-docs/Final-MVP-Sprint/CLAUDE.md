# Final MVP Sprint — AI Assistant Context

**Read the root `CLAUDE.md` first.** This file provides additional context specific to the current sprint.

---

## Sprint Overview

This is the final push to MVP. Two parallel priorities:

| Priority | Owner | Goal |
|----------|-------|------|
| **P1: macOS Feature Parity** | Dedicated dev | Get the C++ CEF shell working on macOS with the same features as Windows |
| **P2: Hardening & Stability** | All devs | Testing, bug fixes, optimization — make the product solid |

## Folder Contents

| File | Purpose |
|------|---------|
| `CLAUDE.md` | This file — sprint context for AI assistants |
| `AI-HANDOFF.md` | Shared log for AI assistants to communicate across sessions and devs |
| `TESTING_GUIDE.md` | Human-friendly exploration missions for manual testing |
| `OPTIMIZATION_PRIORITIES.md` | What to optimize, when, and why (before vs after testing) |
| `SECURITY_MINDSET.md` | Security philosophy, current posture, watch list for all devs |
| `macos-port/` | macOS porting handover and archived docs |

## AI Assistant Rules

1. **Update AI-HANDOFF.md** at the end of every session with: what was done, what's blocked, what's next.
2. **Read AI-HANDOFF.md** at the start of every session to see what other devs/AIs have done.
3. **Follow the root CLAUDE.md invariants** — especially: private keys never in JS, read before edit, build after changes.
4. **Security mindset** — everyone watches for security issues. See `SECURITY_MINDSET.md` for the watch list.
5. **Test after building** — the testing guide exists. Use it. At minimum, verify your changes don't break youtube.com, x.com, and github.com.

## macOS Dev: Start Here

1. Read `macos-port/MACOS-PORT-HANDOVER.md` — comprehensive gap analysis, architecture diffs, sprint plan
2. The Rust wallet and adblock engine are already mac-ready. **Do not modify them** unless you find a bug.
3. All your work is in `cef-native/` (C++ CEF shell)
4. Key pattern: `#ifdef _WIN32` / `#elif defined(__APPLE__)` for platform conditionals
5. Build instructions: `build-instructions/MACOS_BUILD_INSTRUCTIONS.md`

## Hardening Dev: Start Here

1. Read `TESTING_GUIDE.md` — start with Tier 1 missions
2. Read `OPTIMIZATION_PRIORITIES.md` — items marked "Before Testing" should be done first
3. Read `SECURITY_MINDSET.md` — keep the watch list in mind while testing
4. File bugs in `AI-HANDOFF.md` with clear reproduction steps

## Key Build Commands

```bash
# Rust wallet
cd rust-wallet && cargo build --release

# Adblock engine
cd adblock-engine && cargo build --release

# Frontend
cd frontend && npm run build    # or: npm run dev (for dev server)

# C++ CEF shell (Windows)
cd cef-native && cmake --build build --config Release

# C++ CEF shell (macOS)
cd cef-native && cmake --build build --config Release
```

## Key Ports

| Service | Port | Auto-launched by C++? |
|---------|------|-----------------------|
| Frontend (Vite) | 5137 | No — run `npm run dev` manually |
| Rust Wallet | 3301 | Yes (Windows) / No (macOS, not yet ported) |
| Adblock Engine | 3302 | Yes (Windows) / No (macOS, not yet ported) |
