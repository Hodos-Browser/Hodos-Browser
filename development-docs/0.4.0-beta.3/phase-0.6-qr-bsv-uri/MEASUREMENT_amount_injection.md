# P0.6 — MEASUREMENT: unescaped QR `amount` executes JS in the wallet overlay

**Measured:** 2026-08-21 · **Binary:** pre-fix dev build `cef-native/build/bin/Release/HodosBrowser.exe`
(QRScreenCapture.cpp mtime Apr 27, unchanged) · **Subject:** real wallet-panel overlay V8,
`http://127.0.0.1:5137/wallet-panel?iro=50` (proved via `location.href`, not asserted).

## Claim

A scanned QR whose BIP21 `amount` parameter is not a number becomes **executable JavaScript in the
privileged wallet-overlay context** — arbitrary code, not just a wrong number. Pre-existing, reachable
via `bitcoin:` **today**, independent of the `bsv:` scheme widening this phase performs.

## The two links

**Link A — the C++ capture classifier emits `amount` raw. Direct source read (cited, not modeled):**

`QRScreenCapture.cpp :: ClassifyAndBuildJson`, BIP21 branch:
```cpp
std::string val = UrlDecode(pair.substr(eq + 1));   // no numeric validation
if (key == "amount") amount = val;
...
if (!amount.empty())  json += ",\"amount\":" + amount;   // RAW, UNQUOTED, UNESCAPED
if (!label.empty())   json += ",\"label\":\"" + JsonEscape(label) + "\"";  // label IS escaped
```
The asymmetry is the bug: `label` is `JsonEscape`d, `amount` is concatenated raw. `cef_browser_shell_mac.mm ::
ClassifyBSVContent` is a verbatim copy with the same line.

`json` is then delivered to the renderer, which (`simple_render_process_handler.cpp:920`) builds:
```cpp
std::string js = "window.dispatchEvent(new MessageEvent('message',{data:{type:'qr_screen_capture_result',data:" + json + "}}));";
frame->ExecuteJavaScript(js, frame->GetURL(), 0);   // frame = wallet overlay main frame
```
So attacker bytes from `amount` land inside a string handed to `ExecuteJavaScript` in the overlay.

**Link B — the real wallet-overlay V8 executes it. MEASURED (`probes/amount_injection_measure.js`,
result `probes/RESULT_amount_injection_RED.json`):** the probe reproduces `ClassifyAndBuildJson`
verbatim in-page, wraps its output in the exact line-920 template, and `eval`s it in the real overlay:

| Case | payload `amount=` | ran | effect |
|---|---|---|---|
| benign control | `0.11828417` | ✅ | probe **unset** — a numeric amount executes nothing (negative control) |
| non-numeric | `abc` | ❌ | `ReferenceError: abc is not defined` — invalid JS, **silent delivery failure** |
| **injection** | `(window.__hodos_qr_probe=typeof fetch,0)` | ✅ | probe = **`"function"`** — arbitrary JS ran; `fetch` reachable |

Payload uses no `&` (param separator) and no `%` (UrlDecode), so it survives parsing char-for-char and
the address stays valid, so the classifier still "succeeds" and delivers.

## What is measured vs read

- **Measured on the real subject:** the wallet-overlay V8 executes an unescaped-`amount` string exactly
  as `ExecuteJavaScript` would receive it (Link B).
- **Read from source, verbatim-quoted, not executed:** the real quirc→`ClassifyAndBuildJson`→renderer
  chain producing that string (Link A). The physical decode path (real QR on screen → drag-capture) was
  not driven — an elaborate/flaky harness whose only added coverage is "no sanitizer between decode and
  emit that the source read missed," and the source shows none. Closed at T2 by the post-fix
  capture-path run.

## Two defects, one fix

1. **Injection** (severity: high — arbitrary JS in the key-holding overlay's origin from a scanned QR).
2. **Silent failure** on any non-numeric `amount` (robustness; also masks the scan for the user).

Both close by validating `amount` is a finite decimal number before emitting it, in **both** C++
classifiers — matching what the JS site already does implicitly (`parseFloat` + `JSON.stringify`).
An `amount` that fails validation is dropped (treated as "no amount"), never emitted raw.

## Scope note

Not created by Phase 0.6 — `bitcoin:` reaches this today. But the phase edits these exact functions to
widen the scheme, so the validation lands here. The owner approved fixing it in-phase (option (a)).
