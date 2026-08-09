# TICKET — Shipped fingerprint farbling runs on a per-URL CONSTANT

| | |
|---|---|
| **Opened** | 2026-08-07 |
| **Severity** | **High (privacy)** — the mitigation is inverted into a tracking vector |
| **Status** | OPEN — awaiting owner decision on whether to fix ahead of the P4 Blink migration |
| **Affects** | **Every released build.** Confirmed present in `v0.3.0-beta.1` through `v0.3.0-beta.29` (the current public Latest), on both Windows and macOS. Introduced in `0b7288b` ("browser-core sprints 10-11-12"), i.e. it has never worked. |
| **Component** | `cef-native/src/handlers/simple_render_process_handler.cpp` (render process), fed by `simple_handler.cpp :: OnBeforeBrowse` (browser process) |
| **Related** | `development-docs/0.4.0/chromium-rebuild/PLAN_farbling_blink.md` §6 C2 — same root cause as the unreleased C2 farbling-key bug |

---

## 1. Summary

Hodos ships Brave-style fingerprint farbling: per-session, per-domain randomised perturbation of
Canvas / WebGL / AudioContext / Navigator readbacks. The browser process computes a correct
per-domain seed from a CSPRNG session token on every launch.

**The renderer never receives it.** It silently falls back to a seed derived from the URL alone:

```cpp
// simple_render_process_handler.cpp :: SimpleRenderProcessHandler::OnContextCreated
} else {
    // Fallback: use URL hash as seed
    seed = static_cast<uint32_t>(std::hash<std::string>{}(url) & 0xFFFFFFFF);
}
```

`std::hash<std::string>` is deterministic within a toolchain and is not salted per process in
libstdc++/MSVC. So the seed — and therefore every farbled value — is **a pure function of the URL**:
identical across launches, across profiles, and **across users**.

## 2. The measurement (this is confirmed, not inferred)

A temporary browser-side log of the computed seed, two browser restarts, same URL:

| | browser-computed seed | farbled audio output |
|---|---|---|
| Run A | `2030444654` | `a10d2ba4` |
| Run B | `3258985367` | `a10d2ba4` |

The browser computes a fresh, correctly session-derived seed each launch — **that half works**. The
farbled output is byte-identical anyway, which is only possible if the renderer is not using it.

**Controls that make this sound rather than merely "stable":**

- the auth-exempt (never-farbled) `github.com` audio hash was **also** identical across restarts
  (`84551a93`), proving the measurement itself is deterministic across processes — so the stability
  of the farbled value carries information;
- the farbled value was stable across repeated away-and-back navigations, so it is **not** an
  artifact of the one-shot `s_domainSeeds.erase` for main frames.

**Reproduction needs no build** (method is reusable for the fix's acceptance test): render an
`OfflineAudioContext` tone through a `DynamicsCompressor` and hash the `getChannelData` output;
compare across two cold starts, and against a second machine/profile. WebGL `readPixels` works
equally well. ⚠️ **Canvas will not work** — its JS fragment was deleted in C3.

## 3. Why delivery fails

The browser sends the seed from `SimpleHandler::OnBeforeBrowse` — i.e. **pre-commit**, before the
document that needs it exists and before the renderer process that will host it is necessarily even
chosen. The renderer caches into a process-local map (`s_domainSeeds`, URL-keyed) and reads it at
`OnContextCreated`. For a cross-process navigation the message lands in the *outgoing* renderer
process, so the incoming one finds an empty map and takes the fallback.

An earlier hypothesis — that the legacy path survived *because* `s_domainSeeds` is process-wide and
URL-keyed — was **disproven**: the map is never populated in the right process.

This is the **same root cause** as the unreleased C2 native farbling-key bug (`PLAN_farbling_blink.md`
§6 C2): a pre-commit push cannot reach the document it is for. It is therefore not two bugs but one
design error with two victims.

## 4. Impact

The failure is not "farbling is weaker than intended". It is **inverted**:

1. **Zero cross-user unlinkability.** Every Hodos user perturbs a given URL identically. The entire
   purpose of farbling — making two users of the same browser look different — does not happen.
2. **Actively worse than shipping nothing.** The perturbation is a stable, precomputable constant
   applied on top of the native values. A site that fingerprints Canvas/WebGL/Audio sees a value that
   is *not* the stock-Chromium value and *is* the same for all Hodos users ⇒ it is a reliable
   **"this is Hodos Browser" discriminator**, and a high-entropy one, since an attacker can
   precompute the constant offline for any URL. Stock Chromium would have been less identifying.
   This is precisely the failure mode `HodosSessionCache`'s own header warns against: *"a degenerate
   constant-seeded perturbation is a WORSE fingerprint than not farbling at all."*
3. **No re-identification resistance.** Because the seed has no session component, clearing state or
   restarting does not change the fingerprint.
4. **The privacy claim is unmet.** Farbling is user-facing (the Privacy Shield toggle) and is part of
   how the product is described. Today that control does not deliver what it says.

**Not affected:** the auth-domain exemption still works correctly (it is a true native pass-through,
and it is the control that proved the measurement). No wallet, key, or signing material is involved —
Invariants #1/#2/#3 are untouched. This is browsing-privacy state only.

## 5. Recommendation

> **Yes — fix ahead of the full P4 migration, but with the small fix, not the real one.**

The two candidate fixes are not interchangeable, and the important point is that **the real fix cannot
reach shipped users**:

| | Fix A — fail closed | Fix B — renderer-side pull |
|---|---|---|
| Change | when no seed arrives, **do not farble** (skip injection) instead of hashing the URL | browser caches `{domain → key}`; renderer pulls it over a `[Sync]` mojo call at `OnContextCreated` |
| Lands in | `simple_render_process_handler.cpp`, ~5 lines, shell-only | `libcef` + `cef.mojom` — **requires the CEF 150 fork**, i.e. 0.4.0 |
| Backportable to the release line? | **Yes** — M136, no CEF changes | **No.** Released builds are M136; the fork work is CEF 150 / Chromium 150 |
| Restores farbling? | No — removes the tracking vector, leaves users unfarbled | Yes |
| Risk | Very low. Strictly removes behaviour; the auth-exempt path already exercises "no farbling" on every auth site every day | Build + verification cycle; sync-IPC-per-document cost |

Because the shipped browser has **never** had working farbling, Fix A takes nothing real away from
users — it only stops advertising a constant. It converts an active tracking vector into a plain
absence of protection, which is what the codebase's own fail-closed contract already prescribes.

> ### ⚠️ Fix B does not make Fix A unnecessary — 0.4.0 needs BOTH
>
> It is tempting to read "the pull fixes both bugs at once" and skip Fix A. That is wrong, and the
> distinction matters for sequencing:
>
> - Fix B repairs delivery for the **native Blink** path, which after C3 covers **canvas only**.
> - The **legacy JS** path is untouched by Fix B. It is shell code
>   (`simple_render_process_handler.cpp` + `s_domainSeeds`), not libcef, and it still owns
>   **audio, WebGL and navigator** until C4/C5/C6 port them natively.
>
> So a 0.4.0 build with Fix B landed still farbles audio/WebGL/navigator **on the URL-hash constant**.
> The vector is narrowed, not removed. Fix A is what actually removes it, and it is needed on the
> 0.4.0 line as well as the release line — until the JS path is fully retired in the C-teardown.
>
> The two fixes are therefore complementary and neither is a substitute for the other: **B restores
> real farbling where it is implemented natively; A removes the tracking constant everywhere it is
> not.**

**Proposed sequencing:**

1. **Now, on the release line: Fix A.** Make the missing-seed case skip injection entirely. Ship in
   the next patch build. Note that this makes the Privacy Shield farbling toggle a no-op in the
   failure case, so the UI copy should not claim active protection until Fix B lands.
2. **In 0.4.0: Fix B** (the renderer-side pull) restores real farbling natively, and retires the JS
   path per TD-3 / C-teardown.

**A cheap partial alternative to A**, if the owner would rather not ship "no farbling": keep
injecting, but derive the fallback seed from a per-renderer-process CSPRNG value instead of the URL
hash. That restores cross-user unlinkability immediately (each install/process differs) at the cost of
intra-session instability across process swaps — values would change when a navigation crosses a
process boundary, which some fingerprint-assisted logins read as a new device. **Not recommended**:
it trades a privacy bug for the login-breakage bug this whole migration exists to fix.

## 6. Decision requested

- [ ] Ship **Fix A** on the release line ahead of 0.4.0? *(recommended)*
- [ ] If yes: as its own patch build, or folded into the next scheduled beta?
- [ ] Adjust user-facing Privacy Shield copy while farbling is inert?

## 7. Side finding worth its own fix — renderer logging is dead ✅ **FIXED 2026-08-09**

`Logger::Initialize` is only ever called in the browser process (`cef_browser_shell.cpp`,
`cef_browser_shell_mac.mm`). **Every `LOG_*_RENDER` call in the codebase was therefore a silent
no-op**, and `[RENDER]` had never once appeared in `debug_output.log` (measured: 0 occurrences).
That is why a total farbling failure went unnoticed for the entire life of the feature: the one
subsystem that would have reported it could not write.

**Fixed 2026-08-09.** A child process cannot simply call `Logger::Initialize` — renderers run
**sandboxed at UNTRUSTED integrity** and have no write access to `%APPDATA%`, and
`Logger::Initialize` swallows the failed open, so that "fix" would look right and stay broken.
Instead `Logger` gained an injected sink (`Logger::SetSink`), and every child process installs one
that forwards the formatted line to **Chromium's logging**, which is already brokered across the
sandbox and lands in `cef_debug.log` via `settings.log_file`.

- Sink: `cef-native/src/core/ChildProcessLogSink.cpp` (`hodos::InstallChildProcessLogSink`).
- Installed in `RunWinMain` when `--type=` is on the command line (Windows) and at the top of
  `mac/process_helper_mac.mm :: main` (macOS).
- `Logger.cpp` stays **CEF-free** — it is compiled into the CEF-less unit-test target — which is
  why the sink is a function pointer rather than an `#include`.
- Verbosity: INFO/WARNING/ERROR always; the ~90 `LOG_DEBUG_RENDER` sites (documented as
  "every IPC message" noise) only with `--hodos-render-verbose`, which
  `OnBeforeChildProcessLaunch` appends for dev builds. **A switch, not an env var** — a sandboxed
  child does not reliably inherit the environment.

**Verified: 917 `[RENDER]` lines in `cef_debug.log`, up from 0. Negative control: with the sink
install disabled and rebuilt, `[RENDER]` count is 0 while the browser process still writes 238
lines to the same file** — so the log is live and it is specifically the renderer that goes silent.

> Worth noting what the fix immediately revealed: the `🛡️ Injecting fingerprint protection` line
> is **absent** on a farbled page, because the JS path is now fail-closed and injects nothing.
> That is correct behaviour — and before this fix it was indistinguishable from the logging simply
> not working, which is the whole reason §1's bug survived.

## 8. Acceptance test for whichever fix ships

The audio-hash method in §2 is the gate, because it needs no instrumented build:

- **Fix A:** farbled-page audio hash must become **equal** to the auth-exempt page's (no perturbation
  at all), on the URLs where the seed does not arrive.
- **Fix B:** farbled-page hash must **differ** from the auth-exempt page's, must be **stable**
  across restarts of the same profile, and must **differ** across two profiles / two machines. The
  third assertion is the one this bug would have failed and the only one that proves per-user
  unlinkability — it must be run on two distinct profiles, not two tabs.

  > ### ✅ The cheap way to test the third assertion — seed rotation (used to verify Fix B on 2026-08-07)
  >
  > No second machine and no second profile needed. Edit `profileSeed` in
  > `<profile>/fingerprint_settings.json`, restart, re-measure, then restore:
  >
  > ```
  > seed A -> farbled getImageData 0e4e6251 | exempt 53225ec8 | large-canvas control 0cdc9b48
  > seed B -> farbled getImageData d9532c84 | exempt 53225ec8 | large-canvas control 0cdc9b48
  > seed A -> farbled getImageData 0e4e6251   (exact round-trip)
  > ```
  >
  > **What makes it conclusive:** the exempt value and the large-canvas control are *unchanged* across
  > all three runs, so the farbled delta cannot be render variance. And the exact round-trip on seed A
  > proves determinism at the same time — one experiment, both §11 contracts.
  >
  > ⚠️ **A same-profile run proves nothing.** `farbling_probe.py` on one profile would have gone green
  > against this very bug. Rotate the seed or you have not tested it.
