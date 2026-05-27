//! TaskRefreshShipCache — Keep SHIP host discovery warm for identity overlay.
//!
//! Without this task, the first publish/unpublish after a 5-min idle would
//! pay the ~75s SHIP round-trip; with it, `state.ship_cache` always holds a
//! recent (< 5 min) entry and `discover_hosts_for_topic` becomes a synchronous
//! cache hit on the hot path.
//!
//! Pure network + memory — does NOT touch the database. For that reason this
//! task is scheduled outside the Monitor's `db_available()` gate (see
//! `monitor/mod.rs`), so a busy DB never starves SHIP cache refresh.
//!
//! Interval: 300 seconds (5 minutes) — matches `ship_cache::FRESH_TTL` so
//! the cache never enters the stale window during normal operation.

use actix_web::web;
use log::debug;

use crate::AppState;
use crate::overlay::TOPIC_IDENTITY;

/// Run a refresh of all topics the wallet actively uses.
///
/// Today: just `tm_identity`. If additional topics get added to the publish
/// flow, list them here so the cache stays warm for each.
pub async fn run(state: &web::Data<AppState>) -> Result<(), String> {
    debug!("🔄 TaskRefreshShipCache: refreshing '{}'", TOPIC_IDENTITY);
    state.ship_cache.refresh(TOPIC_IDENTITY).await;
    Ok(())
}
