#pragma once

#include "SitePermissionType.h"

#include <sqlite3.h>
#include <string>
#include <mutex>
#include <cstdint>

// Per-site web-content permission store (camera / mic / location / notifications /
// clipboard). Mirrors the CookieBlockManager pattern: SQLite, per-profile,
// Initialize(user_data_path) + idempotent Shutdown() on the clean-exit path.
//
// Permission callbacks (CefPermissionHandler) fire on the browser-process UI
// thread, so a plain std::mutex is sufficient.

class SitePermissionStore {
public:
    static SitePermissionStore& GetInstance();

    bool Initialize(const std::string& user_data_path);
    void Shutdown() { CloseDatabase(); }   // idempotent
    bool IsInitialized() const { return db_ != nullptr; }

    // Stored decision for (host, type), or Ask if none / not initialized.
    SitePermissionState GetState(const std::string& host, SitePermissionType type);

    // Persist a decision. State::Ask DELETEs the row (reverts to "ask next time").
    bool SetState(const std::string& host, SitePermissionType type, SitePermissionState state);

    // Remove all stored decisions for a host ("reset permissions for this site").
    bool ResetDomain(const std::string& host);

    // JSON array [{ "type": N, "state": N }, …] for a host (b2 management UI).
    std::string GetAllForHost(const std::string& host);

    // Normalize an origin/URL to a bare lowercase host (scheme/port/path stripped).
    static std::string NormalizeHost(const std::string& origin);

private:
    SitePermissionStore() = default;
    ~SitePermissionStore();
    SitePermissionStore(const SitePermissionStore&) = delete;
    SitePermissionStore& operator=(const SitePermissionStore&) = delete;

    bool OpenDatabase();
    void CloseDatabase();

    sqlite3* db_ = nullptr;
    std::string db_path_;
    std::mutex mutex_;
};
