# A1 — CEF Build Strategy: Local vs Cloud (Research)

**Created:** 2026-06-01 · **Status:** ✅ Research complete; Mac confirmed **Apple M1** → all-local plan stands
**Method:** primary-sourced (Chromium build docs, CEF forum/issues, sccache docs, AWS/MacStadium pricing, GitHub Actions runner docs).

> Decision context: self-build is mandatory (codecs). The goal is to stop the build being a ~2-week
> ordeal. Hardware on hand: **Windows 32 GB RAM**, **Mac 16 GB RAM (mostly idle)**.

## TL;DR recommendation: **build locally, add caching. Skip cloud (mostly).**

- **Windows builds → local 32 GB box.** Add `sccache` + `concurrent_links=2` + `symbol_level=0`.
  First build 4–6 h; subsequent same-version rebuilds drop toward ~30 min–2 h with a warm cache.
- **macOS builds → the idle 16 GB Mac (confirmed Apple M1 ✅).** Use `concurrent_links=1` +
  `symbol_level=0`. Slow (6–10 h cold) but $0, and it can sit there. M1's unified memory + NVMe make
  16 GB workable; expect heavy-but-survivable swap during the link step.
- **Cloud is not worth the setup for our infrequent cadence**, with two narrow exceptions:
  - *Optional* Windows build on an AWS spot VM (~$5–8/build) only if we want to stop tying up the
    local machine for hours.
  - macOS cloud is **economically bad** for us (AWS EC2 Mac = 24 h minimum billing → $16–26 per
    session; MacStadium = $199–249/mo). Only makes sense if the local Mac is Intel and miserable.

## Key technical findings

| Finding | Detail | Confidence |
|---------|--------|-----------|
| 16 GB Mac viability | Works **with mitigations** if Apple Silicon (fast swap/NVMe); marginal/painful if Intel | HIGH |
| `is_component_build` can't help | It's **mutually exclusive** with `is_official_build=true` (which we need for codecs) — so no component-build RAM relief | HIGH |
| Linker is the RAM hog | LTO linking can OOM even on 32 GB if parallel link jobs aren't capped → set `concurrent_links` | HIGH |
| sccache speedup | ~3× on warm incremental rebuilds (Electron/Chromium report); local disk cache is enough for a single builder; integrate via `cc_wrapper` GN arg (CEF issue #2432) | MEDIUM–HIGH |
| reclient/RBE | Skip — enterprise/Google-internal; reclient "no longer supported for building Chromium" externally | HIGH |
| GitHub Actions | Can't do it: macOS runners cap at **14 GB disk** (need ~150 GB); 6 h job limit; Windows largest runner ~$165/build | HIGH |
| AWS spot Windows | c6i.4xlarge ≈ $1/hr → ~$5/5h build; macOS instances have **no spot** + 24 h min billing | MEDIUM (price) |

## Concrete config changes (apply on both machines)

```
# pass via automate-git.py --gn-extra-args, alongside existing GN_DEFINES
concurrent_links = 1        # Mac (16 GB);  = 2 on the 32 GB Windows box
symbol_level = 0            # also blink_symbol_level=0, v8_symbol_level=0
cc_wrapper = "sccache"      # + set SCCACHE_DIR and SCCACHE_CACHE_SIZE=200G
```
Windows sccache+MSVC has historically needed care (the `/Brepro` flag patch); verify against CEF
issue #2432 before relying on it. Free 150–200 GB disk per platform before building.

## The real A1 win is caching, not cloud
Our trigger is "bump Chromium version OR change patches." A version bump is a near-total cache miss
(slow no matter what). But the *iteration loop* — fixing build errors, tweaking farbling patches on
the same Chromium version — is where sccache turns a 5 h rebuild into minutes. That's most of the
day-to-day pain.

## ✅ Resolved: Mac is Apple M1
Confirmed 2026-06-01. The all-local plan stands: build macOS on the idle M1 (16 GB) with
`concurrent_links=1` + `symbol_level=0`; no hosted Mac needed. Watch peak RAM during linking the
first time — if it OOMs, the fallback is MacStadium M2 (~$199/mo), but M1 + serialized linking should
get through.

## Unknowns / to verify
- Windows sccache+MSVC reliability on Chromium 136 (the 3× figure is from a 2021 Electron report).
- Whether CEF's `automate-git.py` honors `cc_wrapper` natively today (inspect the script / issue #2432).
- Exact AWS Windows spot price + availability at build time.
- Cold-build time on 16 GB Apple Silicon with `concurrent_links=1` (extrapolated as 6–10 h).
