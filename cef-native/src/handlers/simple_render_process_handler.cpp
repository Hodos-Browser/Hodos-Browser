// cef_native/src/simple_render_process_handler.cpp
#include "../../include/handlers/simple_render_process_handler.h"

// V8 handlers (cross-platform)
#include "../../include/core/PortConfig.h"
#include "../../include/core/IdentityHandler.h"

// Cross-platform handlers (work on both platforms)
#include "../../include/core/NavigationHandler.h"
#include "../../include/core/AddressHandler.h"
#include "../../include/core/JsStringEscape.h"  // F6: canonical escapeJsonForJs encoder

#include "wrapper/cef_helpers.h"
#include "include/cef_task.h"
#include "include/cef_v8.h"
#include <iostream>
#include <cstdio>
#include <map>

#include "../../include/core/Logger.h"
#include "../../include/core/LogSafeUrl.h"
#include "../../include/core/FingerprintProtection.h"
#include "../../include/core/CWIShimScript.h"

#include <unordered_map>
#include <unordered_set>
#include <mutex>

// Static cache for pre-loaded cosmetic scriptlets (8e-2).
// Browser process sends scriptlets via IPC before page loads;
// OnContextCreated injects them synchronously before page JS runs.
static std::mutex s_scriptCacheMutex;
static std::unordered_map<std::string, std::string> s_scriptCache; // URL → scriptlet JS

// NOTE: the fingerprint seed cache (s_domainSeeds/s_seedMutex) and the per-site disable
// set (s_fingerprintDisabledUrls/s_fpDisabledMutex) were DELETED 2026-08-09 along with
// FINGERPRINT_PROTECTION_SCRIPT. Farbling is native in Blink now and the renderer holds
// no farbling state at all: libcef pulls the per-origin key at OnContextCreated and hands
// it to HodosSessionCache. Do not reintroduce a renderer-side cache here -- a per-URL map
// in this process is exactly what made the shipped constant-seed bug invisible, because a
// cross-process navigation left it empty in the incoming renderer.

// Convenience macros for easier logging
#define LOG_DEBUG_RENDER(msg) Logger::Log(msg, 0, 1)
#define LOG_INFO_RENDER(msg) Logger::Log(msg, 1, 1)
#define LOG_WARNING_RENDER(msg) Logger::Log(msg, 2, 1)
#define LOG_ERROR_RENDER(msg) Logger::Log(msg, 3, 1)

// escapeJsonForJs() is the canonical JS-string-literal encoder — now defined in
// include/core/JsStringEscape.h (F6: hardened + extracted so it can be unit-tested
// without CEF, and so all injection sites share one correct implementation).

// Handler for cefMessage.send() function
class CefMessageSendHandler : public CefV8Handler {
public:
    CefMessageSendHandler() {}

    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override {

        CEF_REQUIRE_RENDERER_THREAD();

        if (arguments.size() < 1) {
            exception = "cefMessage.send() requires at least one argument (message name)";
            return true;
        }

        std::string messageName = arguments[0]->GetStringValue();
        LOG_DEBUG_RENDER(LogFmt() << "📤 cefMessage.send() called with message: " << messageName);
        LOG_DEBUG_RENDER(LogFmt() << "📤 Arguments count: " << arguments.size());

        // Try multiple logging approaches
        LOG_DEBUG_RENDER("📤 cefMessage.send() called with message: " + messageName);
        LOG_DEBUG_RENDER("📤 Arguments count: " + std::to_string(arguments.size()));

        // Also try writing to a different file

        // Create the process message
        CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create(messageName);
        CefRefPtr<CefListValue> args = message->GetArgumentList();

        // Add arguments if provided (skip first argument which is the message name)
        for (size_t i = 1; i < arguments.size(); i++) {
            LOG_DEBUG_RENDER("📤 Processing argument " + std::to_string(i-1) + ": ");

            if (arguments[i]->IsString()) {
                std::string value = arguments[i]->GetStringValue();
                // ⛔ Length only. This dumps ARBITRARY IPC payloads -- measured leaking a
                // full URL with its query string into cef_debug.log the moment the
                // surrounding std::cout was converted to a real sink. What flows through
                // here is unbounded (wallet call bodies, tab state, page URLs), so no
                // redaction rule can be trusted to cover it; do not log the value.
                LOG_DEBUG_RENDER("String arg (" + std::to_string(value.size()) + " bytes)");
                args->SetString(i - 1, value);
            } else if (arguments[i]->IsBool()) {
                bool value = arguments[i]->GetBoolValue();
                LOG_DEBUG_RENDER("Bool: " + std::string(value ? "true" : "false"));
                args->SetBool(i - 1, value);
            } else if (arguments[i]->IsInt()) {
                int value = arguments[i]->GetIntValue();
                LOG_DEBUG_RENDER("Int: " + std::to_string(value));
                args->SetInt(i - 1, value);
            } else if (arguments[i]->IsDouble()) {
                double value = arguments[i]->GetDoubleValue();
                LOG_DEBUG_RENDER("Double: " + std::to_string(value));
                args->SetDouble(i - 1, value);
            } else if (arguments[i]->IsArray()) {
                // Expand ALL array elements into the message args list
                CefRefPtr<CefV8Value> array = arguments[i];
                LOG_DEBUG_RENDER("Array with length: " + std::to_string(array->GetArrayLength()));
                for (int j = 0; j < array->GetArrayLength(); j++) {
                    CefRefPtr<CefV8Value> element = array->GetValue(j);
                    if (element->IsString()) {
                        std::string value = element->GetStringValue();
                        LOG_DEBUG_RENDER("Array[" + std::to_string(j) + "] String arg ("
                                         + std::to_string(value.size()) + " bytes)");
                        args->SetString(j, value);
                    } else if (element->IsBool()) {
                        bool value = element->GetBoolValue();
                        LOG_DEBUG_RENDER("Array[" + std::to_string(j) + "] Bool: " + std::string(value ? "true" : "false"));
                        args->SetBool(j, value);
                    } else if (element->IsInt()) {
                        int value = element->GetIntValue();
                        LOG_DEBUG_RENDER("Array[" + std::to_string(j) + "] Int: " + std::to_string(value));
                        args->SetInt(j, value);
                    } else if (element->IsDouble()) {
                        double value = element->GetDoubleValue();
                        LOG_DEBUG_RENDER("Array[" + std::to_string(j) + "] Double: " + std::to_string(value));
                        args->SetDouble(j, value);
                    }
                }
            } else {
                LOG_DEBUG_RENDER("Unknown type");
            }
        }

        // Send the message to the browser process
        CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
        if (context && context->GetFrame()) {
            context->GetFrame()->SendProcessMessage(PID_BROWSER, message);
            LOG_DEBUG_RENDER("✅ Process message sent to browser process: " + messageName);
        } else {
            LOG_ERROR_RENDER("❌ Failed to get frame context for sending process message");
        }

        return true;
    }

private:
    IMPLEMENT_REFCOUNTING(CefMessageSendHandler);
};

// Handler for overlay.close() function
class OverlayCloseHandler : public CefV8Handler {
public:
    OverlayCloseHandler() {}

    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override {

        CEF_REQUIRE_RENDERER_THREAD();

        LOG_DEBUG_RENDER("🎯 overlay.close() called from overlay browser");

        // Send overlay_close message via cefMessage
        CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
        if (context && context->GetFrame()) {
            CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create("overlay_close");
            context->GetFrame()->SendProcessMessage(PID_BROWSER, message);

            LOG_DEBUG_RENDER("✅ overlay.close() sent overlay_close message");
        }

        return true;
    }

private:
    IMPLEMENT_REFCOUNTING(OverlayCloseHandler);
};

// Handler for omnibox overlay.close() - sends omnibox_hide message
class OmniboxCloseHandler : public CefV8Handler {
public:
    OmniboxCloseHandler() {}

    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override {

        CEF_REQUIRE_RENDERER_THREAD();

        LOG_DEBUG_RENDER("🔍 omnibox overlay.close() called");

        // Send omnibox_hide message
        CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
        if (context && context->GetFrame()) {
            CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create("omnibox_hide");
            context->GetFrame()->SendProcessMessage(PID_BROWSER, message);
            LOG_DEBUG_RENDER("✅ omnibox overlay.close() sent omnibox_hide message");
        }

        return true;
    }

private:
    IMPLEMENT_REFCOUNTING(OmniboxCloseHandler);
};

// ========== HISTORY V8 HANDLER ==========
//
// window.hodosBrowser.history.* — all 7 methods return a Promise and are serviced by
// the BROWSER process over IPC.
//
// This used to call HistoryManager directly from the renderer, which meant every
// renderer process (including ones hosting arbitrary web pages) held a read/write
// handle on the profile's history SQLite DB. Three reasons that had to go:
//   1. It is the exact capability the Chromium sandbox exists to remove, so it was a
//      hard blocker for turning the sandbox on.
//   2. It is a standing security problem even unsandboxed.
//   3. It never worked on macOS at all — the renderer-side init is #ifdef'd out, so
//      history.* silently returned empty there. Routing through IPC fixes mac too.
//
// The browser process already has an initialized HistoryManager bound to the correct
// profile; `get_most_visited` was already using it this way, so this follows an
// established path rather than inventing one.
//
// Threading: Execute() and OnProcessMessageReceived() both run on the renderer
// thread, so the pending map below needs no lock. CEF_REQUIRE_RENDERER_THREAD()
// asserts that in debug builds.

struct PendingHistoryRequest {
    CefRefPtr<CefV8Value> promise;
    CefRefPtr<CefV8Context> context;
};
static std::map<int, PendingHistoryRequest> s_pendingHistory;
static int s_nextHistoryRequestId = 1;

// Resolve the promise for `requestId` with `json`. Called from the history_response
// IPC handler. No-ops if the context died (page navigated away mid-request).
void ResolveHistoryRequest(int requestId, const std::string& json) {
    CEF_REQUIRE_RENDERER_THREAD();

    auto it = s_pendingHistory.find(requestId);
    if (it == s_pendingHistory.end()) {
        LOG_DEBUG_RENDER("📚 history_response for unknown requestId " + std::to_string(requestId));
        return;
    }

    PendingHistoryRequest pending = it->second;
    s_pendingHistory.erase(it);

    if (!pending.context || !pending.context->IsValid()) {
        LOG_DEBUG_RENDER("📚 history_response dropped — context gone (requestId " +
                         std::to_string(requestId) + ")");
        return;
    }

    // Promise resolution must happen inside the owning V8 context.
    pending.context->Enter();
    try {
        pending.promise->ResolvePromise(jsonToV8(nlohmann::json::parse(json)));
    } catch (const std::exception& e) {
        pending.promise->RejectPromise(std::string("history: bad response — ") + e.what());
    }
    pending.context->Exit();
}

// ========== WALLET BRIDGE — PER-REQUEST ROUTING (Phase 8c, stage 1) ==========
//
// The legacy bridge in `initWindowBridge.ts` resolves each native call through a
// SINGLE global callback slot per METHOD (`window.on<Method>Response`). Two calls to
// the same method in flight together clobber each other: the first reply resolves one
// promise and deletes the handler, and the second caller waits out its own timeout and
// reports a failure that never happened. Measured at 1 run in 3 for `getBalance`
// (`TICKET_bridge_single_slot_callbacks_race.md`).
//
// ⭐ This is deliberately NOT a new mechanism. It is the same shape as
// `s_pendingHistory` directly above — a promise held in C++, keyed by request id,
// resolved inside its owning V8 context — chosen over the ticket's proposed JS-side
// `Map<id,{resolve,reject}>` because that leaves a JS registry that leaks when a page
// navigates mid-request. Owner decision, 2026-09-08 (phase contract §8, Option B).
//
// ⛔ Do NOT "fix" the remaining legacy slots by copying `getBalance`'s in-flight dedupe.
// It is sound only for idempotent reads. Deduping `sendTransaction` would collapse two
// distinct payments into ONE, which is worse than the bug.
struct PendingBridgeCall {
    CefRefPtr<CefV8Value> promise;
    CefRefPtr<CefV8Context> context;
    std::string method;  // diagnostics only — which call this id belongs to
};
static std::map<int, PendingBridgeCall> s_pendingBridgeCalls;
static int s_nextBridgeRequestId = 1;

// Take the pending call for `requestId` out of the map, or return false if it is
// unknown or its context has died. Shared by the resolve and reject paths so the
// "unknown id" and "context gone" rules cannot drift apart.
static bool TakeBridgeCall(int requestId, const char* what, PendingBridgeCall& out) {
    CEF_REQUIRE_RENDERER_THREAD();

    auto it = s_pendingBridgeCalls.find(requestId);
    if (it == s_pendingBridgeCalls.end()) {
        // Not an error: a reply arriving after its caller gave up lands here and is
        // DISCARDED rather than misrouted. That is the defect this phase exists to fix.
        LOG_DEBUG_RENDER(std::string("🌉 bridge ") + what + " for unknown requestId " +
                         std::to_string(requestId) + " — discarded, not misrouted");
        return false;
    }

    out = it->second;
    s_pendingBridgeCalls.erase(it);

    if (!out.context || !out.context->IsValid()) {
        LOG_DEBUG_RENDER(std::string("🌉 bridge ") + what + " dropped — context gone (" +
                         out.method + ", requestId " + std::to_string(requestId) + ")");
        return false;
    }
    return true;
}

// Resolve the promise for `requestId` with `json`. Called from a migrated `*_response`
// IPC arm. No-ops if the id is unknown or the page navigated away mid-request.
void ResolveBridgeCall(int requestId, const std::string& json) {
    PendingBridgeCall pending;
    if (!TakeBridgeCall(requestId, "response", pending)) return;

    pending.context->Enter();
    try {
        pending.promise->ResolvePromise(jsonToV8(nlohmann::json::parse(json)));
    } catch (const std::exception& e) {
        pending.promise->RejectPromise(pending.method + ": bad response — " + e.what());
    }
    pending.context->Exit();
}

// Reject the promise for `requestId`. The legacy bridge reported a native failure by
// invoking a separate `window.on<Method>Error` global — the same single-slot shape, and
// the same race.
void RejectBridgeCall(int requestId, const std::string& error) {
    PendingBridgeCall pending;
    if (!TakeBridgeCall(requestId, "error", pending)) return;

    pending.context->Enter();
    pending.promise->RejectPromise(pending.method + ": " + error);
    pending.context->Exit();
}

// How long a migrated bridge call may stay unanswered before it is rejected.
//
// ⚠️ Deliberately generous. Some wallet calls reach the network, and a deadline that
// kills legitimate slow work is its own defect. This is a backstop against a browser
// process that never replies — not a latency budget.
static const int64_t kBridgeCallTimeoutMs = 30000;

// Rejects one bridge call if it is still pending when the deadline fires.
//
// ⭐ Safe by construction when the reply already arrived: `TakeBridgeCall` no-ops on an
// id that is no longer in the map, so a task that fires after a successful response
// simply finds nothing. That is why this needs no cancellation and no bookkeeping.
class BridgeCallDeadlineTask : public CefTask {
public:
    explicit BridgeCallDeadlineTask(int requestId) : requestId_(requestId) {}

    void Execute() override {
        // Only a call that is STILL pending gets here in any meaningful sense — but log
        // before the no-op check so a genuine timeout is visible. A browser process that
        // stopped answering is exactly the kind of failure that must not be silent.
        if (s_pendingBridgeCalls.count(requestId_)) {
            LOG_WARNING_RENDER("🌉 bridge call " + std::to_string(requestId_) +
                               " (" + s_pendingBridgeCalls[requestId_].method +
                               ") timed out — the browser process never replied");
        }
        RejectBridgeCall(requestId_,
                         "timed out after " + std::to_string(kBridgeCallTimeoutMs / 1000) + "s");
    }

private:
    int requestId_;
    IMPLEMENT_REFCOUNTING(BridgeCallDeadlineTask);
};

// `window.hodosBrowser.bridge.<method>()` — returns a real Promise, routed by id.
//
// ⚠️ Registered on its own `bridge` object rather than onto `hodosBrowser.wallet`.
// C++ `OnContextCreated` runs BEFORE the page's JS, and `initWindowBridge.ts` guards its
// whole wallet block with `if (!window.hodosBrowser.wallet)`. Creating `wallet` here
// would make that guard fail and silently drop the other 40 methods. The TS side assigns
// these onto `wallet` instead, one line per migrated method — which is also what makes
// the migration incremental.
class WalletBridgeV8Handler : public CefV8Handler {
public:
    WalletBridgeV8Handler() {}

    // What the caller passes, if anything. Kept tiny on purpose — the remaining legacy
    // slots are overwhelmingly None or Str, with a handful of Bool.
    enum class Payload { None, Str, Bool };

    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override {
        CEF_REQUIRE_RENDERER_THREAD();

        const std::string method = name.ToString();

        // method name -> the IPC the browser process already handles, and whether the
        // call carries a payload.
        //
        // ⛔ The payload is a STRING, not an object. The caller stringifies (the legacy
        // bridge already did: `send('send_transaction', [JSON.stringify(data)])`), so the
        // wire format is unchanged and no V8→JSON conversion is needed here. Keeping the
        // shape identical is what makes this a routing change and nothing else.
        const char* ipcName = nullptr;
        Payload payload = Payload::None;
        if (method == "getStatus") {
            ipcName = "wallet_status_check";
        } else if (method == "sendTransaction") {
            ipcName = "send_transaction";
            payload = Payload::Str;
        } else if (method == "getBalance") {
            ipcName = "get_balance";
        } else if (method == "getBackupModalState") {
            ipcName = "get_backup_modal_state";
        } else if (method == "setBackupModalState") {
            ipcName = "set_backup_modal_state";
            payload = Payload::Bool;
        } else if (method == "generateAddress") {
            // Stage 3 batch 2. Backs `address.generate()`. (Its caller-less twin
            // `wallet.generateAddress`, which shared the legacy global slot pair with
            // it, was deleted in the same batch.)
            ipcName = "address_generate";
        } else if (method == "getInfo") {
            ipcName = "get_wallet_info";
        } else if (method == "markBackedUp") {
            ipcName = "mark_wallet_backed_up";
        }
        if (!ipcName) return false;  // unknown method — V8 throws for us

        if (payload == Payload::Str && (arguments.empty() || !arguments[0]->IsString())) {
            exception = method + "() requires a string argument";
            return true;
        }
        if (payload == Payload::Bool && (arguments.empty() || !arguments[0]->IsBool())) {
            exception = method + "() requires a boolean argument";
            return true;
        }

        CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
        if (!context) {
            exception = "bridge: no V8 context";
            return true;
        }

        const int requestId = s_nextBridgeRequestId++;

        CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create(ipcName);
        // ⛔ Arg 0 is ALWAYS the request id for a migrated call. The browser process
        // echoes it back in arg 0 of the response. Any payload shifts to arg 1.
        msg->GetArgumentList()->SetInt(0, requestId);
        if (payload == Payload::Str) {
            msg->GetArgumentList()->SetString(1, arguments[0]->GetStringValue());
        } else if (payload == Payload::Bool) {
            msg->GetArgumentList()->SetBool(1, arguments[0]->GetBoolValue());
        }

        retval = CefV8Value::CreatePromise();
        s_pendingBridgeCalls[requestId] = { retval, context, method };
        context->GetBrowser()->GetMainFrame()->SendProcessMessage(PID_BROWSER, msg);

        // ⛔ Arm the deadline. Without it an unanswered call hangs the caller FOREVER and
        // leaks its map entry for the life of the process — `App.tsx` awaits
        // `wallet.getStatus()` during startup, so that would be a silent boot stall.
        // The legacy per-method `setTimeout` this replaced was crude but it was a
        // backstop; dropping it without a replacement would be a regression.
        //
        // ⚠️ Renderer thread: `s_pendingBridgeCalls` is renderer-thread-only
        // (`CEF_REQUIRE_RENDERER_THREAD` in TakeBridgeCall), so the task must land there.
        CefPostDelayedTask(TID_RENDERER, new BridgeCallDeadlineTask(requestId),
                           kBridgeCallTimeoutMs);

        LOG_DEBUG_RENDER("🌉 bridge " + method + " -> " + ipcName +
                         " (requestId " + std::to_string(requestId) + ")");
        return true;
    }

private:
    IMPLEMENT_REFCOUNTING(WalletBridgeV8Handler);
};

class HistoryV8Handler : public CefV8Handler {
public:
    HistoryV8Handler() {}

    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override {

        CEF_REQUIRE_RENDERER_THREAD();

        LOG_DEBUG_RENDER("📚 history." + name.ToString() + "() called");

        CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
        if (!context) {
            exception = "history: no V8 context";
            return true;
        }

        const int requestId = s_nextHistoryRequestId++;
        CefRefPtr<CefProcessMessage> msg;
        CefRefPtr<CefListValue> args;

        auto begin = [&](const char* ipcName) {
            msg = CefProcessMessage::Create(ipcName);
            args = msg->GetArgumentList();
            args->SetInt(0, requestId);
        };

        // Optional-parameter readers matching the previous synchronous defaults.
        auto objArg = [&](size_t i) -> CefRefPtr<CefV8Value> {
            return (arguments.size() > i && arguments[i]->IsObject()) ? arguments[i] : nullptr;
        };
        auto intField = [](CefRefPtr<CefV8Value> o, const char* k, int fallback) {
            if (o && o->HasValue(k) && o->GetValue(k)->IsInt()) return o->GetValue(k)->GetIntValue();
            return fallback;
        };
        auto strField = [](CefRefPtr<CefV8Value> o, const char* k) -> std::string {
            if (o && o->HasValue(k) && o->GetValue(k)->IsString())
                return o->GetValue(k)->GetStringValue().ToString();
            return "";
        };
        // Times cross the wire as doubles: they are Chromium-epoch microseconds and do
        // not fit an int32.
        auto timeField = [](CefRefPtr<CefV8Value> o, const char* k) -> double {
            if (o && o->HasValue(k) && o->GetValue(k)->IsDouble())
                return o->GetValue(k)->GetDoubleValue();
            return 0.0;
        };

        if (name == "get") {
            CefRefPtr<CefV8Value> p = objArg(0);
            begin("history_get");
            args->SetInt(1, intField(p, "limit", 50));
            args->SetInt(2, intField(p, "offset", 0));
        }
        else if (name == "search") {
            CefRefPtr<CefV8Value> p = objArg(0);
            if (!p) { exception = "search() requires a parameters object"; return true; }
            begin("history_search");
            args->SetString(1, strField(p, "search"));
            args->SetInt(2, intField(p, "limit", 50));
            args->SetInt(3, intField(p, "offset", 0));
            args->SetDouble(4, timeField(p, "startTime"));
            args->SetDouble(5, timeField(p, "endTime"));
        }
        else if (name == "searchWithFrecency") {
            CefRefPtr<CefV8Value> p = objArg(0);
            if (!p) { exception = "searchWithFrecency() requires a parameters object with query"; return true; }
            begin("history_search_frecency");
            args->SetString(1, strField(p, "query"));
            args->SetInt(2, intField(p, "limit", 6));
        }
        else if (name == "delete") {
            if (arguments.empty() || !arguments[0]->IsString()) {
                exception = "delete() requires a URL string";
                return true;
            }
            begin("history_delete");
            args->SetString(1, arguments[0]->GetStringValue());
        }
        else if (name == "clearAll") {
            begin("history_clear_all");
        }
        else if (name == "clearRange") {
            CefRefPtr<CefV8Value> p = objArg(0);
            if (!p || !p->HasValue("startTime") || !p->HasValue("endTime")) {
                exception = "clearRange() requires a parameters object with startTime and endTime";
                return true;
            }
            begin("history_clear_range");
            args->SetDouble(1, p->GetValue("startTime")->GetDoubleValue());
            args->SetDouble(2, p->GetValue("endTime")->GetDoubleValue());
        }
        else if (name == "test") {
            begin("history_test");
            args->SetInt(1, 10);
        }
        else {
            return false;
        }

        retval = CefV8Value::CreatePromise();
        s_pendingHistory[requestId] = { retval, context };
        context->GetBrowser()->GetMainFrame()->SendProcessMessage(PID_BROWSER, msg);
        return true;
    }

private:
    IMPLEMENT_REFCOUNTING(HistoryV8Handler);
};


// ========== GOOGLE SUGGEST V8 HANDLER ==========
// Handler for window.hodosBrowser.googleSuggest API
class GoogleSuggestV8Handler : public CefV8Handler {
public:
    GoogleSuggestV8Handler() : nextRequestId_(1) {}

    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override {

        CEF_REQUIRE_RENDERER_THREAD();

        if (name == "fetch") {
            // Expect one argument: query string
            if (arguments.size() < 1 || !arguments[0]->IsString()) {
                exception = "fetch() requires one string argument (query)";
                return true;
            }

            std::string query = arguments[0]->GetStringValue();
            int requestId = nextRequestId_++;

            LOG_DEBUG_RENDER("🔍 googleSuggest.fetch() called with query: " + query + " (requestId: " + std::to_string(requestId) + ")");

            // Send IPC message to browser process with requestId
            CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create("google_suggest_request");
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            args->SetString(0, query);
            args->SetInt(1, requestId);

            CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
            CefRefPtr<CefBrowser> browser = context->GetBrowser();
            browser->GetMainFrame()->SendProcessMessage(PID_BROWSER, message);

            LOG_DEBUG_RENDER("🔍 google_suggest_request sent to browser process with requestId: " + std::to_string(requestId));

            // Return the request ID so JavaScript can match responses
            retval = CefV8Value::CreateInt(requestId);
            return true;
        }

        return false;
    }

private:
    int nextRequestId_;
    IMPLEMENT_REFCOUNTING(GoogleSuggestV8Handler);
};

SimpleRenderProcessHandler::SimpleRenderProcessHandler() {
    LOG_DEBUG_RENDER("🔧 SimpleRenderProcessHandler constructor called!");

#ifdef _WIN32
    LOG_DEBUG_RENDER("🔧 Process ID: " + std::to_string(GetCurrentProcessId()));
    LOG_DEBUG_RENDER("🔧 Thread ID: " + std::to_string(GetCurrentThreadId()));

    // The render process deliberately does NOT open the history database.
    //
    // It used to: this constructor called HistoryManager::Initialize() with the
    // profile path in every --type=renderer process, so every renderer — including
    // ones rendering arbitrary web pages — held a read/write handle on the profile's
    // history SQLite DB. That is precisely what the Chromium sandbox is meant to
    // prevent, so it blocked enabling the sandbox, and it was a standing security
    // problem in its own right.
    //
    // history.* is now serviced by the browser process over IPC (see
    // HistoryV8Handler above and the history_* handlers in simple_handler.cpp),
    // which already owns a correctly profile-bound HistoryManager. That also fixes
    // macOS, where this init never ran at all and history silently returned empty.
    //
    // The per-profile plumbing this replaced is not lost: the browser process resolves
    // the profile itself, so --profile= no longer has to be re-derived here.
#endif
}

void SimpleRenderProcessHandler::OnContextCreated(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefRefPtr<CefV8Context> context) {

    CEF_REQUIRE_RENDERER_THREAD();

    LOG_DEBUG_RENDER("🔧 OnContextCreated called for browser ID: " + std::to_string(browser->GetIdentifier()));
    LOG_DEBUG_RENDER("🔧 Frame URL: " + hodos::LogSafeUrl(frame->GetURL().ToString()));
#ifdef _WIN32
    LOG_DEBUG_RENDER("🔧 Process ID: " + std::to_string(GetCurrentProcessId()));
    LOG_DEBUG_RENDER("🔧 Thread ID: " + std::to_string(GetCurrentThreadId()));
#endif
    LOG_DEBUG_RENDER("🔧 RENDER PROCESS HANDLER IS WORKING!");
    LOG_DEBUG_RENDER("🔧 THIS IS THE RENDER PROCESS HANDLER!");

    // 8e-2: Inject pre-cached scriptlets IMMEDIATELY — before any page JS runs.
    // This is the earliest possible injection point in CEF.
    std::string url = frame->GetURL().ToString();
    if (!url.empty() && url.find("127.0.0.1") == std::string::npos) {
        std::lock_guard<std::mutex> lock(s_scriptCacheMutex);
        auto it = s_scriptCache.find(url);
        if (it != s_scriptCache.end() && !it->second.empty()) {
            LOG_INFO_RENDER("💉 OnContextCreated: injecting scriptlets for " + hodos::LogSafeUrl(url) +
                " (" + std::to_string(it->second.size()) + " chars)");
            frame->ExecuteJavaScript(it->second, url, 0);
            s_scriptCache.erase(it); // One-shot: don't re-inject on subframe contexts
        }
    }

    // Fingerprint farbling used to be INJECTED HERE as FINGERPRINT_PROTECTION_SCRIPT.
    // Deleted 2026-08-09: canvas went native in C3, and WebGL readPixels + WebAudio in
    // C4/C5, so nothing was left for the script to wrap. Navigator values are C6.
    //
    // Farbling now happens inside Blink at API-call time, which is why the patched methods
    // report "[native code]" from toString() again -- the injected overrides were a
    // prototype-tamper tell that bot detection reads, and removing them is a large part of
    // why the migration was worth doing.
    //
    // ⛔ Do NOT re-add an injection here for a "quick" farbling fix. Injection cannot cover
    // workers (OnContextCreated never fires for them), it re-introduces the toString tell,
    // and a JS override wrapping an API Blink already farbles would double-perturb it.
    // Inject window.chrome stub on external pages so bot detection sees a real Chrome signal.
    // Injected separately from fingerprint script so it works even when FP protection is disabled.
    // P0.5 — a FOURTH gate of the same family, found by running the fix for the other
    // three (contract section 4c). This was an unanchored search of the whole URL, so
    // https://example.com/?x=127.0.0.1:5137 read as loopback. Once the internal gates
    // became prefix matches that page was neither internal NOR external and fell
    // through both branches, losing the dApp shim entirely. Anchored, the two
    // classifications are true complements again.
    bool isExternalPage = !url.empty() &&
        !hodos::IsLoopbackUrl(url) &&
        !hodos::IsInternalFrontendUrl(url);
    if (isExternalPage) {
        std::string chromeStub = R"JS(
(function() {
    'use strict';
    if (typeof window.chrome === 'undefined') {
        window.chrome = {
            runtime: {
                connect: function() { return {}; },
                sendMessage: function() {},
                onMessage: { addListener: function() {}, removeListener: function() {} },
                id: undefined
            },
            loadTimes: function() { return {}; },
            csi: function() { return {}; }
        };
    }
})();
)JS";
        frame->ExecuteJavaScript(chromeStub, url, 0);
    }

    // Check if this is an overlay browser (any browser that's not the main root browser)
    bool isMainBrowser = (url == "http://127.0.0.1:5137" || url == "http://127.0.0.1:5137/");
    // P0.5-G2: prefix match, not substring. These two flags gate the privileged V8
    // surface below — hodosBrowser.identity / .navigation / .history and the
    // WALLET_CALL_BRIDGE / CWI shim. Measured 2026-08-19: with the old find(), a page
    // at https://example.com/?x=127.0.0.1:5137 received all three; the same page
    // without the query string received none.
    bool isOverlayBrowser = !isMainBrowser && hodos::IsInternalFrontendUrl(url);
    bool isOmniboxOverlay = (url.find("/omnibox") != std::string::npos);
    bool isInternalPage = hodos::IsInternalFrontendUrl(url);

    if (isOverlayBrowser) {
        LOG_DEBUG_RENDER("🎯 OVERLAY BROWSER V8 CONTEXT CREATED!");
        LOG_DEBUG_RENDER("🎯 URL: " + url);
        LOG_DEBUG_RENDER("🎯 Setting up hodosBrowser for overlay browser");
        if (isOmniboxOverlay) {
            LOG_DEBUG_RENDER("🔍 Detected omnibox overlay - will inject overlay.close()");
        }
    }

    CefRefPtr<CefV8Value> global = context->GetGlobal();

    // Create the hodosBrowser object — available on all pages for BRC-100 protocol.
    // On external pages, only expose brc100 sub-object to minimize fingerprint surface.
    CefRefPtr<CefV8Value> hodosBrowser = CefV8Value::CreateObject(nullptr, nullptr);
    global->SetValue("hodosBrowser", hodosBrowser, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Expose the host OS to React so we can conditionally render Windows vs
    // macOS chrome (traffic lights vs our own min/max/close, tab bar padding,
    // etc.). Always injected, even on external pages — it's a bare string and
    // not fingerprint-sensitive beyond what the user-agent already reveals.
#if defined(__APPLE__)
    hodosBrowser->SetValue("platform",
        CefV8Value::CreateString("macos"),
        V8_PROPERTY_ATTRIBUTE_READONLY);
#else
    hodosBrowser->SetValue("platform",
        CefV8Value::CreateString("windows"),
        V8_PROPERTY_ATTRIBUTE_READONLY);
#endif

    // Identity, navigation, address, history, overlay APIs — internal pages only.
    // External pages get only BRC-100 + cefMessage (injected below).
    if (isInternalPage || isOverlayBrowser) {

    // Create the identity object inside hodosBrowser
    CefRefPtr<CefV8Value> identityObject = CefV8Value::CreateObject(nullptr, nullptr);
    hodosBrowser->SetValue("identity", identityObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Bind the IdentityHandler instance
    CefRefPtr<IdentityHandler> identityHandler = new IdentityHandler();

    identityObject->SetValue("get",
        CefV8Value::CreateFunction("get", identityHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

    identityObject->SetValue("markBackedUp",
        CefV8Value::CreateFunction("markBackedUp", identityHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

#ifdef _WIN32
    // Create the navigation object inside hodosBrowser (Windows-only)
    CefRefPtr<CefV8Value> navigationObject = CefV8Value::CreateObject(nullptr, nullptr);
    hodosBrowser->SetValue("navigation", navigationObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Bind the NavigationHandler instance
    CefRefPtr<NavigationHandler> navigationHandler = new NavigationHandler();

    navigationObject->SetValue("navigate",
        CefV8Value::CreateFunction("navigate", navigationHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
#else
    // macOS: Navigation handler is cross-platform, inject it
    LOG_DEBUG_RENDER("🔧 macOS: Injecting navigation API...");

    CefRefPtr<CefV8Value> navigationObject = CefV8Value::CreateObject(nullptr, nullptr);
    if (!navigationObject) {
        LOG_ERROR_RENDER("❌ Failed to create navigationObject!");
    } else {
        LOG_DEBUG_RENDER(LogFmt() << "✅ navigationObject created");
    }

    bool setResult = hodosBrowser->SetValue("navigation", navigationObject, V8_PROPERTY_ATTRIBUTE_READONLY);
    LOG_DEBUG_RENDER(LogFmt() << "🔧 SetValue('navigation') result: " << setResult);

    CefRefPtr<NavigationHandler> navHandler = new NavigationHandler();
    CefRefPtr<CefV8Value> navFunction = CefV8Value::CreateFunction("navigate", navHandler);

    bool setFuncResult = navigationObject->SetValue("navigate", navFunction, V8_PROPERTY_ATTRIBUTE_NONE);
    LOG_DEBUG_RENDER(LogFmt() << "🔧 SetValue('navigate' function) result: " << setFuncResult);

    LOG_DEBUG_RENDER("✅ Navigation API injection completed on macOS");
    LOG_DEBUG_RENDER(LogFmt() << "✅ Navigation API injection completed on macOS");
#endif

    // overlayPanel object removed - now using process-per-overlay architecture

    // Create the overlay object (for overlay browsers only)
    if (isOverlayBrowser) {
        LOG_DEBUG_RENDER("🎯 Creating overlay object for URL: " + url);

        CefRefPtr<CefV8Value> overlayObject = CefV8Value::CreateObject(nullptr, nullptr);
        hodosBrowser->SetValue("overlay", overlayObject, V8_PROPERTY_ATTRIBUTE_READONLY);

        // Add close method for overlay browsers
        // Omnibox overlay sends "omnibox_hide", other overlays send "overlay_close"
        if (isOmniboxOverlay) {
            overlayObject->SetValue("close",
                CefV8Value::CreateFunction("close", new OmniboxCloseHandler()),
                V8_PROPERTY_ATTRIBUTE_NONE);
            LOG_DEBUG_RENDER("🔍 Omnibox overlay.close() injected (sends omnibox_hide)");
        } else {
            overlayObject->SetValue("close",
                CefV8Value::CreateFunction("close", new OverlayCloseHandler()),
                V8_PROPERTY_ATTRIBUTE_NONE);
        }

        LOG_DEBUG_RENDER("🎯 Overlay object created with close method");
    } else {
        LOG_DEBUG_RENDER("🎯 NOT creating overlay object for URL: " + url);
        LOG_DEBUG_RENDER("🎯 isMainBrowser: " + std::string(isMainBrowser ? "true" : "false"));
    }

#ifdef _WIN32
    // Create the address object (Windows)
    CefRefPtr<CefV8Value> addressObject = CefV8Value::CreateObject(nullptr, nullptr);
    hodosBrowser->SetValue("address", addressObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Bind AddressHandler
    CefRefPtr<AddressHandler> addressHandler = new AddressHandler();
    addressObject->SetValue("generate",
        CefV8Value::CreateFunction("generate", addressHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
#else
    // macOS: AddressHandler is cross-platform now
    CefRefPtr<CefV8Value> addressObject = CefV8Value::CreateObject(nullptr, nullptr);
    hodosBrowser->SetValue("address", addressObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    CefRefPtr<AddressHandler> addressHandler = new AddressHandler();

    addressObject->SetValue("generate",
        CefV8Value::CreateFunction("generate", addressHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

    addressObject->SetValue("getAll",
        CefV8Value::CreateFunction("getAll", addressHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

    addressObject->SetValue("getCurrent",
        CefV8Value::CreateFunction("getCurrent", addressHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

    LOG_DEBUG_RENDER("✅ Address API enabled on macOS");
#endif

    // Create the history object (cross-platform)
    LOG_DEBUG_RENDER("📚 Creating history object for V8 context");
    CefRefPtr<CefV8Value> historyObject = CefV8Value::CreateObject(nullptr, nullptr);
    // Phase 8c stage 1 — per-request-id wallet bridge. See WalletBridgeV8Handler for
    // why this is its own object rather than `hodosBrowser.wallet`.
    CefRefPtr<CefV8Value> bridgeObject = CefV8Value::CreateObject(nullptr, nullptr);
    CefRefPtr<CefV8Handler> bridgeHandler = new WalletBridgeV8Handler();
    bridgeObject->SetValue("getStatus",
        CefV8Value::CreateFunction("getStatus", bridgeHandler),
        V8_PROPERTY_ATTRIBUTE_READONLY);
    // Stage 2 — the money path. ⛔ `sendTransaction` must NEVER be deduped the way
    // `getBalance` is: two sends are two payments, and collapsing them would be worse
    // than the race this replaces.
    bridgeObject->SetValue("sendTransaction",
        CefV8Value::CreateFunction("sendTransaction", bridgeHandler),
        V8_PROPERTY_ATTRIBUTE_READONLY);
    // Stage 3 batch 1 — the three remaining SHAPES: a read that carried an in-flight
    // dedupe workaround, the resolve-on-timeout offender, and a non-string payload.
    bridgeObject->SetValue("getBalance",
        CefV8Value::CreateFunction("getBalance", bridgeHandler),
        V8_PROPERTY_ATTRIBUTE_READONLY);
    bridgeObject->SetValue("getBackupModalState",
        CefV8Value::CreateFunction("getBackupModalState", bridgeHandler),
        V8_PROPERTY_ATTRIBUTE_READONLY);
    bridgeObject->SetValue("setBackupModalState",
        CefV8Value::CreateFunction("setBackupModalState", bridgeHandler),
        V8_PROPERTY_ATTRIBUTE_READONLY);
    // Stage 3 batch 2 — the wallet namespace's live remainder. `generateAddress` backs
    // `address.generate()`.
    bridgeObject->SetValue("generateAddress",
        CefV8Value::CreateFunction("generateAddress", bridgeHandler),
        V8_PROPERTY_ATTRIBUTE_READONLY);
    bridgeObject->SetValue("getInfo",
        CefV8Value::CreateFunction("getInfo", bridgeHandler),
        V8_PROPERTY_ATTRIBUTE_READONLY);
    bridgeObject->SetValue("markBackedUp",
        CefV8Value::CreateFunction("markBackedUp", bridgeHandler),
        V8_PROPERTY_ATTRIBUTE_READONLY);
    hodosBrowser->SetValue("bridge", bridgeObject, V8_PROPERTY_ATTRIBUTE_READONLY);
    LOG_DEBUG_RENDER("🌉 Bound WalletBridgeV8Handler (8 methods migrated)");

    hodosBrowser->SetValue("history", historyObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Bind HistoryV8Handler
    LOG_DEBUG_RENDER("📚 Binding HistoryV8Handler functions");
    CefRefPtr<HistoryV8Handler> historyHandler = new HistoryV8Handler();
    historyObject->SetValue("get",
        CefV8Value::CreateFunction("get", historyHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
    historyObject->SetValue("search",
        CefV8Value::CreateFunction("search", historyHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
    historyObject->SetValue("searchWithFrecency",
        CefV8Value::CreateFunction("searchWithFrecency", historyHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
    historyObject->SetValue("delete",
        CefV8Value::CreateFunction("delete", historyHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
    historyObject->SetValue("clearAll",
        CefV8Value::CreateFunction("clearAll", historyHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
    historyObject->SetValue("clearRange",
        CefV8Value::CreateFunction("clearRange", historyHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
    historyObject->SetValue("test",
        CefV8Value::CreateFunction("test", historyHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

    LOG_DEBUG_RENDER("📚 History object created with " + std::to_string(7) + " functions");

    } // end isInternalPage || isOverlayBrowser — external pages only get BRC-100 + cefMessage

    // Create the cefMessage object for process communication
    CefRefPtr<CefV8Value> cefMessageObject = CefV8Value::CreateObject(nullptr, nullptr);
    global->SetValue("cefMessage", cefMessageObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Create the send function for cefMessage
    CefRefPtr<CefV8Value> sendFunction = CefV8Value::CreateFunction("send", new CefMessageSendHandler());
    cefMessageObject->SetValue("send", sendFunction, V8_PROPERTY_ATTRIBUTE_NONE);

    // Inject Google Suggest API for omnibox overlay only
    if (isOmniboxOverlay) {
        CefRefPtr<CefV8Value> googleSuggestObject = CefV8Value::CreateObject(nullptr, nullptr);
        hodosBrowser->SetValue("googleSuggest", googleSuggestObject, V8_PROPERTY_ATTRIBUTE_READONLY);

        CefRefPtr<GoogleSuggestV8Handler> googleSuggestHandler = new GoogleSuggestV8Handler();
        googleSuggestObject->SetValue("fetch",
            CefV8Value::CreateFunction("fetch", googleSuggestHandler),
            V8_PROPERTY_ATTRIBUTE_NONE);

        LOG_DEBUG_RENDER("🔍 Google Suggest API injected for omnibox overlay");
    }

    // (Removed) Legacy BRC-100 V8 bindings (BRC100Handler/BRC100Bridge) — a dead path
    // that did synchronous WinHTTP on the render thread (its only caller was a startup
    // probe that just logged). The live BRC-100 surfaces are the window.CWI shim (below)
    // and the Phase 2.5 IPC auth bridge; wallet UI calls the Rust API directly.

    // Phase 2 Steps 1 + 2 — inject window.CWI / window.yours / window.panda shim.
    //
    // Gating cascade (each rejection reason logged separately for debuggability):
    //   1. External pages only — Hodos internal UI doesn't need a wallet provider object.
    //   2. Main frame only — skip all iframes. Matches Yours Wallet + Brave default;
    //      verified against the live Yours brc100-remote manifest, which omits
    //      "all_frames": true and so confines content_scripts to top frames.
    //   3. Secure context (https://) only — matches Brave's "no provider on insecure pages"
    //      posture. http:// external pages don't get the shim; localhost is already
    //      excluded by isExternalPage. Developers testing locally should serve dApps via
    //      https (mkcert, ngrok, etc.).
    //   4. TODO (future sprint) — Hodos has no private/incognito browsing mode today.
    //      When one is added, gate the shim here. Per Brave's posture, private windows
    //      receive NO wallet provider injection at all (matches BRAVE_WALLET_REFERENCE.md
    //      §6 — "No injection in Private or Tor windows. Period.").
    //
    // Each shim method POSTs to http://127.0.0.1:31301/<methodName>, captured by
    // HttpRequestInterceptor::isWalletEndpoint() and routed through PermissionEngine —
    // identical gating to canonical BRC-100 calls. No bypass paths.
    // Wallet IPC transport bridge (window.__hodos_walletCall / __hodos_walletResponse).
    // Bridge migration: the first-party wallet UI (internal pages + overlays) now
    // routes its wallet calls through this bridge instead of direct fetch to the
    // Rust port, so C++ owns the port and gates by frame origin. Idempotent IIFE.
    if (isInternalPage || isOverlayBrowser) {
        frame->ExecuteJavaScript(WALLET_CALL_BRIDGE_SCRIPT, url, 0);
    }

    if (isExternalPage) {
        if (!frame->IsMain()) {
            LOG_DEBUG_RENDER("⏭️ Phase 2 shim skipped (iframe, not main frame) for " + url);
        } else if (url.find("https://") != 0) {
            LOG_DEBUG_RENDER("⏭️ Phase 2 shim skipped (insecure context, not https://) for " + url);
        } else {
            // dApp page: inject the transport bridge FIRST (the provider's methods
            // call window.__hodos_walletCall), then the window.CWI/yours/panda provider.
            frame->ExecuteJavaScript(WALLET_CALL_BRIDGE_SCRIPT, url, 0);
            LOG_INFO_RENDER("💉 Injecting window.CWI / window.yours / window.panda shim for " + hodos::LogSafeUrl(url));
            frame->ExecuteJavaScript(CWI_SHIM_SCRIPT, url, 0);
        }
    }

    // For overlay browsers, signal that all systems are ready
    if (isOverlayBrowser) {
        std::string js = R"(
            console.log("🎯 All systems ready - V8 context created, APIs injected");
            // Set a flag that all systems are ready
            window.allSystemsReady = true;
            // Dispatch a custom event to signal all systems are ready
            window.dispatchEvent(new CustomEvent('allSystemsReady'));
            console.log("🎯 allSystemsReady event dispatched");
        )";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        LOG_DEBUG_RENDER("🎯 All systems ready - V8 context created, APIs injected");
    }
}

bool SimpleRenderProcessHandler::OnProcessMessageReceived(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefProcessId source_process,
    CefRefPtr<CefProcessMessage> message) {

    CEF_REQUIRE_RENDERER_THREAD();

    std::string message_name = message->GetName();
    LOG_DEBUG_RENDER(LogFmt() << "📨 Render process received message: " << message_name);
    LOG_DEBUG_RENDER(LogFmt() << "🔍 Browser ID: " << browser->GetIdentifier());
    LOG_DEBUG_RENDER(LogFmt() << "🔍 Frame URL: " << hodos::LogSafeUrl(frame->GetURL().ToString()));
    LOG_DEBUG_RENDER(LogFmt() << "🔍 Source Process: " << source_process);

        if (message_name == "tab_list_response") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string tabListJson = args->GetString(0);

            LOG_DEBUG_RENDER("📑 Tab list response received, dispatching to React");

            // F6: route through the canonical encoder. The old ad-hoc escaper
            // handled only `\` and `"` and MISSED `'` — an apostrophe in a tab
            // title broke out of the single-quoted literal below.
            std::string escaped_json = escapeJsonForJs(tabListJson);

            // Send message to React component
            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'tab_list_response',
                        data: ')" + escaped_json + R"('
                    }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        // ========== FIND IN PAGE ==========
        if (message_name == "find_show") {
            LOG_DEBUG_RENDER("🔍 find_show received, dispatching to React");
            std::string js = R"(
                console.log('[CEF] Executing find_show JS in frame');
                window.postMessage({ type: 'find_show' }, '*');
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        if (message_name == "focus_address_bar") {
            LOG_DEBUG_RENDER("⌨️ focus_address_bar received, dispatching to React");
            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: { type: 'focus_address_bar' }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        if (message_name == "find_result") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string resultJson = args->GetString(0);

            LOG_DEBUG_RENDER("🔍 find_result received, dispatching to React");

            std::string escaped = escapeJsonForJs(resultJson);
            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'find_result',
                        data: ')" + escaped + R"('
                    }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        if (message_name == "qr_scan_result") {
            // QR scan results forwarded from the active page back to the wallet overlay.
            // The JSON is a stringified array produced by our own scanner script (not user input).
            std::string json = message->GetArgumentList()->GetString(0).ToString();
            LOG_INFO_RENDER("📷 qr_scan_result dispatching to React (" + std::to_string(json.size()) + " chars)");

            std::string js = "window.dispatchEvent(new MessageEvent('message',{data:{type:'qr_scan_result',data:" + json + "}}));";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        // Phase 2: Screen capture starting notification
        if (message_name == "qr_screen_capture_starting") {
            LOG_INFO_RENDER("📷 qr_screen_capture_starting — notifying React");
            std::string js = "window.dispatchEvent(new MessageEvent('message',{data:{type:'qr_screen_capture_starting'}}));";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        // Phase 2: Screen capture result delivery
        if (message_name == "qr_screen_capture_result") {
            std::string json = message->GetArgumentList()->GetString(0).ToString();
            LOG_INFO_RENDER("📷 qr_screen_capture_result dispatching to React (" + std::to_string(json.size()) + " chars)");
            std::string js = "window.dispatchEvent(new MessageEvent('message',{data:{type:'qr_screen_capture_result',data:" + json + "}}));";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        if (message_name == "download_state_update") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string downloadsJson = args->GetString(0);

            LOG_DEBUG_RENDER("📥 Download state update received, dispatching to React");

            std::string escaped = escapeJsonForJs(downloadsJson);
            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'download_state_update',
                        data: ')" + escaped + R"('
                    }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        // ========== PAYMENT SUCCESS INDICATOR (tab badge) ==========
        if (message_name == "payment_success_indicator") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string payloadJson = args->GetString(0);

            LOG_DEBUG_RENDER("💰 Payment success indicator received, dispatching to React");

            std::string escaped = escapeJsonForJs(payloadJson);
            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'payment_success_indicator',
                        data: ')" + escaped + R"('
                    }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        // ========== WALLET IPC BRIDGE RESPONSE (Phase 2.5) ==========
        // Pair to the `wallet_call` IPC dispatched by simple_handler.cpp.
        // Args: [requestId, ok, payloadJson] — see PHASE_2_5_IPC_REFACTOR.md.
        // Forwarded into window.__hodos_walletResponse so the shim's promise
        // correlation table can resolve / reject the matching pending call.
        if (message_name == "wallet_response") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            if (args->GetSize() < 3) {
                LOG_WARNING_RENDER("wallet_response missing args (need 3)");
                return true;
            }
            std::string requestId  = args->GetString(0).ToString();
            bool ok                = args->GetBool(1);
            std::string payloadJson = args->GetString(2).ToString();

            // escapeJsonForJs is single-quote-safe; we wrap with single quotes
            // on both args. requestId is a numeric string so escape is a no-op
            // in the common case, but we run it for safety.
            std::string escapedReqId   = escapeJsonForJs(requestId);
            std::string escapedPayload = escapeJsonForJs(payloadJson);

            std::string js = std::string("if (window.__hodos_walletResponse) { ") +
                "window.__hodos_walletResponse('" + escapedReqId + "', " +
                (ok ? "true" : "false") + ", '" + escapedPayload + "'); }";

            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        // ========== HISTORY RESPONSE ==========
        // Reply to any of the seven history_* requests. Resolves the JS Promise that
        // HistoryV8Handler::Execute() handed back, matched on requestId.
        if (message_name == "history_response") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            if (args->GetSize() < 2) {
                LOG_WARNING_RENDER("history_response missing args (need 2)");
                return true;
            }
            ResolveHistoryRequest(args->GetInt(0), args->GetString(1).ToString());
            return true;
        }

        // ========== WALLET IPC BRIDGE RESPONSE — CHUNKED (large payloads) ==========
        // Pair to the browser-process chunking in sendWalletResponseIpc. Each chunk
        // is forwarded to window.__hodos_walletResponseChunk with its framing
        // {index, total, totalByteLength}; the JS reassembler resolves the pending
        // promise only when the response is complete + length-verified, else rejects.
        // Each chunk's escaped source stays bounded (~256 KB), so no single
        // ExecuteJavaScript compiles the whole multi-MB body.
        if (message_name == "wallet_response_chunk") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            if (args->GetSize() < 6) {
                LOG_WARNING_RENDER("wallet_response_chunk missing args (need 6)");
                return true;
            }
            std::string requestId = args->GetString(0).ToString();
            bool ok               = args->GetBool(1);
            int chunkIndex        = args->GetInt(2);
            int totalChunks       = args->GetInt(3);
            std::string totalLen  = args->GetString(4).ToString();  // decimal byte count
            std::string chunkData = args->GetString(5).ToString();

            std::string escapedReqId = escapeJsonForJs(requestId);
            std::string escapedChunk = escapeJsonForJs(chunkData);
            // totalLen is a C++-generated decimal string — safe to embed as a number.
            std::string js = std::string("if (window.__hodos_walletResponseChunk) { ") +
                "window.__hodos_walletResponseChunk('" + escapedReqId + "', " +
                (ok ? "true" : "false") + ", " + std::to_string(chunkIndex) + ", " +
                std::to_string(totalChunks) + ", " + totalLen + ", '" + escapedChunk + "'); }";

            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        // ========== DOWNLOAD FOLDER PICKER RESULT ==========
        if (message_name == "download_folder_selected") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string path = args->GetString(0);
            std::string escaped = escapeJsonForJs(path);

            LOG_DEBUG_RENDER("📂 download_folder_selected: " + path);

            std::string js = "if (window.onDownloadFolderSelected) { window.onDownloadFolderSelected('" + escaped + "'); }";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        // ========== NEW TAB PAGE (G4) ==========

        if (message_name == "most_visited_response") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string jsonStr = args->GetString(0);
            std::string escaped = escapeJsonForJs(jsonStr);

            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'most_visited_response',
                        data: ')" + escaped + R"('
                    }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        if (message_name == "session_blocked_total_response") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string jsonStr = args->GetString(0);
            std::string escaped = escapeJsonForJs(jsonStr);

            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'session_blocked_total_response',
                        data: ')" + escaped + R"('
                    }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        // ========== COSMETIC FILTERING (Sprint 8e) ==========

        // 8e-2: Pre-cache scriptlets for early injection in OnContextCreated
        if (message_name == "preload_cosmetic_script") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string url = args->GetString(0);
            std::string script = args->GetString(1);

            if (!url.empty() && !script.empty()) {
                std::lock_guard<std::mutex> lock(s_scriptCacheMutex);
                s_scriptCache[url] = script;
                LOG_INFO_RENDER("💉 Pre-cached scriptlets for " + hodos::LogSafeUrl(url) +
                    " (" + std::to_string(script.size()) + " chars)");
            }
            return true;
        }

        // The "fingerprint_seed" and "fingerprint_site_disabled" receivers were DELETED
        // 2026-08-09 with the rest of the JS farbling path. Their senders in
        // simple_handler.cpp :: OnBeforeBrowse went in the same commit, so no message with
        // either name is emitted any more.
        //
        // Their replacement is not another IPC message: libcef intercepts
        // "hodos_farble_key" browser-side, files it in hodos::FarblingRegistry, and the
        // renderer PULLS it synchronously at OnContextCreated. A push could not work --
        // pre-commit it reaches the OUTGOING document, and post-commit it is queued behind
        // the FrameAttached ack, which lands after the first inline script has run.

        if (message_name == "inject_cosmetic_css") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string selectors = args->GetString(0);

            if (!selectors.empty()) {
                LOG_DEBUG_RENDER("🎨 Injecting cosmetic CSS (" + std::to_string(selectors.size()) + " chars)");

                // Escape selectors for safe JS string embedding
                std::string escaped;
                escaped.reserve(selectors.size() + 64);
                for (char c : selectors) {
                    switch (c) {
                        case '\\': escaped += "\\\\"; break;
                        case '\'': escaped += "\\'"; break;
                        case '\n': escaped += "\\n"; break;
                        case '\r': escaped += "\\r"; break;
                        default: escaped += c; break;
                    }
                }

                // Inject or append to <style> tag to hide matched elements
                std::string js = R"(
                    (function() {
                        var rule = ')" + escaped + R"( { display: none !important; }';
                        var existing = document.getElementById('hodos-cosmetic-css');
                        if (existing) {
                            existing.textContent += '\n' + rule;
                        } else {
                            var style = document.createElement('style');
                            style.id = 'hodos-cosmetic-css';
                            style.textContent = rule;
                            (document.head || document.documentElement).appendChild(style);
                        }
                    })();
                )";
                frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            }
            return true;
        }

        if (message_name == "inject_cosmetic_script") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string script = args->GetString(0);

            if (!script.empty()) {
                LOG_DEBUG_RENDER("💉 Injecting cosmetic scriptlets (" + std::to_string(script.size()) + " chars)");

                // Execute scriptlets directly — they are self-contained JS from adblock engine
                frame->ExecuteJavaScript(script, "about:blank", 0);
            }
            return true;
        }

        if (message_name == "brc100_auth_request") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string domain = args->GetString(0);
            std::string method = args->GetString(1);
            std::string endpoint = args->GetString(2);
            std::string body = args->GetString(3);
            std::string notifType = (args->GetSize() >= 6) ? args->GetString(5).ToString() : "domain_approval";

            LOG_DEBUG_RENDER("🔐 BRC-100 auth request received: " + domain + " type=" + notifType);

            // Send message to React component
            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'brc100_auth_request',
                        payload: {
                            domain: ')" + escapeJsonForJs(domain) + R"(',
                            method: ')" + escapeJsonForJs(method) + R"(',
                            endpoint: ')" + escapeJsonForJs(endpoint) + R"(',
                            body: ')" + escapeJsonForJs(body) + R"(',
                            notificationType: ')" + escapeJsonForJs(notifType) + R"('
                        }
                    }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

    // MIGRATED (Phase 8c stage 3 batch 2). Args: 0 = requestId, 1 = address JSON.
    //
    // ⚠️ Two JS entry points used to share this IPC — `hodosBrowser.address.generate()`
    // (the live one, behind WalletPanel's receive flow) and `wallet.generateAddress()` —
    // and under the legacy bridge they also shared ONE global slot pair, so a call
    // through either could steal the other's reply. The caller-less twin was deleted;
    // `address.generate()` resolves through `bridge.generateAddress`.
    //
    // This file used to carry this arm TWICE (the second copy unreachable); the
    // duplicate was removed with the migration rather than migrated in parallel.
    if (message_name == "address_generate_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (!args || args->GetSize() < 2) {
            LOG_ERROR_RENDER(LogFmt() << "address_generate_response missing args (need 2)");
            return true;
        }
        ResolveBridgeCall(args->GetInt(0), args->GetString(1).ToString());
        return true;
    }

    if (message_name == "identity_status_check_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        LOG_DEBUG_RENDER(LogFmt() << "✅ Identity status check response received: " << responseJson);
        LOG_DEBUG_RENDER(LogFmt() << "🔍 Browser ID: " << browser->GetIdentifier());
        LOG_DEBUG_RENDER(LogFmt() << "🔍 Frame URL: " << hodos::LogSafeUrl(frame->GetURL().ToString()));

        // Execute JavaScript to dispatch the response event
        std::string js = "window.dispatchEvent(new CustomEvent('cefMessageResponse', { detail: { message: 'identity_status_check_response', args: ['" + responseJson + "'] } }));";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "create_identity_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        LOG_DEBUG_RENDER(LogFmt() << "✅ Create identity response received: " << responseJson);
        LOG_DEBUG_RENDER(LogFmt() << "🔍 Browser ID: " << browser->GetIdentifier());
        LOG_DEBUG_RENDER(LogFmt() << "🔍 Frame URL: " << hodos::LogSafeUrl(frame->GetURL().ToString()));

        // Execute JavaScript to dispatch the response event
        std::string js = "window.dispatchEvent(new CustomEvent('cefMessageResponse', { detail: { message: 'create_identity_response', args: ['" + responseJson + "'] } }));";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "mark_identity_backed_up_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        LOG_DEBUG_RENDER(LogFmt() << "✅ Mark identity backed up response received: " << responseJson);
        LOG_DEBUG_RENDER(LogFmt() << "🔍 Browser ID: " << browser->GetIdentifier());
        LOG_DEBUG_RENDER(LogFmt() << "🔍 Frame URL: " << hodos::LogSafeUrl(frame->GetURL().ToString()));

        // Execute JavaScript to dispatch the response event
        std::string js = "window.dispatchEvent(new CustomEvent('cefMessageResponse', { detail: { message: 'mark_identity_backed_up_response', args: ['" + responseJson + "'] } }));";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    // MIGRATED (Phase 8c stage 3 batch 2). Args: 0 = requestId, 1 = error message.
    //
    // ⭐ Retires the last LIVE JS-injection site of this family: the old arm pasted the
    // wallet's error text UNESCAPED into `window.onAddressError('…')`. No JavaScript is
    // built here any more, so the hazard is deleted rather than escaped.
    if (message_name == "address_generate_error") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (!args || args->GetSize() < 2) {
            LOG_ERROR_RENDER(LogFmt() << "address_generate_error missing args (need 2)");
            return true;
        }
        RejectBridgeCall(args->GetInt(0), args->GetString(1).ToString());
        return true;
    }

    // MIGRATED (Phase 8c stage 2 — the money path). Routed by request id.
    // Args: 0 = requestId (echoed by the browser process), 1 = result JSON.
    if (message_name == "send_transaction_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (!args || args->GetSize() < 2) {
            LOG_ERROR_RENDER(LogFmt() << "send_transaction_response missing args (need 2: requestId, json)");
            return true;
        }
        const int requestId = args->GetInt(0);
        const std::string responseJson = args->GetString(1).ToString();

        LOG_DEBUG_RENDER(LogFmt() << "Send transaction response (requestId " << requestId
                                  << ", length " << responseJson.length() << ")");
        ResolveBridgeCall(requestId, responseJson);
        return true;
    }

    // MIGRATED (Phase 8c stage 2). Args: 0 = requestId, 1 = error JSON.
    //
    // ⭐ This arm also RETIRES A JS-INJECTION SITE. It used to build
    //   "if (window.onSendTransactionError) { window.onSendTransactionError('" + errorMessage + "'); }"
    // with the message pasted UNESCAPED into a single-quoted JS literal — a wallet error
    // containing a quote would have broken out of it. Routing by id means no JavaScript
    // is constructed at all, so the hazard is deleted rather than escaped.
    if (message_name == "send_transaction_error") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (!args || args->GetSize() < 2) {
            LOG_ERROR_RENDER(LogFmt() << "send_transaction_error missing args (need 2: requestId, error)");
            return true;
        }
        const int requestId = args->GetInt(0);
        const std::string errorMessage = args->GetString(1).ToString();

        LOG_DEBUG_RENDER(LogFmt() << "Send transaction error (requestId " << requestId
                                  << "): " << errorMessage);
        RejectBridgeCall(requestId, errorMessage);
        return true;
    }

    // MIGRATED (Phase 8c stage 3 batch 1). Args: 0 = requestId, 1 = payload.
    //
    // ⭐ This one also RETIRES A WORKAROUND. getBalance carried an in-flight dedupe
    // (P2a) because a single global slot could not tell two callers apart. Per-request
    // routing makes the dedupe unnecessary: two concurrent reads now get two correct
    // answers instead of sharing one. ⛔ The dedupe was only ever sound because a balance
    // read is idempotent — it must NOT be copied to any write.
    if (message_name == "get_balance_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (!args || args->GetSize() < 2) {
            LOG_ERROR_RENDER(LogFmt() << "get_balance_response missing args (need 2)");
            return true;
        }
        ResolveBridgeCall(args->GetInt(0), args->GetString(1).ToString());
        return true;
    }

    // MIGRATED (Phase 8c stage 3 batch 1). Args: 0 = requestId, 1 = payload.
    if (message_name == "get_balance_error") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (!args || args->GetSize() < 2) {
            LOG_ERROR_RENDER(LogFmt() << "get_balance_error missing args (need 2)");
            return true;
        }
        RejectBridgeCall(args->GetInt(0), args->GetString(1).ToString());
        return true;
    }

    // Wallet Response Handlers

    // Settings persistence response
    if (message_name == "settings_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string settingsJson = args->GetString(0);

        LOG_DEBUG_RENDER(LogFmt() << "✅ Settings response received");

        // Execute JavaScript to call the callback function directly
        std::string js = "if (window.onSettingsResponse) { window.onSettingsResponse(" + settingsJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    // Profile Manager responses
    if (message_name == "profiles_result") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string profilesJson = args->GetString(0);

        LOG_DEBUG_RENDER(LogFmt() << "👤 Profiles result received");

        std::string js = "if (window.onProfilesResult) { window.onProfilesResult(" + profilesJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    // Profile import responses
    if (message_name == "import_profiles_result") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string profilesJson = args->GetString(0);

        LOG_DEBUG_RENDER(LogFmt() << "📂 Import profiles result received");

        std::string js = "if (window.onImportProfilesResult) { window.onImportProfilesResult(" + profilesJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "import_complete") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string resultJson = args->GetString(0);

        LOG_DEBUG_RENDER(LogFmt() << "📦 Import complete");

        std::string js = "if (window.onImportComplete) { window.onImportComplete(" + resultJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    // MIGRATED (Phase 8c stage 1) — routed by request id, not through a global slot.
    // Args: 0 = requestId (echoed by the browser process), 1 = response JSON.
    // ⛔ No `window.onWalletStatusResponse` any more: a reply that arrives after its
    // caller gave up is discarded by ResolveBridgeCall instead of resolving whichever
    // promise happens to own the global at that moment.
    if (message_name == "wallet_status_check_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() < 2) {
            LOG_WARNING_RENDER("wallet_status_check_response missing args (need 2: requestId, json)");
            return true;
        }
        const int requestId = args->GetInt(0);
        const std::string responseJson = args->GetString(1).ToString();

        LOG_DEBUG_RENDER(LogFmt() << "✅ Wallet status response (requestId " << requestId
                                  << "): " << responseJson);
        ResolveBridgeCall(requestId, responseJson);
        return true;
    }

    // MIGRATED (Phase 8c stage 3 batch 2). Args: 0 = requestId, 1 = payload.
    //
    // The payload carries the recovery phrase (`wallet.mnemonic`), so it is not logged.
    if (message_name == "get_wallet_info_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (!args || args->GetSize() < 2) {
            LOG_ERROR_RENDER(LogFmt() << "get_wallet_info_response missing args (need 2)");
            return true;
        }
        ResolveBridgeCall(args->GetInt(0), args->GetString(1).ToString());
        return true;
    }

    // MIGRATED (Phase 8c stage 3 batch 2). Args: 0 = requestId, 1 = payload.
    if (message_name == "mark_wallet_backed_up_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (!args || args->GetSize() < 2) {
            LOG_ERROR_RENDER(LogFmt() << "mark_wallet_backed_up_response missing args (need 2)");
            return true;
        }
        ResolveBridgeCall(args->GetInt(0), args->GetString(1).ToString());
        return true;
    }

    // MIGRATED (Phase 8c stage 3 batch 1). Args: 0 = requestId, 1 = payload.
    //
    // ⚠️ The legacy JS for this method called resolve(null) on timeout, not reject — so a
    // losing caller got a SILENTLY WRONG VALUE rather than an error (measured, D-5).
    if (message_name == "get_backup_modal_state_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (!args || args->GetSize() < 2) {
            LOG_ERROR_RENDER(LogFmt() << "get_backup_modal_state_response missing args (need 2)");
            return true;
        }
        ResolveBridgeCall(args->GetInt(0), args->GetString(1).ToString());
        return true;
    }

    // MIGRATED (Phase 8c stage 3 batch 1). Args: 0 = requestId, 1 = payload.
    if (message_name == "set_backup_modal_state_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (!args || args->GetSize() < 2) {
            LOG_ERROR_RENDER(LogFmt() << "set_backup_modal_state_response missing args (need 2)");
            return true;
        }
        ResolveBridgeCall(args->GetInt(0), args->GetString(1).ToString());
        return true;
    }

    // ========== OMNIBOX QUERY UPDATE ==========
    if (message_name == "omnibox_query_update") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string query = args->GetString(0);

        LOG_DEBUG_RENDER("🔍 Omnibox query update received in renderer: " + query);

        // Escape query for JavaScript string
        std::string escapedQuery = escapeJsonForJs(query);

        // Dispatch CustomEvent to JavaScript
        std::string js = "window.dispatchEvent(new CustomEvent('omniboxQueryUpdate', { detail: { query: '" + escapedQuery + "' } }));";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        LOG_DEBUG_RENDER("🔍 omniboxQueryUpdate event dispatched");
        return true;
    }

    // ========== OMNIBOX SELECT (ARROW KEY NAVIGATION) ==========
    if (message_name == "omnibox_select") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string direction = args->GetString(0);

        LOG_DEBUG_RENDER("🔍 Omnibox select received in renderer: " + direction);

        std::string js = "window.dispatchEvent(new CustomEvent('omniboxSelect', "
                         "{ detail: { direction: '" + escapeJsonForJs(direction) + "' } }));";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        LOG_DEBUG_RENDER("🔍 omniboxSelect event dispatched");
        return true;
    }

    // ========== OMNIBOX AUTOCOMPLETE UPDATE ==========
    if (message_name == "omnibox_autocomplete_update") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string suggestion = args->GetString(0);

        LOG_DEBUG_RENDER("🔍 Omnibox autocomplete update received in renderer: " + suggestion);

        // Escape suggestion for JavaScript string
        std::string escapedSuggestion = escapeJsonForJs(suggestion);

        // Dispatch via window.postMessage (MainBrowserView listens for this)
        std::string js = "window.postMessage({ type: 'omnibox_autocomplete', suggestion: '" + escapedSuggestion + "' }, '*');";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        LOG_DEBUG_RENDER("🔍 omnibox_autocomplete message posted to window");
        return true;
    }

    // ========== WALLET PAYMENT DISMISSED (forwarded from wallet overlay) ==========
    if (message_name == "wallet_payment_dismissed") {
        frame->ExecuteJavaScript("window.postMessage({ type: 'wallet_payment_dismissed' }, '*');", frame->GetURL(), 0);
        LOG_DEBUG_RENDER("wallet_payment_dismissed posted to header window");
        return true;
    }

    // ========== GOOGLE SUGGEST RESPONSE ==========
    if (message_name == "google_suggest_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string suggestionsJson = args->GetString(0);
        int requestId = args->GetSize() > 1 ? args->GetInt(1) : 0;

        LOG_DEBUG_RENDER("🔍 Google Suggest response received (length: " + std::to_string(suggestionsJson.length()) + "): " + suggestionsJson.substr(0, 200) + " (requestId: " + std::to_string(requestId) + ")");

        try {
            // Escape JSON for JavaScript (using existing escapeJsonForJs helper)
            std::string escapedJson = escapeJsonForJs(suggestionsJson);

            LOG_DEBUG_RENDER("🔍 Escaped JSON (length: " + std::to_string(escapedJson.length()) + "): " + escapedJson.substr(0, 200));

            // Use try-catch in JavaScript to prevent crashes
            std::string js =
                "try { "
                "  var parsedSuggestions = JSON.parse('" + escapedJson + "'); "
                "  window.dispatchEvent(new CustomEvent('googleSuggestResponse', { "
                "    detail: { suggestions: parsedSuggestions, requestId: " + std::to_string(requestId) + " } "
                "  })); "
                "} catch(e) { "
                "  console.error('Failed to parse Google suggestions:', e, 'JSON:', '" + escapedJson + "'); "
                "}";

            if (frame) {
                frame->ExecuteJavaScript(js, frame->GetURL(), 0);
                LOG_DEBUG_RENDER("🔍 Google Suggest response dispatched to window with requestId: " + std::to_string(requestId));
            } else {
                LOG_DEBUG_RENDER("⚠️ Frame is null, cannot dispatch Google Suggest response");
            }
        } catch (const std::exception& e) {
            LOG_DEBUG_RENDER("❌ Exception in google_suggest_response handler: " + std::string(e.what()));
        }

        return true;
    }

    // ========== COOKIE/CACHE RESPONSE HANDLERS ==========

    if (message_name == "cookie_get_all_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string cookiesJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(cookiesJson);
        std::string js = "if (window.onCookieGetAllResponse) { window.onCookieGetAllResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_delete_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieDeleteResponse) { window.onCookieDeleteResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_delete_domain_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieDeleteDomainResponse) { window.onCookieDeleteDomainResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_delete_all_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieDeleteAllResponse) { window.onCookieDeleteAllResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cache_clear_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCacheClearResponse) { window.onCacheClearResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cache_get_size_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCacheGetSizeResponse) { window.onCacheGetSizeResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "paid_cache_clear_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onPaidCacheClearResponse) { window.onPaidCacheClearResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "paid_cache_get_size_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onPaidCacheGetSizeResponse) { window.onPaidCacheGetSizeResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    // ========== COOKIE BLOCKING RESPONSE HANDLERS ==========

    if (message_name == "cookie_block_domain_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieBlockDomainResponse) { window.onCookieBlockDomainResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_unblock_domain_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieUnblockDomainResponse) { window.onCookieUnblockDomainResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_blocklist_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieBlocklistResponse) { window.onCookieBlocklistResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_allow_third_party_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieAllowThirdPartyResponse) { window.onCookieAllowThirdPartyResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_remove_third_party_allow_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieRemoveThirdPartyAllowResponse) { window.onCookieRemoveThirdPartyAllowResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_block_log_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieBlockLogResponse) { window.onCookieBlockLogResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_clear_block_log_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieClearBlockLogResponse) { window.onCookieClearBlockLogResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_blocked_count_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieBlockedCountResponse) { window.onCookieBlockedCountResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_reset_blocked_count_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieResetBlockedCountResponse) { window.onCookieResetBlockedCountResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_check_site_allowed_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieCheckSiteAllowedResponse) { window.onCookieCheckSiteAllowedResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    // ========== ADBLOCK RESPONSE HANDLERS (Sprint 8c) ==========

    if (message_name == "adblock_blocked_count_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onAdblockBlockedCountResponse) { window.onAdblockBlockedCountResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "adblock_reset_blocked_count_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onAdblockResetBlockedCountResponse) { window.onAdblockResetBlockedCountResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "adblock_site_toggle_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onAdblockSiteToggleResponse) { window.onAdblockSiteToggleResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "adblock_scriptlet_toggle_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onAdblockScriptletToggleResponse) { window.onAdblockScriptletToggleResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "adblock_check_site_enabled_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onAdblockCheckSiteEnabledResponse) { window.onAdblockCheckSiteEnabledResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "fingerprint_get_site_enabled_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onFingerprintSiteEnabledResponse) { window.onFingerprintSiteEnabledResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "site_permissions_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onSitePermissionsResponse) { window.onSitePermissionsResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "recently_closed_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onRecentlyClosedResponse) { window.onRecentlyClosedResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "adblock_check_scriptlets_enabled_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onAdblockCheckScriptletsEnabledResponse) { window.onAdblockCheckScriptletsEnabledResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    // ========== BOOKMARK RESPONSE HANDLERS ==========

    if (message_name == "bookmark_add_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkAddResponse) { window.onBookmarkAddResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_get_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkGetResponse) { window.onBookmarkGetResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_update_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkUpdateResponse) { window.onBookmarkUpdateResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_remove_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkRemoveResponse) { window.onBookmarkRemoveResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_search_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkSearchResponse) { window.onBookmarkSearchResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_get_all_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkGetAllResponse) { window.onBookmarkGetAllResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_is_bookmarked_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkIsBookmarkedResponse) { window.onBookmarkIsBookmarkedResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_get_all_tags_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkGetAllTagsResponse) { window.onBookmarkGetAllTagsResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_update_last_accessed_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkUpdateLastAccessedResponse) { window.onBookmarkUpdateLastAccessedResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_folder_create_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkFolderCreateResponse) { window.onBookmarkFolderCreateResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_folder_list_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkFolderListResponse) { window.onBookmarkFolderListResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_folder_update_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkFolderUpdateResponse) { window.onBookmarkFolderUpdateResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_folder_remove_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkFolderRemoveResponse) { window.onBookmarkFolderRemoveResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_folder_get_tree_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkFolderGetTreeResponse) { window.onBookmarkFolderGetTreeResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    return false;
}
