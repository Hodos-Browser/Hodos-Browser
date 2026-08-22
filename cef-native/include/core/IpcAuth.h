// IpcAuth.h — pure, header-only IPC authorization predicates (P0.5-B1).
//
// These classify a browser-process IPC by MESSAGE NAME and by the sending
// browser's ROLE, so the trust decision is one testable predicate instead of a
// role check copied into each arm. They are the whole of the P0.5-B1 fix: called
// once, at the single Layer-2 choke in SimpleHandler::OnProcessMessageReceived
// (right after the Layer-1 internal-origin gate), before any privileged arm runs.
//
// WHY THIS EXISTS (the measured substrate). A web page can self-navigate its own
// TAB to http://127.0.0.1:5137/brc100-auth?type=...&domain=<attacker>. That page
// is internal-origin, so the Layer-1 gate passes it; BRC100AuthOverlayRoot then
// renders the real approval prompt entirely from window.location.search, and one
// Allow click fires a grant/approve/reveal IPC whose C++ handler writes a
// header-free first-party POST to /domain/permissions/* that Rust trusts. Panel
// #3 MEASURED this for `add_domain_permission` and fixed that one arm (ee8f836);
// the same substrate reaches every sibling arm below. A tab always has role
// "tab_<id>", never an approval-overlay role — that is the discriminator.
//
// ⛔ DO NOT widen IsApprovalOverlayRole. The two roles here are exactly the roles
// BRC100AuthOverlayRoot runs under (the "notification" overlay for domain_approval
// and the "brc100auth" overlay for auth/spend/cert/reveal). Every one of the
// messages in IsGrantApproveMessage is emitted ONLY by that component, so this is
// the complete + tight set. Adding a role here re-opens the hole.

#pragma once

#include <string>

namespace hodos {

// The privileged "write a grant / approve a request / reveal an identity"
// message family. Each is emitted SOLELY by frontend BRC100AuthOverlayRoot.tsx
// and each ends in a persistent grant, an approval, or an identity disclosure:
//   add_domain_permission[_advanced] — writes a domain-trust grant
//   grant_scoped_permission          — writes a V18 protocol/basket/counterparty grant
//   approve_cert_fields              — persists which identity-cert fields a domain may read
//   approve_identity_key_reveal      — "always allow" identity-key reveal for a domain
//   approve_key_linkage_reveal       — "always allow" key-linkage reveal for a domain
//   brc100_auth_response             — approves a pending auth/spend (incl. the
//                                      empty-requestId → g_pendingModalDomain fallback)
// NOT here: domain_permission_invalidate — it is legitimately sent from the
// settings AND wallet panels too (ApprovedSitesTab / DomainPermissionsTab), so it
// gets a narrower tab-only denial (see IsTabRole) rather than this allowlist.
inline bool IsGrantApproveMessage(const std::string& name) {
    return name == "add_domain_permission"
        || name == "add_domain_permission_advanced"
        || name == "grant_scoped_permission"
        || name == "approve_cert_fields"
        || name == "approve_identity_key_reveal"
        || name == "approve_key_linkage_reveal"
        || name == "brc100_auth_response";
}

// The ONLY roles that may drive the IsGrantApproveMessage family: the two roles
// BRC100AuthOverlayRoot legitimately runs under. Everything else — a "tab_<id>"
// web page, any other overlay, an empty/unknown role — is refused.
inline bool IsApprovalOverlayRole(const std::string& role) {
    return role == "notification" || role == "brc100auth";
}

// True for a content tab's role ("tab_<id>"). A tab is the only browser that
// hosts arbitrary web content, so it is the self-navigation vector. Used to deny
// tab-driven domain_permission_invalidate (a self-navved page clearing a user's
// grants) without touching the legitimate settings/wallet-panel revoke flows.
inline bool IsTabRole(const std::string& role) {
    return role.rfind("tab_", 0) == 0;
}

}  // namespace hodos
