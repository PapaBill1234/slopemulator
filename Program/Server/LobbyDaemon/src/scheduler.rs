//! Deletion scheduler (`20-handlers.md` §9, `LobbyCharacterDeletionScheduler`).
//! Each tick: `RetrieveExpiredDeletionWaitingList(count)`, then
//! `ExpireCharacterDeletionWaiting(pcid)` for each row. The task talks only to the store.
//! It does not hold the session table. A session that shows the character keeps its
//! countdown at 0 until the client logs in again (`11-character-select.md` §15).
use crate::store::{ExpiredDeletion, Store};
use std::{io, time::Duration};
use tokio::{
    sync::watch,
    time::{interval, MissedTickBehavior},
};

/// One tick. Returns the rows that the store moved from state 4 to state 6.
/// A refused row (a restore or another daemon was first) does not stop the other rows.
pub async fn tick<S: Store>(store: &S, count: u32) -> io::Result<Vec<ExpiredDeletion>> {
    let rows = store.retrieve_expired_deletion_waiting_list(count).await?;
    let mut expired = Vec::with_capacity(rows.len());
    for row in rows {
        if store
            .expire_character_deletion_waiting(row.pcid)
            .await
            .is_ok()
        {
            expired.push(row);
        }
    }
    Ok(expired)
}

/// Run [`tick`] each `period` until the shutdown value is true or its sender is dropped.
/// The first tick runs at once: waits that ended while the daemon was down end now.
pub async fn run<S: Store>(
    store: S,
    period: Duration,
    count: u32,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut timer = interval(period);
    // A slow database must not cause a burst of ticks.
    timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = timer.tick() => {
                if let Err(error) = tick(&store, count).await {
                    eprintln!("lobbyd: deletion scheduler tick failed: {error}");
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
            }
        }
    }
}
