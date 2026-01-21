# Phase 3: Windows Overlay Migration - Context

**Gathered:** 2026-01-20
**Status:** Ready for planning

<vision>
## How This Should Work

Direct port of the Mac overlay system to Windows, with platform-specific adaptations where needed. The approach is to take the working macOS overlay (React components, CEF integration) and adapt it to Windows' windowing model without changing the core logic.

The key structure: shared React UI layer (frontend overlay components remain identical across platforms) with platform-specific CEF layer (C++ windowing, lifecycle, and integration code handles Windows-specific behaviors).

The migration brings the new unified overlay system from Mac to Windows, replacing Windows' old overlay implementation. This gives Windows users the same modern wallet UI, advanced features, and DevTools access that Mac users now have.

</vision>

<essential>
## What Must Be Nailed

- **Feature parity** - Windows must get everything Mac has: wallet UI, advanced features, DevTools access, all functionality working identically
- Windows users should have the exact same capabilities as Mac users after this phase completes

</essential>

<boundaries>
## What's Out of Scope

- Windows-specific features beyond parity - not adding features Mac doesn't have
- Refactoring the Mac side - Mac overlay already works, don't touch it
- UI redesign or new features - this is migration, not redesign
- This is a pure migration phase: bring Mac's overlay to Windows, nothing more

</boundaries>

<specifics>
## Specific Ideas

- Use Windows conventions where appropriate - match Mac's functionality but adapt things like window chrome, shadows, or keyboard shortcuts to feel native on Windows
- The overlay should feel like a Windows application, not just Mac code running on Windows
- Platform-specific adaptations should be in the CEF layer (C++) while keeping React components truly shared

</specifics>

<notes>
## Additional Context

This phase completes the cross-platform overlay unification started in Phase 1-2. After this, both platforms will be using the same modern overlay system with only platform-specific windowing code differing.

The goal is functional parity first, native feel second. Windows must be able to do everything Mac can do, and it should feel appropriate for the platform.

</notes>

---

*Phase: 03-windows-overlay-migration*
*Context gathered: 2026-01-20*
