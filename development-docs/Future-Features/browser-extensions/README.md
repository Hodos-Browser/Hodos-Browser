# Browser Extensions Research

> ⚠️ **UNTRUSTED — VERIFY EVERY CLAIM AGAINST PRIMARY SOURCE.** These docs predate the 0.4.0
> planning session and have not been fact-checked. Treat all assertions (APIs, CEF capabilities,
> security models, implementation steps) as unverified leads, not ground truth. This is B4
> background only; the authoritative item doc is `../B4-extensions.md`. Relocated here 2026-06-01.

Research, planning, and security documentation for browser extension support in Hodos.

## Documents

| File | Purpose |
|------|---------|
| [BROWSER_PLUGINS_DEEP_DIVE.md](./BROWSER_PLUGINS_DEEP_DIVE.md) | Comprehensive background: history, architecture, landscape |
| [IMPLEMENTATION_OUTLINE.md](./IMPLEMENTATION_OUTLINE.md) | High-level implementation reference for sprint planning |
| [EXTENSION_SECURITY.md](./EXTENSION_SECURITY.md) | Security risks & best practices (marketing + development) |
| [CRYPTO_WALLET_EXTENSIONS.md](./CRYPTO_WALLET_EXTENSIONS.md) | Competitor wallet analysis & user transition strategy |

## Key Decisions

### Why Native First?

Hodos builds critical features natively rather than relying on extensions:

1. **Native BSV Wallet** — No extension attack surface, full security control
2. **Native Ad Blocking** — Better performance, no extension overhead
3. **Extension support (future)** — Carefully isolated from wallet context

### Security Position

> "51% of browser extensions are high-risk" — CrowdStrike

Extensions are the #1 attack vector for crypto users. By building natively, Hodos eliminates this risk for core functionality.

---

*Created: 2026-03-04*
