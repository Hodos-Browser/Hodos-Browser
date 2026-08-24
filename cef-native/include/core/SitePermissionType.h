#pragma once

// Hodos-stable site-permission ids and states.
//
// Split out of SitePermissionStore.h (beta.3 Phase 0.9) so the pure CEF-bit ->
// Hodos-type mapping in SitePermissionMapping.h — and its unit test — can use
// these without pulling in sqlite3.

// Hodos-stable permission ids — DECOUPLED from CEF's bitflag enums so a Chromium
// bump that renumbers cef_permission_request_types_t can't corrupt stored rows.
// CEF <-> these are mapped only at the callback boundary (SitePermissionMapping.h,
// whose mirrored bit constants are static_assert'd against the real CEF values in
// simple_handler.cpp).
enum class SitePermissionType : int {
    Camera        = 1,
    Microphone    = 2,
    Location      = 3,
    Notifications = 4,
    Clipboard     = 5,
    // beta.3 Phase 0.9 — Chromium's Local Network Access prompts. Two distinct
    // asks, kept distinct because the risk differs: Loopback is "a server on
    // THIS machine", LocalNetwork is "a device on your LAN".
    LocalNetwork  = 6,   // CEF_PERMISSION_TYPE_LOCAL_NETWORK   (LAN / private IPs)
    Loopback      = 7,   // CEF_PERMISSION_TYPE_LOOPBACK_NETWORK (127.0.0.1 / localhost)
    // v2 (schema already supports; not yet wired): Midi=8, Usb=9, Bluetooth=10, ...
};

enum class SitePermissionState : int {
    Ask   = 0,   // no stored decision (absence of a row) — defer to a prompt
    Allow = 1,
    Block = 2,
};
