# beta.4 tickets

**Opened:** 2026-08-29.

This folder replaces beta.3's flat `TICKET_*.md` at folder root. The difference is not cosmetic:

> ⛔ **A ticket is not work until the owner has assigned it to a track.**
> This folder is a **review queue**, not a backlog anyone picks from.

---

## How it works

1. Anyone (session or owner) files a ticket here from `TICKET_TEMPLATE.md`.
2. It sits at **Track: unassigned**.
3. **The owner reviews and assigns it** to a track, to the misc bucket, or closes it.
4. Only then does it become work, and only inside the track it was assigned to.

⚠️ The owner has items on paper that are not here yet. ⛔ **Do not chase them.** They arrive when he
files them.

## Naming

`TICKET_<short_snake_case_description>.md` — the beta.3 convention, kept, because it greps well and
existing links use it. Describe the **symptom or the defect**, not the fix:
`TICKET_recovery_sweep_ignores_classification.md`, not `TICKET_add_classification_to_sweep.md`.

## Status values

| Status | Meaning |
|---|---|
| ⬜ **UNASSIGNED** | Filed, not yet reviewed by the owner. The default. |
| 📌 **ASSIGNED** | Owner has put it in a track. Names which one. |
| 🚧 **IN PROGRESS** | Being worked, inside its track's phase structure. |
| ✅ **CLOSED** | Fixed, with evidence, or closed with a written reason. |
| ❄️ **DEFERRED** | Deliberately not now, **with a re-check condition** — the `WATCH_fungibles.md` pattern. A deferral with no condition is a ticket rotting. |

## Rules

- ⛔ **State method.** Say whether a claim is a **code reading** or a **measurement**
  (`HARNESS.md` §8). Both are legitimate; mislabelling one as the other is not. The dust-consolidator
  ticket does this well — it opens with a method note naming the one thing it did *not* verify.
- ⛔ **Say what you did not check.** A ticket that reads as complete when it is partial is worse than
  a short one.
- **Every ticket that proposes a fix proposes its negative control.** Per `HARNESS.md`: not done
  until the check has been *seen* to fail.
- **Do not size a fix you have not scoped.** "Small" is a claim.

## Related tickets living elsewhere

| Ticket | Where | Why it is not here |
|---|---|---|
| `TICKET_token_outputs_destroyed_by_dust_paths.md` | `../../0.4.0-beta.3/` | Its **minimal defensive floor** ships in beta.3 (owner, 2026-08-29). The **full classification guard** is beta.4 release 1. ⛔ Read-only from here — another session owns that folder. ⏳ Its "Links" section still points at the pre-move ordinals path; that fix is owed to whoever next touches beta.3. |

---

## Register

| Ticket | Status | Track | Filed |
|---|---|---|---|
| `TICKET_e2e_specs_wrong_subject_and_never_run.md` | ⬜ UNASSIGNED | — | 2026-08-29 |
| `TICKET_dapp_reachable_surface_is_a_denylist_not_an_allowlist.md` | ⬜ UNASSIGNED | — (⭐ suggest track 1) | 2026-09-19 |
| `TICKET_brc100_consent_model_diverges_from_1sat_wallet_api.md` | ⬜ UNASSIGNED | — (⭐ suggest track 2) | 2026-09-21 |
| `TICKET_derived_public_keys_have_no_prompt_and_can_match_across_sites.md` | ⬜ UNASSIGNED | — | 2026-09-21 |
| `TICKET_well_known_auth_returns_a_key_it_cannot_sign_for.md` | ⬜ UNASSIGNED | — | 2026-09-21 |
| `TICKET_brc121_client_has_no_body_transport_for_large_beef.md` | ⬜ UNASSIGNED | — (decision at the microscope pass; waits on BRCs #261) | 2026-09-23 |
| `TICKET_mkcert_dev_private_key_is_tracked_and_public.md` | ⬜ UNASSIGNED | — (🟡 low; regenerate, do NOT rewrite history) | 2026-09-23 |
| `TICKET_chromium_default_debug_log_lands_in_install_root.md` | ⬜ UNASSIGNED | — (🟡 low; ⛔ NOT the wallet-data stray log, which is FIXED) | 2026-09-23 |

🚨 **This index is behind the folder.** There are **16 tickets** on disk (17 files, one is the
template) and the table lists **8**. The eight missing rows are `active_user_count…`,
`brc103_server_identity_unverified`, `brc140_key_shares_vs_bip39`,
`menu_exit_closes_primary_not_the_clicked_window`, `multiwindow_session_restore…`,
`split_view_needs_multi_visible_tab_model`, `tab_pin_and_mute_need_model_changes` and
`window_scoped_work_uses_process_globals` *(measured 2026-09-19)*. ⛔ Not reconciled here — filing one
ticket is not licence to rewrite the index.

⚠️ `TICKET_e2e_specs_wrong_subject_and_never_run.md` is **second-hand** — reported by research (c),
**not independently verified**. Its first proposed step is verification. That is deliberate: a ticket
may record an unverified report as long as it says so.

⚠️ `TICKET_dapp_reachable_surface_is_a_denylist_not_an_allowlist.md` is **code reading, not
measurement** — the owner was using the app, so nothing was executed. Its RED is written to be run
first, before the fix, because it is also what confirms the finding.
