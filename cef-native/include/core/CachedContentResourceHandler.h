#pragma once

#include "include/cef_resource_handler.h"
#include "include/cef_resource_request_handler.h"
#include "include/wrapper/cef_helpers.h"

#include <algorithm>
#include <cstring>

#include "PaidContentCache.h"

// Plays back a paid HTTP response from PaidContentCache. Returned by
// SimpleHandler::GetResourceRequestHandler when PaidContentCache::Get hits
// for the request URL. Short-circuits the entire 402-detection / Async402
// path: the bytes are served from disk, no payment IPC fires, no
// SessionManager state changes.
//
// Two-class shape mirrors LocalFileResourceHandler.h:
//   CachedContentRequestHandler returns CachedContentPlaybackHandler from
//   GetResourceHandler().
class CachedContentPlaybackHandler : public CefResourceHandler {
public:
    explicit CachedContentPlaybackHandler(PaidContentEntry entry)
        : entry_(std::move(entry)), offset_(0) {}

    bool Open(CefRefPtr<CefRequest> /*request*/, bool& handle_request,
              CefRefPtr<CefCallback> callback) override {
        CEF_REQUIRE_IO_THREAD();
        // We have all bytes already; tell CEF to call GetResponseHeaders +
        // ReadResponse synchronously without further callback.
        handle_request = true;
        callback->Continue();
        return true;
    }

    void GetResponseHeaders(CefRefPtr<CefResponse> response,
                            int64_t& response_length,
                            CefString& /*redirectUrl*/) override {
        CEF_REQUIRE_IO_THREAD();
        response->SetStatus(entry_.status);
        response->SetStatusText("OK");

        // Strip transport-level headers that don't apply to bytes we already
        // hold (mirrors Async402ResourceHandler::onUpstreamComplete:
        // content-encoding/content-length/transfer-encoding are stripped
        // there before being stored, but defend twice).
        CefResponse::HeaderMap header_map;
        std::string mime;
        for (const auto& [name, value] : entry_.headers) {
            std::string lower = ToLower(name);
            if (lower == "content-encoding" || lower == "content-length" ||
                lower == "transfer-encoding") {
                continue;
            }
            header_map.insert({CefString(name), CefString(value)});
            if (lower == "content-type") {
                std::string ct = value;
                auto semi = ct.find(';');
                mime = (semi == std::string::npos ? ct : ct.substr(0, semi));
            }
        }
        if (!header_map.empty()) {
            response->SetHeaderMap(header_map);
        }
        if (mime.empty()) {
            mime = "text/html";
        }
        response->SetMimeType(mime);
        response_length = static_cast<int64_t>(entry_.body.size());
    }

    bool ReadResponse(void* data_out, int bytes_to_read, int& bytes_read,
                      CefRefPtr<CefCallback> /*callback*/) override {
        CEF_REQUIRE_IO_THREAD();
        if (offset_ >= entry_.body.size()) {
            bytes_read = 0;
            return false;
        }
        size_t remaining = entry_.body.size() - offset_;
        size_t to_copy = static_cast<size_t>(bytes_to_read) < remaining
                             ? static_cast<size_t>(bytes_to_read)
                             : remaining;
        std::memcpy(data_out, entry_.body.data() + offset_, to_copy);
        offset_ += to_copy;
        bytes_read = static_cast<int>(to_copy);
        return true;
    }

    void Cancel() override { CEF_REQUIRE_IO_THREAD(); }

private:
    static std::string ToLower(std::string s) {
        std::transform(s.begin(), s.end(), s.begin(),
                       [](unsigned char c) { return std::tolower(c); });
        return s;
    }

    PaidContentEntry entry_;
    size_t offset_;

    IMPLEMENT_REFCOUNTING(CachedContentPlaybackHandler);
    DISALLOW_COPY_AND_ASSIGN(CachedContentPlaybackHandler);
};

class CachedContentRequestHandler : public CefResourceRequestHandler {
public:
    explicit CachedContentRequestHandler(PaidContentEntry entry)
        : entry_(std::move(entry)) {}

    CefRefPtr<CefResourceHandler> GetResourceHandler(
        CefRefPtr<CefBrowser> /*browser*/,
        CefRefPtr<CefFrame> /*frame*/,
        CefRefPtr<CefRequest> /*request*/) override {
        return new CachedContentPlaybackHandler(entry_);
    }

    CefRefPtr<CefCookieAccessFilter> GetCookieAccessFilter(
        CefRefPtr<CefBrowser> /*browser*/,
        CefRefPtr<CefFrame> /*frame*/,
        CefRefPtr<CefRequest> /*request*/) override {
        return nullptr;
    }

private:
    PaidContentEntry entry_;

    IMPLEMENT_REFCOUNTING(CachedContentRequestHandler);
    DISALLOW_COPY_AND_ASSIGN(CachedContentRequestHandler);
};
