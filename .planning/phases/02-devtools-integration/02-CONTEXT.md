# Phase 2: DevTools Integration - Context

**Gathered:** 2026-01-20
**Status:** Ready for research

<vision>
## How This Should Work

DevTools access should work exactly like Chrome - familiar and intuitive for developers. Press F12 (or Cmd+Option+I on Mac) and DevTools open in a separate window. Right-click anywhere and see "Inspect" in the context menu.

This works for every CEF window in HodosBrowser: the main browser window, the wallet overlay, settings, auth modals - all of them inspectable. When you're debugging the wallet panel or a BRC-100 auth flow, you should be able to inspect it just as easily as the main browser content.

DevTools appear as a separate detached window, not docked inside the parent window. This makes it easier to move to a second monitor and avoids layout complexity with the overlay system.

</vision>

<essential>
## What Must Be Nailed

All three of these are equally critical:

- **Cross-platform consistency** - Same shortcuts, same behavior on both macOS and Windows. A developer switching between platforms shouldn't notice any difference.
- **Full DevTools feature set** - All Chrome DevTools panels work (Elements, Console, Network, Sources, Performance, etc.). Not a stripped-down version.
- **Easy discoverability** - Developers can easily find and activate DevTools without reading documentation. Standard shortcuts (F12, Cmd+Option+I) and right-click Inspect menu.

</essential>

<boundaries>
## What's Out of Scope

- **Custom DevTools themes/styling** - Use default Chrome DevTools appearance. No custom branding or theming.
- **DevTools extensions/plugins** - No support for React DevTools, Redux DevTools, or other extensions. Just core CEF DevTools.
- **Remote debugging protocol** - No remote debugging setup. DevTools only for local development on the same machine.

</boundaries>

<specifics>
## Specific Ideas

No specific requirements - open to standard approaches. Just make it work like Chrome with the standard developer experience.

</specifics>

<notes>
## Additional Context

Phase depends on Phase 1 completion (macOS wallet UI). Once DevTools are working, it will be easier to debug issues during Phase 3 (Windows overlay migration) and Phase 4 (cross-platform testing).

Roadmap notes this will likely need research into CEF DevTools configuration, keyboard shortcut registration across platforms, and DevTools window management in CEF.

</notes>

---

*Phase: 02-devtools-integration*
*Context gathered: 2026-01-20*
