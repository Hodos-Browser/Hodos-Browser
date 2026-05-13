# Linux Build

**Status:** BACKLOG (per [marketing/intelligence/FEATURE_PRIORITY.md](../../../Marston%20Enterprises/Hodos/marketing/intelligence/FEATURE_PRIORITY.md))
**Effort:** XL (per [marketing/intelligence/EFFORT_MATRIX.md](../../../Marston%20Enterprises/Hodos/marketing/intelligence/EFFORT_MATRIX.md#linux-build))
**First logged:** 2026-05-11

## The ask

Native Linux build of Hodos Browser. 3 distinct early users have asked for it as of 2026-05-11 — see `USER_SIGNALS.md` entry of that date. Proportionally large for the early BSV-native demographic; small in absolute terms.

## Why this is not in NOW or NEXT QUARTER

Two real constraints:

1. **macOS port is the current second-platform investment.** Sprint folder `Final-MVP-Sprint/macos-port/` is the active work. Adding a third platform before the second stabilizes is asking for divergence between the two non-Windows ports.
2. **Real engineering surface is large.** CEF on Linux, GTK/Qt overlay layer, libsecret/GNOME-Keyring + KWallet replacement for the DPAPI (Windows) / macOS Keychain (stubbed) path in `rust-wallet/src/crypto/dpapi.rs`, adblock-engine cross-compile, frontend build target verification, and packaging (.deb / .rpm / AppImage / flatpak — at least one). See `EFFORT_MATRIX.md` for the dimension breakdown.

## Revisit trigger

Promote to RESEARCH (and then NEXT QUARTER) when **either**:

- macOS port reaches "stable on M1+M2, parity with Windows" — meaning the cross-platform discipline is proven, not just attempted.
- Linux demand grows past the early-niche slice. Suggested threshold: 10+ distinct asks across at least 2 demographic groups (not just BSV-native early users), OR a partner-level ask that ties to a specific deal.

## What we're NOT planning to do here

- No Rust-side Linux work yet — `rust-wallet` and `adblock-engine` are mostly platform-agnostic already; the deep work is in `cef-native/` (C++/CEF shell).
- No early "Linux command-line wallet" half-measure. Hodos's value is the integrated wallet+browser; shipping a CLI-only Linux build would dilute that.
- No flatpak/snap targeting until the build pipeline can produce reproducible binaries on a baseline distro.

## Related

- `marketing/intelligence/USER_SIGNALS.md` — the demand log entry (2026-05-11).
- `marketing/intelligence/EFFORT_MATRIX.md#linux-build` — full effort scoring.
- `Final-MVP-Sprint/macos-port/` — the second-platform precedent that gates this work.
- Root `CLAUDE.md` invariant #9 — "All new C++ code must use `#ifdef _WIN32` / `#elif defined(__APPLE__)` platform conditionals" — when Linux work begins, this pattern extends to `#elif defined(__linux__)`.
