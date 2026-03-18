#pragma once

#include "include/cef_resource_request_handler.h"
#include "include/cef_resource_handler.h"
#include "include/cef_request.h"
#include "include/cef_stream.h"
#include "include/cef_parser.h"
#include "include/wrapper/cef_stream_resource_handler.h"
#include "include/wrapper/cef_helpers.h"

#include <string>
#include <algorithm>

#ifdef _WIN32
#include <windows.h>
#endif

// Serves frontend files from a local directory for production builds.
// Replaces the Vite dev server (port 5137) when frontend/ exists next to the .exe.
//
// Two-layer pattern (matches HttpRequestInterceptor -> AsyncWalletResourceHandler):
//   GetResourceRequestHandler() returns LocalFileResourceRequestHandler
//   LocalFileResourceRequestHandler::GetResourceHandler() returns CefStreamResourceHandler
class LocalFileResourceRequestHandler : public CefResourceRequestHandler {
public:
    explicit LocalFileResourceRequestHandler(const std::string& base_dir,
                                              const std::string& url)
        : base_dir_(base_dir), url_(url) {
        // Ensure trailing separator
        if (!base_dir_.empty() && base_dir_.back() != '\\' && base_dir_.back() != '/') {
            base_dir_ += '\\';
        }
    }

    CefRefPtr<CefResourceHandler> GetResourceHandler(
        CefRefPtr<CefBrowser> browser,
        CefRefPtr<CefFrame> frame,
        CefRefPtr<CefRequest> request) override {

        std::string url_path = ExtractPath(url_);

        // Security: block directory traversal and absolute paths
        if (url_path.find("..") != std::string::npos) {
            return nullptr;
        }
        if (url_path.length() >= 2 && url_path[1] == ':') {
            return nullptr;  // Block Windows absolute paths (C:\...)
        }
        if (!url_path.empty() && (url_path[0] == '\\' || url_path[0] == '/')) {
            return nullptr;  // Block rooted paths
        }

        // Build filesystem path
        std::string file_path = base_dir_ + url_path;

        // Try to open the file
        CefRefPtr<CefStreamReader> stream =
            CefStreamReader::CreateForFile(file_path);

        if (stream) {
            // File found - serve it with correct MIME type
            std::string mime = GetMimeForPath(url_path);
            return new CefStreamResourceHandler(mime, stream);
        }

        // SPA fallback: file not found -> serve index.html
        // React Router handles client-side routing for paths like
        // /wallet-panel, /settings, /settings-menu, /omnibox, etc.
        std::string index_path = base_dir_ + "index.html";
        stream = CefStreamReader::CreateForFile(index_path);
        if (stream) {
            return new CefStreamResourceHandler("text/html", stream);
        }

        // index.html not found - frontend dir is broken
        return nullptr;
    }

    CefRefPtr<CefCookieAccessFilter> GetCookieAccessFilter(
        CefRefPtr<CefBrowser> browser,
        CefRefPtr<CefFrame> frame,
        CefRefPtr<CefRequest> request) override {
        return nullptr;
    }

private:
    std::string base_dir_;
    std::string url_;

    // Extract path from URL: "http://127.0.0.1:5137/assets/index.js" -> "assets/index.js"
    // Empty or "/" -> "index.html"
    static std::string ExtractPath(const std::string& url) {
        // Find path after host:port
        const std::string prefix = "://";
        size_t prefix_pos = url.find(prefix);
        if (prefix_pos == std::string::npos) return "index.html";

        size_t host_start = prefix_pos + prefix.length();
        size_t path_start = url.find('/', host_start);
        if (path_start == std::string::npos) return "index.html";

        std::string path = url.substr(path_start + 1); // skip leading /

        // Strip query string (?iro=123, etc.)
        size_t query_pos = path.find('?');
        if (query_pos != std::string::npos) {
            path = path.substr(0, query_pos);
        }

        // Strip fragment
        size_t frag_pos = path.find('#');
        if (frag_pos != std::string::npos) {
            path = path.substr(0, frag_pos);
        }

        // Normalize separators
        std::replace(path.begin(), path.end(), '/', '\\');

        // Empty path -> index.html
        if (path.empty()) return "index.html";

        return path;
    }

    // Get MIME type for a file path. Uses CefGetMimeType with fallbacks.
    static std::string GetMimeForPath(const std::string& path) {
        size_t dot_pos = path.rfind('.');
        if (dot_pos == std::string::npos) return "application/octet-stream";

        std::string ext = path.substr(dot_pos + 1);

        // Try CEF's built-in MIME detection first
        CefString mime = CefGetMimeType(ext);
        if (!mime.empty()) return mime.ToString();

        // Fallbacks for types CEF may not cover
        if (ext == "js" || ext == "mjs") return "application/javascript";
        if (ext == "css")   return "text/css";
        if (ext == "html")  return "text/html";
        if (ext == "svg")   return "image/svg+xml";
        if (ext == "json")  return "application/json";
        if (ext == "woff2") return "font/woff2";
        if (ext == "woff")  return "font/woff";
        if (ext == "wasm")  return "application/wasm";
        if (ext == "ico")   return "image/x-icon";
        if (ext == "png")   return "image/png";
        if (ext == "webp")  return "image/webp";
        if (ext == "map")   return "application/json";

        return "application/octet-stream";
    }

    IMPLEMENT_REFCOUNTING(LocalFileResourceRequestHandler);
    DISALLOW_COPY_AND_ASSIGN(LocalFileResourceRequestHandler);
};

// Checks if frontend/ directory exists next to the executable.
// Result is cached - only checks filesystem once.
// Called on CEF IO thread (single-threaded, static init is safe).
inline bool IsFrontendAvailable(std::string& out_frontend_dir) {
    static bool checked = false;
    static bool available = false;
    static std::string frontend_dir;

    if (!checked) {
#ifdef _WIN32
        char exe_path[MAX_PATH];
        GetModuleFileNameA(nullptr, exe_path, MAX_PATH);
        std::string exe_dir(exe_path);
        size_t last_slash = exe_dir.find_last_of("\\/");
        if (last_slash != std::string::npos) {
            exe_dir = exe_dir.substr(0, last_slash);
        }
        frontend_dir = exe_dir + "\\frontend\\";
        // Check if the directory exists by looking for index.html
        std::string index_path = frontend_dir + "index.html";
        available = (GetFileAttributesA(index_path.c_str()) != INVALID_FILE_ATTRIBUTES);
#endif
        checked = true;
    }

    out_frontend_dir = frontend_dir;
    return available;
}
