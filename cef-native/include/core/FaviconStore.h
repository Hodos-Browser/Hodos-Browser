#pragma once

#include <sqlite3.h>
#include <cstdint>
#include <mutex>
#include <string>
#include <vector>

// Local favicon store — beta.3 Phase 7b.
//
// 🚨 WHY THIS EXISTS. Four surfaces rendered
// `https://www.google.com/s2/favicons?domain=<site>`: the consent modal, the
// omnibox, the new-tab tiles and the bookmarks panel. Measured on the wire, that
// endpoint REDIRECTS to `t2.gstatic.com/faviconV2?...&url=<full url>`, so two
// Google-owned hosts learned which sites the user was visiting, bookmarking, or
// being asked to trust — from a privacy browser. The consent modal was fixed
// first, using the live `Tab::favicon_url`; the other three have no live tab to
// read, which is what this store is for.
//
// ⭐ PRIOR ART (`development-docs/PRIOR_ART.md`, 2026-09-04). Every major browser
// keeps a LOCAL icon store keyed by page URL — Firefox `favicons.sqlite`
// (`moz_icons` / `moz_icons_to_pages`), Chromium's `Favicons` DB (`favicons`,
// `favicon_bitmaps`, `icon_mapping`). Chrome is not a clean example: it keeps a
// Google-hosted fallback for surfaces with no local icon, and **Brave and
// ungoogled-chromium strip exactly that**. This is the same de-Googling change.
//
// ⚠️ DELIBERATE SIMPLIFICATION vs that prior art, stated so it is a decision and
// not an oversight: they use TWO tables (icon → bitmaps, plus page → icon) because
// many PAGES of a site share one icon. We key on **host**, and all four consumers
// are host-oriented (a domain suggestion, a top-site tile, a bookmark's site, a
// consent prompt's origin). Host-keying collapses the mapping table into the key
// and gets that de-duplication for free. If a per-page icon is ever needed, that
// is when the second table earns its place — not before.
//
// Bytes are fetched with `CefBrowserHost::DownloadImage(url, is_favicon=true, …)`,
// which is purpose-built for this and, per the CEF header, "cookies are not sent
// and not accepted during download".

namespace hodos {

struct FaviconRow {
    std::string host;
    std::string icon_url;
    std::vector<uint8_t> png;
    int width = 0;
    int64_t updated_at = 0;   // unix seconds
};

class FaviconStore {
public:
    static FaviconStore& GetInstance();

    bool Initialize(const std::string& user_data_path);
    void Shutdown() { CloseDatabase(); }   // idempotent
    bool IsInitialized() const { return db_ != nullptr; }

    // Upsert one host's icon. `png` is the encoded image; empty is rejected.
    bool Put(const std::string& host, const std::string& icon_url,
             const std::vector<uint8_t>& png, int width);

    // PNG bytes for a host, or empty when absent/stale/uninitialised.
    std::vector<uint8_t> Get(const std::string& host);

    // True when we hold a FRESH icon for this host, so the caller can skip the
    // download. Freshness matters because a site can change its icon and a store
    // with no expiry would pin the old one forever (Chromium and Firefox both
    // carry an expiry stamp for this reason).
    bool HasFresh(const std::string& host);

    // `data:image/png;base64,…` for a host, or "" — the form the React surfaces
    // consume, so an <img src> needs no new URL scheme and makes no request.
    std::string GetDataUri(const std::string& host);

    // Drop everything (privacy: "clear browsing data" must reach this too).
    bool Clear();

    // How long a stored icon is trusted before it is re-downloaded.
    static constexpr int64_t kMaxAgeSeconds = 30LL * 24 * 60 * 60;   // 30 days
    // Guard against a hostile or broken site pushing a huge "icon" into our DB.
    static constexpr size_t kMaxPngBytes = 256 * 1024;

private:
    FaviconStore() = default;
    ~FaviconStore();
    FaviconStore(const FaviconStore&) = delete;
    FaviconStore& operator=(const FaviconStore&) = delete;

    bool EnsureSchema();
    void CloseDatabase();

    sqlite3* db_ = nullptr;
    std::mutex mutex_;
};

}  // namespace hodos
