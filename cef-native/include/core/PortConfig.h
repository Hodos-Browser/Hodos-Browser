// PortConfig.h — single source of truth for the wallet/adblock backend ports.
//
// Dev builds (env HODOS_DEV=1) talk to the wallet on 31401 and adblock on 31402
// so the dev browser and the INSTALLED browser (which use 31301/31302) can run
// at the SAME TIME without fighting over the ports. A release build never sets
// HODOS_DEV, so it always uses 31301/31302. This mirrors the Rust side's
// wallet_port()/adblock_port() gate (rust-wallet/src/main.rs,
// adblock-engine/src/main.rs) — keep them in lockstep.
//
// HARD RELEASE-SAFETY RULE: the dev port must apply ONLY when HODOS_DEV=1. Never
// hardcode 31401/31402 anywhere; always route through these helpers so the dev
// port can never leak into a release build.
//
// Usage:
//   - URL string literals:  hodos::WalletUrl("/wallet/status")  // http://127.0.0.1:<port>/wallet/status
//   - WinHttpConnect / integer port args:  hodos::WalletPort()
//   - Port-recognition gates (find("localhost:31301")):  hodos::IsWalletHostPort(url)
//     (checks BOTH "localhost:<port>" AND "127.0.0.1:<port>" — the codebase
//      uses both host forms and they must move in lockstep).

#pragma once

#include <cstdlib>
#include <string>

namespace hodos {

// Computed once (HODOS_DEV cannot change mid-run) so this is cheap on hot paths
// such as the per-request interceptor.
inline bool IsDevEnv() {
    static const bool dev = []() {
        const char* v = std::getenv("HODOS_DEV");
        return v != nullptr && std::string(v) == "1";
    }();
    return dev;
}

inline int  WalletPort()      { return IsDevEnv() ? 31401 : 31301; }
inline int  AdblockPort()     { return IsDevEnv() ? 31402 : 31302; }
inline std::string WalletPortStr()  { return std::to_string(WalletPort()); }
inline std::string AdblockPortStr() { return std::to_string(AdblockPort()); }

// Base URLs (127.0.0.1 host form — the canonical one for outbound C++ calls).
inline std::string WalletBaseUrl()  { return "http://127.0.0.1:" + WalletPortStr(); }
inline std::string AdblockBaseUrl() { return "http://127.0.0.1:" + AdblockPortStr(); }

// Convenience: full wallet URL for a leading-slash path.
inline std::string WalletUrl(const std::string& path)  { return WalletBaseUrl()  + path; }
inline std::string AdblockUrl(const std::string& path) { return AdblockBaseUrl() + path; }

// True if `url` targets the local wallet on the active port, in EITHER host form.
// Replaces literal find("localhost:31301") / find("127.0.0.1:31301") gates.
inline bool IsWalletHostPort(const std::string& url) {
    const std::string p = WalletPortStr();
    return url.find("localhost:" + p) != std::string::npos
        || url.find("127.0.0.1:" + p) != std::string::npos;
}

}  // namespace hodos
