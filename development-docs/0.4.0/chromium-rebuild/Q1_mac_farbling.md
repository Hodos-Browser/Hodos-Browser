# Q1 — Mac Farbling: does the Blink farbling patch set need Mac-specific edits, or is it one cross-platform patch set built per-OS?

**Created:** 2026-07-10 · **Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude (authors patches) · **Executor:** Mac Claude (owns the glue/build/verify listed here)
**Status:** DETAILED PLAN — Workflow-2 expansion of `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §6-Q1 + §5 ownership table. Research + design only — **NO code, NO builds.**
**Answers:** Do the farbling edits have to be re-authored for macOS, or is the Blink C++ patch set **one shared cross-platform set compiled per-OS**? What is the exact platform glue (seed storage, seed→renderer plumbing, Mac-specific Blink readback/WebGL paths), and precisely what does **Mac Claude own** vs **inherit**?

**Cross-refs (source-of-truth):** outline §3c (C1–C7 patch set + M1 teardown + C2 seed model + P4e OOP workers), §5 (Win/Mac ownership table + I8 arch), §7 (farbling acceptance). `B1-farbling-design.md` (persistent per-profile seed, Supplement architecture). `Q5_full_edit_list.md` §A.3 (C1–C7 rows) + §B-VALUES (Mac GPU-string map is Mac-owned). `Q2_farbling_adblock.md` §3 (teardown hygiene TP-1/TP-2 — same deletions apply per-OS). **`PLAN_farbling_blink.md` (standalone C1–C7 patch plan) is planned, not yet written** — where this doc says a value/channel "resolves in the farbling plan," treat it as a proposed dependency, not a settled decision.

---

## TL;DR — VERDICT

**ONE shared cross-platform Blink patch set. Mac does NOT author separate farbling logic.** Blink (`third_party/blink/renderer/...`) is platform-agnostic C++; the `.patch` files are literally the same text applied to the same Chromium source tree before each OS's compile. Windows Claude authors C1–C7 once; **Mac inherits the identical patches and compiles them into `Chromium Embedded Framework.framework`.**

**But the build and the *glue* are a first-class parallel Mac effort, not an inherit-and-verify afterthought (outline I8).** Mac Claude owns four concrete glue/verify areas where the shared patch touches OS-specific plumbing:

1. **Seed→renderer delivery plumbing** — the top Mac verification item (not a design fork). The delivery *callback* is already cross-platform: `SimpleApp::OnBeforeChildProcessLaunch` (a `CefBrowserProcessHandler` override) appends `--profile=` to every child on **both** OSes; only the *reader* differs (`GetCommandLineW()` on Windows vs `argv` in the mac helper). The C2 seed channel rides this same callback, so the Mac task is a smoke check ("the OBCPL-appended switch reaches the mac helper" — already proven for `--profile=`), not a separate mechanism.
2. **Per-profile seed storage** — `ProfileManager`/`SettingsManager` are already cross-platform; Mac verifies the storage path resolves and the seed persists under `~/Library/Application Support/HodosBrowser/<Profile>/`.
3. **Mac-specific Blink value entries** — the shared *patch* is identical, but the C4 WebGL "common real GPU strings" **data** needs Apple-Silicon + Intel-Mac ANGLE (Metal backend) entries that only exist on Mac.
4. **Mac build/arch/minos/plist + macOS farbling acceptance** — universal2 arch decision, minos guard, and the P6 acceptance run on real Mac hardware.

**Bottom line: the patch *set* is shared; the *build, the seed-delivery glue, the Mac GPU-string data, and the acceptance run* are Mac-owned. Q1 is GREEN. The C2 delivery channel is cross-platform by construction (the OBCPL append-callback fires on both OSes); the remaining Mac items are a delivery-*timing* design question (seed must be resident before the first farbled read — §2.2/I-1) plus a smoke check that the appended switch reaches the mac helper — MED, not a design fork.**

---

## 1. Why the patch set is shared (the core claim, substantiated)

Farbling lives in **Blink**, the rendering engine, in files like `third_party/blink/renderer/modules/canvas/canvas2d/base_rendering_context_2d.cc` and `modules/webgl/webgl_rendering_context_base.cc`. These files:

- Are compiled from the **same Chromium checkout** for every target OS (the per-OS difference is the *toolchain* — MSVC/clang-cl on Windows, Xcode/clang on macOS — not the source text).
- Expose the farbling-relevant readback as **platform-neutral Web-API surface** — `getImageData`/`toDataURL`/`getParameter`/`getChannelData` sit above the OS graphics backend and take the same patch text on both OSes. (The one path that sits closer to the per-OS ANGLE backend — WebGL framebuffer `readPixels` — is called out in §3.1/§3.2 and verified in the farbling plan; we do not claim zero `#ifdef` anywhere in the stack, only that the Web-API readback surface we patch is platform-neutral.)
- Are patched by CEF's `patch/patcher.py` **before compile**, from `patch/patches/*.patch` registered in `patch/patch.cfg` (outline §3b, CEF-1). The patcher applies the same text on both build hosts.

So C1 (HodosSessionCache Supplement), C3 (Canvas 2D), C4 (WebGL), C5 (WebAudio), C6 (Navigator), and the renderer-side seed-derivation half of C2 are **one authored artifact**. Mac's `Chromium Embedded Framework.framework` gets the farbling behavior for free once the shared patch compiles — DLLs cannot be reused on Mac (`CEF_BUILD_RUNBOOK.md` Step 3), but the *patch text* is fully reused.

**Corollary:** there is exactly **one** place the "one shared patch set" can legitimately split per-OS: a Blink file whose *value* is platform-dependent (the C4 GPU-string map — §5). Even there, the split is a small platform-gated data table inside the shared patch, not a separate Mac patch file (§5 decision).

---

## 2. The glue layer — where Windows and Mac genuinely diverge

Farbling is not *only* Blink. It has a browser-process C++ half (in our `cef-native/` shell) that generates/stores the per-profile seed and delivers it to the renderer. **That half is our own cross-platform-but-`#ifdef`'d code**, and it is where Mac work is real. Three glue concerns:

### 2.1 Per-profile seed storage (LOW Mac risk — already cross-platform)

`profile_seed` lives in **C++ profile data, not the wallet** (outline C2; `B1-farbling-design.md`). The storage substrate already exists and is cross-platform:

- `ProfileManager::GetProfileDataPath()` / `GetCurrentProfileDataPath()` (`ProfileManager.cpp:501/511`) resolve per-profile dirs on both OSes.
- `SettingsManager` already persists `PrivacySettings` (incl. the fingerprint toggle) to `<profile>/settings.json`, and **already migrates global→per-profile** (`SettingsManager.cpp`; core CLAUDE.md).
- Storage roots (verified, core CLAUDE.md): Windows `%APPDATA%/HodosBrowser/<Profile>/`, macOS `~/Library/Application Support/HodosBrowser/<Profile>/`.

**Mac owns:** confirm the seed read/write path resolves on macOS (it reuses the existing cross-platform resolver — expected trivial), and that the seed persists across restarts (the load-fix invariant). **No new Mac storage code expected** — this is a verify, not an author.

### 2.2 Seed → renderer delivery (MED Mac risk — same callback, per-OS reader + a timing question) ⚠️

The delivery *callback* is cross-platform; the reader differs; the real open question is **delivery timing vs first script**, not reachability. Three facts from the current tree:

- **The append side is one cross-platform callback.** `SimpleApp::OnBeforeChildProcessLaunch` (`simple_app.cpp:58`, `AppendSwitchWithValue`) is a `CefBrowserProcessHandler` override. OBCPL is dispatched from Chromium's `CefContentBrowserClient::AppendExtraCommandLineSwitches`, which is cross-platform — CEF's General Usage doc imposes **no platform restriction**, and the repo already relies on OBCPL firing on macOS to propagate `--profile=` for per-profile history isolation (a landed, smoke-tested feature; `MACOS_PORT_0_4_0.md`: *"the `OnBeforeChildProcessLaunch` override is cross-platform (CefBrowserProcessHandler) — it will run on macOS too once the mac render helper consumes `--profile`"*). A switch appended in OBCPL therefore reaches **both** Windows and macOS child processes via the **same** callback.
- **Only the *reader* is per-OS.** Windows renderer reads its switches from `GetCommandLineW()` — a **Win32-only API** (`simple_render_process_handler.cpp:527`, inside `#ifdef _WIN32`). macOS reads from **`argv` in a separate helper binary** (`mac/process_helper_mac.mm:main()` parses `--profile=` at `:58–64`; sanitization + path build at `:65–72`), because Mac uses the CEF helper-app subprocess model. Both are just parsing what OBCPL appended.
- **Chromium does not blindly forward parent switches to children.** Child command lines are rebuilt from a filtered switch set plus `AppendExtraCommandLineSwitches` (= where OBCPL runs) — so the delivery does **not** depend on any "parent-cmdline-forwarding" path; it depends on OBCPL re-appending the switch, which is exactly what `simple_app.cpp:64–68` does on both OSes.

**Why this matters for farbling — the load-bearing question is timing, not reachability:** C2 (outline) deliberately does **NOT** put the persistent seed on any renderer command line (threat model: a stable machine-local secret must not leak to every local process — outline OQ #3, §7 acceptance "No persistent seed on any renderer command line"). But the seed must also be **resident before the first farbled read** — i.e. before `HodosSessionCache` is first queried in `OnContextCreated`, before any page script runs. That ordering constraint is why farbling seeds are classically passed as command-line switches (available *synchronously at process spawn*). C2's two candidate channels trade off differently on this axis:

- **(a) per-launch ephemeral nonce on the cmdline** (browser maps nonce→real seed). Reaches the mac helper via the same OBCPL→argv path as `--profile=`, and is **timing-safe by construction** — the switch is present at process creation, so it is available before the first `OnContextCreated`. Satisfies "no *persistent* secret on cmdline" because the on-cmdline value is a throwaway nonce, not the seed. Mac cost: add an argv-parse in `process_helper_mac.mm` mirroring the `--profile=` parse (small, already-proven pattern).
- **(b) a non-inspectable channel (mojo / pref-on-navigation)** — CEF/Chromium IPC, cross-platform by construction. Reachability is a non-issue, but it is **async**: a mojo message or pref-on-navigation can race first script execution, so a frame could read Canvas/WebGL before the seed arrives → un-farbled (or default-seeded) first paint. To use (b), C2 must **prove the seed is resident before `OnContextCreated` queries the Supplement**, or the Supplement must lazily block/refetch until the seed is present.

> **Recommendation → feeds `PLAN_farbling_blink.md` C2 decision:** decide C2 on **delivery-timing merits, not reachability** (both channels reach the mac helper). Option (a) ephemeral-nonce-on-cmdline is timing-safe by construction and is the conservative default for first-paint correctness; option (b) mojo/pref is cleaner operationally (nothing on the cmdline at all) but must carry an explicit ordering guarantee (seed resident before first Supplement query, or a blocking/lazy Supplement). Do **not** treat mojo/pref as strictly superior. Either way the Mac side is a **verification item** (argv-parse for (a); ordering proof for (b)), not a separate mechanism.

**Note (why the old IPC is torn out):** today's farbling seed is delivered by the `fingerprint_seed` **IPC** (`simple_handler.cpp:7515` → `simple_render_process_handler.cpp:1198`, into `s_domainSeeds`), which is already cross-platform CEF process messaging. It is being **deleted** in the M1 teardown (TD-3, `Q2_farbling_adblock.md` §3) not because it fails to reach Mac, but because (i) it is a per-message *push* that can arrive **after** first script (the same async-timing hole as option (b) above), and (ii) it seeds `s_domainSeeds` **per-URL**, not the persistent **per-profile** seed model B1 requires. C2 replaces it with a seed resident at/near process spawn. If C2 lands on a CEF IPC/mojo channel, delivery stays cross-platform exactly like the old IPC was — the teardown is a timing + seed-model change, not a cross-platform→platform-specific regression.

### 2.3 Auth-domain exemption plumbing (C7) — cross-platform, one caveat

C7 re-implements `FingerprintProtection::IsAuthDomain` (the hardcoded C++ auth allowlist) at source: the browser passes the eTLD+1 auth allowlist to the renderer alongside the seed; `HodosSessionCache` returns pass-through when the top-frame origin is on it (outline C7, Q3). Because it rides the **same channel as C2's seed**, its Mac reachability is settled by the §2.2 decision — no separate Mac plumbing. **Mac owns:** verifying an auth-domain (Google/Microsoft sign-in) is un-farbled on macOS in acceptance (§6 T-M6).

---

## 3. Mac-specific Blink code paths — do the readback/WebGL patch points differ on Mac?

The patch *text* is shared, but Mac Claude must confirm the shared patch actually intercepts the right code on macOS's graphics stack, because macOS's GPU backend differs from Windows.

### 3.1 Canvas 2D / `readPixels` readback (`static_bitmap_image.cc`) — VERIFY, likely shared

C3 prefers the shared bitmap-readback path `platform/graphics/static_bitmap_image.cc` for canvas-2D readback (outline C3). This is **above** the GPU backend — it operates on the `StaticBitmapImage` after rasterization, so the perturbation point is platform-neutral. **Expected: identical on Mac.** C4 flags that WebGL `readPixels` (framebuffer readback) does **not** funnel through C3's `StaticBitmapImage` path and needs its **own** patch point in the WebGL contexts (outline C4, "verify code paths before sizing C3/C4"). That verification is a **shared** concern (Windows authors it), but **Mac re-confirms the readPixels patch fires on the Metal/ANGLE backend** (§3.2) — because the framebuffer-readback plumbing is closer to the GPU backend than the canvas-2D path is.

### 3.2 WebGL backend — Metal/ANGLE (Mac) vs D3D11/ANGLE (Windows)

macOS Chromium runs WebGL over **ANGLE→Metal** (Metal has been the default ANGLE backend on macOS since ~M112; **confirm it still holds for the target M150-class / CEF-149 branch** — this is a Mac-owned build-time check, not an assumption); Windows runs **ANGLE→D3D11**. The C4 patch points (`webgl_rendering_context_base.cc` `getParameter`/`getSupportedExtensions`/`readPixels`) sit in **Blink, above ANGLE**, so the patch text is backend-agnostic and shared. **Two Mac-owned verifications:**

- **readPixels actually perturbs on Metal.** Confirm the Blink-layer `readPixels` interception runs before the value is returned to JS regardless of the Metal backend (expected yes — the patch is above ANGLE — but this is the exact spot a backend difference could bite, so it is a named Mac acceptance check, §6 T-M2).
- **UNMASKED_VENDOR/RENDERER values differ.** This is the real Mac divergence (§5): the *strings* Mac reports are Apple-Silicon/Intel-Mac ANGLE-Metal strings, not Windows D3D strings. The patch that *maps* them is shared; the **map contents for Mac are Mac-authored data** (Q5 §B-VALUES; owner-gated on the arm64/x64/universal2 arch decision).

### 3.3 WebAudio (C5) & Navigator (C6) — fully shared, no Mac backend concern

`AnalyserNode`/`AudioBuffer` readback (C5) and `deviceMemory`/`hardwareConcurrency`/plugins (C6) are platform-neutral Web API surface with no GPU/Metal dependency. **Fully shared; Mac inherits.** The only Mac angle is that C6's constrained valid-set values (deviceMemory ∈ {2,4,8,16,32}; plausible core counts — Q5 §B-VALUES) should produce plausible-for-Mac values (e.g. an 8-core Apple Silicon reporting an implausible count is itself a tell) — but this is a **value-table** concern owned by the farbling plan, not a Mac code fork.

---

## 4. Mac-owned OS glue: build / arch / minos / plist (inherited verbatim from outline §5 + §3f)

These are Mac-exclusive and already assigned to Mac Claude in the outline; restated here as Q1's Mac work list so it is self-contained:

- **Arch decision (arm64 / x86_64 / universal2)** — Mac-specific, materially changes build time/cost/disk **and the C4 GPU-string set** (Apple Silicon vs Intel ANGLE). Outline default lean = **universal2** with owner sign-off (outline §5, I8). **This decision gates the C4 Mac GPU-string entries.** Note universal2 is **not a single-pass `BUILDFLAG` toggle**: CEF/Chromium macOS framework builds are **per-arch (arm64, then x86_64), then `lipo`-combined** — so "universal2" means **two full framework builds + a merge**, which is what actually drives the ~10–12 hr estimate below (not one build with a flag flipped). The executor should not expect a one-shot universal build.
- **Xcode/clang build host** — Mac owns its own from-source CEF framework build (not a light inherit — I8). Budget **~10–12 hr for universal2 (two per-arch builds + `lipo`)**, roughly half that for a single arch.
- **CI app-build runner pin** — `macos-NN` (never `macos-latest`) so its clang matches the CEF framework's toolset (ABI, outline VER-3/I9).
- **minos / deployment-target / plist** — Mac owns entirely: `vtool`-measure the framework `minos`; set published min = `max(Chromium floor, measured minos)` in all three (`cef-native/CMakeLists.txt` `CMAKE_OSX_DEPLOYMENT_TARGET`, `cef-native/Info.plist`, `cef-native/mac/helper-Info.plist.in` `LSMinimumSystemVersion`); CI **minos guard** green (outline VER-4).
- **Framework-embed file-manifest drift audit (Step 5.5, Mac list)** — feeds the auto-update apply gate (outline VER-5).
- **Real N-1→N Sparkle auto-update apply on Mac** — the reinstall-forcer gate; verify **signer continuity (Team ID unchanged)** given the pending Apple individual→org signing migration (outline §7; MEMORY signing-gate — a signer change forces reinstall and would make the update-apply test pass while prod reinstalls).

---

## 5. The one place the "shared patch" splits per-OS: C4 GPU strings

If the §3c/Q5 conflict resolves to **map** WebGL vendor/renderer (rather than **drop** it), the C4 patch needs a small table of "common real GPU strings" keyed to the machine's real GPU class:

- **Windows entries:** common D3D11/ANGLE strings (e.g. Intel/NVIDIA/AMD via ANGLE Direct3D11).
- **macOS entries (Mac Claude authors):** Apple Silicon ANGLE-Metal strings (e.g. `Apple M1`/`M2`/`M3` families) **and** Intel-Mac ANGLE strings (Intel iGPU / AMD dGPU) — Q5 §B-VALUES, outline I8.

**Design rule:** the OS split must be **runtime data selection keyed off the real reported `UNMASKED_RENDERER`**, **not** a compile-time `#if BUILDFLAG(IS_MAC)` constant. A compile-time table is fixed per build, and it cannot even distinguish the cases we need: in a universal2 binary **both** arch slices are `BUILDFLAG(IS_MAC)`, so `#if BUILDFLAG(IS_MAC)` can't separate Apple-Silicon from Intel, and an Intel Mac can report either an Intel iGPU or an AMD dGPU string. Realistic vendor/renderer farbling must therefore select a plausible value **around the real runtime string the machine exposes** (read the true `UNMASKED_RENDERER`, map it to a small plausible set for that GPU class, pick from it deterministically by seed) — never a build-time constant. Keep it as **one shared C4 patch** carrying a runtime map for all OSes (still one file to rebase per Chromium bump — the "one file, OS split is data not logic" rebase argument survives; only the `#if BUILDFLAG` framing is wrong), **not** a separate Mac `.patch`. **Mac owns the Mac rows of that map; Windows owns the patch scaffold.** This is also the highest-risk *design* point (mapping to a *small set of real* strings, never random noise — random vendor/renderer is *more* unique than truth; Q5 §B-VALUES marks it UNRESOLVED). **If the conflict resolves to DROP vendor/renderer, this split disappears entirely and the patch is 100% shared.**

---

## 6. Mac acceptance run (Mac Claude executes; folds into outline §7 P6)

Mac runs the full farbling acceptance on real Mac hardware (both arches if universal2). Mac-flavored subset of the §7 criteria:

| # | Test | Pass criterion |
|---|---|---|
| **T-M1** | Canvas/WebGL/audio farbled on macOS build | CreepJS + browserleaks show perturbed values; `toDataURL.toString()`/`getParameter.toString()` return `[native code]` (proves below-JS on Mac) |
| **T-M2** | **WebGL `readPixels` perturbs on Metal/ANGLE** | Framebuffer readback differs from raw, stable within session+domain (the §3.2 Metal-backend check) |
| **T-M3** | worker column == window column (incl. service/shared worker + OffscreenCanvas-in-worker) | Matches window; OOP-worker seed plumbing (P4e) reaches Mac helper processes |
| **T-M4** | **No persistent seed on any renderer/helper command line** | `ps -ax`/Activity Monitor inspection of helper cmdlines shows no stable per-profile secret (C2 threat model on Mac) |
| **T-M5** | Intra-session consistency + cross-profile difference + cross-site iframe (first-party keying) | Same as Windows §7 — verifies C2 seed derivation is identical on Mac |
| **T-M6** | Cross-session login (load-bearing) + auth-domain exemption (C7) | Create account → restart → revisit → login not broken; a Google/Microsoft sign-in is un-farbled on macOS |
| **T-M7** | Navigator values plausible-for-Mac | deviceMemory ∈ valid set; core count plausible for the Mac's real hardware class |
| **T-M8** | Mac GPU strings (if mapped) are real Apple/Intel ANGLE strings | Not noise; selected by the §5 runtime map around the machine's real reported `UNMASKED_RENDERER` (Apple-Silicon slice reports an Apple-GPU string; Intel slice reports Intel iGPU / AMD dGPU) — plausible for the real GPU class, not a build-time constant |
| **T-M9** | minos guard green + auto-update apply on Mac with signer continuity | Every exe/helper/Rust-bin `minos ≥` framework minos; N-1→N Sparkle apply clean; Team ID unchanged (§4) |

Results reconciled with Windows in the coordination doc (**`CHROMIUM_BUILD_RELAY.md`**, outline §5).

---

## 7. Ownership matrix — INHERITS vs OWNS (the crisp Q1 answer)

| Item | Shared / Mac-specific | Mac Claude role |
|---|---|---|
| C1 HodosSessionCache Supplement | Shared patch text | **INHERITS** (compiles into framework) |
| C2 renderer-side HMAC seed derivation | Shared patch text | **INHERITS** |
| **C2 seed → renderer *delivery channel*** | **Shared callback** (OBCPL fires on both OSes), **per-OS reader** (`GetCommandLineW` vs mac helper `argv`); open question is **delivery timing**, not reachability | **OWNS verification** (argv-parse if nonce-cmdline; ordering proof if mojo/pref) — §2.2/I-1 |
| Per-profile seed *storage* | Cross-platform (`ProfileManager`/`SettingsManager`) | **OWNS verify** (path resolves + persists on macOS) — no new code expected |
| C3 Canvas 2D / `static_bitmap_image` readback | Shared patch text | **INHERITS** + **OWNS verify** it fires on Mac |
| C4 WebGL `getParameter`/`readPixels` patch scaffold | Shared patch text | **INHERITS** + **OWNS verify** readPixels perturbs on Metal/ANGLE (§3.2) |
| **C4 WebGL vendor/renderer Mac GPU-string *data*** | **Mac-specific data** in the shared patch | **OWNS** (Apple Silicon + Intel ANGLE strings; §5) |
| C5 WebAudio / C6 Navigator | Shared patch text | **INHERITS** (C6 values plausible-for-Mac = value-table concern) |
| C7 auth-domain exemption | Shared (rides C2 channel) | **INHERITS** + **OWNS** acceptance verify |
| P4e OOP-worker seed plumbing | Shared design | **INHERITS** + **OWNS** Mac helper-process verify |
| M1 teardown (TD-1..TD-4, delete JS farbling) | Shared `cef-native/` deletions (already `#ifdef`'d cross-platform) | **INHERITS** (same deletions apply; verify Mac build still links — the deleted `FingerprintProtection.h`/`FingerprintScript.h` are header-only cross-platform) |
| Mac arch (arm64/x64/universal2) | Mac-specific | **OWNS** (owner sign-off) |
| Xcode/clang build host | Mac-specific | **OWNS** |
| `macos-NN` CI runner pin (ABI) | Mac-specific | **OWNS** |
| minos / deployment-target / plist / minos guard | Mac-specific | **OWNS entirely** |
| Framework file-manifest drift audit (Step 5.5) | Mac list | **OWNS** |
| Real N-1→N auto-update apply + signer continuity | Mac (Sparkle) | **OWNS** run |
| macOS farbling acceptance (T-M1..T-M9) | Mac run | **OWNS** |

---

## 8. Open questions → recommended defaults

| # | Question | Recommended default | Why |
|---|---|---|---|
| Q1-1 | C2 seed delivery channel: ephemeral-nonce cmdline vs mojo/pref? | **Decide on delivery-*timing*, not reachability** (both reach the mac helper). Lean = **ephemeral-nonce-on-cmdline** as the timing-safe default (seed resident at process spawn, before first `OnContextCreated`); mojo/pref is acceptable **only** with an explicit ordering guarantee (seed resident before first Supplement query, or a blocking/lazy Supplement). Resolves in `PLAN_farbling_blink.md`. | Cmdline switches are synchronous at spawn → no un-farbled first paint. Mojo/pref is async and can race first script. Reachability is a non-issue on both OSes (OBCPL is cross-platform). If nonce is chosen, Mac owns the helper argv-parse (mirrors `--profile=`). |
| Q1-2 | Does the C4 patch split into a separate Mac `.patch` file? | **No — one shared C4 patch carrying a runtime GPU-string map** keyed off the real reported `UNMASKED_RENDERER` (NOT a `#if BUILDFLAG(IS_MAC)` constant — see I-3/§5). | One file to rebase per bump; OS split is data, not logic. A compile-time `#if` can't distinguish Apple-Silicon vs Intel in a universal2 binary. Vanishes entirely if vendor/renderer is dropped. |
| Q1-3 | Universal2 vs arm64-only for the Mac build? | **universal2 (outline lean), owner sign-off.** | Distribution breadth; but doubles the C4 Mac string set (Apple + Intel) and build time. Owner-gated. |
| Q1-4 | Does readPixels perturb correctly on ANGLE→Metal? | **Assume yes (patch is above ANGLE); make it a named Mac acceptance gate (T-M2), not an assumption.** | Framebuffer readback is the spot closest to the backend; cheap to verify, expensive to miss. |
| Q1-5 | Where do the Mac GPU strings come from? | **Enumerate real `UNMASKED_RENDERER` strings from Apple Silicon + Intel Macs (Chrome/Brave on macOS) and pick a small common set.** | Must be real, not noise (Q5 §B-VALUES); mirrors what a stock Chrome on that Mac reports. |
| Q1-6 | Does the M1 JS-farbling teardown need Mac-specific deletion work? | **No — the deleted files are header-only, cross-platform; the `simple_render_process_handler.cpp`/`simple_handler.cpp` deletions are shared. Mac just re-links.** | `FingerprintProtection.h`/`FingerprintScript.h` have no Mac fork; TP-1/TP-2 (Q2 §3) are in cross-platform files. |

---

## 9. Risks

- **[MED] C2 seed arrives after first farbled read (delivery-timing race).** Not a Mac-reachability problem — OBCPL is cross-platform and reaches the mac helper (proven for `--profile=`). The real hazard is an **async** channel (mojo/pref, or the deleted per-message `fingerprint_seed` IPC) delivering the seed **after** the first `OnContextCreated`/Supplement query → un-farbled or default-seeded first paint, on either OS. **Mitigation:** prefer the ephemeral-nonce-on-cmdline channel (present at spawn, timing-safe); if mojo/pref is chosen, carry an explicit ordering guarantee (seed resident before first Supplement query, or blocking/lazy Supplement). Make T-M4/T-M5 hard Mac gates. *(A separate, smaller risk: if the ephemeral nonce is chosen but the mac helper argv-parse is not added, the mac renderer gets no seed — a compile/smoke miss, not a design trap; caught by T-M5.)*
- **[MED] readPixels Metal divergence.** Low-probability but named (T-M2).
- **[MED] GPU-string realism.** Wrong/implausible Mac ANGLE strings are *more* fingerprintable than truth (Q5 §B-VALUES highest-risk value decision). **Mitigation:** enumerate from real Macs; small common set; or drop vendor/renderer.
- **[MED] Signer-migration reinstall.** The pending Apple individual→org migration can make the Mac auto-update-apply gate pass while prod forces reinstall. **Mitigation:** verify Team ID continuity in T-M9 (§4; MEMORY signing-gate).
- **[LOW] universal2 build cost** doubles Mac build time + the C4 Mac string set — owner-gated (Q1-3).

---

*Feeds `IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md` (Mac column of P4/P6) and the outline §7 farbling acceptance. Reconcile §5/§7 against `PLAN_farbling_blink.md` (C2 channel + WebGL value decision) and `Q3-farbling-x-oauth.md` (C7) once they land — no new Mac patch categories expected, only the C2-channel confirmation and the Mac GPU-string data fill.*
