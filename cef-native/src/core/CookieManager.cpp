#include "../../include/core/CookieManager.h"
#include "../../include/core/Logger.h"
#include "../../include/core/LogSafeUrl.h"
#include "../../include/core/AppPaths.h"

#include "include/cef_cookie.h"
#include "include/cef_task.h"
#include "include/wrapper/cef_helpers.h"

#include <nlohmann/json.hpp>
#include <vector>
#include <string>
#include <filesystem>
#include <cstdlib>

#define LOG_DEBUG_COOKIE(msg) Logger::Log(msg, 0, 2)
#define LOG_INFO_COOKIE(msg) Logger::Log(msg, 1, 2)
#define LOG_ERROR_COOKIE(msg) Logger::Log(msg, 3, 2)

// ============================================================================
// Helper: Send a process message `[requestId, json]` to the renderer.
// Must be called on the UI thread. Phase 8c: arg 0 is the bridge request id.
// ============================================================================
static void SendJsonResponseToRenderer(CefRefPtr<CefBrowser> browser,
                                       const std::string& message_name,
                                       int requestId,
                                       const std::string& json_str) {
    CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create(message_name);
    CefRefPtr<CefListValue> args = msg->GetArgumentList();
    args->SetInt(0, requestId);
    args->SetString(1, json_str);
    browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
}

// ============================================================================
// SendResponseTask - Posts a JSON response to the renderer on the UI thread.
// Used by IO-thread callbacks (CookieCollector, DeleteCallback) that cannot
// send IPC messages directly.
// ============================================================================
class SendResponseTask : public CefTask {
public:
    SendResponseTask(CefRefPtr<CefBrowser> browser,
                     const std::string& message_name,
                     int requestId,
                     const std::string& json_str)
        : browser_(browser), message_name_(message_name),
          request_id_(requestId), json_str_(json_str) {}

    void Execute() override {
        SendJsonResponseToRenderer(browser_, message_name_, request_id_, json_str_);
        // Length only: the cookie dump carries every cookie VALUE on the profile, and this
        // line used to write all of them into the log at INFO.
        LOG_INFO_COOKIE("Sent " + message_name_ + " (requestId " + std::to_string(request_id_) +
                        ", " + std::to_string(json_str_.length()) + " bytes)");
    }

private:
    CefRefPtr<CefBrowser> browser_;
    std::string message_name_;
    int request_id_;
    std::string json_str_;

    IMPLEMENT_REFCOUNTING(SendResponseTask);
};

// ============================================================================
// CookieCollector - CefCookieVisitor that collects all cookies into JSON.
// Visit() runs on the IO thread. On the last cookie, posts the JSON response
// back to the UI thread via SendResponseTask.
//
// ⚠️ CEF: "This method may never be called if no cookies are found"
// (cef_cookie.h, CefCookieVisitor::Visit). With an empty jar no Visit() ever
// fires, so the reply is posted from the DESTRUCTOR when nothing was sent —
// CEF releases the visitor once the walk is over. The React hook used to hide
// this with a 5 s timeout that resolved `[]`; under per-request-id routing
// there is no such timeout, so the empty case has to be answered here.
// ============================================================================
class CookieCollector : public CefCookieVisitor {
public:
    CookieCollector(CefRefPtr<CefBrowser> browser, int requestId)
        : browser_(browser), request_id_(requestId) {}

    ~CookieCollector() override {
        if (!sent_) {
            CefPostTask(TID_UI,
                        new SendResponseTask(browser_, "cookie_get_all_response", request_id_, "[]"));
        }
    }

    bool Visit(const CefCookie& cookie, int count, int total,
               bool& deleteCookie) override {
        // Runs on IO thread

        nlohmann::json cookie_json;

        std::string name = CefString(&cookie.name).ToString();
        std::string value = CefString(&cookie.value).ToString();
        std::string domain = CefString(&cookie.domain).ToString();
        std::string path = CefString(&cookie.path).ToString();

        cookie_json["name"] = name;
        cookie_json["value"] = value;
        cookie_json["domain"] = domain;
        cookie_json["path"] = path;
        cookie_json["secure"] = cookie.secure ? true : false;
        cookie_json["httponly"] = cookie.httponly ? true : false;
        cookie_json["sameSite"] = static_cast<int>(cookie.same_site);
        cookie_json["hasExpires"] = cookie.has_expires ? true : false;

        if (cookie.has_expires) {
            // Convert cef_basetime_t to Unix timestamp in milliseconds
            CefBaseTime base_time = cookie.expires;
            CefTime cef_time;
            if (cef_time_from_basetime(base_time, &cef_time)) {
                // Compute epoch milliseconds from CefTime fields
                struct tm timeinfo = {};
                timeinfo.tm_year = cef_time.year - 1900;
                timeinfo.tm_mon = cef_time.month - 1;
                timeinfo.tm_mday = cef_time.day_of_month;
                timeinfo.tm_hour = cef_time.hour;
                timeinfo.tm_min = cef_time.minute;
                timeinfo.tm_sec = cef_time.second;
                timeinfo.tm_isdst = -1;

                // Use _mkgmtime on Windows for UTC conversion
#ifdef _WIN32
                time_t epoch_secs = _mkgmtime(&timeinfo);
#else
                time_t epoch_secs = timegm(&timeinfo);
#endif
                if (epoch_secs != -1) {
                    int64_t epoch_ms = static_cast<int64_t>(epoch_secs) * 1000 +
                                       cef_time.millisecond;
                    cookie_json["expires"] = epoch_ms;
                }
            }
        }

        // Size as Chrome DevTools calculates it: name length + value length
        cookie_json["size"] = static_cast<int>(name.length() + value.length());

        cookies_.push_back(cookie_json);

        // On the last cookie, send the collected array to the renderer
        if (count == total - 1) {
            nlohmann::json response = cookies_;
            std::string json_str = response.dump();
            sent_ = true;
            CefPostTask(TID_UI,
                        new SendResponseTask(browser_, "cookie_get_all_response", request_id_, json_str));
        }

        return true; // Continue visiting
    }

private:
    CefRefPtr<CefBrowser> browser_;
    int request_id_;
    bool sent_ = false;
    std::vector<nlohmann::json> cookies_;

    IMPLEMENT_REFCOUNTING(CookieCollector);
};

// ============================================================================
// DeleteCallback - CefDeleteCookiesCallback for deletion operations.
// OnComplete() runs on IO thread. Posts response to UI thread.
// ============================================================================
class DeleteCallback : public CefDeleteCookiesCallback {
public:
    DeleteCallback(CefRefPtr<CefBrowser> browser,
                   const std::string& response_message_name,
                   int requestId)
        : browser_(browser), response_message_name_(response_message_name),
          request_id_(requestId) {}

    void OnComplete(int num_deleted) override {
        // Runs on IO thread
        nlohmann::json response;
        response["success"] = true;
        response["deleted"] = num_deleted;
        std::string json_str = response.dump();

        CefPostTask(TID_UI,
                    new SendResponseTask(browser_, response_message_name_, request_id_, json_str));
    }

private:
    CefRefPtr<CefBrowser> browser_;
    std::string response_message_name_;
    int request_id_;

    IMPLEMENT_REFCOUNTING(DeleteCallback);
};

// A delete that cannot even start (no cookie manager) used to return WITHOUT a reply,
// leaving the caller to its timeout — and the writes in the React hook had none.
static void ReplyDeleteUnavailable(CefRefPtr<CefBrowser> browser,
                                   const std::string& message_name,
                                   int requestId) {
    nlohmann::json response;
    response["success"] = false;
    response["deleted"] = 0;
    response["error"] = "cookie manager unavailable";
    SendJsonResponseToRenderer(browser, message_name, requestId, response.dump());
}

// ============================================================================
// CacheSizeTask - Walks the cache directory on a background thread and sends
// the total byte count back to the renderer via UI thread.
// ============================================================================
class CacheSizeTask : public CefTask {
public:
    CacheSizeTask(CefRefPtr<CefBrowser> browser, int requestId)
        : browser_(browser), request_id_(requestId) {}

    void Execute() override {
        int64_t total_bytes = 0;

        try {
            // Build cache directory path
            std::string cache_dir;
#ifdef _WIN32
            const char* appdata = std::getenv("APPDATA");
            if (appdata) {
                cache_dir = std::string(appdata) + "\\" + AppPaths::GetAppDirName() + "\\Default\\Cache";
            }
#else
            const char* home = std::getenv("HOME");
            if (home) {
                cache_dir = std::string(home) +
                            "/Library/Application Support/" + AppPaths::GetAppDirName() + "/Default/Cache";
            }
#endif

            if (!cache_dir.empty() &&
                std::filesystem::exists(cache_dir) &&
                std::filesystem::is_directory(cache_dir)) {

                std::error_code ec;
                for (const auto& entry :
                     std::filesystem::recursive_directory_iterator(
                         cache_dir,
                         std::filesystem::directory_options::skip_permission_denied,
                         ec)) {
                    if (!ec && entry.is_regular_file(ec) && !ec) {
                        total_bytes += entry.file_size(ec);
                        if (ec) ec.clear();
                    }
                    if (ec) ec.clear();
                }
            }

            LOG_INFO_COOKIE("Cache size calculated: " +
                            std::to_string(total_bytes) + " bytes");
        } catch (const std::exception& e) {
            LOG_ERROR_COOKIE("Error calculating cache size: " +
                             std::string(e.what()));
            total_bytes = 0;
        }

        // Post result back to UI thread for sending to renderer
        nlohmann::json response;
        response["totalBytes"] = total_bytes;
        std::string json_str = response.dump();

        CefPostTask(TID_UI,
                    new SendResponseTask(browser_, "cache_get_size_response", request_id_, json_str));
    }

private:
    CefRefPtr<CefBrowser> browser_;
    int request_id_;

    IMPLEMENT_REFCOUNTING(CacheSizeTask);
};

// ============================================================================
// CookieManager static method implementations
// ============================================================================

void CookieManager::HandleGetAllCookies(CefRefPtr<CefBrowser> browser, int requestId) {
    LOG_INFO_COOKIE("HandleGetAllCookies called (requestId " + std::to_string(requestId) + ")");

    CefRefPtr<CefCookieManager> manager =
        CefCookieManager::GetGlobalManager(nullptr);

    if (!manager) {
        LOG_ERROR_COOKIE("Failed to get global cookie manager");
        SendJsonResponseToRenderer(browser, "cookie_get_all_response", requestId, "[]");
        return;
    }

    // The collector answers in every case: from Visit() on the last cookie, or from its
    // destructor when CEF never called Visit() (empty jar) or VisitAllCookies failed.
    CefRefPtr<CookieCollector> collector = new CookieCollector(browser, requestId);
    bool result = manager->VisitAllCookies(collector);

    if (!result) {
        LOG_INFO_COOKIE("VisitAllCookies returned false (cookies cannot be accessed)");
    }
}

void CookieManager::HandleDeleteCookie(CefRefPtr<CefBrowser> browser,
                                        int requestId,
                                        const std::string& url,
                                        const std::string& name) {
    LOG_INFO_COOKIE("HandleDeleteCookie: url=" + hodos::LogSafeUrl(url) + ", name=" + name);

    CefRefPtr<CefCookieManager> manager =
        CefCookieManager::GetGlobalManager(nullptr);

    if (!manager) {
        LOG_ERROR_COOKIE("Failed to get global cookie manager for delete");
        ReplyDeleteUnavailable(browser, "cookie_delete_response", requestId);
        return;
    }

    // Ensure URL has a protocol
    std::string cookie_url = url;
    if (cookie_url.find("://") == std::string::npos) {
        cookie_url = "https://" + cookie_url;
    }

    manager->DeleteCookies(
        cookie_url, name,
        new DeleteCallback(browser, "cookie_delete_response", requestId));
}

void CookieManager::HandleDeleteDomainCookies(CefRefPtr<CefBrowser> browser,
                                               int requestId,
                                               const std::string& domain) {
    LOG_INFO_COOKIE("HandleDeleteDomainCookies: domain=" + domain);

    CefRefPtr<CefCookieManager> manager =
        CefCookieManager::GetGlobalManager(nullptr);

    if (!manager) {
        LOG_ERROR_COOKIE("Failed to get global cookie manager for domain delete");
        ReplyDeleteUnavailable(browser, "cookie_delete_domain_response", requestId);
        return;
    }

    // CEF DeleteCookies with a URL and empty name deletes all cookies matching that URL
    std::string cookie_url = "https://" + domain;
    manager->DeleteCookies(
        cookie_url, "",
        new DeleteCallback(browser, "cookie_delete_domain_response", requestId));
}

void CookieManager::HandleDeleteAllCookies(CefRefPtr<CefBrowser> browser, int requestId) {
    LOG_INFO_COOKIE("HandleDeleteAllCookies called");

    CefRefPtr<CefCookieManager> manager =
        CefCookieManager::GetGlobalManager(nullptr);

    if (!manager) {
        LOG_ERROR_COOKIE("Failed to get global cookie manager for delete all");
        ReplyDeleteUnavailable(browser, "cookie_delete_all_response", requestId);
        return;
    }

    // Empty URL and empty name deletes all cookies
    manager->DeleteCookies(
        "", "",
        new DeleteCallback(browser, "cookie_delete_all_response", requestId));
}

void CookieManager::HandleClearCache(CefRefPtr<CefBrowser> browser, int requestId) {
    LOG_INFO_COOKIE("HandleClearCache called");

    // ExecuteDevToolsMethod runs on UI thread (we're already here from OnProcessMessageReceived)
    browser->GetHost()->ExecuteDevToolsMethod(
        0, "Network.clearBrowserCache", nullptr);

    // Send success response back to renderer
    nlohmann::json response;
    response["success"] = true;
    std::string json_str = response.dump();

    SendJsonResponseToRenderer(browser, "cache_clear_response", requestId, json_str);
    LOG_INFO_COOKIE("Cache clear executed via CDP, response sent");
}

void CookieManager::HandleGetCacheSize(CefRefPtr<CefBrowser> browser, int requestId) {
    LOG_INFO_COOKIE("HandleGetCacheSize called");

    // Run directory walk on a background thread to avoid blocking UI
    CefPostTask(TID_FILE_USER_BLOCKING, new CacheSizeTask(browser, requestId));
}
