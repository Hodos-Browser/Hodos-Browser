# TICKET — the remote debugging port is open in release builds; decision D2 was approved and never implemented

**Filed:** 2026-08-17 (found while sourcing the beta.2 farbling rotation token)
**Severity:** security — an unauthenticated local control channel over the browser, in shipped builds
**Status:** 🚧 beta.3 Phase 9 — **D2 + D3 LANDED 2026-09-14** (Windows shell + shared `simple_app.cpp`; dev halves measured `P9-A1`/`P9-A2`); **D4 LANDED 2026-09-14** (`P9-A3`, posted-F12 instrument, RED seen on a gate-less build); release-shaped proof = `INSTALL_TEST_BATCH.md` **I8** (owed); macOS `.mm` mirror = relay item. Contract: `phase-9-release-readiness/PHASE_CONTRACT.md`
**Sprint:** 📌 Phase 9 (release readiness) — bundled 2026-08-31.
**Design already exists:** `development-docs/0.4.0/DEVTOOLS_SECURITY_DESIGN.md` (2026-08-04)

---

## Statement

`DEVTOOLS_SECURITY_DESIGN.md` is marked **"owner-approved in principle (2026-08-04), not
implemented"** and carries **D2 — Close the remote debugging port in release**. Verified against the
tree at `0da5262` (the beta.2 tag): **still not implemented.** Release builds bind CDP on `9222`.

`cef_browser_shell.cpp`, the only gate is picker mode:

```cpp
if (g_picker_mode) {
    settings.remote_debugging_port = 0;
} else if (profileId == "Default") {
    settings.remote_debugging_port = 9222;
} else {
    settings.remote_debugging_port = 9222 + portOffset;
}
if (hodos::IsDevEnv() && settings.remote_debugging_port != 0) {
    settings.remote_debugging_port += 100;   // dev OFFSETS, it does not gate
}
```

`IsDevEnv()` only *offsets* dev to 9322 so the two builds don't collide. There is no dev/release
gate, no `#ifdef`, no build-type condition. A shipped Hodos on the Default profile listens on
`127.0.0.1:9222`.

## Why this matters more for us than for a generic browser

Anything that can reach that port can drive the browser: read any page's DOM, execute script in any
frame, navigate, and enumerate targets. In Hodos, the target list includes **the wallet overlay and
the BRC-100 auth overlay**, which are separate CEF browsers. The overlay isolation this project
treats as a security boundary is a process boundary, not a CDP boundary — CDP sees them all as
`type: "page"` (the same fact that killed three farbling harnesses).

The design doc already reasons this through and reaches the same place; D2 exists because someone
did that analysis. It simply never got built.

⚠️ **Do not overstate it either.** This is a **loopback** port, so it needs local code execution or
a local process to abuse — it is not remotely reachable. That is why this is a real item and not an
incident.

## Ties into the farbling gate

`TICKET_farbling_gate_engine_binding.md` §"second gap" notes that the rotation harness measures a
**local dev build**, not the promoted installer, and that running it against the installed artifact
is *technically possible today precisely because this port is open in release*.

⛔ **Do not let that become an argument for keeping it open.** If we ever want to measure the shipped
artifact, the answer is a deliberate, gated diagnostic switch — not leaving an unauthenticated
control channel open for every user so that our test harness has somewhere to attach. Decide D2 on
its own merits; if D2 closes the port, the harness question gets solved separately.

## What the design doc already specifies

Read `DEVTOOLS_SECURITY_DESIGN.md` in full before implementing. Its four decisions:

- **D2** — close the port in release (this ticket)
- keep **DevTools itself** available — DevTools and the debug port are independent, and the doc is
  explicit that closing one must not remove the other
- drop `--remote-allow-origins=*`
- a **role-only gate** so overlays don't offer Inspect Element

It also carries an acceptance table (release build: nothing listening on 9222; F12 and right-click
Inspect still work on web pages; the wallet overlay offers no Inspect and refuses DevTools if
invoked).

## Consequences to handle

- **Q5 in the design doc** — the `9222 + N` per-profile scheme exists so multiple profiles don't
  collide. Closing the port in release makes that scheme dev-only; check nothing else depends on it.
- **Every harness that drives CDP** (`farbling_*_check.py`, `farbling_acceptance_battery.py`, the
  widget/regression checks) must keep working against **dev** builds. They already default to the
  dev port, so this should be a non-event — verify rather than assume.
- ⚠️ **Per-profile CDP ports differ on macOS** (recorded in the P5 round). Whoever implements this
  should confirm the macOS binding, not extrapolate from Windows.

## ⛔ Negative control

"Nothing listening on 9222" must be **measured on a release build**, not inferred from the diff:

```powershell
Get-NetTCPConnection -LocalPort 9222 -ErrorAction SilentlyContinue   # expect: nothing
```

and the same probe on a **dev** build must still find 9322, proving the check can detect a bound
port at all. A "nothing listening" result from a browser that failed to start is not a pass.

## Acceptance

- [ ] release build: nothing listening on 9222 (measured, with the dev-build positive control) — ⏳ **I8** (install batch, owed by Phase 9). Dev positive control 📏 2026-09-14: 9322 bound by the build-dir exe; picker mode on the same binary ⇒ port 0, nothing bound
- [ ] release build: F12 / Ctrl+Shift+I / menu / right-click Inspect all still open DevTools on web pages
- [x] release build: wallet overlay offers no Inspect Element and refuses DevTools if invoked — D4 landed (`P9-A3`): refusal measured on the dev build by posted F12 (`DevTools refused on role=wallet`), gate-less build opened it; the context-menu half is the owner's right-click (I8 also lists it)
- [x] dev build unchanged — every CDP harness still runs on 9322 — 📏 2026-09-14: `/json/version` answers, `websocket-client` attaches with its default `Origin` header (and with a foreign one), effective browser command line carries `--remote-allow-origins=*`; the rotation harness's own run is `P9-C1`
- [ ] macOS binding confirmed independently
- [x] `DEVTOOLS_SECURITY_DESIGN.md` status line moved off "not implemented" (2026-09-14)
