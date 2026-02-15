#pragma once

#include <vector>
#include <string>
#include <utility>

// Default tracker domains pre-populated on first run.
// pair<domain, is_wildcard>: all defaults use exact-match (false)
// because cookie domain matching already handles subdomains via
// leading-dot convention in the CefCookie.domain field.
static const std::vector<std::pair<std::string, bool>> DEFAULT_TRACKERS = {
    // Google tracking
    {"google-analytics.com", false},
    {"googletagmanager.com", false},
    {"googlesyndication.com", false},
    {"doubleclick.net", false},
    {"googleadservices.com", false},
    // Facebook/Meta tracking
    {"facebook.net", false},
    {"fbcdn.net", false},
    // Other major trackers
    {"scorecardresearch.com", false},
    {"quantserve.com", false},
    {"criteo.com", false},
    {"taboola.com", false},
    {"outbrain.com", false},
    {"hotjar.com", false},
    {"mouseflow.com", false},
    {"fullstory.com", false},
    // Ad networks
    {"adnxs.com", false},
    {"rubiconproject.com", false},
    {"pubmatic.com", false},
    {"openx.net", false},
    {"casalemedia.com", false},
    // Analytics
    {"mixpanel.com", false},
    {"amplitude.com", false},
    {"segment.io", false},
    {"newrelic.com", false},
};
