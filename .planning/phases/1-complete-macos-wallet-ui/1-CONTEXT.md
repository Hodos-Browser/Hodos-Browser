# Phase 1: Complete macOS Wallet UI - Context

**Gathered:** 2026-01-20
**Status:** Ready for planning

<vision>
## How This Should Work

A new wallet panel overlay (WalletPanel.tsx) already exists with the UI built specifically for macOS. This phase is about wiring that UI to the Rust wallet backend so users can actually use it.

When complete, users should be able to open the wallet panel and:
- View their BSV balance and addresses
- Generate new receive addresses
- Send BSV to other addresses
- Handle BRC-100 authentication requests from websites

The UI is already built - this phase is about connecting it to the existing Rust wallet API endpoints (the same backend that powers the Windows version). The wallet panel should work seamlessly as a macOS overlay, communicating with the Rust backend through the established HTTP interception pattern.

</vision>

<essential>
## What Must Be Nailed

- **Reliable backend wiring** - The connection between WalletPanel.tsx and the Rust wallet backend (localhost:3301) must be rock-solid. No dropped requests, proper error handling, secure communication. This is the foundation everything else depends on.
- **Core wallet operations functional** - View balance, generate addresses, send BSV, receive BSV, and respond to BRC-100 auth challenges must all work end-to-end
- **Follow existing patterns** - Use the established codebase patterns: `useHodosBrowser` hook, overlay communication model, HTTP interception for wallet API calls

</essential>

<boundaries>
## What's Out of Scope

- **Windows compatibility** - Phase 1 is macOS-only. Windows overlay migration happens in Phase 3
- **Advanced wallet features** - Custom UTXO selection, advanced fee settings, multi-sig support - keep it simple for now
- **DevTools integration** - CEF DevTools access is Phase 2, not needed for wallet functionality
- **New Rust endpoints** - Use existing wallet API endpoints; don't add new backend functionality beyond what's already there

</boundaries>

<specifics>
## Specific Ideas

Follow the patterns already established in the codebase:
- Use `useHodosBrowser()` hook for wallet operations (like Windows version does)
- Use the HTTP interception pattern in `HttpRequestInterceptor.cpp` to route wallet API calls
- WalletPanel.tsx should communicate with backend the same way other overlays do
- Reference existing implementations (if any on Windows) for how wallet operations are wired

No need to reinvent patterns - just connect the new macOS UI to the existing backend infrastructure.

</specifics>

<notes>
## Additional Context

The UI work is done. This phase is purely about integration:
1. WalletPanel.tsx (React) needs to call wallet operations
2. Those calls go through the CEF layer via window.hodosBrowser
3. CEF intercepts and forwards to Rust backend (localhost:3301)
4. Responses flow back through the same chain

The Rust wallet backend is already tested and working (proven by Windows version). This is about making the macOS overlay communicate with it properly.

</notes>

---

*Phase: 1-complete-macos-wallet-ui*
*Context gathered: 2026-01-20*
