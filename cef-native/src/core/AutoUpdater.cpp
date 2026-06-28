// AutoUpdater.cpp — Windows implementation using WinSparkle
//
// WinSparkle is a DLL that handles the entire update lifecycle:
// - Periodic background check against appcast.xml
// - User-facing "update available" dialog
// - Download + verification (DSA signature)
// - Launch installer and request app shutdown
//
// All WinSparkle configuration must happen BEFORE win_sparkle_init().

#ifdef _WIN32

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#include "../../include/core/AutoUpdater.h"
#include <winsparkle.h>
#include <string>
#include <mutex>

// Forward declaration for the Logger (defined in cef_browser_shell.cpp)
namespace {
    void LogInfo(const std::string& msg) {
        // Use OutputDebugStringA as a simple logging fallback
        // The main Logger is in cef_browser_shell.cpp
        OutputDebugStringA(("[AutoUpdater] " + msg + "\n").c_str());
    }
}

// Static instance
AutoUpdater& AutoUpdater::GetInstance() {
    static AutoUpdater instance;
    return instance;
}

// WinSparkle callbacks — these are called from WinSparkle's background thread
static std::function<void()> s_shutdownCallback;

static int __cdecl CanShutdownCallback() {
    // Always allow shutdown — the app will handle graceful cleanup
    return 1;
}

static void __cdecl ShutdownRequestCallback() {
    LogInfo("WinSparkle requested application shutdown for update");
    if (s_shutdownCallback) {
        s_shutdownCallback();
    }
}

static void __cdecl DidFindUpdateCallback() {
    LogInfo("WinSparkle found an update");
}

static void __cdecl DidNotFindUpdateCallback() {
    LogInfo("WinSparkle: no update available");
}

static void __cdecl UpdateErrorCallback() {
    LogInfo("WinSparkle encountered an error checking for updates");
}

void AutoUpdater::Initialize(const std::string& version, const std::string& appcastUrl, bool autoCheck) {
    if (initialized_) return;

    LogInfo("Initializing WinSparkle v" + version + " (autoCheck=" + (autoCheck ? "true" : "false") + ")");
    LogInfo("Appcast URL: " + appcastUrl);

    // Set app metadata (used for registry path and User-Agent)
    win_sparkle_set_app_details(L"Marston Enterprises", L"Hodos Browser",
        std::wstring(version.begin(), version.end()).c_str());

    // Set appcast URL
    win_sparkle_set_appcast_url(appcastUrl.c_str());

    // Configure automatic checking
    win_sparkle_set_automatic_check_for_updates(autoCheck ? 1 : 0);

    // Default check interval: 24 hours
    win_sparkle_set_update_check_interval(86400);

    // Register callbacks
    win_sparkle_set_can_shutdown_callback(CanShutdownCallback);
    win_sparkle_set_shutdown_request_callback(ShutdownRequestCallback);
    win_sparkle_set_did_find_update_callback(DidFindUpdateCallback);
    win_sparkle_set_did_not_find_update_callback(DidNotFindUpdateCallback);
    win_sparkle_set_error_callback(UpdateErrorCallback);

    // DSA public key for signature verification.
    // The corresponding private key is stored as a GitHub Secret
    // (WINSPARKLE_DSA_PRIVATE_KEY) and used by CI/CD to sign release artifacts.
    static const char* DSA_PUB_KEY =
        "-----BEGIN PUBLIC KEY-----\n"
        "MIIDQzCCAjYGByqGSM44BAEwggIpAoIBAQD9xs8OWZuxOPKZzfel/eJRYKuksjdl\n"
        "3vJgO8miselAg9bmbAkdks1Mcx3Ze7T1oFqrlPgMwYyDVQteBxwmQN/F7t41et0Z\n"
        "8csix7skltlu5peUw2FVCpbgPnWXrmnTk+fn/QjAqKIUctST+Xe17XVH4gUWEgR3\n"
        "FAU0uGNo0wcd/6MsPXODnVH86XWaxqPwV3RVVtdeile5YrbBnFvYPOtUofG5iGM2\n"
        "bCNxG8oRcNrfv4xeWuWnjOdFyjsTMVy7bc9vICqwbBNKIMb2ebKjAOs7uUB0n0DO\n"
        "lP1fYc0+fpSEs1IB5sp6r5OjH3EojjaGSxCLAlsgVE1F87mwV7kAjT+zAh0AvpTm\n"
        "6/Sm0LatYZFDNgSkQtkRxB/bpoa++N+p0wKCAQEAzUWOrN9mPe+7t1jhZbBKY2Ur\n"
        "d7cb57yYW3JUwJgrZKc/dUT3HiOcCjxQlnZqEFY+xI4LSCvywWG9wicUWwEwvxRl\n"
        "dQvX+cdW2ywCeBeeA/XZp3002nf7QmeIDQHBs21fLeebqH5LJXcZDehgVxWsBUjN\n"
        "3oNV16cndVQMoGjJb9J1v/Ut9VZOwZWYrDUDG+ewO5BHJgrxcMHT57MWZnq/jY+V\n"
        "mmmJKkZ5OItmj8Dmd6O3ZuwUSEe+esyew0/63XoOdgH6tFlT1IyjuCheokwt9Hkq\n"
        "mhaykfcn4A1afWZhcu5yqdPnN3iGYrGbGPe6O5/hYRd7Wh7j4egczlyAQlwbtAOC\n"
        "AQUAAoIBADGWt/342McnzG5Yg9rGgFEGXryJIsUmmRo1meoh2g+mj70sK8WMLwOQ\n"
        "UvUIMUTtJKWudx2LLiT8DzgjwRcaZIMfYpCmYsSC8W9LjJSFEaftlOMjNKzuPGA3\n"
        "aumop0Jix6duSgJ7fj/OeMjFa6Y7WxpcZEdh6GctrIZv6ElqEpjdl7QnMSedd8ru\n"
        "Rh/UgP5dLJtg1JVqciIjqet5K1dttwj98nX+FWvakYoIAQEb0kvO+3R6UWpp8RMz\n"
        "mEe3JzT8CvBrXDjCv3Y3XDjavnVywPeyuqMq27+sHNddH9HZEOWKzMraUzNavOVg\n"
        "OND9tLAqt6+ph7cMG0xx2hqryPrlk7g=\n"
        "-----END PUBLIC KEY-----";

    // Signature-verification key is MANDATORY (fail-closed, WINDOWS_AUTOUPDATE_PLAN
    // §B.2a). If it fails to load, WinSparkle would otherwise run with verification
    // DISABLED — but a build that cannot load its own update key must run with the
    // updater OFF, never with verification off (verification-off would let an
    // unsigned / MITM'd update install). So abort init and leave the updater inert:
    // initialized_ stays false, so every other AutoUpdater method no-ops, and the
    // browser runs normally — just without auto-updates.
    // (Commit 3b will add win_sparkle_set_eddsa_public_key to this same gate.)
    if (!win_sparkle_set_dsa_pub_pem(DSA_PUB_KEY)) {
        LogInfo("ERROR: failed to load update-signature public key — auto-updater "
                "DISABLED (fail-closed). WinSparkle will NOT be started; the browser "
                "runs normally without updates.");
        return;  // do NOT win_sparkle_init(); do NOT set initialized_ = true.
    }
    LogInfo("DSA public key set successfully");

    // Start WinSparkle (non-blocking)
    win_sparkle_init();
    initialized_ = true;

    LogInfo("WinSparkle initialized successfully");
}

void AutoUpdater::CheckForUpdatesInteractively() {
    if (!initialized_) return;
    LogInfo("Manual update check requested");
    win_sparkle_check_update_with_ui();
}

void AutoUpdater::CheckForUpdatesInBackground() {
    if (!initialized_) return;
    win_sparkle_check_update_without_ui();
}

void AutoUpdater::SetAutoCheckEnabled(bool enabled) {
    if (!initialized_) return;
    win_sparkle_set_automatic_check_for_updates(enabled ? 1 : 0);
    LogInfo("Auto-check " + std::string(enabled ? "enabled" : "disabled"));
}

void AutoUpdater::SetUpdateMode(UpdateMode mode) {
    if (!initialized_) return;
    update_mode_ = mode;

    std::string modeStr = mode == UpdateMode::Off ? "off" :
                          mode == UpdateMode::Notify ? "notify" : "silent";
    LogInfo("Update mode set to: " + modeStr);

    // WinSparkle only supports auto-check on/off — no silent download mode.
    // Map Off → disable checks; Notify/Silent → enable checks.
    win_sparkle_set_automatic_check_for_updates(mode != UpdateMode::Off ? 1 : 0);
}

bool AutoUpdater::IsAutoCheckEnabled() const {
    if (!initialized_) return false;
    return win_sparkle_get_automatic_check_for_updates() != 0;
}

void AutoUpdater::SetCheckInterval(int seconds) {
    if (!initialized_) return;
    if (seconds < 3600) seconds = 3600; // WinSparkle minimum
    win_sparkle_set_update_check_interval(seconds);
}

void AutoUpdater::Cleanup() {
    if (!initialized_) return;
    LogInfo("Cleaning up WinSparkle...");
    win_sparkle_cleanup();
    initialized_ = false;
    LogInfo("WinSparkle cleanup complete");
}

void AutoUpdater::SetShutdownCallback(ShutdownCallback callback) {
    shutdown_callback_ = callback;
    s_shutdownCallback = callback;
}

#endif // _WIN32
