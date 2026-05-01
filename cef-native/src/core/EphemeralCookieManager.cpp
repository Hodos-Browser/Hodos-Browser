#include "../../include/core/EphemeralCookieManager.h"
#include "../../include/core/Logger.h"

#include "include/cef_task.h"
#include "include/cef_cookie.h"
#include "include/wrapper/cef_helpers.h"

#include <algorithm>

// Logging macros
#define LOG_DEBUG_EPHEMERAL(msg) Logger::Log(msg, 0, 2)
#define LOG_INFO_EPHEMERAL(msg) Logger::Log(msg, 1, 2)
#define LOG_WARNING_EPHEMERAL(msg) Logger::Log(msg, 2, 2)
#define LOG_ERROR_EPHEMERAL(msg) Logger::Log(msg, 3, 2)

// Grace period duration in milliseconds (30 seconds)
static const int GRACE_PERIOD_MS = 30000;

// ============================================================================
// GraceExpiredTask — CefTask subclass posted via CefPostDelayedTask.
// Fires on IO thread after 30s grace period.
// ============================================================================
class GraceExpiredTask : public CefTask {
public:
    explicit GraceExpiredTask(const std::string& site) : site_(site) {}

    void Execute() override {
        EphemeralCookieManager::GetInstance().OnGraceExpired(site_);
    }

private:
    std::string site_;
    IMPLEMENT_REFCOUNTING(GraceExpiredTask);
};

// ============================================================================
// SilentDeleteCookiesCallback — swallows the completion event from DeleteCookies.
// ============================================================================
class SilentDeleteCookiesCallback : public CefDeleteCookiesCallback {
public:
    explicit SilentDeleteCookiesCallback(const std::string& domain)
        : domain_(domain) {}

    void OnComplete(int num_deleted) override {
        if (num_deleted > 0) {
            Logger::Log("Ephemeral cleanup: deleted " + std::to_string(num_deleted)
                        + " cookies for domain: " + domain_, 1, 2);
        }
    }

private:
    std::string domain_;
    IMPLEMENT_REFCOUNTING(SilentDeleteCookiesCallback);
};

// ============================================================================
// Singleton
// ============================================================================

EphemeralCookieManager& EphemeralCookieManager::GetInstance() {
    static EphemeralCookieManager instance;
    return instance;
}

// ============================================================================
// ExtractSiteFromUrl — eTLD+1 extraction (simple approach)
//
// "https://www.example.com/path" -> "example.com"
// "https://accounts.google.com/signin" -> "google.com"
// "https://example.co.uk/path" -> "co.uk" (known limitation, no PSL)
// ============================================================================
std::string EphemeralCookieManager::ExtractSiteFromUrl(const std::string& url) {
    // Strip scheme
    size_t scheme_end = url.find("://");
    size_t host_start = (scheme_end != std::string::npos) ? scheme_end + 3 : 0;

    // Strip path
    size_t host_end = url.find('/', host_start);
    std::string host;
    if (host_end != std::string::npos) {
        host = url.substr(host_start, host_end - host_start);
    } else {
        host = url.substr(host_start);
    }

    // Strip port
    size_t colon = host.find(':');
    if (colon != std::string::npos) {
        host = host.substr(0, colon);
    }

    // Lowercase
    std::transform(host.begin(), host.end(), host.begin(),
                   [](unsigned char c) { return std::tolower(c); });

    if (host.empty()) return "";

    // Strip www. prefix
    if (host.length() > 4 && host.substr(0, 4) == "www.") {
        host = host.substr(4);
    }

    // Take last two segments for eTLD+1 approximation
    // "accounts.google.com" -> "google.com"
    // "example.com" -> "example.com"
    size_t last_dot = host.rfind('.');
    if (last_dot == std::string::npos || last_dot == 0) {
        return host; // single-label domain or no dots
    }

    size_t second_last_dot = host.rfind('.', last_dot - 1);
    if (second_last_dot == std::string::npos) {
        return host; // already two segments (e.g. "example.com")
    }

    return host.substr(second_last_dot + 1);
}

// ============================================================================
// OnTabNavigated — UI thread
// ============================================================================
void EphemeralCookieManager::OnTabNavigated(int browser_id, const std::string& url) {
    std::string new_site = ExtractSiteFromUrl(url);
    if (new_site.empty()) return;

    // Skip internal URLs
    if (url.find("localhost") != std::string::npos ||
        url.find("127.0.0.1") != std::string::npos) {
        return;
    }

    std::unique_lock<std::shared_mutex> lock(mutex_);

    // Check if this browser was previously on a different site
    auto it = browser_sites_.find(browser_id);
    if (it != browser_sites_.end()) {
        const std::string& old_site = it->second;
        if (old_site == new_site) {
            return; // Same site, nothing to do
        }

        // Decrement old site ref count
        auto old_it = active_sites_.find(old_site);
        if (old_it != active_sites_.end()) {
            old_it->second.tab_ref_count--;
            LOG_DEBUG_EPHEMERAL("Tab " + std::to_string(browser_id) + " navigated away from "
                                + old_site + " (ref_count=" + std::to_string(old_it->second.tab_ref_count) + ")");

            if (old_it->second.tab_ref_count <= 0) {
                // Last tab left this site — start grace period
                old_it->second.tab_ref_count = 0;
                old_it->second.grace_active = true;
                LOG_INFO_EPHEMERAL("Starting 30s grace period for: " + old_site);
                CefPostDelayedTask(TID_IO, new GraceExpiredTask(old_site), GRACE_PERIOD_MS);
            }
        }
    }

    // Update browser -> site mapping
    browser_sites_[browser_id] = new_site;

    // Increment new site ref count (or create session)
    auto& session = active_sites_[new_site];
    if (session.site.empty()) {
        session.site = new_site;
    }
    session.tab_ref_count++;

    // Cancel grace period if site was reopened
    if (session.grace_active) {
        session.grace_active = false;
        LOG_INFO_EPHEMERAL("Grace period cancelled — site reopened: " + new_site);
    }

    LOG_DEBUG_EPHEMERAL("Tab " + std::to_string(browser_id) + " navigated to "
                        + new_site + " (ref_count=" + std::to_string(session.tab_ref_count) + ")");
}

// ============================================================================
// OnTabClosed — UI thread
// ============================================================================
void EphemeralCookieManager::OnTabClosed(int browser_id) {
    std::unique_lock<std::shared_mutex> lock(mutex_);

    auto it = browser_sites_.find(browser_id);
    if (it == browser_sites_.end()) {
        return; // Browser wasn't tracked (e.g. overlay, internal page)
    }

    const std::string site = it->second;
    browser_sites_.erase(it);

    auto site_it = active_sites_.find(site);
    if (site_it == active_sites_.end()) {
        return; // Shouldn't happen, but defensive
    }

    site_it->second.tab_ref_count--;
    LOG_DEBUG_EPHEMERAL("Tab " + std::to_string(browser_id) + " closed for site "
                        + site + " (ref_count=" + std::to_string(site_it->second.tab_ref_count) + ")");

    if (site_it->second.tab_ref_count <= 0) {
        // Last tab for this site — start grace period
        site_it->second.tab_ref_count = 0;
        site_it->second.grace_active = true;
        LOG_INFO_EPHEMERAL("Starting 30s grace period for: " + site);
        CefPostDelayedTask(TID_IO, new GraceExpiredTask(site), GRACE_PERIOD_MS);
    }
}

// ============================================================================
// IsSiteActive — IO thread (shared_lock for concurrent reads)
// ============================================================================
bool EphemeralCookieManager::IsSiteActive(const std::string& site) {
    std::shared_lock<std::shared_mutex> lock(mutex_);
    auto it = active_sites_.find(site);
    if (it == active_sites_.end()) {
        return false;
    }
    // Active if has open tabs OR is in grace period
    return (it->second.tab_ref_count > 0 || it->second.grace_active);
}

// ============================================================================
// RecordThirdPartyCookie — IO thread
// ============================================================================
void EphemeralCookieManager::RecordThirdPartyCookie(const std::string& site,
                                                     const std::string& cookie_domain) {
    std::unique_lock<std::shared_mutex> lock(mutex_);
    auto it = active_sites_.find(site);
    if (it != active_sites_.end()) {
        it->second.third_party_domains.insert(cookie_domain);
    }
}

// ============================================================================
// OnGraceExpired — IO thread (via CefPostDelayedTask)
// ============================================================================
void EphemeralCookieManager::OnGraceExpired(const std::string& site) {
    std::unordered_set<std::string> domains_to_delete;

    {
        std::unique_lock<std::shared_mutex> lock(mutex_);

        auto it = active_sites_.find(site);
        if (it == active_sites_.end()) {
            return; // Already cleaned up
        }

        // Check if site was reopened (grace cancelled)
        if (!it->second.grace_active) {
            LOG_DEBUG_EPHEMERAL("Grace expired but already cancelled for: " + site);
            return;
        }

        // Check if tabs reopened during grace period
        if (it->second.tab_ref_count > 0) {
            it->second.grace_active = false;
            LOG_DEBUG_EPHEMERAL("Grace expired but tabs reopened for: " + site);
            return;
        }

        // Grace expired and no tabs — collect domains to delete
        domains_to_delete = std::move(it->second.third_party_domains);
        active_sites_.erase(it);
    }

    // Delete cookies outside the lock
    if (domains_to_delete.empty()) {
        LOG_DEBUG_EPHEMERAL("Grace expired for " + site + " — no third-party cookies to clean up");
        return;
    }

    LOG_INFO_EPHEMERAL("Grace expired for " + site + " — deleting cookies from "
                       + std::to_string(domains_to_delete.size()) + " third-party domains");

    auto manager = CefCookieManager::GetGlobalManager(nullptr);
    if (!manager) {
        LOG_ERROR_EPHEMERAL("Failed to get global cookie manager for ephemeral cleanup");
        return;
    }

    // Auth provider domains whose cookies must survive ephemeral cleanup —
    // these are needed for cross-site login (OAuth/SSO) to persist.
    static const char* auth_providers[] = {
        "google.com", "accounts.google.com", "googleapis.com",
        "microsoftonline.com", "login.live.com", "login.microsoftonline.com",
        "appleid.apple.com", "github.com",
        nullptr
    };

    for (const auto& domain : domains_to_delete) {
        // Skip auth provider domains — their cookies are needed for login persistence
        bool is_auth_provider = false;
        for (int i = 0; auth_providers[i]; i++) {
            if (domain == auth_providers[i] ||
                (domain.length() > strlen(auth_providers[i]) &&
                 domain[domain.length() - strlen(auth_providers[i]) - 1] == '.' &&
                 domain.substr(domain.length() - strlen(auth_providers[i])) == auth_providers[i])) {
                is_auth_provider = true;
                break;
            }
        }
        if (is_auth_provider) {
            LOG_DEBUG_EPHEMERAL("Skipping auth provider cookie cleanup: " + domain);
            continue;
        }

        // DeleteCookies with URL and empty name deletes all cookies for that URL
        std::string cookie_url = "https://" + domain;
        manager->DeleteCookies(cookie_url, "",
                               new SilentDeleteCookiesCallback(domain));
    }
}
