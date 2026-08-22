//! TaskSweepReservations — release UTXO reservations that outlived their transaction.
//!
//! `create_action_internal` (and the backup / certificate-unpublish paths) reserve their
//! selected outputs by marking them spent against a `pending-` placeholder, resolving it to
//! the real txid only on the success path. A `ReservationGuard` releases that reservation on
//! every failure exit, but a guard cannot survive a process kill: a wallet killed between
//! reserving and signing leaves the rows reserved with nothing left to clean them up.
//!
//! This task is that backstop.
//!
//! ## 🚨 Why this is not `UPDATE ... WHERE updated_at < cutoff`
//!
//! A row holding a `pending-` placeholder is *usually* a transaction that never broadcast —
//! but not always. Every path resolves placeholder → real txid **before** broadcasting, yet
//! that update's failure is only logged as a warning and the broadcast proceeds anyway. So a
//! `pending-` row can belong to a transaction that is already on-chain.
//!
//! Un-spending such a row hands an already-spent output back to the selector, and the next
//! send double-spends it (`R-NODOUBLE`). This task therefore releases an outpoint **only**
//! after positively observing it in the wallet's on-chain unspent set. Every uncertainty —
//! API down, address not ours, outpoint simply absent — leaves the reservation in place. A
//! leak costs the user an unspendable coin until the next sweep; a wrong release costs a
//! double-spend.
//!
//! The predecessor of this task was exactly the unsafe form: a blanket
//! `UPDATE ... WHERE spending_description LIKE 'pending-%'` run unconditionally at startup,
//! with no age filter and no on-chain check.

use actix_web::web;
use log::{info, warn};
use std::collections::HashSet;

use crate::AppState;

/// Minimum reservation age before an outpoint is even a candidate.
///
/// Comfortably longer than any legitimate in-flight `createAction` (the user-facing approval
/// modal times out in minutes), so the sweep never races a live flow — and short enough that
/// a user who mistyped an address is not left staring at "insufficient funds".
pub const MAX_AGE_SECS: i64 = 15 * 60;

pub async fn run(state: &web::Data<AppState>) -> Result<(), String> {
    // ---- 1. Candidates: old enough, still holding a `pending-` placeholder ----
    let candidates = {
        let db = state.database.lock().map_err(|e| format!("DB mutex poisoned: {}", e))?;
        let output_repo = crate::database::OutputRepository::new(db.connection());
        output_repo
            .list_stale_pending_reservations(MAX_AGE_SECS)
            .map_err(|e| format!("Failed to list stale reservations: {}", e))?
    };
    if candidates.is_empty() {
        return Ok(());
    }

    // ---- 2. Drop anything still in flight ----
    // A placeholder held by a live PENDING_TRANSACTIONS entry belongs to a createAction
    // awaiting signAction, however old the reservation looks.
    let live = crate::handlers::live_reservation_placeholders();
    let candidates: Vec<_> = candidates
        .into_iter()
        .filter(|c| !live.contains(&c.placeholder))
        .collect();
    if candidates.is_empty() {
        return Ok(());
    }

    info!(
        "🧹 TaskSweepReservations: {} reservation(s) older than {}m — verifying on-chain before release",
        candidates.len(),
        MAX_AGE_SECS / 60
    );

    // ---- 3. On-chain evidence ----
    let address_infos = {
        let db = state.database.lock().map_err(|e| format!("DB mutex poisoned: {}", e))?;
        let wallet_repo = crate::database::WalletRepository::new(db.connection());
        let wallet = match wallet_repo.get_primary_wallet() {
            Ok(Some(w)) => w,
            Ok(None) => return Ok(()), // no wallet — nothing to sweep
            Err(e) => return Err(format!("Failed to read wallet: {}", e)),
        };
        let wallet_id = match wallet.id {
            Some(id) => id,
            None => return Ok(()),
        };
        let address_repo = crate::database::AddressRepository::new(db.connection());
        address_repo
            .get_all_by_wallet(wallet_id)
            .map_err(|e| format!("Failed to read addresses: {}", e))?
            .iter()
            .map(crate::database::helpers::address_to_address_info)
            .collect::<Vec<_>>()
    };

    // `fetch_all_utxos` unions the confirmed and mempool endpoints and returns Err when no
    // address could be checked at all. Both matter: a confirmed-only read would miss a spend
    // sitting in the mempool and wrongly call the outpoint unspent, and treating an API
    // outage as "nothing is unspent" would be harmless here (we release nothing) but
    // treating it as evidence would not be. On Err we abort the sweep entirely.
    let api_utxos = crate::utxo_fetcher::fetch_all_utxos(&address_infos)
        .await
        .map_err(|e| format!("On-chain check unavailable, releasing nothing: {}", e))?;

    let unspent: HashSet<(String, u32)> = api_utxos
        .into_iter()
        .map(|u| (u.txid, u.vout))
        .collect();

    // ---- 4. Release only what is provably still unspent ----
    let mut released = 0usize;
    let mut released_sats = 0i64;
    let mut withheld = 0usize;

    for c in &candidates {
        if !unspent.contains(&(c.txid.clone(), c.vout)) {
            // Not provably unspent: either the transaction really did broadcast, or the
            // outpoint belongs to an address outside this wallet (a user-supplied basket
            // input). Either way there is no evidence, so the reservation stands.
            warn!(
                "   ⚠️  {}:{} ({} sats) is not in the on-chain unspent set — leaving reserved",
                &c.txid[..std::cmp::min(16, c.txid.len())],
                c.vout,
                c.satoshis
            );
            withheld += 1;
            continue;
        }

        let db = match state.database.lock() {
            Ok(db) => db,
            Err(e) => return Err(format!("DB mutex poisoned mid-sweep: {}", e)),
        };
        let output_repo = crate::database::OutputRepository::new(db.connection());
        match output_repo.restore_outpoint_if_reserved(&c.txid, c.vout, &c.placeholder) {
            Ok(1) => {
                released += 1;
                released_sats += c.satoshis;
            }
            // 0 rows means the row changed under us — a concurrent createAction re-reserved
            // it, or the placeholder resolved to a real txid. Correct outcome, nothing to do.
            Ok(_) => {}
            Err(e) => warn!("   ⚠️  Failed to release {}:{}: {}", c.txid, c.vout, e),
        }
        drop(db);
    }

    if released > 0 {
        state.balance_cache.invalidate();
        info!(
            "   ♻️  TaskSweepReservations: released {} stale reservation(s), {} sats returned to spendable",
            released, released_sats
        );
    }
    if withheld > 0 {
        info!(
            "   🔒 TaskSweepReservations: {} reservation(s) withheld — no proof they are unspent",
            withheld
        );
    }

    Ok(())
}
