# B2 — Move Header from React → C++

**Status:** ⛔ Deferred to its **own dedicated multi-agent planning session** (post-A4)
**Type:** refactor (large) · **0.4.0:** candidate

## Summary
The header/toolbar loads slowly. Proposal: render it natively in C++ for speed while keeping the
**exact CSS / branding / look** we have today. Leaning: **header only** — keep the wallet, settings,
download panel, and popup modals as React overlays (NOT a full frontend → C++ migration).

## Why its own session
Per the locked principle — *do it the right way the first time, no re-refactor in two months* — B2
needs deep research before any design:
- **Measure first:** is the header slow because of React render, CEF subprocess spawn, or IPC warmup?
  C++ may not be the fix; confirm the bottleneck before committing.
- **Study prior art:** how Chrome / Firefox / Brave / Vivaldi / Safari render the toolbar and *why*
  (Chrome/Brave use native Views, not a web layer).
- **Preserve branding:** how to keep the exact look if leaving the web rendering layer (native draw
  vs. a dedicated lightweight web surface).

## Known facts (verified locations)
- Header is React today: `frontend/src/.../MainBrowserView.tsx` (header_hwnd). CLAUDE.md UI rules
  forbid adding panels here — overlays only. A header→C++ move is an architectural change to revisit.

## Dependencies
A4 (Brave feasibility) frames the options. Then spin up the dedicated session with several agents.

## To fill in the dedicated session
Bottleneck measurement · approach comparison (benefits/disadvantages each) · scope of work ·
branding-preservation strategy · Acceptance criteria · Reuse map · Risk table · Implementation order · Test plan.
