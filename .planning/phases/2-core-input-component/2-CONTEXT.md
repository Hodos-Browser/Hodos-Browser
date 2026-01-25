# Phase 2: Core Input Component - Context

**Gathered:** 2026-01-24
**Status:** Ready for planning

<vision>
## How This Should Work

Chrome-style instant dropdown experience - the moment someone types a character, the dropdown appears with suggestions. This is about nailing the foundational feel and UI structure before we add real data sources.

At this stage, the dropdown shows mock/placeholder suggestions just to make the interaction feel real. The focus is on building the component architecture and making it feel smooth and responsive - that instant, polished Chrome-like tactile experience when typing.

It should use Material-UI's native aesthetic rather than trying to clone Chrome's exact visual design. Let Material-UI's design language guide the look - modern, clean, professional - while the behavior mimics Chrome's instant-feedback interaction pattern.

</vision>

<essential>
## What Must Be Nailed

All three of these are essential for this phase:

- **Instant, smooth feel** - Input responsiveness, dropdown animation, zero lag. The tactile experience should feel polished and fast, just like Chrome's omnibox.
- **Clean component architecture** - Structure the React component so that adding history autocomplete (Phase 3) and Google search integration (Phase 4) will be straightforward extensions, not refactors.
- **Visual polish with Material-UI** - Professional styling from day one. Proper spacing, shadows, focus states that integrate seamlessly with the rest of the browser UI using Material-UI components.

</essential>

<boundaries>
## What's Out of Scope

Phase 2 is purely the UI foundation. Explicitly excluded:

- **No database/history integration** - No SQLite queries, no HistoryManager C++ calls. Real data comes in Phase 3.
- **No Google search API or URL detection** - No external APIs, no smart logic to distinguish URLs from search queries. That's Phase 4.
- **No full keyboard navigation** - Basic Enter/Escape is fine, but sophisticated keyboard shortcuts (Tab to autocomplete, arrow key navigation through suggestions) are deferred to Phase 5.
- **No cross-platform testing** - Build for the current development platform. Optimization and cross-platform verification happen in Phase 6.

</boundaries>

<specifics>
## Specific Ideas

- Use Material-UI components in their standard form - embrace MUI's design language rather than custom-styling everything to match Chrome pixel-for-pixel
- Mock suggestions should be realistic enough to test the interaction (e.g., "google.com", "github.com", "wikipedia.org") but clearly placeholders, not real data
- The component should be a drop-in replacement for the existing InputBase in MainBrowserView, maintaining the same integration points

</specifics>

<notes>
## Additional Context

This phase sets the foundation for all subsequent phases. The quality of the component architecture here directly impacts how easy Phases 3-5 will be to implement.

Priority order: Get the feel right first (responsiveness, animations), then ensure the architecture is extensible, then polish the visuals. All three matter, but smooth interaction is what users notice first.

</notes>

---

*Phase: 2-core-input-component*
*Context gathered: 2026-01-24*
