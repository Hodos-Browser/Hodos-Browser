#pragma once

#include <string>
#include <unordered_map>
#include <unordered_set>
#include <shared_mutex>

// ============================================================================
// EphemeralCookieManager — Brave-style ephemeral third-party cookie storage.
//
// Third-party cookies are ALLOWED while a top-level site has open tabs.
// When the last tab closes, a 30-second grace period starts. After the grace
// period, all third-party cookies from that site session are deleted from
// CEF's cookie store. If the user reopens the site within 30 seconds, the
// grace period is cancelled and cookies survive.
//
// This replaces the old AUTH_COOKIE_DOMAINS hardcoded allowlist — OAuth/SSO
// works naturally because cookies persist during the browsing session.
//
// Thread safety:
//   - OnTabNavigated / OnTabClosed: called on UI thread
//   - IsSiteActive / RecordThirdPartyCookie: called on IO thread
//   - OnGraceExpired: called on IO thread (via CefPostDelayedTask)
//   - Protected by shared_mutex (IO reads, UI writes)
// ============================================================================

struct SiteSession {
    std::string site;                                    // eTLD+1 (e.g. "example.com")
    int tab_ref_count = 0;                               // open tabs on this site
    std::unordered_set<std::string> third_party_domains; // cookie domains to clean up
    bool grace_active = false;                           // in 30s grace period?
};

class EphemeralCookieManager {
public:
    static EphemeralCookieManager& GetInstance();

    // --- UI thread methods ---

    // Called when a tab navigates to a new URL. Updates ref counts,
    // cancels grace periods if site reopened within 30s.
    void OnTabNavigated(int browser_id, const std::string& url);

    // Called when a tab is closed. Decrements ref count for the site,
    // starts 30s grace period if last tab for that site.
    void OnTabClosed(int browser_id);

    // --- IO thread methods ---

    // Returns true if the site has open tabs OR is in grace period.
    // O(1) lookup used by CookieBlockManager to decide third-party policy.
    bool IsSiteActive(const std::string& site);

    // Records a third-party cookie domain associated with a site session.
    // Called from CookieBlockManager::CanSaveCookie when allowing a
    // third-party cookie due to an active site session.
    void RecordThirdPartyCookie(const std::string& site, const std::string& cookie_domain);

    // Called via CefPostDelayedTask after 30s grace period expires.
    // If the site still has no open tabs, deletes all recorded
    // third-party cookies and removes the SiteSession.
    void OnGraceExpired(const std::string& site);

    // --- Utility ---

    // Extract eTLD+1 from a full URL. Simple approach: strips scheme/path/port,
    // strips www. prefix, takes last two domain segments.
    // Same limitation as IsThirdParty() — no public suffix list.
    static std::string ExtractSiteFromUrl(const std::string& url);

private:
    EphemeralCookieManager() = default;
    ~EphemeralCookieManager() = default;

    // Shared mutex: IO thread takes shared_lock for reads,
    // UI thread takes unique_lock for writes
    mutable std::shared_mutex mutex_;

    // site (eTLD+1) -> session state
    std::unordered_map<std::string, SiteSession> active_sites_;

    // browser_id -> current site (eTLD+1) for that tab
    std::unordered_map<int, std::string> browser_sites_;

    // Prevent copying
    EphemeralCookieManager(const EphemeralCookieManager&) = delete;
    EphemeralCookieManager& operator=(const EphemeralCookieManager&) = delete;
};
