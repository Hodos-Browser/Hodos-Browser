# Phase 8c — how to migrate one slot

**Written 2026-09-09**, at the end of stage 3 batch 1, **so the remaining 36 can be done from a cold
context.** 5 of 41 are done and every *shape* has been exercised — nothing below is exploratory.

⛔ Read `PHASE_CONTRACT.md` §0a (the owner register) and §0.4 first. This file is the *procedure*;
the contract is the *reasoning*.

---

## The one rule that outranks the recipe

⛔ **Never dedupe a write.** `getBalance` once carried an in-flight dedupe; it was sound only because
a balance read is idempotent. Applying that pattern to `sendTransaction` would silently collapse two
distinct payments into one — worse than the bug being fixed. If a slot is a *write*, it gets a
request id and nothing else.

---

## Six edits per slot

Worked example: `wallet.getInfo` → `get_wallet_info`.

### 1. `simple_render_process_handler.cpp` — the method→IPC table

Inside `WalletBridgeV8Handler::Execute`:

```cpp
} else if (method == "getInfo") {
    ipcName = "get_wallet_info";
    // payload = Payload::Str;   // only if the call carries an argument
}
```

`Payload` is `None | Str | Bool`. ⚠️ If a slot needs a shape that is not one of those (an int, an
object), add it to the enum — do **not** stringify around it, because the browser handler reads a
typed arg (`GetInt`, `GetBool`) and would silently read the wrong thing.

### 2. Same file — register the V8 function

```cpp
bridgeObject->SetValue("getInfo",
    CefV8Value::CreateFunction("getInfo", bridgeHandler),
    V8_PROPERTY_ATTRIBUTE_READONLY);
```

⚠️ Bump the count in the `LOG_DEBUG_RENDER("🌉 Bound WalletBridgeV8Handler (N methods migrated)")`
line, or it lies.

### 3. Same file — the response arm(s)

Replace the `frame->ExecuteJavaScript("if (window.on…Response) …")` body with:

```cpp
if (message_name == "get_wallet_info_response") {
    CefRefPtr<CefListValue> args = message->GetArgumentList();
    if (!args || args->GetSize() < 2) {
        LOG_ERROR_RENDER(LogFmt() << "get_wallet_info_response missing args (need 2)");
        return true;
    }
    ResolveBridgeCall(args->GetInt(0), args->GetString(1).ToString());
    return true;
}
```

⭐ If the slot has a separate `*_error` arm, give it the identical shape but call
**`RejectBridgeCall`**. ⛔ Do not resolve an error arm — see the `resolve(null)` trap below.

### 4. `simple_handler.cpp` — echo the id

```cpp
const int reqId = message->GetArgumentList()->GetInt(0);   // ⛔ BEFORE any try block
...
responseArgs->SetInt(0, reqId);
responseArgs->SetString(1, payload);                        // was SetString(0, …)
```

⚠️ **If the handler reads a request payload, its index shifts by one** — `GetString(0)` → `GetString(1)`,
`GetBool(0)` → `GetBool(1)`, and any `args->GetSize() > 0` guard becomes `> 1`.

### 5. `initWindowBridge.ts` — replace the whole method body

```ts
getInfo: () => {
  if (!window.hodosBrowser?.bridge?.getInfo) {
    return Promise.reject(new Error('wallet.getInfo: native bridge unavailable'));
  }
  return window.hodosBrowser.bridge.getInfo();
},
```

Delete the `setTimeout`, both globals, and every `delete window.on…`. ⭐ Remove any module-level
variable the change orphans (stage 3 removed `balanceInFlight` this way).

### 6. `types/hodosBrowser.d.ts` — two edits

Add the signature under `bridge?: { … }`, and **replace** the `window.on…Response` / `…Error`
declarations with a comment saying which stage removed them. Leaving the declaration invites the next
author to reintroduce the race.

---

## 🚨 Three traps, all of which have already bitten

1. **The request id must be read BEFORE the `try`.** `send_transaction`'s catch echoes it; read after
   a parse that throws, the error path has no id and the caller hangs to the **30 s deadline** instead
   of rejecting. Measured: 82 ms with the hoist, would have been 30 s without.
2. **Check the legacy method's timeout behaviour before deleting it.** The class has **two** flavours:
   `getInfo` *rejects*, `getBackupModalState` *resolved `null`* — a silent wrong value. ⛔ One
   `resolve(null)` remains, in **`markBackedUp`**, which records that the user backed up their
   recovery phrase. Read each one; do not assume.
3. **Captureless lambdas.** `get_balance`'s handler replies from a lambda kept captureless *on
   purpose* so it can be bound into a CEF task. Thread the id through as a **parameter**. Capturing
   compiles and breaks the property the comment above it exists to protect.

---

## Per-batch checklist

- [ ] Group by namespace (cookies, cookieBlocking, bookmarks, …) — one commit per batch.
- [ ] `cd frontend && npm run build` — ⛔ **never `npx tsc --noEmit`**, it passes on code the build rejects.
- [ ] `cmake --build build --config Release` — ⛔ check the **exit code**, not a `grep -c error` on a
      pipeline. A running dev browser gives `LNK1104` and a piped grep will read 0.
- [ ] `scripts/preflight.ps1 -Full` (⛔ bare `preflight.ps1` skips `T1d` and says so).
- [ ] Live GREEN/RED: N concurrent calls on a migrated method, and the **same** experiment on a
      still-legacy sibling. ⚠️ Assert the subject first — `bridge.<m>.toString()` contains
      `[native code]`, and the old globals are `undefined`.
- [ ] ⚠️ If a call **mutates** state, restore it (stage 3 set `setBackupModalState` back to `false`).

### ⛔ The RED control is a wasting asset

Every migration burns one. `getBackupModalState` was stage 1's control; batch 1 had to use `getInfo`.
**At zero remaining slots there is no legacy method left to compare against** — the final batch's RED
must be a deliberate stub (e.g. drop the id echo in C++ and observe the mismatch), or its green
proves nothing. This is `O7` in the contract's owner register.

---

## Rig

```
.\dev-wallet.ps1                      # 31401
cd frontend && npm run dev            # 5137
Start-Process …\build\bin\Release\HodosBrowser.exe  -ArgumentList '--profile=Default','--remote-debugging-port=9322'   # with HODOS_DEV=1
.\scripts\stop-dev.ps1                # ⛔ path-matched; NEVER kill by image name
```

⚠️ **11 CDP targets all report `type:"page"`.** The header is `http://127.0.0.1:5137/` — the others
are overlays. Driving the wrong one faked an "intermittent" bug earlier in 0.4.0.
⚠️ Hard-reload before measuring: Vite Fast Refresh has printed a **GREEN negative control** against a
build with the guard deleted.
⛔ The owner's **installed** browser and wallet are normally running. Check ports/exe paths first;
`stop-dev.ps1` spares them.

## Remaining: 36 live slots

Roughly: cookies (6) · cookie blocking (9) · bookmarks (14) · cache (2) · the rest of the wallet
namespace (`create`, `load`, `getInfo`, `generateAddress`, `getCurrentAddress`, `getAddresses`,
`markBackedUp`, `getTransactionHistory`).

⭐ **Do the wallet namespace first** — it holds `markBackedUp` (the last `resolve(null)`) and four of
the five remaining **JS-injection** sites (`onAddressError`, `onSignTransactionError`,
`onBroadcastTransactionError`, `onGetTransactionHistoryError`). Bookmarks and cookies are the low-risk
tail and can go last.
