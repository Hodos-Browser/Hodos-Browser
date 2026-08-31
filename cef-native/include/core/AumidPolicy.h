// cef-native/include/core/AumidPolicy.h
//
// The AppUserModelID (AUMID) decision, as a pure function — Phase 3 (WS2), beta.3.
//
// WHAT AN AUMID IS. A string Windows uses to answer "are these two things the same
// application?" It groups windows under one taskbar button and, via a matching
// shortcut or a registered display name, supplies the name shown on that button and
// in its jump list. Chrome's is "Chrome"; Brave's is "Brave".
//
// ─────────────────────────────────────────────────────────────────────────────
// WHY THIS FILE EXISTS — the bug it fixes, measured, not assumed.
//
// The taskbar button read "HodosBrowser.exe" and refused to group with the user's
// pinned icon. The desk diagnosis said the cause was "production single-profile
// never sets an AUMID". That was REFUTED during the Phase 3 kickoff: the reporting
// user had two profiles, so the old `GetAllProfiles().size() > 1` gate DID fire on
// their machine — 61 "AUMID set" lines in their production log prove it, six of them
// on the day they reported the symptom.
//
// The real cause is that the identity CHANGED UNDER THE USER, and then matched
// nothing:
//
//   2026-04-27  user pins Hodos. One profile ⇒ the old gate does NOT fire ⇒ no
//               explicit AUMID ⇒ Chromium's default applies ⇒ the pinned .lnk is
//               stamped "Chromium.OYX4LNTP4DA5GSWHROKEK3C7BM".
//   2026-07-06  user creates a 2nd profile. EIGHT SECONDS LATER the gate fires for
//               the first time ever and the process identity becomes "HodosBrowser".
//   thereafter  running window (HodosBrowser) != pinned .lnk (Chromium.OYX…)
//               ⇒ separate taskbar button; and "HodosBrowser" is declared by NO
//               shortcut on the system ⇒ Windows has no display name for it
//               ⇒ falls back to the exe filename ⇒ "HodosBrowser.exe".
//
// ⛔ THEREFORE: widening the gate ALONE would do to every remaining single-profile
// user exactly what 2026-07-06 did to the reporting user — move them off the
// identity their pin currently matches. The three parts must ship together:
//
//   1. this file        — set an explicit AUMID ALWAYS, so identity stops depending
//                         on how many profiles happen to exist;
//   2. installer .iss   — [Icons] declare the SAME string, so shortcut and process
//                         agree (they must be byte-identical or the mismatch persists);
//   3. EnsureProfileShortcut — because a PER-PROFILE AUMID matches none of the
//                         installer's shortcuts, and the owner confirmed the second
//                         profile's button ALSO read "hodosbrowser.exe". Each
//                         non-Default profile therefore gets its own shortcut
//                         declaring its own AUMID. This is what Chrome does
//                         (Chrome.UserData.Profile1 was observed on the owner's box).
//
// ⛔ MEASURED REFUTATION, 2026-08-30 — do not "simplify" this back to a registry write.
// This step was FIRST implemented as RegisterAumidDisplayName, writing ApplicationName
// under HKCU\Software\Classes\AppUserModelId\<aumid>. The key was written correctly and
// verified present — and the taskbar IGNORED IT: the button still read
// "HodosBrowser.exe". That key drives toast notifications, NOT the taskbar. Replacing it
// with a matching shortcut named "Hodos Browser DEVTEST" made the button read
// "Hodos Browser DEVTEST" immediately, on the same binary. The shortcut is the mechanism.
//
// ⛔ AND IT IS NOT THE VERSION RESOURCE either — also measured, twice. FileDescription is
// already "Hodos Browser" on BOTH the dev and the installed exe, and the taskbar still
// read "HodosBrowser.exe". Windows falls back to the exe FILENAME, not its description.
//
// ⚠️ Existing pinned shortcuts carry the OLD identity and will stop grouping once.
// Owner decision (beta.3 Phase 3, Q4): OPTION A — release-note "re-pin once". We do
// NOT rewrite the user's pinned .lnk files. Deliberate: writing another app's user
// data to save one drag is not a trade we wanted.
//
// ─────────────────────────────────────────────────────────────────────────────
// PER-PROFILE BUTTONS ARE A FEATURE, NOT A BUG. Extra profiles intentionally get
// their own taskbar button — the same thing Chrome does (its per-profile AUMID form
// "Chrome.UserData.Profile1" was observed on the reporting machine). The suffix logic
// below is unchanged from the code it replaces; only the GATE changed. Multi-profile
// users therefore compute the IDENTICAL string they computed before, which is why
// this change cannot regress them.
//
// See development-docs/0.4.0-beta.3/phase-3-window-identity/ (PHASE_CONTRACT.md §4,
// MEASUREMENTS.md M1/M2/M3/M7/M9.1a).

#pragma once

#include <optional>
#include <string>

namespace hodos {

// The base identities. ⚠️ The production value MUST stay byte-identical to the
// AppUserModelID declared in installer/hodos-browser.iss [Icons]. If one changes and
// the other does not, the process and its shortcut become different applications
// again and this whole bug returns.
inline constexpr wchar_t kAumidBaseProd[] = L"HodosBrowser";
inline constexpr wchar_t kAumidBaseDev[]  = L"HodosBrowser.Dev";

// Human-readable name Windows shows for the AUMID (see RegisterAumidDisplayName).
inline constexpr wchar_t kAumidDisplayName[] = L"Hodos Browser";

// Decide this process's explicit AUMID.
//
//   isDev       — hodos::IsDevEnv(). A dev build ALWAYS gets a ".Dev" identity, even
//                 single-profile, so it never merges with the installed build's
//                 taskbar button.
//   pickerMode  — the profile picker owns no profile, so it takes the BASE identity.
//                 (Previously it was skipped entirely and got no identity at all,
//                 despite a comment claiming it kept the base one — code and comment
//                 disagreed. Phase 3 Q5.)
//   profileId   — "Default" is unsuffixed so the primary profile matches the plain
//                 shortcut; every other profile gets its own suffixed identity and
//                 therefore its own taskbar button.
//
// Returns std::optional deliberately: the return type is what makes the "no identity
// at all" outcome REPRESENTABLE, and therefore what makes the negative control for
// P3-A1 possible (reintroduce a profile-count gate → prod+single-profile returns
// nullopt → the test goes red). Post-fix this never returns nullopt for a real
// launch; an empty profileId is the one defensive case.
inline std::optional<std::wstring> ComputeAumid(bool isDev,
                                                bool pickerMode,
                                                const std::string& profileId) {
    std::wstring aumid = isDev ? kAumidBaseDev : kAumidBaseProd;

    // The picker owns no profile — base identity, no suffix.
    if (pickerMode) {
        return aumid;
    }

    // An empty id means the profile was never resolved. Suffixing with nothing would
    // silently produce the base identity and hide the failure, so keep the base but
    // say so via the type: callers log the difference.
    if (profileId.empty()) {
        return aumid;
    }

    if (profileId != "Default") {
        // profileId is validated by ProfileManager::IsValidProfileId (ASCII, no path
        // separators or shell metacharacters), so this widening is safe.
        aumid += L".";
        aumid.append(profileId.begin(), profileId.end());
    }
    return aumid;
}

}  // namespace hodos

#ifdef _WIN32

namespace hodos {

// Filename (no directory) of the Start Menu shortcut that names `profileName`'s taskbar
// button. Exposed so the delete path and any test can derive the same name.
std::wstring ProfileShortcutFileName(const std::string& profileName);

// ⛔ THERE IS NO EnsureProfileShortcut. Removed 2026-08-31 by owner decision.
//
// Writing one Start Menu shortcut per non-Default profile DOES name that profile's
// taskbar button — the mechanism is real and was confirmed live. The owner rejected the
// **UX**, not the mechanism:
//
//   "the start menu should just say Hodos Browser, nothing else and then that opens the
//    profile picker if the user has more than one profile"
//
// ⭐ That already works: `ProfileManager::ResolveStartup` returns picker mode for a
// no-argument launch whenever more than one profile exists, and the installer's single
// `{group}\Hodos Browser` shortcut passes no argument. Nothing to build for that half.
//
// ⚠️ STILL OPEN — the owner also wants the profile NAME shown on a window's taskbar
// button ("Hodos Browser - <name>", including for Default "where needed"). Without a
// shortcut there is nothing for Windows to take that name from, so it needs a
// WINDOW-level mechanism instead:
//     PKEY_AppUserModel_RelaunchDisplayNameResource + …_RelaunchCommand + …_RelaunchIconResource
// set on the window's own property store (SHGetPropertyStoreForWindow).
// 🧠 CANDIDATE, **UNVERIFIED** — and note `…DisplayNameResource` is documented to take an
// indirect resource reference ("path\app.exe,-101"), not necessarily a plain string.
// ⛔ This file has already been wrong TWICE about what names a taskbar button (the exe
// version resource, then the AppUserModelId registry key). Measure it before writing it.
// Tracked as P3-A5d in the phase contract.

// Remove a profile's Start Menu shortcut.
// ⚠️ Kept although nothing creates these any more: it cleans up after the intermediate
// build that did, on a developer machine. Nothing was ever shipped with the creation path.
// Also called on profile deletion, so a stale entry can never outlive its profile.
bool RemoveProfileShortcut(const std::string& profileName);

}  // namespace hodos

#endif  // _WIN32
