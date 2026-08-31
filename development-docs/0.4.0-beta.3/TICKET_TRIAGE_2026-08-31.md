# beta.3 ticket triage — 2026-08-31

**Why:** 27 `TICKET_*.md` at this folder's root. Their own status lines had drifted — several read
OPEN after being fixed, and 11 carried no status field at all. A backlog that cannot tell you what is
outstanding is not doing its job, and the beta.4 review would have been spent re-deriving this.

> ⛔ **Method.** Status was determined from **code and commit history**, never from the ticket's own
> status line — that is the thing that drifted. Each row says how it was settled. Where a cheap check
> could not settle it, the row says **NEEDS OWNER REVIEW** rather than guessing.
> ⚠️ "Fix is present in code" is **not** "fix was verified against the defect". Rows marked ✅ have a
> landed fix; only the owner's phase evidence tables say whether it was proven.

---

## Summary

| State | Count |
|---|---|
| ✅ **CLOSED** — fix landed, verified in code | **6** |
| 🟡 **PARTIAL** — some landed, remainder scheduled | **3** |
| 🔴 **OPEN** — verified still present | **11** |
| ❔ **NEEDS OWNER REVIEW** — not cheaply determinable | **6** |
| 🔵 **ACCEPTED** — deliberate no-fix | **1** |

⭐ **Headline for the money path:** `TICKET_token_outputs_destroyed_by_dust_paths` is OPEN and its
path 1 is an **automatic daily task** that needs no user action. It is the only open ticket that can
destroy an asset without the user doing anything.

🚨 **Headline for privacy:** `TICKET_consent_surface_fetches_third_party_favicon` is **still live** —
see below. In a privacy browser, this one is worse than its filing suggests.

---

## ✅ CLOSED — 6

| Ticket | Closed by | How I settled it |
|---|---|---|
| `qr_bsv_uri_scheme_rejected` | Phase 0.6, `e2fae9d` | 📏 `QRPayloadClassify.h:39` carries the `bitcoin:`/`bsv:` allowlist. ⛔ Ticket still said **OPEN** |
| `production_debug_logging_unbounded` | Phase 2b, `fa0c143` + `c3604f9` | 📏 Level gate, rotation, retention and URL redaction all present. ⛔ Ticket still said **OPEN — beta.3 candidate, high** |
| `createaction_strands_utxos_on_error` | Phase 0.7, `e42cd04` | ✓ Ticket's own FIXED claim matches the commit |
| `locked_wallet_is_unrecoverable` | `0420b74` | ✓ Ticket's FIXED claim matches the commit |
| `prompt_denials_should_not_persist` | Phase 0.9 | 📏 `HttpRequestInterceptor.cpp:3474` blocks **in-memory for this session** only; 👤 confirmed live in the 2 → 3 regression run — deny left **no** `domain_permissions` row. Had **no status line** |
| `deleted_profile_id_reused_over_orphaned_data` | Phase 1, `84997eb` + `47d9a06` | 📏 Startup orphan sweep renames to `.orphaned-<stamp>`; `DeleteProfile` renames-then-deletes so a freed id can never land on live data (`ProfileManager.cpp:231`, `:551`). Had **no status line**. ⚠️ macOS marker-depth gap from the P1 relay is a **separate** item |

## 🟡 PARTIAL — 3

| Ticket | State | Evidence |
|---|---|---|
| `loopback_host_form_wallet_routing` | (a) done Phase 0.5; **(b) is Phase 5**, not started | Ticket's own SCHEDULED status is accurate — no drift |
| `stray_log_in_install_root` | Code complete Phase 0; **T2/T3 rows owed** | Ticket accurate. ⚠️ Phase 3 re-touched this constraint (installer `[Icons]`) — the "nothing new inside `{app}`" assertion is owed again at `P3-A7` |
| `bridge_single_slot_callbacks_race` | `getBalance` fixed Phase 2a; **the pattern is not** | ⛔ Read its "NOT fixed and why" before touching — deduping `sendTransaction` would **collapse two payments into one** |

## 🔴 OPEN — verified still present — 11

| Ticket | Evidence it is still live |
|---|---|
| 🚨 `consent_surface_fetches_third_party_favicon` | 📏 `BRC100AuthOverlayRoot.tsx:1517` still fetches `https://www.google.com/s2/favicons?domain=<site>`. ⇒ **every consent prompt tells Google which site the user is connecting a wallet to**, at the exact moment of a privacy decision. Had **no status line** |
| 🚨 `token_outputs_destroyed_by_dust_paths` | Owner-added 2026-08-29. **Path 1 is an automatic daily task** — no user action needed to lose an asset |
| `quiet_mode_wider_than_manifest` | 📏 V25 made the default *configurable*; `migrations.rs:1361` states narrowing to the manifest **"is the open follow-up"**. Had **no status line** |
| `placeholder_resolution_failure_broadcasts_anyway` | 📏 `handlers.rs:2492`, `:2730` — the proof placeholders are still there. Had **no status line** |
| `modal_buttons_unclickable_small_screen` | 📏 Phase 1's `a3d8202` touched **only** `WalletDashboard.css` — a different surface. The modal is untouched. Had **no status line** |
| `site_permission_dual_store` | Ticket accurate — affects location, notifications, clipboard; shipped today |
| `brand_remaining_permission_prompts` | 21 prompts, inventory in `PROMPT_BRANDING_INVENTORY.md`. Filed `9aae090`. Had **no status line** |
| `appcast_missing_minimum_system_version` | Ticket accurate — **candidate blocker for promoting 0.4.0** |
| `cdp_port_open_in_release` | Ticket accurate |
| `dependency_freshness_review` | Ticket accurate — cheap |
| `engine_pins_are_branches_not_tags` | Ticket accurate — cheap |

## ❔ NEEDS OWNER REVIEW — 6

Not settled by a cheap check; each needs a judgement I should not make alone.

| Ticket | The open question |
|---|---|
| `farbling_gate_engine_binding` | Claims OPEN. Interacts with the standing farbling release gate and `CEF_VERSION` identity — needs the release-gate owner's read, not a grep |
| `chrome_ui_scales_but_its_window_does_not` | Filed Phase 1; no status. Is it superseded by Phase 1's DPI work or genuinely distinct? |
| `disable_features_autofill_is_a_noop` | 📏 `simple_app.cpp:153` **does** append `disable-features=Autofill,…`. Whether it is still a no-op is the ticket's actual claim and needs a runtime check |
| `connect_modal_two_views_drift` | P0.8 rewrote much of this surface. Is the drift closed or just moved? |
| `manifest_description_can_misdescribe_protocol` | P0.8 territory; unclear whether the BRC-73 work closed it |
| `longlived_surfaces_snapshot_state_at_startup` | Filed unscheduled; no status. Scope unclear |

## 🔵 ACCEPTED — 1

| Ticket | Note |
|---|---|
| `wallet_global_profiles_isolated` | Precedent recorded, no fix planned. ⚠️ Worth re-reading against the owner's 2026-08-31 remark that *"there is not really any reason for us to have a default at all"* — the profile model may be about to change |

---

## What I did NOT do

- ⛔ **Did not move anything into `0.4.0-beta.4/tickets/`.** That folder is a **review queue** and its
  README is explicit: *"a ticket is not work until the owner has assigned it to a sprint."* Moving the
  11 open ones is the owner's call, not a tidy-up.
- ⛔ **Did not re-verify the ✅ rows against their original defects.** A landed fix is not a proven
  fix; the phase evidence tables own that claim.
- ⛔ **Did not add status lines by guessing.** The six ❔ rows stay unlabelled until someone decides.

## Recommended next actions

1. **Two open tickets are qualitatively different from the rest** and are worth pulling forward:
   `token_outputs_destroyed_by_dust_paths` (automatic, destroys assets) and
   `consent_surface_fetches_third_party_favicon` (leaks the consent decision to Google, cheap to fix).
2. **Four cheap ones could close in one sitting:** `dependency_freshness_review`,
   `engine_pins_are_branches_not_tags`, `cdp_port_open_in_release`,
   `appcast_missing_minimum_system_version` — the last is a promotion blocker.
3. **Settle the six ❔ rows** so the beta.4 review starts from a clean queue.
